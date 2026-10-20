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

// ── 超时中断（M3；worker_term.rs 的 terminate 钩子同源模式）──────────────────
// 原理：JS_AddInterruptCallback（每 JS 线程装一次）+ 看门线程到期
// JS_RequestInterruptCallback。回调只读共享"存活表"（try_lock，从不阻塞；
// 任意 Rust 借用期内触发安全），最早到期者到点即返回 false 中止脚本。
// 看门线程一律 sleep 后查表：generation 已摘即过期请求，永不碰已结束会话。
// Runtime 全泄漏（§4.8，run_isolated 亦 forget），cx 裸指针终身有效。

/// 存活超时（(cx, generation) → (deadline_ms, timeout_ms)；回调按 cx 过滤取最早）。
static VM_TIMEOUT_LIVE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<(usize, u64), (u64, u64)>>,
> = std::sync::OnceLock::new();
/// 世代单调（u64::MAX 保留位外，永不回绕到在用值）。
static VM_TIMEOUT_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn vm_timeout_live(
) -> &'static std::sync::Mutex<std::collections::HashMap<(usize, u64), (u64, u64)>> {
    VM_TIMEOUT_LIVE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

thread_local! {
    /// 每线程装一次中断钩子。
    static VM_TIMEOUT_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// 回调命中的超时毫秒（报错文案用最早到期的 ms，非调用方 ms；嵌套外短内长口径）。
    static VM_TIMEOUT_FIRED_MS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// SAFETY-BOUNDARY（interrupt 回调）：仅读存活表（try_lock）+ TLS Cell，
/// 无 JSAPI、无重入（Interrupt.h：回调内禁止再入引擎）。返回 false = 中止脚本。
/// 覆盖：`tests/node/vm.rs` 超时黑盒（逐行为单位）。
unsafe extern "C" fn vm_timeout_cb(cx: *mut mozjs::jsapi::JSContext) -> bool {
    let now = now_ms();
    let live = VM_TIMEOUT_LIVE.get().and_then(|m| m.try_lock().ok());
    let Some(live) = live else { return true };
    let key = cx as usize;
    let mut earliest: Option<(u64, u64)> = None;
    for ((c, _), (dl, ms)) in live.iter() {
        if *c != key {
            continue;
        }
        if earliest.map(|(d, _)| *dl < d).unwrap_or(true) {
            earliest = Some((*dl, *ms));
        }
    }
    match earliest {
        Some((dl, ms)) if now >= dl => {
            VM_TIMEOUT_FIRED_MS.with(|f| f.set(ms));
            false
        }
        _ => true,
    }
}

/// 本线程装钩子（幂等；调用处于 JS 执行中）。
fn vm_timeout_install(raw_cx: *mut mozjs::jsapi::JSContext) {
    VM_TIMEOUT_INSTALLED.with(|f| {
        if !f.get() {
            // SAFETY: JS 线程；回调仅读表/TLS（见 vm_timeout_cb）
            unsafe {
                mozjs::jsapi::JS_AddInterruptCallback(raw_cx, Some(vm_timeout_cb));
            }
            f.set(true);
        }
    });
}

/// 超时入栈：返回世代；0/ms=0 即无超时（None）。看门线程只读表 + 请求中断。
pub(super) fn vm_timeout_enter(raw_cx: *mut mozjs::jsapi::JSContext, ms: u64) -> Option<u64> {
    if ms == 0 {
        return None;
    }
    vm_timeout_install(raw_cx);
    let generation = VM_TIMEOUT_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dl = now_ms().saturating_add(ms);
    if let Ok(mut live) = vm_timeout_live().lock() {
        live.insert((raw_cx as usize, generation), (dl, ms));
    }
    let cx_key = raw_cx as usize;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        // 中断请求一次一耗（同步段命中即清）：到点后每 5ms 补发，直到表项
        // 摘除（守卫 drop）——微任务段长循环同样武装（escape-promise 口径：
        // 同步 `loop()` 命中消耗首发后，排空的 `.then(loop)` 仍须中断）。
        loop {
            let live = vm_timeout_live()
                .lock()
                .ok()
                .and_then(|v| v.get(&(cx_key, generation)).copied())
                .is_some();
            if !live {
                break;
            }
            // SAFETY: cx 经 §4.8 泄漏保活终身有效；RequestInterruptCallback
            // 线程安全（Interrupt.h: any thread；只置请求旗）。
            unsafe {
                mozjs::jsapi::JS_RequestInterruptCallback(cx_key as *mut mozjs::jsapi::JSContext);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    Some(generation)
}

/// 中断补请求（同步段已消耗请求时，排空孤儿任务前补发，微任务段确定性武装；
/// 看门 5ms 粒度兜底，本函数消竞态窗）。无到期表项即无操作。
pub(super) fn vm_timeout_rearm(raw_cx: *mut mozjs::jsapi::JSContext) {
    let key = raw_cx as usize;
    let now = now_ms();
    let armed = vm_timeout_live()
        .try_lock()
        .map(|live| {
            live.iter()
                .any(|((c, _), (dl, _))| *c == key && now >= *dl)
        })
        .unwrap_or(false);
    if armed {
        // SAFETY: JS 线程自身；仅置请求旗（Interrupt.h）。
        unsafe {
            mozjs::jsapi::JS_RequestInterruptCallback(raw_cx);
        }
    }
}

/// 超时出栈（成功/失败路径必调；看门线程据此判定过期）。
pub(super) fn vm_timeout_exit(raw_cx: *mut mozjs::jsapi::JSContext, generation: u64) {
    if let Ok(mut live) = vm_timeout_live().lock() {
        live.remove(&(raw_cx as usize, generation));
    }
}

/// 取并清命中标记（中断回调置位；报错文案用命中 ms）。
pub(super) fn vm_timeout_fired_ms() -> Option<u64> {
    VM_TIMEOUT_FIRED_MS.try_with(|f| {
        let ms = f.get();
        f.set(0);
        (ms != 0).then_some(ms)
    })
    .unwrap_or(None)
}

/// 只读命中标记（不清；native 报错路径决策用，消费权归 JS 壳）。
pub(super) fn vm_timeout_peek_ms() -> Option<u64> {
    VM_TIMEOUT_FIRED_MS.try_with(|f| {
        let ms = f.get();
        (ms != 0).then_some(ms)
    })
    .unwrap_or(None)
}

/// 超时命中查询（`__wjs2_vm_timeout_fired()` → 命中 ms，无即 0；消费标记）。
/// UNSAFE-BOUNDARY: 只读 TLS Cell，无 JSAPI。覆盖 tests/node/vm.rs。
pub unsafe extern "C" fn timeout_fired(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_create（本函数不用 cx，仅占位签名）
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let _ = &mut cx;
    let ms = vm_timeout_fired_ms().unwrap_or(0);
    frame.set_rval(mozjs::jsval::Int32Value(ms as i32));
    true
}

/// 超时守卫（RAII 出栈；成功/失败路径全覆盖）。
pub(super) struct VmTimeoutGuard {
    raw: *mut mozjs::jsapi::JSContext,
    generation: Option<u64>,
}

impl VmTimeoutGuard {
    pub(super) fn enter(raw_cx: *mut mozjs::jsapi::JSContext, ms: u64) -> Self {
        Self {
            raw: raw_cx,
            generation: vm_timeout_enter(raw_cx, ms),
        }
    }
}

impl Drop for VmTimeoutGuard {
    fn drop(&mut self) {
        if let Some(generation) = self.generation {
            vm_timeout_exit(self.raw, generation);
        }
    }
}

/// 第 `ti` 个实参读超时毫秒（缺席/非法即 0=无超时）。
pub(super) fn vm_timeout_arg(
    cx: &mut mozjs::context::JSContext,
    frame: &Frame,
    ti: u32,
) -> u64 {
    if frame.argc() <= ti {
        return 0;
    }
    value_to_string(cx, frame.arg(ti)).parse::<u64>().unwrap_or(0)
}

/// 命中即报超时错（清 pending 残留，真机文案逐字），返回 true；未命中 false。
/// 前置：realm 已进入（§4.1 求值失败路径同）。标记不清，消费权归 JS 壳
///（`__wjs2_vm_timeout_fired`），避免 native/壳双读竞态。
pub(super) fn vm_timeout_report(realm: &mut AutoRealm, fallback_ms: u64) -> bool {
    match vm_timeout_peek_ms() {
        Some(hit_ms) => {
            let _ = crate::jsapi_glue::take_pending_exception(realm);
            let ms = if hit_ms != 0 { hit_ms } else { fallback_ms };
            crate::jsapi_glue::report_error(
                realm,
                &format!(
                    "ERR_SCRIPT_EXECUTION_TIMEOUT: Script execution timed out after {ms}ms"
                ),
            );
            true
        }
        None => false,
    }
}

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
    let _timeout_guard = VmTimeoutGuard::enter(cx_raw, timeout_ms);
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
            // 孤儿微任务排空（真机 afterEvaluate 口径：逃逸 promise 不得存活；
            // 同步段已消耗中断请求，此处补发 + 看门 5ms 续发，长循环同样中止；
            // 实测中断中的 job 以 uncatchable 中止、promise 永不落定 → 无
            // unhandled 报告，真机"双杀瞬落"口径一致）。
            vm_timeout_rearm(cx_raw);
            // SAFETY: realm 内排空（主循环同款）
            unsafe { RunJobs((&mut realm).raw_cx()) };
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
    frame.set_rval(rval.get());
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
    let _timeout_guard = VmTimeoutGuard::enter(cx_raw, timeout_ms);
    rooted!(&in(cx) let global = state::global());
    rooted!(&in(cx) let mut rval = UndefinedValue());
    let filename_c =
        std::ffi::CString::new(filename.as_str()).unwrap_or_else(|_| c"vm.js".to_owned());
    let options = CompileOptionsWrapper::new(&cx, filename_c, 1);
    if mozjs::rust::evaluate_script(&mut cx, global.handle(), &code, rval.handle_mut(), options).is_err() {
        let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
        // 超时命中优先（同 vm_run：先补发中断再排空孤儿任务）。
        if vm_timeout_peek_ms().is_some() {
            vm_timeout_rearm(cx_raw);
            // SAFETY: realm 内排空（主循环同款）
            unsafe { RunJobs((&mut realm).raw_cx()) };
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
    frame.set_rval(rval.get());
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
    let wrapped = format!("(function ({params}) {{\n{code}\n}})");
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
    frame.set_rval(rval.get());
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
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let Ok(c_key) = std::ffi::CString::new(key.as_str()) else {
        report_error(&mut realm, "TypeError: vm sandbox key must not contain NUL");
        return false;
    };
    if !define_prop(&mut realm, global.get(), &c_key, val) {
        // 已有属性的重定义在跨 compartment 值（sync-out 又 sync-in 的函数等）
        // 下失败：回落赋值语义（更新值、保留既有描述符；双失败（如只读无 setter）
        // 即跳过——真机口径以目标描述符为准，源端只读不同步，见 vm_set 头注）。
        let _ = crate::jsapi_glue::set_prop_value(&mut realm, global.get(), &c_key, val);
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
    rooted!(&in(cx) let global = ptr);
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let Ok(c_key) = std::ffi::CString::new(key.as_str()) else {
        report_error(&mut realm, "TypeError: vm sandbox key must not contain NUL");
        return false;
    };
    match get_prop_value(&mut realm, global.get(), &c_key) {
        Some(v) => frame.set_rval(v),
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
    frame.set_rval(BooleanValue(state::vm_release(id)));
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
