//! tests/node/events.rs — 对齐 src/builtins/node/events.rs（node:events）。

use crate::common::*;
use crate::helpers::*;
use assert_fs::prelude::*;

#[test]
fn events_basic_emit_on_off() {
    // test-events.js: emit 返回值/once/移除后不触发/eventNames
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("e.mjs");
    file.write_str(
        r#"import EE from "node:events";
const ee = new EE();
let calls = [];
function fn1() { calls.push("f1"); }
ee.on("x", fn1);
console.log(ee.emit("x"), ee.emit("nope"));
ee.once("y", () => calls.push("once"));
ee.emit("y"); ee.emit("y");
ee.removeListener("x", fn1);
console.log(calls.join(","), ee.listenerCount("x"), ee.emit("x"));
ee.on("z", () => {});
console.log(ee.eventNames().map(String).join(","));
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("true false"), "out: {out}");
    assert!(out.contains("f1,once"), "out: {out}");
    assert!(out.contains("z"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn events_unhandled_error_throws_original() {
    // test-events.js: 无 error 监听时 emit('error', er) 重抛原 Error（非包裹）
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("u.mjs");
    file.write_str(
        r#"import EE from "node:events";
const ee = new EE();
try { ee.emit("error", new TypeError("boom")); } catch (e) {
  console.log(e instanceof TypeError, e.message, "code" in e && e.code === undefined);
}
// 非 Error 实参 → ERR_UNHANDLED_ERROR 包裹
try { ee.emit("error", "str"); } catch (e) {
  console.log(e.code, e.message.startsWith("Unhandled error."));
}
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("true boom false"), "out: {out}");
    assert!(out.contains("ERR_UNHANDLED_ERROR true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn events_error_monitor_capture_rejections() {
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("c.mjs");
    file.write_str(
        r#"import EE from "node:events";
// errorMonitor: errorMonitor 监听在无 error 监听时也不抛（先于 doError 判定）
const em = new EE();
let seen = 0;
em.on(EE.errorMonitor, () => seen++);
try { em.emit("error", new Error("no-handler")); } catch {}
console.log("mon", seen);
// captureRejections: rejected listener 走 [captureRejectionSymbol] 而非 error
const cap = new EE({ captureRejections: true });
let handled = 0;
cap.on("x", async () => { throw new Error("rej"); });
cap[EE.captureRejectionSymbol] = (err, type) => { handled++; console.log("cap", type, err.message); };
cap.emit("x");
await new Promise((r) => setTimeout(r, 10));
console.log("handled", handled);
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("mon 1"), "out: {out}");
    assert!(
        out.contains("cap x rej") && out.contains("handled 1"),
        "out: {out}"
    );
    dir.close().unwrap();
}

#[test]
fn events_max_listeners_warning_and_validation() {
    // test-event-emitter-max-listeners.js: 泄漏警告 + warning 事件；参数校验消息逐字
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("m.mjs");
    file.write_str(
        r#"import EE from "node:events";
const warnings = [];
process.on("warning", (w) => warnings.push(w));
const ee = new EE();
for (let i = 0; i < 12; i++) ee.on("l", () => {});
// node 口径：emitWarning nextTick 异步派发（§4.102）——同步收集恒空，
// 转一 tick 后断言。
await new Promise((r) => setTimeout(r, 5));
console.log("warned", warnings.length, warnings[0]?.name, warnings[0]?.count);
// Node 原文消息格式（test-events-common 断言口径）
try { ee.once("x", 42); } catch (e) {
  console.log(e.code, e.message);
}
try { EE.setMaxListeners(-1); } catch (e) {
  console.log(e.code, e.constructor.name);
}
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(
        out.contains("warned 1 MaxListenersExceededWarning 11"),
        "out: {out}"
    );
    assert!(
        out.contains(
            "ERR_INVALID_ARG_TYPE The \"listener\" argument must be of type function. Received type number (42)"
        ),
        "out: {out}"
    );
    assert!(out.contains("ERR_OUT_OF_RANGE RangeError"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn events_once_and_on_iterator() {
    // test-events-on.js + test-events-on-async-iterator.js 语义子集
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("i.mjs");
    file.write_str(
        r#"import EE, { once, on } from "node:events";
const ee = new EE();
setTimeout(() => ee.emit("tick", 7, "s"), 5);
const [n, s] = await once(ee, "tick");
console.log("once", n, s);
// 异步迭代器 + close 事件
const src = new EE();
setTimeout(() => { src.emit("data", "a"); src.emit("data", "b"); src.emit("end"); }, 5);
const got = [];
for await (const [v] of on(src, "data", { close: ["end"] })) got.push(v);
console.log("iter", got.join(""));
// once + AbortSignal（已中止即 AbortError）
try {
  await once(new EE(), "x", { signal: AbortSignal.abort(new Error("why")) });
} catch (e) { console.log("abort", e.code, e.cause?.message); }
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(
        out.contains("once 7 s") && out.contains("iter ab"),
        "out: {out}"
    );
    assert!(out.contains("abort ABORT_ERR why"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn events_require_and_error_boundary() {
    // require('node:events') CJS 面 + 非法 emitter 报可读错（边界三件之一）
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("r.cjs");
    file.write_str(
        r#"const { EventEmitter, getEventListeners } = require("node:events");
const ee = new EventEmitter();
ee.on("a", () => 1);
console.log(require("node:events").EventEmitter === EventEmitter, getEventListeners(ee, "a").length);
try { getEventListeners(42, "a"); } catch (e) { console.log(e.code); }
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(
        out.contains("true 1") && out.contains("ERR_INVALID_ARG_TYPE"),
        "out: {out}"
    );
    dir.close().unwrap();
}

#[test]
fn event_target_null_passive_signal_lazy_handle() {
    // 正常：函数监听 this 绑定；报错：非法 signal 挂 ERR_INVALID_ARG_TYPE；
    // 边界：null 监听不抛（扁平化先行读 passive）、passive 下 preventDefault
    // 无效、后加 handleEvent 仍调用、函数自带 handleEvent 不干扰。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("et.mjs");
    file.write_str(
        r#"const t = (f) => { try { f(); return 'NO-THROW'; } catch (e) { return e.constructor.name + ':' + e.code; } };
const doc = new EventTarget();
let seen = false;
const q = { get passive() { seen = true; return false; }, get dummy() { throw new Error('dummy'); } };
doc.addEventListener('e', null, q);
console.log('null-ok', seen);
const d2 = new EventTarget();
let dp;
d2.addEventListener('t', function (e) { e.preventDefault(); dp = e.defaultPrevented; }, { passive: true });
console.log('passive-uncanceled', d2.dispatchEvent(new Event('t', { cancelable: true })), dp);
const d3 = new EventTarget();
console.log('sig-bad', t(() => d3.addEventListener('x', () => {}, { signal: {} })));
console.log('sig-null', t(() => d3.addEventListener('x', () => {}, { signal: null })));
{
  const ac = new AbortController();
  let n = 0;
  d3.addEventListener('y', () => { n++; }, { signal: ac.signal });
  d3.dispatchEvent(new Event('y'));
  ac.abort();
  d3.dispatchEvent(new Event('y'));
  console.log('sig-abort', n);
}
{
  const n = new EventTarget();
  const h = {};
  let who = null;
  n.addEventListener('e', h);
  h.handleEvent = function () { who = (this === h); };
  n.dispatchEvent(new Event('e'));
  let fnThis = null, handleHit = false;
  function g() { fnThis = true; }
  g.handleEvent = () => { handleHit = true; };
  n.addEventListener('e2', g);
  n.dispatchEvent(new Event('e2'));
  console.log('lazy', who, fnThis, !handleHit);
}
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("null-ok true"), "out: {out}");
    assert!(out.contains("passive-uncanceled true false"), "out: {out}");
    assert!(out.contains("sig-bad TypeError:ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("sig-null TypeError:ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("sig-abort 1"), "out: {out}");
    assert!(out.contains("lazy true true true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn events_signal_listenercount() {
    // 10f：once() 的 abort 接线对 listenerCount 可见（原生侧表；test-events-once 点名）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import { once, listenerCount, EventEmitter } from "node:events";
const ee = new EventEmitter();
const ac = new AbortController();
const p = once(ee, "x", { signal: ac.signal });
console.log("pending", listenerCount(ac.signal, "abort"), ee.listenerCount("error"));
ac.abort();
try { await p; } catch (e) { console.log("aborted", e.name); }
console.log("after", listenerCount(ac.signal, "abort"), ee.listenerCount("error"));
"#,
    );
    assert!(out.contains("pending 1 1"), "out: {out}");
    assert!(out.contains("aborted AbortError"), "out: {out}");
    assert!(out.contains("after 0 0"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn events_max_listeners_target() {
    // 10f：getMaxListeners(EventTarget) 回默认、AbortSignal 回 0（test-events-getmaxlisteners 点名）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import { getMaxListeners, setMaxListeners, defaultMaxListeners, EventEmitter, captureRejections, usingDomains, init } from "node:events";
const ee = new EventEmitter();
console.log("ee", getMaxListeners(ee) === defaultMaxListeners);
setMaxListeners(101, ee);
console.log("ee-set", getMaxListeners(ee));
const et = new EventTarget();
console.log("et", getMaxListeners(et) === defaultMaxListeners);
setMaxListeners(101, et);
console.log("et-set", getMaxListeners(et));
const sig = new AbortController().signal;
console.log("sig", getMaxListeners(sig));
setMaxListeners(5, sig);
console.log("sig-set", getMaxListeners(sig));
try { setMaxListeners(-1, et); } catch (e) { console.log("neg", e.code); }
console.log("named", captureRejections === false, usingDomains === false, typeof init, defaultMaxListeners);
"#,
    );
    assert!(out.contains("ee true"), "out: {out}");
    assert!(out.contains("ee-set 101"), "out: {out}");
    assert!(out.contains("et true"), "out: {out}");
    assert!(out.contains("et-set 101"), "out: {out}");
    assert!(out.contains("sig 0"), "out: {out}");
    assert!(out.contains("sig-set 5"), "out: {out}");
    assert!(out.contains("neg ERR_OUT_OF_RANGE"), "out: {out}");
    assert!(out.contains("named true true function 10"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn events_web_event_faces() {
    // Event 体系：正常（只读面/派发/once/phase/path）+ 报错（构造校验/
    // 品牌检查/元数）+ 边界（Symbol type/重派发/空派发）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
const t = (f) => { try { return f(); } catch (e) { return (e.code ?? "nocode") + "|" + e.constructor.name; } };
// 正常：只读面 + 派发 + once + phase/path。
const et = new EventTarget();
console.log("ro", new Event("a").type, new Event("b", {bubbles: true}).bubbles, new CustomEvent("c", {detail: 7}).detail);
console.log("tags", Object.prototype.toString.call(new Event("x")), Object.prototype.toString.call(et));
console.log("statics", Event.NONE, Event.CAPTURING_PHASE, Event.AT_TARGET, Event.BUBBLING_PHASE);
let seen = [];
et.addEventListener("e", (ev) => seen.push([ev.type, ev.eventPhase, ev.composedPath().length, ev.target === et]));
et.addEventListener("e", () => seen.push("once"), { once: true });
console.log("dispatch", et.dispatchEvent(new Event("e")), JSON.stringify(seen));
console.log("after", new Event("z").eventPhase, new Event("z").composedPath().length);
// 报错：构造校验 + 品牌 + 元数。
console.log("ctor0", t(() => new Event()));
console.log("ctoropt", t(() => new Event("a", 1)));
console.log("ctorsym", t(() => new Event(Symbol())));
console.log("brand", t(() => Reflect.get(Event.prototype, "type", {})));
console.log("brandm", t(() => Reflect.apply(Event.prototype.stopPropagation, [], {})));
console.log("brandt", t(() => Reflect.apply(EventTarget.prototype.addEventListener, [], {})));
console.log("arity", t(() => et.addEventListener("x")), t(() => et.dispatchEvent()));
console.log("badls", t(() => et.addEventListener("x", 42)));
console.log("badsym", t(() => et.addEventListener(Symbol(), () => {})));
console.log("badev", t(() => et.dispatchEvent({})));
// 边界：重派发 + 空 target 派发 + onfoo。
const e2 = new Event("r");
et.addEventListener("r", () => console.log("redispatch", t(() => et.dispatchEvent(e2))));
et.dispatchEvent(e2);
"#,
    );
    assert!(out.contains("ro a true 7"), "out: {out}");
    assert!(out.contains("tags [object Event] [object EventTarget]"), "out: {out}");
    assert!(out.contains("statics 0 1 2 3"), "out: {out}");
    assert!(out.contains("dispatch true"), "out: {out}");
    assert!(out.contains("after 0 0"), "out: {out}");
    assert!(out.contains("ctor0 nocode|TypeError"), "out: {out}");
    assert!(out.contains("ctoropt ERR_INVALID_ARG_TYPE|TypeError"), "out: {out}");
    assert!(out.contains("ctorsym nocode|TypeError"), "out: {out}");
    assert!(out.contains("brand ERR_INVALID_THIS|TypeError"), "out: {out}");
    assert!(out.contains("brandm ERR_INVALID_THIS|TypeError"), "out: {out}");
    assert!(out.contains("brandt ERR_INVALID_THIS|TypeError"), "out: {out}");
    assert!(out.contains("arity ERR_MISSING_ARGS|TypeError ERR_MISSING_ARGS|TypeError"), "out: {out}");
    assert!(out.contains("badls ERR_INVALID_ARG_TYPE|TypeError"), "out: {out}");
    assert!(out.contains("badsym nocode|TypeError"), "out: {out}");
    assert!(out.contains("badev ERR_INVALID_ARG_TYPE|TypeError"), "out: {out}");
    assert!(out.contains("redispatch ERR_EVENT_RECURSION|Error"), "out: {out}");
    dir.close().unwrap();
}
