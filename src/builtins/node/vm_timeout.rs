//! `node:vm` 求值超时（M3；worker_term.rs 的 terminate 钩子同源模式）。
//!
//! 纯搬移归位（原 `vm.rs` 超时节）：中断回调 + 看门线程 + 守卫 + 命中查询 +
//! 孤儿排空（`ORPHAN_DRAIN_MS` 限时武装，见 4.300）+ 上报。调用方经原位重导出
//! （`super::vm::…` 路径不变）。

use mozjs::jsapi::RunJobs;
use mozjs::jsval::JSVal;
use mozjs::realm::AutoRealm;

use crate::jsapi_glue::{value_to_string, wrap_cx, Frame};

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

    /// 提前摘表项（孤儿排空前调用；Drop 不再重复摘）。
    pub(super) fn disarm(&mut self) {
        if let Some(generation) = self.generation.take() {
            vm_timeout_exit(self.raw, generation);
        }
    }
}

impl Drop for VmTimeoutGuard {
    fn drop(&mut self) {
        self.disarm();
    }
}

/// 孤儿排空预算（超时命中后的嵌套 RunJobs 用；有限 job 照常结算，无限逃逸
/// loop 到期中止。原地 rearm 即全杀（含无辜兄弟 job，见 4.295）。
/// 上限 1s：escape 套件"快速返回"（<5s）仍绿，重载下有限 job 不误杀）。
pub(super) const ORPHAN_DRAIN_MS: u64 = 1000;

/// 孤儿排空（限时武装版）：摘原表项 → 消原命中标记 → 短预算重武装后排空 →
/// 消排空标记 → 恢复原命中标记（供 `vm_timeout_report` 按原预算文案上报）。
/// 前置：realm 已进入；调用方已确认命中（peek 非空）。
pub(super) fn vm_timeout_drain_orphans(
    raw_cx: *mut mozjs::jsapi::JSContext,
    realm: &mut AutoRealm,
    guard: &mut VmTimeoutGuard,
) {
    let orig_ms = vm_timeout_fired_ms();
    guard.disarm();
    {
        let _orphan = VmTimeoutGuard::enter(raw_cx, ORPHAN_DRAIN_MS);
        // SAFETY: realm 内排空（主循环同款）
        unsafe { RunJobs((&mut *realm).raw_cx()) };
    }
    let _ = vm_timeout_fired_ms();
    if let Some(ms) = orig_ms {
        VM_TIMEOUT_FIRED_MS.with(|f| f.set(ms));
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
