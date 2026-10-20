//! `node:vm`：同 Runtime 多 global 沙箱（9f-1，引擎深水）。
//!
//! 落法：每个 `createContext` 经 `JS_NewGlobalObject(SIMPLE_GLOBAL_CLASS)` 建独立
//! global（新 compartment，标准类走同一套懒 resolve，无需 prelude；job queue 是
//! per-context 共享，无需重装）。global 以 `Box<Heap>` 入 `state::vm_contexts`
//!（§4.40 定址），id 单调不复用；JS 侧 `FinalizationRegistry` 自动 release。
//!
//! 求值走 `evaluate_script`（自进目标 realm）+ 同步一轮 `RunJobs`（vm 内 promise
//! 当轮决议，更接近 Node 的 afterEvaluate 语义）；完成值对象跨 compartment 以
//! CCW 传递（透明访问，`instanceof` 等跨域判定以宿主为准，记档）。
//! 沙箱同步为快照式（create/run 前 sync-in，run 后 sync-out，非活绑定，见下）。
//!
//! 偏差记档：
//! - 沙箱非活绑定：`createContext` 后改 sandbox 属性需下次 run 前才 sync-in；
//!   sync-out 只回写"非标准初始键"（标准构造器名视为上下文自有，不污染 sandbox）。
//! - 错误只保 name/message（ Norton 式 `{name,message}` 包络重建同名 Error；
//!   抛非 Error 值退化为 `Error(String(v))`；identity/stack 不跨域）。
//! - `timeout`/`breakOnSigint` 只校验不执行（能正常结束的脚本行为一致；死循环
//!   与主脚本同等待遇——进程 hang，测试禁写此类用例）；`cachedData`/
//!   `produceCachedData` 接受忽略（无字节码缓存，每次 run 重解析）。
//! - `microtaskMode` 接受忽略（恒 afterEvaluate 等效：run 后同步排空一轮）。
//! - `importModuleDynamically`/模块系：9i-1 落地 `Module` 基类 +
//!   `SourceTextModule`（零导入全链：compile/link/evaluate/namespace/status；
//!   带导入者 link 报 `ERR_VM_MODULE_LINK_FAILURE`，linker 切片后续）+
//!   `SyntheticModule`（纯 JS：evaluateCallback 回填导出）；
//!   vm 内 `import()` 仍走主模块管线，行为未定义（记档）。
//! - `measureMemory` 恒 reject `ERR_CONTEXT_NOT_INITIALIZED`（实验警告照发）。

use mozjs::jsapi::{JSObject, RunJobs};
use mozjs::jsval::{BooleanValue, JSVal, UndefinedValue};
use mozjs::conversions::ToJSValConvertible as _;
use mozjs::realm::AutoRealm;
use mozjs::rooted;
use mozjs::rust::{CompileOptionsWrapper, RealmOptions, SIMPLE_GLOBAL_CLASS, transform_str_to_source_text};

use crate::jsapi_glue::{
    define_prop, exc_name_is, get_prop_value, own_keys_json, report_error, same_value,
    value_to_string, wrap_cx, Frame,
};
use crate::state;

use super::vm_error::{throw_vm, vm_stk_envelope};


/// 字符串实参（缺省/非串 → TypeError 错，None）。
pub(super) fn arg_string(
    cx: &mut mozjs::context::JSContext,
    frame: &Frame,
    i: u32,
    what: &str,
) -> Option<String> {
    if frame.argc() <= i {
        report_error(cx, &format!("TypeError: {what} needs an argument"));
        return None;
    }
    Some(value_to_string(cx, frame.arg(i)))
}

/// vm id 实参（字符串形态数字，约定见 §4.33）。
fn arg_id(cx: &mut mozjs::context::JSContext, frame: &Frame, i: u32) -> Option<u64> {
    let s = arg_string(cx, frame, i, "vm context")?;
    s.parse::<u64>().ok().or_else(|| {
        report_error(cx, "TypeError: vm context id must be a context id string");
        None
    })
}

