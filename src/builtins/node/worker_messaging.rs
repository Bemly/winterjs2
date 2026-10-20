//! worker 线程直投面（`postMessageToThread` 底座；对齐 worker.rs 纯搬移 + 新面）。
//!
//! 本文件收 worker 句柄直投三件（`worker_attach/post/tid`，worker.rs 纯搬移，
//! 调用方经 `pub use` 原位重导出）与线程直投新面（`post_to_thread` native +
//! `ThreadMsg/ThreadAck` 派发 helper，见 worker.rs `dispatch`）。

use mozjs::jsval::{JSVal, UndefinedValue};

use crate::jsapi_glue::{call_one, report_error, value_to_string, wrap_cx, Frame};
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

/// UNSAFE-BOUNDARY: worker 会话子输出上报。`__wjs2_worker_stdio(fd, wire)` →
/// boolean（fd "1"/"2"，wire JSON {s}/{b}；主 inbox WStdio 投递，exit 前保序）。
/// 前置：worker 会话内调用（自 id + 主 inbox 来自 TLS；主会话调即 false）。
/// 覆盖：`tests/node/worker.rs::worker_stdio_surface`。
pub unsafe extern "C" fn worker_stdio_out(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 worker::port_pair
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: worker stdio needs (fd, wire)");
        return false;
    }
    let fd = value_to_string(&mut cx, frame.arg(0));
    let wire = value_to_string(&mut cx, frame.arg(1));
    let stream: u8 = match fd.as_str() {
        "2" => 2,
        "1" => 1,
        _ => {
            report_error(&mut cx, "TypeError: worker stdio fd must be 1 or 2");
            return false;
        }
    };
    let (id, inbox) = match state::with_plain(|p| (p.worker_self_id, p.worker_main_inbox.clone())) {
        (Some(id), Some(inbox)) => (id, inbox),
        _ => {
            frame.set_rval(mozjs::jsval::BooleanValue(false));
            return true;
        }
    };
    let ok = inbox.send(WorkerEvent::WStdio { worker_id: id, stream, data: wire }).is_ok();
    frame.set_rval(mozjs::jsval::BooleanValue(ok));
    true
}

