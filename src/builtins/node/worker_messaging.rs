//! worker 线程直投面（`postMessageToThread` 底座；对齐 worker.rs 纯搬移 + 新面）。
//!
//! 本文件收 worker 句柄直投三件（`worker_attach/post/tid`，worker.rs 纯搬移，
//! 调用方经 `pub use` 原位重导出）与线程直投新面（`post_to_thread` native +
//! `ThreadMsg/ThreadAck` 派发 helper，见 worker.rs `dispatch`）。

use mozjs::jsval::{JSVal, UndefinedValue};

use crate::jsapi_glue::{report_error, value_to_string, wrap_cx, Frame};
use crate::state;

use super::worker::{WorkerEvent, arg_id, with_str_args};
use super::worker_term::worker_route;

/// 登记 Worker JS 目标。`__wjs2_worker_attach(workerId, target)` → undefined。
pub unsafe extern "C" fn worker_attach(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 worker::port_pair
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    if frame.argc() < 2 || !frame.arg(1).is_object() {
        report_error(&mut cx, "TypeError: worker attach needs a target object");
        return false;
    }
    state::worker_target_add(id, frame.arg(1));
    frame.set_rval(UndefinedValue());
    true
}

/// 主→worker 投递（parentPort 收）。`__wjs2_worker_post(workerId, json)` → boolean。
pub unsafe extern "C" fn worker_post(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 worker::port_pair
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: worker postMessage needs a message");
        return false;
    }
    let json = value_to_string(&mut cx, frame.arg(1));
    let ok = match worker_route(id) {
        Some((inbox, parent)) => inbox
            .send(WorkerEvent::PortMsg { to: parent, json })
            .is_ok(),
        None => false,
    };
    frame.set_rval(mozjs::jsval::BooleanValue(ok));
    true
}

/// worker 线程 id。`__wjs2_worker_tid(workerId)` → 数字串（已退出即空串）。
pub unsafe extern "C" fn worker_tid(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 worker::port_pair
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    use mozjs::conversions::ToJSValConvertible as _;
    state::worker_tid(id).map(|t| t.to_string()).unwrap_or_default().to_jsval(&mut cx, frame.rval_mut());
    true
}

/// 登记直投桥接目标（`node:worker_threads` 模块求值期 attach 一次；收发共用，
/// Rust 侧保留 id，会话生命期常驻）。`__wjs2_thread_bridge_attach(target)` → undefined。
pub unsafe extern "C" fn thread_bridge_attach(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 worker::port_pair
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_object() {
        report_error(&mut cx, "TypeError: thread bridge needs a target object");
        return false;
    }
    state::thread_bridge_attach(frame.arg(0));
    frame.set_rval(UndefinedValue());
    true
}

/// 跨线程直投（`postMessageToThread` 底座）。
/// `__wjs2_post_to_thread(dest, nonce, source, wire)` → boolean（无路由即 false）。
/// `dest` 非数字串即无路由（JS 侧已把非 number 映射为空串）；`wire` 为空即纯路由
/// 探针（主线程发送序：先探针再组装——node 主线程"先查路由后序列化"，worker 线程
/// 反之，见 JS 侧）。dest 0（主线程）直走发送方会话 `worker_main_inbox`（node 主
/// 线程接收特殊化）；其余走进程级线程注册表。发送失败（收件箱已死）回扫注册表。
/// UNSAFE-BOUNDARY：纯 Rust 串进出，无 JS 值存留；覆盖 `tests/node/worker.rs` 直投三件。
pub unsafe extern "C" fn post_to_thread(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 worker::port_pair
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 {
        report_error(&mut cx, "TypeError: postMessageToThread needs a destination, nonce, source and message");
        return false;
    }
    let dest = value_to_string(&mut cx, frame.arg(0)).parse::<u64>().ok();
    let nonce = value_to_string(&mut cx, frame.arg(1));
    let source = value_to_string(&mut cx, frame.arg(2)).parse::<u64>().unwrap_or(u64::MAX);
    let wire = value_to_string(&mut cx, frame.arg(3));
    let tx = match dest {
        Some(0) => state::with_plain(|p| p.worker_main_inbox.clone()),
        Some(t) => state::thread_route(t),
        None => None,
    };
    let Some(tx) = tx else {
        frame.set_rval(mozjs::jsval::BooleanValue(false));
        return true;
    };
    if wire.is_empty() {
        frame.set_rval(mozjs::jsval::BooleanValue(true));
        return true;
    }
    let Some(reply_tx) = state::with_plain(|p| p.worker_tx.clone()) else {
        frame.set_rval(mozjs::jsval::BooleanValue(false));
        return true;
    };
    let ok = tx.send(WorkerEvent::ThreadMsg { nonce, source, wire, reply_tx }).is_ok();
    if !ok && let Some(t) = dest {
        state::thread_unregister(t);
    }
    frame.set_rval(mozjs::jsval::BooleanValue(ok));
    true
}