/// 查注册表并重 root（调用方 realm 内使用；查无即 TypeError 错）。
/// 返回裸指针：调用方必须在同一作用域内 rooted 后用（无 GC 间隙）。
pub(super) fn lookup_global(cx: &mut mozjs::context::JSContext, id: u64) -> Option<*mut JSObject> {
    match state::vm_global(id) {
        Some(g) if !g.is_null() => Some(g),
        _ => {
            report_error(cx, "ERR_INVALID_ARG_TYPE: contextifiedObject must be a vm.Context");
            None
        }
    }
}


/// 求值超时底座（纯搬移归位见 `vm_timeout.rs`；调用方路径不变）。
pub(super) use super::vm_timeout::{
    vm_timeout_arg, vm_timeout_drain_orphans, vm_timeout_peek_ms, vm_timeout_report,
    VmTimeoutGuard,
};

/// 建上下文：新 global（新 compartment）+ 入表，返回 id 字符串。
/// `__wjs2_vm_create()` → id。
pub unsafe extern "C" fn vm_create(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let mut options = RealmOptions::default();
    // Atomics/SharedArrayBuffer 与主域同开关（jsdom 等生态取此面）。
    options.creationOptions_.sharedMemoryAndAtomics_ = true;
    rooted!(&in(cx) let global = unsafe {
        mozjs::rust::wrappers2::JS_NewGlobalObject(
            &mut cx,
            &SIMPLE_GLOBAL_CLASS,
            std::ptr::null_mut(),
            mozjs::jsapi::OnNewGlobalHookOption::FireOnNewGlobalHook,
            &*options,
        )
    });
    if global.is_null() {
        report_error(&mut cx, "OperationError: vm could not create a context");
        return false;
    }
    let id = state::vm_add(global.get());
    id.to_string().to_jsval(&mut cx, frame.rval_mut());
    true
}

/// `__wjs2_vm_global(id)` → 该 context 的 global 对象本体（DONT_CONTEXTIFY 用：
/// jsdom 29 拿它当 window 直装 DOM 全局，写入即落 vm global）。
/// UNSAFE-BOUNDARY: id 为 vm 表有效 id；出参经 rooted（覆盖 tests/node/vm.rs）。
pub unsafe extern "C" fn vm_global(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_string(&mut cx, &frame, 0, "vm global") {
        Some(s) => match s.parse::<u64>() {
            Ok(v) => v,
            Err(_) => {
                report_error(&mut cx, "ERR_INVALID_ARG_TYPE: vm global id must be a module id string");
                return false;
            }
        },
        None => return false,
    };
    let Some(g) = (match state::vm_global(id) {
        Some(g) if !g.is_null() => Some(g),
        _ => None,
    }) else {
        report_error(&mut cx, "ERR_INVALID_ARG_TYPE: contextifiedObject must be a vm.Context");
        return false;
    };
    rooted!(&in(cx) let g_root: *mut JSObject = g);
    frame.set_rval(mozjs::jsval::ObjectValue(g_root.get()));
    true
}

/// 语法预检（`new Script`/`compileFunction` 构造期用，不执行）。
/// `__wjs2_vm_compile(code, filename)` → undefined；失败抛包络 SyntaxError。
pub unsafe extern "C" fn vm_compile(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let code = match arg_string(&mut cx, &frame, 0, "vm compile") {
        Some(s) => s,
        None => return false,
    };
    let filename = arg_string(&mut cx, &frame, 1, "vm compile").unwrap_or_default();
    let c_filename = std::ffi::CString::new(filename.as_str()).unwrap_or_else(|_| c"vm.js".to_owned());
    let options = CompileOptionsWrapper::new(&cx, c_filename, 1);
    let mut src = transform_str_to_source_text(&code);
    // SAFETY: cx 在 realm 内；options/src 存活到调用返回；空指针即语法失败
    let script = unsafe { mozjs::rust::wrappers2::Compile1(&mut cx, options.ptr, &mut src) };
    if script.is_null() {
        rooted!(&in(cx) let mut exc = UndefinedValue());
        let msg = match mozjs::rust::error_info_from_exception_stack(&mut cx, exc.handle_mut()) {
            Some(info) => info.message,
            None => "invalid script".to_string(),
        };
        throw_vm(&mut cx, "SyntaxError", &msg);
        return false;
    }
    frame.set_rval(UndefinedValue());
    true
}