/// UNSAFE-BOUNDARY: 主→worker 子输入。`__wjs2_worker_stdin(workerId, wire)` →
/// boolean（wire JSON {s}/{b}/{end}；worker 收件箱 WStdin 投递）。
/// 前置：id 为已登记 worker；覆盖：同上。
pub unsafe extern "C" fn worker_stdin_to(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let id = match arg_id(&mut cx, &frame, 0) {
        Some(id) => id,
        None => return false,
    };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: worker stdin needs a wire string");
        return false;
    }
    let wire = value_to_string(&mut cx, frame.arg(1));
    let ok = match worker_route(id) {
        Some((inbox, _)) => inbox.send(WorkerEvent::WStdin { data: wire }).is_ok(),
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
    let ok = tx.send(WorkerEvent::ThreadMsg { nonce, source, dest: dest.unwrap_or(u64::MAX), wire, reply_tx }).is_ok();
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

/// stdio 派发（`WStdio/WStdin` 用；`dispatch` 经 `&failed` 保底；worker.rs 纯搬移）。
pub fn dispatch_stdio(
    cx: &mut JSContext,
    global: *mut JSObject,
    failed: &dyn Fn(&mut JSContext) -> crate::error::Error,
    ev: &WorkerEvent,
) -> Result<(), crate::error::Error> {
    match ev {
        WorkerEvent::WStdio { worker_id, stream, data } => {
            // 只进主会话；worker 会话收到即丢（绝不回流）。
            if !state::worker_is_main() {
                return Ok(());
            }
            let Some(target) = state::worker_target(*worker_id) else {
                return Ok(());
            };
            if !target.is_object() {
                return Ok(());
            }
            rooted!(&in(cx) let t: *mut JSObject = target.to_object());
            let Some(fun) = get_prop_value(cx, t.get(), c"__ev") else {
                return Err(failed(cx));
            };
            let kind = if *stream == 2 { "stderr" } else { "stdout" };
            if with_str_args(cx, global, fun, kind, data).is_none() {
                return Err(failed(cx));
            }
            Ok(())
        }
        WorkerEvent::WStdin { data } => {
            // 只进 worker 会话；主会话收到即丢。全局钩子缺失（旧快照）即丢。
            if state::worker_is_main() {
                return Ok(());
            }
            let Some(hook) = get_prop_value(cx, global, c"__wjs2_stdin_push") else {
                return Ok(());
            };
            rooted!(&in(cx) let hook_root = hook);
            rooted!(&in(cx) let mut arg = UndefinedValue());
            {
                use mozjs::conversions::ToJSValConvertible as _;
                data.to_jsval(cx, arg.handle_mut());
            }
            rooted!(&in(cx) let arg_root = arg.get());
            if call_one(cx, global, hook_root.get(), arg_root.get()).is_none() {
                return Err(failed(cx));
            }
            Ok(())
        }
        _ => Ok(()),
    }
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

// ── worker stdio 面（stdio 套件；worker.rs 纯搬移，调用方经 `pub use` 原位重导出）──

/// worker 会话 stdio 接线脚本（`runtime::init_session` 内 worker 分支求值；
/// 用户脚本前运行）：stdout/stderr 写拦截上报（本地 TEE 不变）+ stdin 换
/// fed-Readable（`__wjs2_stdin_push` 进流）。全程设防，永不抛（抛即会话起不来）。
pub const WORKER_STDIO_INIT: &str = r#"
(() => {
  try {
    const proc = globalThis.process;
    if (!proc || proc.__wjs2_stdioInit) return;
    proc.__wjs2_stdioInit = true;
    const encChunk = (chunk) => {
      if (typeof chunk === "string") return JSON.stringify({ s: chunk });
      try {
        if (typeof Buffer !== "undefined" && Buffer.isBuffer(chunk)) {
          return JSON.stringify({ b: chunk.toString("base64") });
        }
        if (typeof Uint8Array !== "undefined" && chunk instanceof Uint8Array) {
          return JSON.stringify({
            b: Buffer.from(chunk.buffer, chunk.byteOffset, chunk.byteLength).toString("base64"),
          });
        }
      } catch {}
      try { return JSON.stringify({ s: String(chunk) }); } catch { return JSON.stringify({ s: "" }); }
    };
    const wrapStream = (stream, fd) => {
      if (!stream || typeof stream.write !== "function" || stream.__wjs2_stdioWrapped) return;
      const orig = stream.write.bind(stream);
      // 管道独占（stdout:true/stderr:true）：上报照常、本地不再回显（node 真机
      // 口径；独占位经 `globalThis.__wjs2_stdio_pipe` 由 Rust 侧置入）。
      const exclusive = (globalThis.__wjs2_stdio_pipe ?? "").includes(fd);
      const wrapped = function (chunk, ...rest) {
        try { __wjs2_worker_stdio(fd, encChunk(chunk)); } catch {}
        if (exclusive) return true;
        return orig(chunk, ...rest);
      };
      wrapped.__wjs2_stdioWrapped = true;
      try { stream.write = wrapped; } catch {}
    };
    try { wrapStream(proc.stdout, "1"); } catch {}
    try { wrapStream(proc.stderr, "2"); } catch {}
    const listeners = {};
    const queue = [];
    const piped = [];
    let live = true;
    const emit = (ev, arg) => {
      const list = listeners[ev];
      if (!list) return;
      for (const cb of [...list]) { try { cb(arg); } catch {} }
    };
    const flushQueue = () => {
      while (queue.length > 0 && (piped.length > 0 || (listeners["data"] ?? []).length > 0)) {
        const c = queue.shift();
        emit("data", c);
        for (const d of [...piped]) { try { d.write(c); } catch {} }
      }
    };
    const stdin = {
      readable: true, readableEnded: false, destroyed: false,
      on(ev, cb) {
        if (typeof cb !== "function") return this;
        ((listeners[String(ev)] ??= [])).push(cb);
        if (String(ev) === "data") flushQueue();
        return this;
      },
      once(ev, cb) {
        if (typeof cb !== "function") return this;
        const self = this;
        const wrapped = (...args) => { self.off(ev, wrapped); cb(...args); };
        return this.on(ev, wrapped);
      },
      off(ev, cb) {
        const list = listeners[String(ev)];
        if (list) {
          let i = list.findIndex((l) => l === cb);
          while (i >= 0) { list.splice(i, 1); i = list.findIndex((l) => l === cb); }
        }
        return this;
      },
      removeListener(ev, cb) { return this.off(ev, cb); },
      read() { return queue.length > 0 ? queue.shift() : null; },
      pipe(dest) {
        piped.push(dest);
        flushQueue();
        return dest;
      },
      unpipe(dest) {
        if (dest === undefined) piped.length = 0;
        else {
          let i = piped.findIndex((d) => d === dest);
          while (i >= 0) { piped.splice(i, 1); i = piped.findIndex((d) => d === dest); }
        }
        return this;
      },
      __pushWire(wire) {
        let obj;
        try { obj = JSON.parse(String(wire)); } catch { return; }
        if (!obj || typeof obj !== "object") return;
        if (obj.end) {
          if (!live) return;
          live = false;
          this.readableEnded = true;
          flushQueue();
          for (const d of [...piped]) { try { d.end(); } catch {} }
          piped.length = 0;
          emit("end");
          emit("close");
          return;
        }
        if (!live) return;
        let chunk = null;
        if (typeof obj.b === "string") {
          try { chunk = Buffer.from(obj.b, "base64"); } catch { return; }
        } else if (typeof obj.s === "string") chunk = obj.s;
        else return;
        queue.push(chunk);
        flushQueue();
      },
    };
    globalThis.__wjs2_stdin_push = (wire) => { try { stdin.__pushWire(wire); } catch {} };
    try { proc.stdin = stdin; } catch {}
  } catch {}
})();
"#;

/// worker 会话 `--require` 预加载脚本（`runtime::init_session` 内 worker 分支、
/// stdio 接线后求值）：`globalThis.__wjs2_preload_list`（JSON 数组，Rust 侧置入）
/// 按序 require（stdio-from-preload 套件：预加载输出先于用户脚本进管道）。
/// CJS 限定（ESM 预加载 `--import` 另案）；缺失模块即抛（worker 起不来，
/// WError + exit 1，真机口径）。
pub const WORKER_PRELOAD_RUN: &str = r#"
(() => {
  const list = globalThis.__wjs2_preload_list ?? [];
  for (const f of list) {
    require(String(f));
  }
})();
"#;

// ── worker CPU 采样面（cpu-usage 套件；主→worker 自采回执，promise 结算）──

/// UNSAFE-BOUNDARY: 主→worker CPU 采样请求。`__wjs2_worker_cpu_sample(id, nonce)`
/// → boolean（worker 收件箱 `WCpuSample` 投递；无路由即 false→NOT_RUNNING）。
/// 前置：id 为已登记 worker；覆盖：`tests/node/worker_medium.rs::worker_cpu_usage_faces`。
pub unsafe extern "C" fn worker_cpu_sample(
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
        report_error(&mut cx, "TypeError: worker cpu sample needs a nonce");
        return false;
    }
    let nonce = value_to_string(&mut cx, frame.arg(1));
    let ok = match worker_route(id) {
        Some((inbox, _)) => inbox.send(WorkerEvent::WCpuSample { worker_id: id, nonce }).is_ok(),
        None => false,
    };
    frame.set_rval(mozjs::jsval::BooleanValue(ok));
    true
}

/// CPU 采样派发（`WCpuSample/WCpuResult` 用；`dispatch` 经 `&failed` 保底）。
/// Sample 只进 worker 会话（本线程 `thread_micros` 真值经主 inbox 回执）；
/// Result 只进主会话（worker 目标 `__ev("cpu-result")` 结算，JSON 载荷）。
pub fn dispatch_cpu(
    cx: &mut JSContext,
    global: *mut JSObject,
    failed: &dyn Fn(&mut JSContext) -> crate::error::Error,
    ev: &WorkerEvent,
) -> Result<(), crate::error::Error> {
    match ev {
        WorkerEvent::WCpuSample { worker_id, nonce } => {
            if state::worker_is_main() {
                return Ok(());
            }
            let (user, system) = super::process_::thread_micros();
            if let Some(tx) = state::with_plain(|p| p.worker_main_inbox.clone()) {
                let _ = tx.send(WorkerEvent::WCpuResult { worker_id: *worker_id, nonce: nonce.clone(), user, system });
            }
            Ok(())
        }
        WorkerEvent::WCpuResult { worker_id, nonce, user, system } => {
            if !state::worker_is_main() {
                return Ok(());
            }
            let Some(target) = state::worker_target(*worker_id) else {
                return Ok(());
            };
            if !target.is_object() {
                return Ok(());
            }
            rooted!(&in(cx) let t: *mut JSObject = target.to_object());
            let Some(fun) = get_prop_value(cx, t.get(), c"__ev") else {
                return Err(failed(cx));
            };
            let payload = serde_json::json!({ "nonce": nonce, "user": user, "system": system }).to_string();
            if with_str_args(cx, global, fun, "cpu-result", &payload).is_none() {
                return Err(failed(cx));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
