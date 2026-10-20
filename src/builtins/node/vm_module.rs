//! `node:vm` 模块系 natives（9i-1；自 `vm.rs` 纯搬移，§0.9 单文件≤1000）。
//!
//! - `vm_mod_*`：SourceTextModule 编译/link/evaluate/namespace（见 vm.rs 头注记档）；
//! - `promise_state`/`promise_result`：Promise 结算内态（`util.inspect` 显示用）。

use mozjs::jsapi::JSObject;
use mozjs::jsval::{BooleanValue, JSVal, UndefinedValue};
use mozjs::conversions::ToJSValConvertible as _;
use mozjs::realm::AutoRealm;
use mozjs::rooted;
use mozjs::rust::{transform_str_to_source_text, CompileOptionsWrapper};

use crate::jsapi_glue::{exc_name, raw_handle, report_error, value_to_string, wrap_cx, Frame};
use crate::state;

use super::vm::{arg_string, lookup_global};
use super::vm_error::throw_vm;

// ── 静态请求表（linker 切片；`state::VmMod` 在 state/mod.rs 跨 lane 文件中，
// 本 lane 不碰——请求 JSON 存本文件进程级表，id 单调不复用无 ABA，
// `vm_mod_release`/ctx 摘除时清（见 `vm_mod_requests_drop_ctx`）。────────────

/// 静态请求表（纯逻辑子模块；`#[path]` 挂接，不碰 `mod.rs`）。
/// `pub(crate)` 公开动态 `with` 属性解析（`vm_dyn.rs` 同域复用）。
#[path = "vm_requests.rs"]
pub(crate) mod requests;
use requests::{VmRequest, extract_requests, requests_json};

static VM_MOD_REQUESTS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u64, (u64, String)>>> =
    std::sync::OnceLock::new();

fn vm_mod_requests_map(
) -> &'static std::sync::Mutex<std::collections::HashMap<u64, (u64, String)>> {
    VM_MOD_REQUESTS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// ctx 摘除时清其名下请求表（`vm.rs` release 路径调用；幂等）。
pub(super) fn vm_mod_requests_drop_ctx(ctx: u64) {
    if let Ok(mut m) = vm_mod_requests_map().lock() {
        m.retain(|_, (c, _)| *c != ctx);
    }
}

/// 模块 ctx 实参（"main"→主 realm 哨兵；真机无 context 模块跑主域）。
fn mod_ctx_id(
    cx: &mut mozjs::context::JSContext,
    frame: &Frame,
    i: u32,
) -> Option<u64> {
    let s = arg_string(cx, frame, i, "vm SourceTextModule")?;
    if s == "main" {
        return Some(state::VM_MAIN_CTX);
    }
    s.parse::<u64>().ok().or_else(|| {
        report_error(cx, "ERR_INVALID_ARG_TYPE: vm Module id must be a module id string");
        None
    })
}

/// 取模块 realm global（哨兵即当前主 global；调用方必须立即重 root，见 runtime 约定）。
/// UNSAFE-BOUNDARY: 哨兵分支读当前 global（调用处于 JS 执行中，非空）；其余走表查。
fn mod_global(cx: &mut mozjs::context::JSContext, ctx: u64) -> Option<*mut JSObject> {
    if ctx == state::VM_MAIN_CTX {
        // SAFETY: 引擎回调身处 JS 执行，当前 global 非空；裸指针调用方立即 rooted
        let g = unsafe { mozjs::jsapi::CurrentGlobalOrNull(cx.raw_cx()) };
        if g.is_null() {
            report_error(cx, "ERR_VM_MODULE_NOT_FOUND: vm main context has no global");
            return None;
        }
        return Some(g);
    }
    lookup_global(cx, ctx)
}

// ── 9i-1 模块系（SourceTextModule；SyntheticModule 纯 JS，见 SOURCE）─────────
//
// 落法：`compile` 在目标 compartment 内 `load_js` 转译（含 TS）+ `CompileModule1`
// （filename=identifier，使 `referrer_base` 天然分流）；`link` 走
// `load_dependencies` + `ModuleLink`（零导入恒过；`has_imports` 者 v1 拒绝——
// 进程级 load hook 按主 global/主注册表工作，vm 记录带入即 compartment 错配，
// linker 切片后续做）；`evaluate` 走 `ModuleEvaluate`（求值点不排空 jobqueue，
// TLA 的 pending 留给事件循环，见 vm_mod_evaluate）；namespace 经 `GetModuleNamespace` 以 CCW 回主域。
// 记录以 `Box<Heap>` 入 `state::vm_mods`（§4.40 定址），状态位防重复 link/evaluate。