/// 在指定上下文求值（`evaluate_script` 自进目标 realm；成功后同步排空一轮 microtask）。
/// `__wjs2_vm_run(id, code, filename, timeoutMs?)` → completion；失败抛包络错。
pub unsafe extern "C" fn vm_run(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let code = match arg_string(&mut cx, &frame, 1, "vm run") {
        Some(s) => s,
        None => return false,
    };
    let filename = arg_string(&mut cx, &frame, 2, "vm run").unwrap_or_default();
    let timeout_ms = vm_timeout_arg(&mut cx, &frame, 3);
    vm_run_core(&mut cx, &frame, id, code, filename, timeout_ms)
}

/// 完成值跨域包装（非 promise 对象进调用方 compartment，跨域 `===` 恒等；
/// promise 保持裸值——结算内态读取需原生对象，见 4.296；
/// global 保持裸值——身份基础设施（bookkeeping/DONT_CONTEXTIFY），见 4.296）。
/// 前置：cx 有效；caller 为调用方 global（可空，即不包装）。
pub(super) fn vm_wrap_rval(
    cx: &mut mozjs::context::JSContext,
    caller: *mut JSObject,
    rval: JSVal,
) -> JSVal {
    if caller.is_null() || !rval.is_object() {
        return rval;
    }
    rooted!(&in(cx) let rval_root = rval);
    let obj = rval_root.get().to_object();
    rooted!(&in(cx) let obj_root: *mut JSObject = obj);
    // SAFETY: obj_root 为调用方 rooted 的对象；谓词无 GC 点
    let is_promise = unsafe { mozjs::jsapi::IsPromiseObject(crate::jsapi_glue::raw_handle(obj_root.as_ptr())) };
    if is_promise {
        return rval_root.get();
    }
    // global 免包装（身份基础设施；DONT_CONTEXTIFY 的 `this === w` 口径）。
    if state::vm_global_all().iter().any(|g| *g == obj) {
        return rval_root.get();
    }
    rooted!(&in(cx) let caller_root: *mut JSObject = caller);
    let mut realm = AutoRealm::new_from_handle(cx, caller_root.handle());
    crate::jsapi_glue::wrap_value(&mut realm, rval_root.get()).unwrap_or(rval)
}

