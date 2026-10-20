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

use crate::jsapi_glue::{report_error, wrap_cx, Frame};
use crate::state;

use super::vm::{arg_string, lookup_global};
use super::vm_error::throw_vm;

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
    let c_filename =
        std::ffi::CString::new(identifier.as_str()).unwrap_or_else(|_| c"vm_module.js".to_owned());
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let options = CompileOptionsWrapper::new(&realm, c_filename, 1);
    let mut src = transform_str_to_source_text(&loaded.js);
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
    let id = state::vm_mod_add(ctx, identifier, record, has_imports, loaded.imports);
    id.to_string().to_jsval(&mut realm, frame.rval_mut());
    true
}

/// 取静态依赖表：`__wjs2_vm_mod_deps(modId)` → JSON 数组串。
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
    let Some(json) = state::vm_mod_deps_json(id) else {
        report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm Module has been released");
        return false;
    };
    json.to_jsval(&mut cx, frame.rval_mut());
    true
}

/// 链接模块：`__wjs2_vm_link(modId)` → undefined。
/// 零导入恒过；带导入 v1 报 `ERR_VM_MODULE_LINK_FAILURE`（linker 切片后续）。
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
    use super::vm::{vm_timeout_arg, vm_timeout_peek_ms, vm_timeout_rearm, vm_timeout_report, VmTimeoutGuard};
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
    let _timeout_guard = VmTimeoutGuard::enter(cx_raw, timeout_ms);
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
        // 超时命中优先（同 vm_run：先补发中断再排空孤儿任务）。
        if vm_timeout_peek_ms().is_some() {
            use mozjs::jsapi::RunJobs;
            vm_timeout_rearm(cx_raw);
            // SAFETY: realm 内排空（主循环同款）
            unsafe { RunJobs((&mut realm).raw_cx()) };
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
            use mozjs::jsapi::RunJobs;
            // 中止 completion 自身的 unhandled 认领（因恒 undefined 的终止
            // rejection，真机不报告；调用方经 evaluate() 认领超时错，此处
            // 等效 Handled 摘除，防事件循环尾 fatal）。
            let rval_ptr = rval.to_object();
            state::with_rooted(|s| s.unhandled.retain(|h| h.get() != rval_ptr));
            vm_timeout_rearm(cx_raw);
            // SAFETY: realm 内排空（主循环同款）
            unsafe { RunJobs((&mut realm).raw_cx()) };
            if vm_timeout_report(&mut realm, timeout_ms) {
                return false;
            }
        } else {
            super::vm::vm_timeout_fired_ms();
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
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    // SAFETY: record 有效；返回的 namespace 由记录保活（引擎内边）
    let ns = unsafe { GetModuleNamespace(&mut realm, record_root.handle()) };
    if ns.is_null() {
        throw_vm(&mut realm, "Error", "vm could not read module namespace");
        return false;
    }
    rooted!(&in(&mut realm) let ns_root: *mut JSObject = ns);
    frame.set_rval(mozjs::jsval::ObjectValue(ns_root.get()));
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