/// 编译模块：`__wjs2_vm_compile_mod(ctxId, identifier, code)` → modId 字符串。
/// 失败抛包络 SyntaxError（转译错/编译错，含行列信息）。
pub unsafe extern "C" fn vm_mod_compile(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    use mozjs::rust::wrappers2::CompileModule1;
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let ctx = match mod_ctx_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let identifier = match arg_string(&mut cx, &frame, 1, "vm SourceTextModule") {
        Some(s) => s,
        None => return false,
    };
    let code = match arg_string(&mut cx, &frame, 2, "vm SourceTextModule") {
        Some(s) => s,
        None => return false,
    };
    let ptr = match mod_global(&mut cx, ctx) {
        Some(p) => p,
        None => return false,
    };
    rooted!(&in(cx) let global = ptr);
    // 转译（TS 免费；ext 从 identifier 点后缀取，无点按 js）。
    let ext = identifier.rsplit('.').next().filter(|e| {
        identifier.contains('.')
            && e.len() <= 5
            && !e.is_empty()
            && e.chars().all(|c| c.is_ascii_alphanumeric())
    });
    let fake_path = std::path::PathBuf::from(format!("vm_mod.{}", ext.unwrap_or("js")));
    let loaded = match crate::loader::load_js(&code, &identifier, &fake_path) {
        Ok(l) => l,
        Err(e) => {
            throw_vm(&mut cx, "SyntaxError", &e.to_string());
            return false;
        }
    };
    let has_imports = !loaded.imports.is_empty();
    // 静态请求表（linker/moduleRequests 同源）：二次解析引擎编译文本
    //（转译后，语句序，属性/phase 俱全）；解析失手（理论不可达——引擎已编译
    // 通过）回落 loader 的 spec 表，保证 deps 永与引擎请求集一致。
    // 附带 `with` 子句剥离物（V8 不透明口径，见 extract_requests）：引擎喂剥离版。
    let (mut reqs, stripped) = extract_requests(&loaded.js, ext.unwrap_or("js"));
    if reqs.is_empty() && has_imports {
        reqs = loaded
            .imports
            .iter()
            .map(|spec| VmRequest { spec: spec.clone(), attrs: Vec::new(), phase: "evaluation" })
            .collect();
    }
    let compile_js = stripped.as_deref().unwrap_or(&loaded.js);
    let specs: Vec<String> = reqs.iter().map(|r| r.spec.clone()).collect();
    let has_imports = !reqs.is_empty();
    let c_filename =
        std::ffi::CString::new(identifier.as_str()).unwrap_or_else(|_| c"vm_module.js".to_owned());
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let options = CompileOptionsWrapper::new(&realm, c_filename, 1);
    let mut src = transform_str_to_source_text(compile_js);
    // SAFETY: realm 内；options/src 存活到调用返回；null 即编译失败
    let record = unsafe { CompileModule1(&mut realm, options.ptr, &mut src) };
    if record.is_null() {
        rooted!(&in(&mut realm) let mut exc = UndefinedValue());
        let msg = match mozjs::rust::error_info_from_exception_stack(&mut realm, exc.handle_mut()) {
            Some(info) => format!("{}:{}:{}: {}", identifier, info.line.max(1), info.col.max(1), info.message),
            None => format!("{identifier}: invalid module"),
        };
        throw_vm(&mut realm, "SyntaxError", &msg);
        return false;
    }
    let id = {
        rooted!(&in(&mut realm) let record_root: *mut JSObject = record);
        // SAFETY: record 有效 rooted；编译直后脚本槽恒在（求值后引擎清槽，见 4.293）
        let script = unsafe {
            mozjs::rust::wrappers2::GetModuleScript(record_root.handle())
        };
        state::vm_mod_add(ctx, identifier, record, script, has_imports, specs)
    };
    if let Ok(mut m) = vm_mod_requests_map().lock() {
        m.insert(id, (ctx, requests_json(&reqs)));
    }
    id.to_string().to_jsval(&mut realm, frame.rval_mut());
    true
}

/// 取静态请求表：`__wjs2_vm_mod_deps(modId)` → JSON 数组串
///（`[{specifier,attributes,phase}]`，首现序全键去重；`dependencySpecifiers`
/// 与 `moduleRequests` 同源派生）。
pub unsafe extern "C" fn vm_mod_deps(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let s = match arg_string(&mut cx, &frame, 0, "vm Module") {
        Some(s) => s,
        None => return false,
    };
    let Ok(id) = s.parse::<u64>() else {
        report_error(&mut cx, "ERR_INVALID_ARG_TYPE: vm Module id must be a module id string");
        return false;
    };
    // state 侧存在性（记录与请求表同生共死；ctx 摘除即双清，见 drop_ctx）。
    if state::vm_mod_deps_json(id).is_none() {
        report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
        return false;
    }
    let json = vm_mod_requests_map()
        .lock()
        .ok()
        .and_then(|m| m.get(&id).map(|(_, j)| j.clone()));
    let Some(json) = json else {
        report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
        return false;
    };
    json.to_jsval(&mut cx, frame.rval_mut());
    true
}