/// 求值核心（`vm_run` 与动态导入 Script 帧共用；参数已备好）。
/// 超时守卫/realm/同步排空语义与 `vm_run` 完全一致。
pub(crate) fn vm_run_core(
    mut cx: &mut mozjs::context::JSContext,
    frame: &Frame,
    id: u64,
    code: String,
    filename: String,
    timeout_ms: u64,
) -> bool {
    let cx_raw = unsafe { cx.raw_cx() };
    let caller = unsafe { mozjs::jsapi::CurrentGlobalOrNull(cx_raw) };
    let mut _timeout_guard = VmTimeoutGuard::enter(cx_raw, timeout_ms);
    let ptr = match lookup_global(&mut cx, id) {
        Some(p) => p,
        None => return false,
    };
    rooted!(&in(cx) let global = ptr);
    rooted!(&in(cx) let mut rval = UndefinedValue());
    let filename_c =
        std::ffi::CString::new(filename.as_str()).unwrap_or_else(|_| c"vm.js".to_owned());
    let options = CompileOptionsWrapper::new(&cx, filename_c, 1);
    if mozjs::rust::evaluate_script(&mut cx, global.handle(), &code, rval.handle_mut(), options).is_err() {
        // §4.1：evaluate 返回后已出 realm，重进目标 realm 再读异常
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        // 超时命中优先（中断残留不清，直接报真机口径错）。
        if vm_timeout_peek_ms().is_some() {
            // 孤儿微任务排空（真机 afterEvaluate 口径：逃逸 promise 不得存活）。
            // 限时武装（见 4.295）：原地 rearm 即全杀（含无辜兄弟 job）；
            // 先摘原表项再以短预算重武装——有限 job 照常结算，无限逃逸到期中止。
            vm_timeout_drain_orphans(cx_raw, &mut realm, &mut _timeout_guard);
        }
        if vm_timeout_report(&mut realm, timeout_ms) {
            return false;
        }
        // 先取原始异常对象暂存（take 即清 pending），再恢复 pending 供 error_info
        // 读 name/message/位置（读完再清）——__vmCall 优先取原物（保 vm realm
        // 身份/原型/栈，跨域 instanceof 点名），信封重建只作兜底。
        if let Some(v) = crate::jsapi_glue::take_pending_exception(&mut realm) {
            rooted!(&in(&mut realm) let orig_root: JSVal = v);
            state::with_rooted(|s| s.vm_last_error.set(orig_root.get()));
            crate::jsapi_glue::set_pending_exception(&mut realm, orig_root.get());
        }
        rooted!(&in(&mut realm) let mut exc = UndefinedValue());
        let (name, message) = match mozjs::rust::error_info_from_exception_stack(&mut realm, exc.handle_mut()) {
            Some(info) => {
                let name = ["SyntaxError", "RangeError", "ReferenceError", "TypeError", "URIError", "EvalError"]
                    .into_iter()
                    .find(|n| exc_name_is(&mut realm, exc.get(), n))
                    .unwrap_or("Error");
                (name.to_string(), vm_stk_envelope(&filename, &code, &info))
            }
            None => ("Error".to_string(), value_to_string(&mut realm, exc.get())),
        };
        throw_vm(&mut realm, &name, &message);
        return false;
    }
    // Node afterEvaluate 等效：当轮排空 vm 内 promise 反应（同一 JobQueue）；
    // 排空后命中检查（微任务逃逸 loop：escape-2 口径）。
    {
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        // SAFETY: realm 内排空内部 job queue（主循环同款）
        unsafe { RunJobs((&mut realm).raw_cx()) };
        // 微任务期命中的超时：求值本身已返回，此处改报。
        if vm_timeout_report(&mut realm, timeout_ms) {
            return false;
        }
    }
    frame.set_rval(vm_wrap_rval(&mut cx, caller, rval.get()));
    true
}

/// 主 global 求值（`runInThisContext`；inspector_eval 同款嵌套求值）。
/// `__wjs2_vm_run_this(code, filename, timeoutMs?)` → completion；失败抛包络错。
pub unsafe extern "C" fn vm_run_this(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let code = match arg_string(&mut cx, &frame, 0, "vm run") {
        Some(s) => s,
        None => return false,
    };
    let filename = arg_string(&mut cx, &frame, 1, "vm run").unwrap_or_default();
    let timeout_ms = vm_timeout_arg(&mut cx, &frame, 2);
    vm_run_this_core(&mut cx, &frame, code, filename, timeout_ms)
}