// ── 线程直投派发（`ThreadMsg/ThreadAck` 用；`dispatch` 经 `&failed`  closures 保底）
// 回执码：0 投递（有监听）/1 无监听（桥接缺失/非对象/调用失联亦按此）/2 监听抛错。
// JS 侧 `__ev` 恒返回 "0"/"1"/"2"（处理器全捕获）；Rust 侧调用失败（理论不可达）
// 按 2 回执并经 `failed` 上报（发送方不悬挂——W1 teardown 铁律：回执与派发同点）。

use mozjs::context::JSContext;
use mozjs::jsapi::JSObject;
use mozjs::rooted;

use crate::jsapi_glue::get_prop_value;

/// 直投派发：目标会话 `process.emit("workerMessage")`，经 `reply_tx` 回执。
/// 桥接缺失即无监听（模块未加载）；`wire` 解码/监听抛错一律 JS 侧消化为回执码。
pub fn thread_deliver(
    cx: &mut JSContext,
    global: *mut JSObject,
    failed: &dyn Fn(&mut JSContext) -> crate::error::Error,
    nonce: &str,
    source: u64,
    wire: &str,
    reply_tx: tokio::sync::mpsc::UnboundedSender<WorkerEvent>,
) -> Result<(), crate::error::Error> {
    let code: u64 = match state::thread_bridge() {
        Some(t) if t.is_object() => {
            rooted!(&in(cx) let b: *mut JSObject = t.to_object());
            match get_prop_value(cx, b.get(), c"__ev") {
                Some(fun) => {
                    let payload = format!("{nonce}\n{source}\n{wire}");
                    match with_str_args(cx, global, fun, "thread-msg", &payload) {
                        Some(r) => value_to_string(cx, r).parse::<u64>().unwrap_or(1),
                        None => {
                            let _ = reply_tx.send(WorkerEvent::ThreadAck { nonce: nonce.into(), result: 2 });
                            return Err(failed(cx));
                        }
                    }
                }
                None => 1,
            }
        }
        _ => 1,
    };
    let _ = reply_tx.send(WorkerEvent::ThreadAck { nonce: nonce.into(), result: code });
    Ok(())
}

/// 直投回执：唤醒发送方 `postMessageToThread` 等待者（桥接缺失即发送方已去，丢弃）。
pub fn thread_ack(
    cx: &mut JSContext,
    global: *mut JSObject,
    failed: &dyn Fn(&mut JSContext) -> crate::error::Error,
    nonce: &str,
    result: u64,
) -> Result<(), crate::error::Error> {
    let Some(t) = state::thread_bridge() else {
        return Ok(());
    };
    if !t.is_object() {
        return Ok(());
    }
    rooted!(&in(cx) let b: *mut JSObject = t.to_object());
    let Some(fun) = get_prop_value(cx, b.get(), c"__ev") else {
        return Ok(());
    };
    let payload = format!("{nonce} {result}");
    match with_str_args(cx, global, fun, "thread-ack", &payload) {
        Some(_) => Ok(()),
        None => Err(failed(cx)),
    }
}