/// 顶层 await 探测（`__wjs2_vm_mod_tla(code, identifier)` → boolean；纯解析，
/// 无状态。扩展名按 identifier 后缀定方言，规则同编译）。
/// UNSAFE-BOUNDARY: 实参 Frame rooted 后读（§4.80）；覆盖
/// `tests/node/vm.rs::vm_module_tla_faces`（经黑盒三件）。
pub unsafe extern "C" fn vm_mod_tla(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let code = match arg_string(&mut cx, &frame, 0, "vm module") {
        Some(s) => s,
        None => return false,
    };
    let identifier = arg_string(&mut cx, &frame, 1, "vm module").unwrap_or_default();
    // ext 规则同编译（identifier 点后缀，无点/超长/非标按 js；见上）。
    let ext = identifier.rsplit('.').next().filter(|e| {
        identifier.contains('.')
            && e.len() <= 5
            && !e.is_empty()
            && e.chars().all(|c| c.is_ascii_alphanumeric())
    });
    requests::has_top_level_await(&code, ext.unwrap_or("js")).to_jsval(&mut cx, frame.rval_mut());
    true
}

/// 链接模块：`__wjs2_vm_link(modId[, sessionJson])` → undefined。
/// 无会话即零导入旧路（`load_dependencies` + `ModuleLink`；带导入仍拒绝——
/// JS 壳将会话路由到图路，此处为防御）；有会话走链接器图（见下）。
pub unsafe extern "C" fn vm_mod_link(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    use mozjs::rust::wrappers2::ModuleLink;
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let s = match arg_string(&mut cx, &frame, 0, "vm Module") {
        Some(s) => s,
        None => return false,
    };
    let id = match s.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            report_error(&mut cx, "ERR_INVALID_ARG_TYPE: vm Module id must be a module id string");
            return false;
        }
    };
    // 图路：JS 壳已做 linker BFS（校验 + 去重 + transitivity），此处只做引擎接线。
    if frame.argc() > 1 {
        let sess = value_to_string(&mut cx, frame.arg(1));
        return vm_mod_link_graph(&mut cx, frame, id, &sess);
    }
    let Some((record, ctx, has_imports, _linked, _evaluated)) = state::vm_mod_get(id) else {
        report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
        return false;
    };
    if has_imports {
        let ident = state::vm_mod_identifier(id).unwrap_or_default();
        report_error(
            &mut cx,
            &format!(
                "ERR_VM_MODULE_LINK_FAILURE: module '{ident}' has imports but no linker was provided (v1: zero-import only)"
            ),
        );
        return false;
    }
    let Some(ptr) = mod_global(&mut cx, ctx) else {
        return false;
    };
    rooted!(&in(cx) let global = ptr);
    rooted!(&in(cx) let record_root: *mut JSObject = record);
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    if let Err(e) = crate::modules::load_dependencies(&mut realm, record_root.get()) {
        throw_vm(&mut realm, "Error", &e.to_string());
        return false;
    }
    // SAFETY: record 有效 rooted；加载态已就绪，realm 内同步 link
    if !unsafe { ModuleLink(&mut realm, record_root.handle()) } {
        let msg = crate::modules::module_error(&mut realm, "vm_module").to_string();
        throw_vm(&mut realm, "Error", &msg);
        return false;
    }
    state::vm_mod_set_linked(id);
    frame.set_rval(UndefinedValue());
    true
}