/// 主域求值核心（`vm_run_this` 与动态导入 Script 帧共用；参数已备好）。
pub(crate) fn vm_run_this_core(
    mut cx: &mut mozjs::context::JSContext,
    frame: &Frame,
    code: String,
    filename: String,
    timeout_ms: u64,
) -> bool {
    let cx_raw = unsafe { cx.raw_cx() };
    let caller = unsafe { mozjs::jsapi::CurrentGlobalOrNull(cx_raw) };
    let mut _timeout_guard = VmTimeoutGuard::enter(cx_raw, timeout_ms);
    rooted!(&in(cx) let global = state::global());
    rooted!(&in(cx) let mut rval = UndefinedValue());
    let filename_c =
        std::ffi::CString::new(filename.as_str()).unwrap_or_else(|_| c"vm.js".to_owned());
    let options = CompileOptionsWrapper::new(&cx, filename_c, 1);
    if mozjs::rust::evaluate_script(&mut cx, global.handle(), &code, rval.handle_mut(), options).is_err() {
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        // 超时命中优先（同 vm_run：限时武装排空孤儿，见 4.295）。
        if vm_timeout_peek_ms().is_some() {
            vm_timeout_drain_orphans(cx_raw, &mut realm, &mut _timeout_guard);
        }
        if vm_timeout_report(&mut realm, timeout_ms) {
            return false;
        }
        // 同 vm_run：取原物暂存 + 恢复 pending。
        if let Some(v) = crate::jsapi_glue::take_pending_exception(&mut realm) {
            rooted!(&in(&mut realm) let orig_root: JSVal = v);
            state::with_rooted(|s| s.vm_last_error.set(orig_root.get()));
            crate::jsapi_glue::set_pending_exception(&mut realm, orig_root.get());
        }
        rooted!(&in(&mut realm) let mut exc = UndefinedValue());
        let (name, message) = match mozjs::rust::error_info_from_exception_stack(&mut realm, exc.handle_mut()) {
            Some(info) => {
                let name = ["SyntaxError", "RangeError", "ReferenceError", "TypeError", "URIError", "EvalError"]
                    .into_iter()
                    .find(|n| exc_name_is(&mut realm, exc.get(), n))
                    .unwrap_or("Error");
                (name.to_string(), vm_stk_envelope(&filename, &code, &info))
            }
            None => ("Error".to_string(), value_to_string(&mut realm, exc.get())),
        };
        throw_vm(&mut realm, &name, &message);
        return false;
    }
    {
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        // SAFETY: realm 内排空（主循环同款）
        unsafe { RunJobs((&mut realm).raw_cx()) };
        // 微任务期命中的超时（同 vm_run）。
        if vm_timeout_report(&mut realm, timeout_ms) {
            return false;
        }
    }
    frame.set_rval(vm_wrap_rval(&mut cx, caller, rval.get()));
    true
}

