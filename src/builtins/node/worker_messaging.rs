//! worker 线程直投面（`postMessageToThread` 底座；对齐 worker.rs 纯搬移 + 新面）。
//!
//! 本文件收 worker 句柄直投三件（`worker_attach/post/tid`，worker.rs 纯搬移，
//! 调用方经 `pub use` 原位重导出）与线程直投新面（`post_to_thread` native +
//! `ThreadMsg/ThreadAck` 派发 helper，见 worker.rs `dispatch`）。

use mozjs::jsval::{JSVal, UndefinedValue};

use crate::jsapi_glue::{report_error, value_to_string, wrap_cx, Frame};
use crate::state;

use super::worker::{WorkerEvent, arg_id};
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