/// 求值模块：`__wjs2_vm_evaluate(modId, timeoutMs?)` → completion（promise 照常回调用方）。
/// 未 link 即报 `ERR_VM_MODULE_STATUS`（Node 口径：先 link 后 evaluate）。
pub unsafe extern "C" fn vm_mod_evaluate(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    use mozjs::rust::wrappers2::ModuleEvaluate;
    use super::vm_timeout::{vm_timeout_arg, vm_timeout_peek_ms, vm_timeout_report, VmTimeoutGuard};
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let s = match arg_string(&mut cx, &frame, 0, "vm Module") {
        Some(s) => s,
        None => return false,
    };
    let id = match s.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            report_error(&mut cx, "ERR_INVALID_ARG_TYPE: vm Module id must be a module id string");
            return false;
        }
    };
    let timeout_ms = vm_timeout_arg(&mut cx, &frame, 1);
    let mut _timeout_guard = VmTimeoutGuard::enter(cx_raw, timeout_ms);
    let Some((record, ctx, _has_imports, linked, _evaluated)) = state::vm_mod_get(id) else {
        report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
        return false;
    };
    if !linked {
        report_error(&mut cx, "ERR_VM_MODULE_STATUS: module must be linked before evaluate");
        return false;
    }
    let Some(ptr) = mod_global(&mut cx, ctx) else {
        return false;
    };
    rooted!(&in(cx) let global = ptr);
    rooted!(&in(cx) let record_root: *mut JSObject = record);
    rooted!(&in(cx) let mut rval = UndefinedValue());
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    // SAFETY: record 有效 rooted；realm 内同步求值
    if !unsafe { ModuleEvaluate(&mut realm, record_root.handle(), rval.handle_mut()) } {
        // 超时命中优先（同 vm_run：限时武装排空孤儿，见 4.295）。
        if vm_timeout_peek_ms().is_some() {
            super::vm::vm_timeout_drain_orphans(cx_raw, &mut realm, &mut _timeout_guard);
        }
        if vm_timeout_report(&mut realm, timeout_ms) {
            return false;
        }
        let msg = crate::modules::module_error(&mut realm, "vm_module").to_string();
        throw_vm(&mut realm, "Error", &msg);
        return false;
    }
    // 超时中断的模块求值：ModuleEvaluate 同步返回 rejected completion
    //（uncatchable 中止，因恒 undefined）——命中标记在即按超时失败处理：
    // 先补发中断排空孤儿任务（escape-promise-module 口径：逃逸 `.then(loop)`
    // 中止、永不落定、无 unhandled），再报真机口径错。pending/fulfilled +
    // 命中（竞态：回调置位与同步完成同时）则清标记正常返回（V8 口径：终
    // 止未送达即完成，不报错；清标记防同线程后继定时求值误报）。
    if vm_timeout_peek_ms().is_some() {
        let rejected = rval.is_object() && {
            rooted!(&in(&mut realm) let rval_obj: *mut JSObject = rval.to_object());
            // 只读 state（promise_state 同源；safe 读，不推进 jobqueue）
            matches!(
                crate::jsapi_glue::promise_settled_value(&mut realm, rval_obj.get()),
                Some(Some((false, _)))
            )
        };
        if rejected {
            // 中止 completion 自身的 unhandled 认领（因恒 undefined 的终止
            // rejection，真机不报告；调用方经 evaluate() 认领超时错，此处
            // 等效 Handled 摘除，防事件循环尾 fatal）。
            let rval_ptr = rval.to_object();
            state::with_rooted(|s| s.unhandled.retain(|h| h.get() != rval_ptr));
            // 孤儿排空（限时武装，见 4.295；escape-promise-module 口径不变）。
            super::vm::vm_timeout_drain_orphans(cx_raw, &mut realm, &mut _timeout_guard);
            if vm_timeout_report(&mut realm, timeout_ms) {
                return false;
            }
        } else {
            super::vm_timeout::vm_timeout_fired_ms();
        }
    }
    // Node afterEvaluate 等效（runInContext 系由各自求值点排空；模块求值不
    // 在此处排空——TLA 模块的 pending 必须留给事件循环，否则同步结算偏离真机；
    // 上方超时分支是唯一例外：命中即求值已失败，排空只杀孤儿）。
    // 落定判定一律由 JS 壳经 `__wjs2_promise_state/result` 做（跨 compartment
    // 安全：GetPromiseState 内解包；result 侧本文件解包后读），此处原样返回
    // completion（非 promise 同步值即置 evaluated 位）。
    // 前置：rval 已 rooted。
    let is_promise = if rval.is_object() {
        rooted!(&in(&mut realm) let rval_obj: *mut JSObject = rval.to_object());
        // SAFETY: rval_obj 为有效 rooted 对象；realm 内读，无跨域
        unsafe { mozjs::jsapi::IsPromiseObject(crate::jsapi_glue::raw_handle(rval_obj.as_ptr())) }
    } else {
        false
    };
    if !is_promise {
        state::vm_mod_set_evaluated(id);
    }
    frame.set_rval(rval.get());
    true
}