/// 编译函数（`compileFunction`；目标 realm 由 id 定，`""` 为主 global）。
/// `__wjs2_vm_compile_fn(idStr, paramsCsv, code, filename)` → function。
pub unsafe extern "C" fn vm_compile_fn(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id_s = arg_string(&mut cx, &frame, 0, "vm compileFunction").unwrap_or_default();
    let params = arg_string(&mut cx, &frame, 1, "vm compileFunction").unwrap_or_default();
    let code = match arg_string(&mut cx, &frame, 2, "vm compileFunction") {
        Some(s) => s,
        None => return false,
    };
    let filename = arg_string(&mut cx, &frame, 3, "vm compileFunction").unwrap_or_default();
    // 目标 global：空串即主 global（Node 默认 parsingContext=主上下文口径）。
    let ptr = if id_s.is_empty() {
        state::global()
    } else {
        let id = match id_s.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                report_error(&mut cx, "ERR_INVALID_ARG_TYPE: options.parsingContext must be a Context");
                return false;
            }
        };
        match lookup_global(&mut cx, id) {
            Some(p) => p,
            None => return false,
        }
    };
    if ptr.is_null() {
        report_error(&mut cx, "OperationError: vm has no global");
        return false;
    }
    rooted!(&in(cx) let global = ptr);
    rooted!(&in(cx) let mut rval = UndefinedValue());
    let filename_c =
        std::ffi::CString::new(filename.as_str()).unwrap_or_else(|_| c"vm.js".to_owned());
    let options = CompileOptionsWrapper::new(&cx, filename_c, 1);
    // 体级语法预检（声明位包络）：compileFunction 的体必须能独立作为函数体解析。
    // 表达式包络 `(function(){ … })` 会被体首 `});` 提前闭合吸收（测试
    // `});\n(function(){…})();\n(function() {` 套件点名）——真机 V8 以声明位
    // 包络编译，未闭合括号悬到 EOF 即错；声明位同构，预检失败即 SyntaxError。
    // 预检产物弃置，实编译仍走表达式包络（toString === `function (p) {\n…\n}`）。
    let wrapped_chk = format!("function __wjs2_vm_body_chk({params}) {{\n{code}\n}}");
    {
        let mut src_chk = transform_str_to_source_text(&wrapped_chk);
        // SAFETY: cx 在 realm 内；options/src 存活到调用返回；空指针即语法失败
        let chk = unsafe { mozjs::rust::wrappers2::Compile1(&mut cx, options.ptr, &mut src_chk) };
        if chk.is_null() {
            rooted!(&in(cx) let mut exc = UndefinedValue());
            let msg = match mozjs::rust::error_info_from_exception_stack(&mut cx, exc.handle_mut()) {
                Some(info) => info.message,
                None => "invalid function".to_string(),
            };
            throw_vm(&mut cx, "SyntaxError", &msg);
            return false;
        }
    }
    // 包成匿名函数表达式求值（toString 对真机：`function (p) {\n…\n}`——
    // 无 "anonymous" 名、params 后无换行，真机 compileFunction fn.name === ""）。
    // 动态 with 剥离（SM153 仅认 type 键；剥离换行保留，attrs 行列仍对原文，
    // 见 vm_dyn；JS 侧包装源码保持原文，字节一致）。
    let compile_code = super::vm_module::requests::strip_dynamic_with(&code, "js")
        .unwrap_or(code);
    let wrapped = format!("(function ({params}) {{\n{compile_code}\n}})");
    if mozjs::rust::evaluate_script(&mut cx, global.handle(), &wrapped, rval.handle_mut(), options).is_err() {
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        rooted!(&in(&mut realm) let mut exc = UndefinedValue());
        let (name, message) = match mozjs::rust::error_info_from_exception_stack(&mut realm, exc.handle_mut()) {
            Some(info) => ("SyntaxError".to_string(), info.message),
            None => ("SyntaxError".to_string(), "invalid function".to_string()),
        };
        throw_vm(&mut realm, &name, &message);
        return false;
    }
    if !rval.is_object() || unsafe { !mozjs::jsapi::IsFunctionObject(rval.to_object()) } {
        throw_vm(&mut cx, "Error", "vm did not produce a function");
        return false;
    }
    let caller = unsafe { mozjs::jsapi::CurrentGlobalOrNull(cx.raw_cx()) };
    frame.set_rval(vm_wrap_rval(&mut cx, caller, rval.get()));
    true
}

/// 沙箱属性写入目标 global（sync-in 用；值可跨 compartment，引擎包 CCW）。
/// 语义：`define` 优先（可枚举数据描述符）；已有属性重定义失败回落赋值；
/// 双失败（目标只读无 setter）即跳过——真机以目标描述符为准（`inherited_properties`
/// 只读继承、`preserves-property` 等），源端不同步，不抛（10c-3 回落的静默形）。
/// `__wjs2_vm_set(id, key, value)` → undefined。
pub unsafe extern "C" fn vm_set(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let key = match arg_string(&mut cx, &frame, 1, "vm sandbox key") {
        Some(s) => s,
        None => return false,
    };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: vm sandbox set needs a value");
        return false;
    }
    let val = frame.arg(2);
    let ptr = match lookup_global(&mut cx, id) {
        Some(p) => p,
        None => return false,
    };
    rooted!(&in(cx) let global = ptr);
    rooted!(&in(cx) let val_root = val);
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let Ok(c_key) = std::ffi::CString::new(key.as_str()) else {
        report_error(&mut realm, "TypeError: vm sandbox key must not contain NUL");
        return false;
    };
    // 跨域恒等统一（见 4.296）：值先包装进目标 compartment 再落定；
    // global 免包装（身份基础设施，与 rval 侧同源豁免）。
    let val_wrapped = {
        let raw = val_root.get();
        let skip = raw.is_object() && {
            let o = raw.to_object();
            state::vm_global_all().iter().any(|g| *g == o)
        };
        if skip {
            raw
        } else {
            match crate::jsapi_glue::wrap_value(&mut realm, raw) {
                Some(w) => w,
                None => return false,
            }
        }
    };
    if !define_prop(&mut realm, global.get(), &c_key, val_wrapped) {
        // 已有属性的重定义在跨 compartment 值（sync-out 又 sync-in 的函数等）
        // 下失败：回落赋值语义（更新值、保留既有描述符；双失败（如只读无 setter）
        // 即跳过——真机口径以目标描述符为准，源端只读不同步，见 vm_set 头注）。
        let _ = crate::jsapi_glue::set_prop_value(&mut realm, global.get(), &c_key, val_wrapped);
    }
    frame.set_rval(UndefinedValue());
    true
}

