//! runtime/unhandled：未处理 rejection 收尾（Node 式 fatal + `unhandledRejection` 投递）。
//!
//! 落法：事件循环尾（`event_loop`）、常驻会话尾（`serve_session`）、REPL 每轮
//! 共用：先读各未处理 promise 的结算 reason（`promise_settled_value` 只读），
//! 再按监听/模式分流——
//! - 有 `process.on('unhandledRejection')` 监听：逐个 `emit('unhandledRejection',
//!   reason, promise)`（内部调用经默认导出对象，用户可 mock；监听抛错即按
//!   未捕获异常 fatal），清表返回 Ok（不退出，node 同款）。
//! - 无监听：`--unhandled-rejections=` 模式（`__wjs2_nodeCompat` 实旗值，D1；
//!   缺省 throw）：throw/strict → 既有 fatal 文案（`unhandled rejection: …`）
//!   退出 1；warn → `emitWarning` + 一轮 bounded 排空（`warning` 事件同步送达
//!   默认打印器与用户监听）后继续；none → 静默继续；warn-with-error-code →
//!   警告后 `Exit(1)`。
//! 注意：reason 取值时已落定的才上报（迟挂载处理器已被 tracker 摘除）；
//! `rejectionHandled` 事件与 warn 双警告（reason + 出处注记）系 promise 域
//! 另案，此处不实现（见 journal）。

use mozjs::conversions::ToJSValConvertible as _;
use mozjs::jsapi::JSObject;
use mozjs::jsval::{JSVal, UndefinedValue};
use mozjs::realm::AutoRealm;
use mozjs::rooted;
use mozjs::rust::RootedGuard;
use mozjs::rust::Runtime;

use crate::error::Error;
use crate::jsapi_glue::{
    call_method, get_prop_u32, get_prop_value, promise_settled_value, take_pending_exception,
    value_to_string,
};
use crate::state;

/// `--unhandled-rejections` 模式（D1 精确名单；缺省 throw，真机口径）。
#[derive(Clone, Copy, PartialEq, Debug)]
enum UnhandledMode {
    Throw,
    Warn,
    None,
    Strict,
    WarnWithErrorCode,
}

/// 实旗值读模式（`globalThis.__wjs2_nodeCompat` 数组；无即 throw）。
fn unhandled_mode(cx: &mut mozjs::context::JSContext, global: *mut JSObject) -> UnhandledMode {
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    let compat = match get_prop_value(cx, global_root.get(), c"__wjs2_nodeCompat") {
        Some(v) => v,
        None => return UnhandledMode::Throw,
    };
    if !compat.is_object() {
        return UnhandledMode::Throw;
    }
    rooted!(&in(cx) let compat_root = compat);
    rooted!(&in(cx) let compat_obj: *mut JSObject = compat_root.get().to_object());
    let len = get_prop_u32(cx, compat_obj.get(), c"length").unwrap_or(0);
    for i in 0..len.min(64) {
        let key = std::ffi::CString::new(i.to_string()).unwrap_or_else(|_| c"0".to_owned());
        let Some(v) = get_prop_value(cx, compat_obj.get(), &key) else {
            continue;
        };
        let s = value_to_string(cx, v);
        if let Some(mode) = s.strip_prefix("--unhandled-rejections=") {
            return match mode {
                "warn" => UnhandledMode::Warn,
                "none" => UnhandledMode::None,
                "strict" => UnhandledMode::Strict,
                "warn-with-error-code" => UnhandledMode::WarnWithErrorCode,
                _ => UnhandledMode::Throw,
            };
        }
    }
    UnhandledMode::Throw
}