/// 取模块 namespace：`__wjs2_vm_mod_ns(modId)` → namespace 对象（CCW 回主域）。
pub unsafe extern "C" fn vm_mod_ns(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    use mozjs::rust::wrappers2::GetModuleNamespace;
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let s = match arg_string(&mut cx, &frame, 0, "vm Module") {
        Some(s) => s,
        None => return false,
    };
    let id = match s.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            report_error(&mut cx, "ERR_INVALID_ARG_TYPE: vm Module id must be a module id string");
            return false;
        }
    };
    let Some((record, ctx, _has_imports, linked, evaluated)) = state::vm_mod_get(id) else {
        report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
        return false;
    };
    // TLA 求值中（已 link 未落定）的命名空间可读（未初始化导出访问即
    // ReferenceError，真机口径）；未 link 仍拒。
    if !evaluated && !linked {
        report_error(&mut cx, "ERR_VM_MODULE_STATUS: module must be evaluated before reading namespace");
        return false;
    }
    let Some(ptr) = mod_global(&mut cx, ctx) else {
        return false;
    };
    rooted!(&in(cx) let global = ptr);
    rooted!(&in(cx) let record_root: *mut JSObject = record);
    // 调用方 global（返回前重进并包装，跨域恒等统一，见 4.296；裸指针拷出，
    // 守卫即释，避免与下文 realm 借用冲突）。
    let caller_ptr = unsafe { mozjs::jsapi::CurrentGlobalOrNull(cx.raw_cx()) };
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    // SAFETY: record 有效；返回的 namespace 由记录保活（引擎内边）
    let ns = unsafe { GetModuleNamespace(&mut realm, record_root.handle()) };
    if ns.is_null() {
        throw_vm(&mut realm, "Error", "vm could not read module namespace");
        return false;
    }
    rooted!(&in(&mut realm) let ns_root: *mut JSObject = ns);
    let mut out = mozjs::jsval::ObjectValue(ns_root.get());
    if !caller_ptr.is_null() {
        rooted!(&in(&mut realm) let caller_root: *mut JSObject = caller_ptr);
        let mut caller_realm = AutoRealm::new_from_handle(&mut realm, caller_root.handle());
        // 调用方跨域 `===` 恒等：包装进调用方 compartment（同域无操作）。
        match crate::jsapi_glue::wrap_value(&mut caller_realm, out) {
            Some(w) => out = w,
            None => return false,
        }
    }
    frame.set_rval(out);
    true
}

/// 摘除模块：`__wjs2_vm_mod_release(modId)` → boolean。
pub unsafe extern "C" fn vm_mod_release(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let s = match arg_string(&mut cx, &frame, 0, "vm Module") {
        Some(s) => s,
        None => return false,
    };
    let Ok(id) = s.parse::<u64>() else {
        report_error(&mut cx, "ERR_INVALID_ARG_TYPE: vm Module id must be a module id string");
        return false;
    };
    let _ = &mut cx;
    if let Ok(mut m) = vm_mod_requests_map().lock() {
        m.remove(&id);
    }
    frame.set_rval(BooleanValue(state::vm_mod_release(id)));
    true
}

/// 异步落定补记：`__wjs2_vm_mod_settled(modId)` → undefined。
/// 跨域求值恒异步（§4.57），promise 路径的 evaluated 位由 JS 壳在落定后补记。
pub unsafe extern "C" fn vm_mod_settled(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let s = match arg_string(&mut cx, &frame, 0, "vm Module") {
        Some(s) => s,
        None => return false,
    };
    let Ok(id) = s.parse::<u64>() else {
        report_error(&mut cx, "ERR_INVALID_ARG_TYPE: vm Module id must be a module id string");
        return false;
    };
    state::vm_mod_set_evaluated(id);
    frame.set_rval(UndefinedValue());
    true
}

// ── 链接器图（link/linkRequests 引擎接线；Gecko153 无 SetModuleResolveHook，
// 唯一解析通道是进程级 load hook——此处临时换装 vm 会话 hook，复原；见 Modules.h
// `LoadRequestedModules` 注释：边经 hook 命中，循环由引擎处理）────────────────
///
/// 会话 JSON（JS 壳 BFS 产物）：`{modules:[id…], edges:[[referrerId,spec,depId]…]}`
///（id 皆字符串形态数字）。JS 已校验（模块身份/同 ctx/键一致），此处复核存在性
/// 与 ctx 一致即接线；缺边（内部不一致）报 `ERR_VM_MODULE_LINK_FAILURE`。
struct VmLinkSession {
    edges: std::collections::HashMap<(u64, String), u64>,
    modules: Vec<u64>,
}