/// 读目标 global 属性（sync-out/探针用；跨 compartment 值透明为 CCW）。
/// `__wjs2_vm_get(id, key)` → value。
pub unsafe extern "C" fn vm_get(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let key = match arg_string(&mut cx, &frame, 1, "vm sandbox key") {
        Some(s) => s,
        None => return false,
    };
    let ptr = match lookup_global(&mut cx, id) {
        Some(p) => p,
        None => return false,
    };
    // 调用方 global（读值包装用；裸指针拷出，守卫即释）。
    let caller_ptr = unsafe { mozjs::jsapi::CurrentGlobalOrNull(cx.raw_cx()) };
    rooted!(&in(cx) let global = ptr);
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let Ok(c_key) = std::ffi::CString::new(key.as_str()) else {
        report_error(&mut realm, "TypeError: vm sandbox key must not contain NUL");
        return false;
    };
    match get_prop_value(&mut realm, global.get(), &c_key) {
        Some(v) => {
            // 跨域恒等统一（见 4.296）：读值包装进调用方 compartment；
            // global 免包装（身份基础设施）。
            rooted!(&in(&mut realm) let v_root = v);
            let skip = v_root.is_object() && {
                let o = v_root.get().to_object();
                state::vm_global_all().iter().any(|g| *g == o)
            };
            if caller_ptr.is_null() || skip {
                frame.set_rval(v_root.get());
            } else {
                rooted!(&in(&mut realm) let caller_root: *mut JSObject = caller_ptr);
                let mut caller_realm =
                    AutoRealm::new_from_handle(&mut realm, caller_root.handle());
                match crate::jsapi_glue::wrap_value(&mut caller_realm, v_root.get()) {
                    Some(w) => frame.set_rval(w),
                    None => return false,
                }
            }
        }
        None => frame.set_rval(UndefinedValue()),
    }
    true
}

/// 目标 global 自有可枚举键（sync-out 差集用；目标 realm 内求值，JSON 串回传）。
/// `__wjs2_vm_keys(id)` → `'["a","b"]'`。
pub unsafe extern "C" fn vm_keys(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let ptr = match lookup_global(&mut cx, id) {
        Some(p) => p,
        None => return false,
    };
    rooted!(&in(cx) let global = ptr);
    rooted!(&in(cx) let mut rval = UndefinedValue());
    let filename = c"vm.js".to_owned();
    let options = CompileOptionsWrapper::new(&cx, filename, 1);
    let probe = "JSON.stringify(Object.keys(globalThis))";
    if mozjs::rust::evaluate_script(&mut cx, global.handle(), probe, rval.handle_mut(), options).is_err() {
        throw_vm(&mut cx, "Error", "vm could not enumerate context keys");
        return false;
    }
    frame.set_rval(rval.get());
    true
}

/// 目标 global 全部自有字符串键（sync-out/创建快照用；含不可枚举，JSON 数组回传）。
/// `__wjs2_vm_keys_all(id)` → `'["a","b"]'`。
/// UNSAFE-BOUNDARY: 前置同 `vm_keys`；`GetPropertyKeys` 失败 None（覆盖测试同 `vm_keys`）。
pub unsafe extern "C" fn vm_keys_all(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let ptr = match lookup_global(&mut cx, id) {
        Some(p) => p,
        None => return false,
    };
    rooted!(&in(cx) let global = ptr);
    let json = {
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        own_keys_json(&mut realm, global.get()).unwrap_or_default()
    };
    if json.is_empty() {
        report_error(&mut cx, "OperationError: vm could not enumerate context keys");
        return false;
    }
    json.to_jsval(&mut cx, frame.rval_mut());
    true
}