/// 未处理 rejection 收尾上报（Node 式 fatal + 事件投递）。
/// `event_loop` 尾与 `repl` 每轮共用；REPL 侧出错只打印不退出（调用方定）。
pub(crate) fn report_unhandled_rejections(
    rt: &mut Runtime,
    global: &RootedGuard<'_, *mut JSObject>,
) -> Result<(), Error> {
    // 快照（裸指针 + settled 值；borrow 即释，引擎调用在后）。
    struct Pending {
        promise: *mut JSObject,
        reason: JSVal,
        text: String,
    }
    let mut realm = AutoRealm::new_from_handle(rt.cx(), global.handle());
    let pending: Vec<Pending> = {
        let unhandled = state::with_rooted(|s| {
            s.unhandled.iter().map(|h| h.get()).collect::<Vec<*mut JSObject>>()
        });
        let mut out = Vec::new();
        for promise_obj in unhandled {
            rooted!(&in(&mut realm) let promise = promise_obj);
            let reason = match promise_settled_value(&mut realm, promise.get()) {
                // 仍 rejected：reason 原值（emit 用）+ 串（fatal 文案用）。
                Some(Some((false, v))) => {
                    rooted!(&in(&mut realm) let v_root = v);
                    let text = value_to_string(&mut realm, v_root.get());
                    (v_root.get(), text)
                }
                // 已处理/已落定/不可读：跳过（Handled 摘除竞态）。
                _ => continue,
            };
            out.push(Pending { promise: promise_obj, reason: reason.0, text: reason.1 });
        }
        out
    };
    if pending.is_empty() {
        return Ok(());
    }
    tracing::warn!(target: "winterjs2::runtime", count = pending.len(), "unhandled rejections detected");
    // process 对象（默认导出对象，内部调用经它，用户可 mock）。
    rooted!(&in(&mut realm) let global_root: *mut JSObject = global.get());
    let process = match get_prop_value(&mut realm, global_root.get(), c"process") {
        Some(p) if p.is_object() => p,
        _ => {
            // 无 process（理论不可达）：沿旧 fatal 路径上报。
            return Err(Error::Other(format!(
                "unhandled rejection: {}",
                pending.into_iter().map(|p| p.text).collect::<Vec<_>>().join("; ")
            )));
        }
    };
    rooted!(&in(&mut realm) let process_root = process);
    // 监听计数（`listenerCount` 经 process 对象，用户可 mock）。
    rooted!(&in(&mut realm) let mut ev_name = UndefinedValue());
    "unhandledRejection".to_jsval(&mut realm, ev_name.handle_mut());
    rooted!(&in(&mut realm) let ev_name_root = ev_name.get());
    let listened = match call_method(
        &mut realm,
        process_root.get(),
        c"listenerCount",
        &[ev_name_root.get()],
    ) {
        Some(v) => {
            rooted!(&in(&mut realm) let v_root = v);
            (v_root.is_int32() && v_root.to_int32() > 0)
                || (v_root.is_double() && v_root.to_double() > 0.0)
        }
        // 查询失败（pending 原样清理）：按无监听走。
        None => {
            let _ = take_pending_exception(&mut realm);
            false
        }
    };
    if listened {
        // 逐个投递；监听抛错即按未捕获异常 fatal（node 同款）。
        for p in &pending {
            rooted!(&in(&mut realm) let promise_root: *mut JSObject = p.promise);
            rooted!(&in(&mut realm) let reason_root = p.reason);
            rooted!(&in(&mut realm) let promise_v = mozjs::jsval::ObjectValue(promise_root.get()));
            let ok = call_method(
                &mut realm,
                process_root.get(),
                c"emit",
                &[ev_name_root.get(), reason_root.get(), promise_v.get()],
            );
            if ok.is_none() {
                let msg = crate::jsapi_glue::pending_exception_error(
                    &mut realm,
                    global_root.get(),
                    "",
                    "",
                )
                .to_string();
                state::with_rooted(|s| {
                    s.unhandled.retain(|h| !pending.iter().any(|q| q.promise == h.get()));
                });
                return Err(crate::error::Error::Other(msg));
            }
        }
        state::with_rooted(|s| {
            s.unhandled.retain(|h| !pending.iter().any(|q| q.promise == h.get()));
        });
        return Ok(());
    }
    // 无监听：按模式分流。
    let mode = unhandled_mode(&mut realm, global_root.get());
    match mode {
        UnhandledMode::Throw | UnhandledMode::Strict => Err(Error::Other(format!(
            "unhandled rejection: {}",
            pending.into_iter().map(|p| p.text).collect::<Vec<_>>().join("; ")
        ))),
        UnhandledMode::None => {
            state::with_rooted(|s| {
                s.unhandled.retain(|h| !pending.iter().any(|q| q.promise == h.get()));
            });
            Ok(())
        }
        UnhandledMode::Warn | UnhandledMode::WarnWithErrorCode => {
            // 警告经 process.emitWarning（nextTick 派发 'warning'），再一轮
            // bounded 排空让默认打印器与用户监听同步收到（退出前不再循环）。
            // 警告对象为新 Error（message = reason 串，真机口径；原 reason 不改）。
            for p in &pending {
                rooted!(&in(&mut realm) let mut text_v = UndefinedValue());
                p.text.clone().to_jsval(&mut realm, text_v.handle_mut());
                rooted!(&in(&mut realm) let text_root = text_v.get());
                rooted!(&in(&mut realm) let mut tag_v = UndefinedValue());
                "UnhandledPromiseRejectionWarning".to_jsval(&mut realm, tag_v.handle_mut());
                rooted!(&in(&mut realm) let tag_root = tag_v.get());
                let _ = call_method(
                    &mut realm,
                    process_root.get(),
                    c"emitWarning",
                    &[text_root.get(), tag_root.get()],
                );
                let _ = take_pending_exception(&mut realm);
            }
            // 一轮 bounded 排空（nextTick + 微任务；回调抛错即 fatal）。
            {
                use crate::runtime::ErrorSource;
                if let Err(e) = crate::builtins::node::process_::drain_next_ticks(
                    &mut realm,
                    global_root.get(),
                    ErrorSource::Script { source: "", filename: "" },
                ) {
                    return Err(e);
                }
                // SAFETY: realm 内排空内部 job queue（主循环同款）
                unsafe { mozjs::jsapi::RunJobs((&mut realm).raw_cx()) };
            }
            state::with_rooted(|s| {
                s.unhandled.retain(|h| !pending.iter().any(|q| q.promise == h.get()));
            });
            if mode == UnhandledMode::WarnWithErrorCode {
                return Err(Error::Exit(1));
            }
            Ok(())
        }
    }
}