thread_local! {
    static VM_LINK_SESSIONS: std::cell::RefCell<Vec<VmLinkSession>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static VM_LINK_ERROR: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// 图链接（`vm_mod_link` 图路；调用方已进 realm，record 有效 rooted 用）。
/// UNSAFE-BOUNDARY: session 来自 JS 壳 BFS（id 串 → u64 解析失败即内部错）；
/// hook 换装期同线程无重入（LoadRequestedModules 同步驱动），覆盖见 tests/node/vm.rs。
fn vm_mod_link_graph(
    cx: &mut mozjs::context::JSContext,
    frame: Frame,
    entry: u64,
    sess: &str,
) -> bool {
    use mozjs::rust::wrappers2::{LoadRequestedModules, ModuleLink};
    let fail = |cx: &mut mozjs::context::JSContext, msg: &str| {
        report_error(cx, msg);
        false
    };
    let v: serde_json::Value = serde_json::from_str(sess).unwrap_or(serde_json::Value::Null);
    let mut modules = Vec::new();
    let mut edges = std::collections::HashMap::new();
    for m in v.get("modules").and_then(|m| m.as_array()).into_iter().flatten() {
        let Some(id) = m.as_str().and_then(|s| s.parse::<u64>().ok()) else {
            return fail(cx, "ERR_VM_MODULE_LINK_FAILURE: vm link session has a bad module id");
        };
        modules.push(id);
    }
    for e in v.get("edges").and_then(|m| m.as_array()).into_iter().flatten() {
        let arr = e.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        if arr.len() != 3 {
            return fail(cx, "ERR_VM_MODULE_LINK_FAILURE: vm link session has a bad edge");
        }
        let (Some(r), Some(sp), Some(d)) = (
            arr[0].as_str().and_then(|s| s.parse::<u64>().ok()),
            arr[1].as_str(),
            arr[2].as_str().and_then(|s| s.parse::<u64>().ok()),
        ) else {
            return fail(cx, "ERR_VM_MODULE_LINK_FAILURE: vm link session has a bad edge");
        };
        edges.insert((r, sp.to_owned()), d);
    }
    if !modules.contains(&entry) {
        return fail(cx, "ERR_VM_MODULE_LINK_FAILURE: vm link session misses the entry");
    }
    // 存在性 + ctx 一致（JS 已查，此处引擎接线前复核；跨 ctx 按真机拒）。
    let mut entry_ctx = None;
    for id in modules.iter().chain(edges.values()) {
        let Some((_, ctx, _, _, _)) = state::vm_mod_get(*id) else {
            return fail(cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
        };
        match entry_ctx {
            None => entry_ctx = Some(ctx),
            Some(c) if c != ctx => {
                return fail(
                    cx,
                    "ERR_VM_MODULE_DIFFERENT_CONTEXT: Linked modules must use the same context",
                );
            }
            Some(_) => {}
        }
    }
    let Some((record, _, _, _, _)) = state::vm_mod_get(entry) else {
        return fail(cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
    };
    VM_LINK_SESSIONS.with(|s| {
        s.borrow_mut().push(VmLinkSession { edges, modules: modules.clone() });
    });
    VM_LINK_ERROR.with(|e| *e.borrow_mut() = None);
    // SAFETY: 单 Runtime 进程级 hook；换装—驱动—复原全同步，无重入点
    let rt = unsafe { mozjs::jsapi::JS_GetRuntime(cx.raw_cx()) };
    let prev = unsafe { mozjs::jsapi::GetModuleLoadHook(rt) };
    unsafe { mozjs::jsapi::SetModuleLoadHook(rt, Some(vm_link_hook)) };
    let out = {
        rooted!(&in(cx) let entry_root: *mut JSObject = record);
        rooted!(&in(cx) let undef = UndefinedValue());
        // SAFETY: entry 有效 rooted；hook 按会话表命中（miss/属性墙即 rejected 暂存）
        let ok = unsafe {
            LoadRequestedModules(
                cx,
                entry_root.handle(),
                undef.handle(),
                Some(vm_link_resolved),
                Some(vm_link_rejected),
            )
        };
        // rejected 先行（属性墙等同步拒收：LoadRequestedModules 照常 true，
        // 真因在 stash；此时接线必败，不再进 ModuleLink 掩盖真因）。
        if VM_LINK_ERROR.with(|e| e.borrow().is_some()) {
            None
        } else if !ok {
            None
        } else {
            // SAFETY: 加载态已就绪；失败 pending 原样透传（SyntaxError 等引擎错）
            let linked = unsafe { ModuleLink(cx, entry_root.handle()) };
            Some(linked)
        }
    };
    unsafe { mozjs::jsapi::SetModuleLoadHook(rt, prev) };
    VM_LINK_SESSIONS.with(|s| {
        s.borrow_mut().pop();
    });
    match out {
        None => {
            if let Some(msg) = VM_LINK_ERROR.with(|e| e.borrow_mut().take()) {
                fail(cx, &msg)
            } else {
                // SAFETY: 仅清残留 pending（hook 已按约定处理），防污染后继
                unsafe { mozjs::jsapi::JS_ClearPendingException(cx.raw_cx()) };
                fail(cx, "ERR_VM_MODULE_LINK_FAILURE: vm module dependencies failed to load")
            }
        }
        Some(false) => {
            rooted!(&in(cx) let mut exc = UndefinedValue());
            // SAFETY: ModuleLink 失败必有 pending；error_info 消费为定位错
            //（compile 路同款：先取 info，再读异常名，信封重建保 instanceof）
            match { let i = mozjs::rust::error_info_from_exception_stack(cx, exc.handle_mut()); crate::jsapi_glue::fill_message(cx, i, exc.get()) } {
                Some(info) => {
                    let name = exc_name(cx, exc.get()).unwrap_or_else(|| "Error".to_owned());
                    throw_vm(cx, &name, &info.message);
                    false
                }
                None => fail(cx, "ERR_VM_MODULE_LINK_FAILURE: vm module failed to link"),
            }
        }
        Some(true) => {
            for id in modules.iter() {
                state::vm_mod_set_linked(*id);
            }
            frame.set_rval(UndefinedValue());
            true
        }
    }
}

/// 成功置旗（无状态可记；失败侧 hook 暂存）。
unsafe extern "C" fn vm_link_resolved(
    _cx: *mut mozjs::jsapi::JSContext,
    _v: mozjs::jsapi::Handle<JSVal>,
) -> bool {
    true
}

/// 会话 miss/引擎拒收（属性墙等）：错误值串化暂存；hook 按约定返回 false。
/// 注：SM153 仅支持 `type` 属性键（Modules.cpp `We only support "type"`），
///
/// 其余键走 `FailWithUnsupportedAttributeException` 经 rejected 回调到此。
unsafe extern "C" fn vm_link_rejected(
    cx_raw: *mut mozjs::jsapi::JSContext,
    _host_defined: mozjs::jsapi::Handle<JSVal>,
    error: mozjs::jsapi::Handle<JSVal>,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效
    let mut cx = unsafe { wrap_cx(cx_raw) };
    // SAFETY: 参数 Handle 在调用期内有效
    let msg = unsafe { value_to_string(&mut cx, *error.ptr) };
    VM_LINK_ERROR.with(|e| {
        *e.borrow_mut() = Some(format!("ERR_VM_MODULE_LINK_FAILURE: {msg}"));
    });
    // SAFETY: 拒绝侧清场，避免 pending 污染外层接线判定
    unsafe { mozjs::jsapi::JS_ClearPendingException(cx.raw_cx()) };
    true
}

/// SAFETY: 引擎以有效参数回调（HostLoadImportedModule 约定；签名与进程 hook 同构）。
unsafe extern "C" fn vm_link_hook(
    cx_raw: *mut mozjs::jsapi::JSContext,
    referrer: mozjs::jsapi::Handle<*mut mozjs::jsapi::JSScript>,
    module_request: mozjs::jsapi::Handle<*mut JSObject>,
    _host_defined: mozjs::jsapi::Handle<JSVal>,
    payload: mozjs::jsapi::Handle<JSVal>,
    _line: u32,
    _col: mozjs::jsapi::ColumnNumberOneOrigin,
) -> bool {
    use mozjs::rust::wrappers2::{
        FinishLoadingImportedModule, GetModuleRequestSpecifier, GetModuleScript,
    };
    // SAFETY: 引擎回调提供的 raw cx 有效
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let miss = |cx: &mut mozjs::context::JSContext, msg: &str| {
        VM_LINK_ERROR.with(|e| *e.borrow_mut() = Some(msg.to_owned()));
        report_error(cx, msg);
        false
    };
    // SAFETY: 参数 Handle 在调用期内有效
    let referrer_ptr = unsafe { *referrer.ptr };
    // 会话定位（内层优先）+ referrer→模块 id（脚本指针比对）。
    let found = VM_LINK_SESSIONS.with(|s| {
        let sessions = s.borrow();
        let mut out = None;
        for sess in sessions.iter().rev() {
            for id in sess.modules.iter() {
                let rec = state::vm_mod_get(*id).map(|(r, _, _, _, _)| r);
                if rec.is_none() {
                    continue;
                }
                rooted!(&in(cx) let rec_root: *mut JSObject = rec.unwrap());
                // SAFETY: rec 有效 rooted；GetModuleScript 纯读
                let script = unsafe { GetModuleScript(rec_root.handle()) };
                if !script.is_null() && script == referrer_ptr {
                    out = Some((*id, sess as *const VmLinkSession));
                    break;
                }
            }
            if out.is_some() {
                break;
            }
        }
        out.map(|(id, p)| (id, unsafe { &*p }))
    });
    let Some((referrer_id, sess)) = found else {
        return miss(&mut cx, "ERR_VM_MODULE_LINK_FAILURE: vm link request misses its session");
    };
    // 同一会话必同 ctx（入口复核过）；进其 realm 读请求（§4.1 铁律）。
    let Some((_, ctx, _, _, _)) = state::vm_mod_get(referrer_id) else {
        return miss(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
    };
    let Some(ptr) = mod_global(&mut cx, ctx) else {
        return miss(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
    };
    rooted!(&in(cx) let global = ptr);
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let cx: &mut mozjs::context::JSContext = &mut realm;
    // SAFETY: request Handle 调用期内有效；转成带生命期的 rust Handle
    let spec = unsafe {
        let req = mozjs::gc::Handle::from_marked_location(&*module_request.ptr);
        let jsstr = GetModuleRequestSpecifier(cx, req);
        if jsstr.is_null() {
            return miss(cx, "ERR_VM_MODULE_LINK_FAILURE: cannot read import specifier");
        }
        mozjs::conversions::jsstr_to_string(cx, std::ptr::NonNull::new_unchecked(jsstr))
    };
    let Some(dep) = sess.edges.get(&(referrer_id, spec.clone())) else {
        return miss(
            cx,
            &format!("ERR_VM_MODULE_LINK_FAILURE: vm link session misses a dependency"),
        );
    };
    let Some((record, _, _, _, _)) = state::vm_mod_get(*dep) else {
        return miss(cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
    };
    let dynamic = {
        // SAFETY: payload 只读出（是否 Promise 决定动/静路径）
        let v = unsafe { *payload.ptr };
        v.is_object() && {
            let obj = v.to_object();
            rooted!(&in(cx) let obj_root: *mut JSObject = obj);
            // SAFETY: obj_root 为有效 rooted 对象；谓词无 GC 点
            unsafe { mozjs::jsapi::IsPromiseObject(raw_handle(obj_root.as_ptr())) }
        }
    };
    rooted!(&in(cx) let record_root: *mut JSObject = record);
    // SAFETY: 槽位转换（from_marked_location）+ safe wrapper 调用
    unsafe {
        let referrer_h = mozjs::gc::Handle::from_marked_location(&*referrer.ptr);
        let request = mozjs::gc::Handle::from_marked_location(&*module_request.ptr);
        let payload_h = mozjs::gc::Handle::from_marked_location(&*payload.ptr);
        FinishLoadingImportedModule(cx, referrer_h, request, payload_h, record_root.handle(), dynamic)
    }
}

// ── Promise 内态 ─────────────────────────────────────────────────────────────

/// Promise 结算内态（`util.inspect` 显示用）：`__wjs2_promise_state(p)` → 0/1/2
///（pending/fulfilled/rejected）；`__wjs2_promise_result(p)` → 结算值（仅已结算）。
/// UNSAFE-BOUNDARY: p 为 Promise 对象（非即 TypeError）；只读 state/result
///（`jsapi_glue::promise_settled_value`，不推进 jobqueue），覆盖 tests/node/vm.rs。
pub unsafe extern "C" fn promise_state(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_object() {
        report_error(&mut cx, "TypeError: promise state needs a promise argument");
        return false;
    }
    rooted!(&in(cx) let obj: *mut JSObject = frame.arg(0).to_object());
    let state_no = match crate::jsapi_glue::promise_settled_value(&mut cx, obj.get()) {
        Some(None) => 0,
        Some(Some((true, _))) => 1,
        Some(Some((false, _))) => 2,
        None => {
            report_error(&mut cx, "Error: could not read promise state");
            return false;
        }
    };
    frame.set_rval(mozjs::jsval::Int32Value(state_no));
    true
}

/// Promise 结算值（`util.inspect` 显示用）：`__wjs2_promise_result(p)` → 结算值。
/// UNSAFE-BOUNDARY: p 为 Promise 对象（CCW 先解包；非即 TypeError）；只读
/// result（`jsapi_glue::promise_settled_value`，不推进 jobqueue），覆盖 tests/node/vm.rs。
pub unsafe extern "C" fn promise_result(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 promise_state
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_object() {
        report_error(&mut cx, "TypeError: promise result needs a promise argument");
        return false;
    }
    rooted!(&in(cx) let obj: *mut JSObject = frame.arg(0).to_object());
    // SAFETY: obj 为调用方 rooted；解包 CCW 得底层对象（GetPromiseState 同源做法）
    let raw = unsafe { mozjs::glue::UnwrapObjectDynamic(obj.get(), cx.raw_cx(), false) };
    if raw.is_null() {
        report_error(&mut cx, "TypeError: promise result needs a promise argument");
        return false;
    }
    rooted!(&in(cx) let unwrapped: *mut JSObject = raw);
    match crate::jsapi_glue::promise_settled_value(&mut cx, unwrapped.get()) {
        Some(Some((_, v))) => {
            frame.set_rval(v);
            true
        }
        _ => {
            report_error(&mut cx, "TypeError: promise result needs a settled promise");
            return false;
        }
    }
}