/// 目标 global 自有键计数快照（仅调试/探针用；`{"strings": [...], "symbols": n}` JSON 回传，
/// symbol 只计数——跨 realm 无字符串身份，存在性由计数断言）。
/// `__wjs2_vm_keys_count(id)` → `'{"strings":[...],"symbols":0}'`。
/// UNSAFE-BOUNDARY: 前置同 `vm_keys`；枚举经 `own_keys_json` 同源（覆盖测试 `vm_sync_all_keys`）。
pub unsafe extern "C" fn vm_keys_count(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let ptr = match lookup_global(&mut cx, id) {
        Some(p) => p,
        None => return false,
    };
    rooted!(&in(cx) let global = ptr);
    let json = {
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        match own_keys_json(&mut realm, global.get()) {
            Some(json) => {
                let v: serde_json::Value =
                    serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
                let (strings, symbols) = match v {
                    serde_json::Value::Array(items) => {
                        let mut strings = Vec::new();
                        let mut symbols = 0usize;
                        for it in items {
                            match it {
                                serde_json::Value::String(s) => strings.push(s),
                                _ => symbols += 1,
                            }
                        }
                        (strings, symbols)
                    }
                    _ => (Vec::new(), 0usize),
                };
                serde_json::json!({ "strings": strings, "symbols": symbols }).to_string()
            }
            None => String::new(),
        }
    };
    if json.is_empty() {
        report_error(&mut cx, "OperationError: vm could not enumerate context keys");
        return false;
    }
    json.to_jsval(&mut cx, frame.rval_mut());
    true
}

/// 跨 compartment SameValue 比较（sync-out 快照比较用；`__wjs2_vm_same(a, b)` → boolean）。
/// UNSAFE-BOUNDARY: 前置——cx 在 realm 内；a/b 由 Frame rooted 后传入（§4.80）。
/// 覆盖：`tests/node/vm.rs::vm_sync_snapshot`。
pub unsafe extern "C" fn vm_same(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create；a/b 由 Frame rooted 后传入（§4.80）
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: vm SameValue needs two arguments");
        return false;
    }
    match same_value(&mut cx, frame.arg(0), frame.arg(1)) {
        Some(same) => {
            same.to_jsval(&mut cx, frame.rval_mut());
            true
        }
        None => {
            report_error(&mut cx, "OperationError: vm SameValue comparison failed");
            false
        }
    }
}

/// 摘除上下文（FinalizationRegistry/显式释放用；重复释放 false）。
/// `__wjs2_vm_release(id)` → boolean。
pub unsafe extern "C" fn vm_release(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    let _ = &mut cx;
    let released = state::vm_release(id);
    if released {
        super::vm_module::vm_mod_requests_drop_ctx(id);
    }
    frame.set_rval(BooleanValue(released));
    true
}

/// 取 vm_run/vm_run_this 暂存的原始异常对象并清槽：
/// `__wjs2_vm_take_error()` → value（无则 undefined）。
/// UNSAFE-BOUNDARY: 槽值经 RootedState.vm_last_error（Heap，trace 覆盖）保活；
/// JS 单线程专用；读后即清（信封/原物一一对应）。
/// 覆盖：`tests/node/vm.rs` vm 对拍黑盒（跨域 SyntaxError instanceof）。
pub unsafe extern "C" fn vm_take_error(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let _ = &mut cx;
    let mut out = UndefinedValue();
    state::with_rooted(|s| {
        out = s.vm_last_error.get();
        s.vm_last_error.set(UndefinedValue());
    });
    frame.set_rval(out);
    true
}

/// 内嵌 ESM 源（`node:vm`）。

/// 内嵌 ESM 源（`node:vm`；§0.9 按域分块：`vm.js` 全量，concat 字节恒等）。
pub const SOURCE: &str = concat!(
    include_str!("vm.js"),
    include_str!("vm_modules.js"),
);
