//! tests/node/worker.rs — 对齐 src/builtins/node/worker.rs（node:worker_threads）。

use crate::helpers::*;
use assert_fs::prelude::*;

#[test]
fn worker_channel_roundtrip() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { MessageChannel, MessagePort, receiveMessageOnPort } from "node:worker_threads";
const { port1, port2 } = new MessageChannel();
console.log("ch-ports", port1 instanceof MessagePort, port2 instanceof MessagePort);
port1.on("message", (m) => {
  console.log("ch-p1", JSON.stringify(m) === JSON.stringify({ n: 41 }));
  port1.postMessage([1, "x", true]);
});
port2.on("message", (m) => {
  console.log("ch-p2", Array.isArray(m) && m[1] === "x");
  port1.close(); port2.close();
});
port2.postMessage({ n: 41 });
// 迟挂监听：先投递再 on，newListener 开闸照样收到
const late = new MessageChannel();
late.port2.postMessage("late-hi");
await new Promise((r) => setTimeout(r, 20));
late.port1.on("message", (m) => {
  console.log("ch-late", m === "late-hi");
  late.port1.close(); late.port2.close();
});
// 无监听排队：receiveMessageOnPort 同步取出
const q = new MessageChannel();
q.port2.postMessage("q1");
q.port2.postMessage("q2");
await new Promise((r) => setTimeout(r, 20));
console.log("ch-recv", receiveMessageOnPort(q.port1).message === "q1", receiveMessageOnPort(q.port1).message === "q2", receiveMessageOnPort(q.port1) === undefined);
try { q.port2.postMessage(() => {}); } catch (e) { console.log("ch-fn", e.name === "DataCloneError"); }
q.port1.close(); q.port2.close();
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("ch-ports true true"), "out: {out}");
    assert!(out.contains("ch-p1 true"), "out: {out}");
    assert!(out.contains("ch-p2 true"), "out: {out}");
    assert!(out.contains("ch-late true"), "out: {out}");
    assert!(out.contains("ch-recv true true true"), "out: {out}");
    assert!(out.contains("ch-fn true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn worker_thread_info_boundary() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { isMainThread, threadId, parentPort, workerData, resourceLimits, SHARE_ENV, setEnvironmentData, getEnvironmentData, markAsUncloneable, moveMessagePortToContext, MessageChannel } from "node:worker_threads";
console.log("th-self", isMainThread === true, threadId === 0, parentPort === null, workerData === null);
console.log("th-res", typeof resourceLimits === "object", typeof SHARE_ENV === "symbol");
setEnvironmentData("wk", { v: 7 });
console.log("th-env", JSON.stringify(getEnvironmentData("wk")) === JSON.stringify({ v: 7 }), getEnvironmentData("missing") === undefined);
try { setEnvironmentData(42, 1); } catch (e) { console.log("th-badkey", e.code === "ERR_INVALID_ARG_TYPE"); }
try { getEnvironmentData(42); } catch (e) { console.log("th-badkey2", e.code === "ERR_INVALID_ARG_TYPE"); }
const o = { a: 1 };
markAsUncloneable(o);
const { port1, port2 } = new MessageChannel();
try { port2.postMessage(o); } catch (e) { console.log("th-unc", e.name === "DataCloneError"); }
console.log("th-move", moveMessagePortToContext(port1, {}) === port1);
port1.close(); port2.close();
// unref 端口不续命：不 close 照样退出
const u = new MessageChannel();
u.port1.unref(); u.port2.unref();
u.port2.postMessage("dropped");
console.log("th-unref", true);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("th-self true true true true"), "out: {out}");
    assert!(out.contains("th-res true true"), "out: {out}");
    assert!(out.contains("th-env true true"), "out: {out}");
    assert!(out.contains("th-badkey true"), "out: {out}");
    assert!(out.contains("th-badkey2 true"), "out: {out}");
    assert!(out.contains("th-unc true"), "out: {out}");
    assert!(out.contains("th-move true"), "out: {out}");
    assert!(out.contains("th-unref true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn worker_eval_and_data() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Worker, isMainThread, threadId } from "node:worker_threads";
console.log("wk-self", isMainThread === true, threadId === 0);
const w = new Worker("import { parentPort } from 'node:worker_threads'; parentPort.postMessage(40 + 2);", { eval: true });
console.log("wk-tid", w.threadId > 0);
w.on("online", () => console.log("wk-online", true));
w.on("message", (m) => console.log("wk-msg", m === 42));
w.on("error", (e) => console.log("wk-err", e.message));
w.on("exit", (c) => console.log("wk-exit", c === 0));
const d = new Worker("import { parentPort, workerData } from 'node:worker_threads'; parentPort.postMessage({ e: workerData.n + 1 });", { eval: true, workerData: { n: 41 } });
d.on("message", (m) => console.log("wk-data", m.e === 42));
d.on("exit", () => {});
d.on("error", (e) => console.log("wk-derr", e.message));
console.log("wk-ref", w.unref() === w, w.ref() === w);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("wk-self true true"), "out: {out}");
    assert!(out.contains("wk-tid true"), "out: {out}");
    assert!(out.contains("wk-online true"), "out: {out}");
    assert!(out.contains("wk-msg true"), "out: {out}");
    assert!(out.contains("wk-exit true"), "out: {out}");
    assert!(out.contains("wk-data true"), "out: {out}");
    assert!(out.contains("wk-ref true true"), "out: {out}");
    assert!(!out.contains("wk-err"), "out: {out}");
    assert!(!out.contains("wk-derr"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn worker_twoway_terminate() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Worker } from "node:worker_threads";
const w = new Worker("import { parentPort } from 'node:worker_threads'; parentPort.on('message', (m) => parentPort.postMessage(m * 2));", { eval: true });
w.on("online", () => w.postMessage(21));
w.on("message", (m) => {
  console.log("wx-msg", m === 42);
  w.terminate().then((c) => console.log("wx-term", c === 1));
});
w.on("exit", (c) => console.log("wx-exit", c === 1));
w.on("error", (e) => console.log("wx-err", e.message));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("wx-msg true"), "out: {out}");
    assert!(out.contains("wx-term true"), "out: {out}");
    assert!(out.contains("wx-exit true"), "out: {out}");
    assert!(!out.contains("wx-err"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn worker_errors_boundary() {
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("wfile.js").write_str("import { parentPort } from \"node:worker_threads\";\nparentPort.postMessage(\"file-ok\");\n").unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import { Worker } from "node:worker_threads";
try { new Worker(42); } catch (e) { console.log("we-badfile", e.code === "ERR_INVALID_ARG_TYPE"); }
const f = new Worker("./wfile.js");
f.on("message", (m) => console.log("we-file", m === "file-ok"));
f.on("exit", () => {});
f.on("error", (e) => console.log("we-ferr", e.message));
const t = new Worker("throw new Error('boom-x')", { eval: true });
t.on("error", (e) => console.log("we-throw", e.message.includes("boom-x")));
t.on("exit", (c) => console.log("we-texit", c === 1));
const m = new Worker("./nope-missing.js");
m.on("error", (e) => console.log("we-miss", e.message.includes("nope-missing")));
m.on("exit", (c) => console.log("we-mexit", c === 1));
const e2 = new Worker("void 0", { eval: true });
e2.on("exit", (c) => {
  console.log("we-e2", c === 0);
  e2.postMessage("late-drop");
  e2.terminate().then((cc) => console.log("we-term2", cc === 0));
});
e2.on("error", (e) => console.log("we-e2err", e.message));
const n = new Worker("import { workerData } from 'node:worker_threads'; import { parentPort } from 'node:worker_threads'; parentPort.postMessage(workerData === null);", { eval: true });
n.on("message", (mm) => console.log("we-novalue", mm === true));
n.on("exit", () => {});
n.on("error", (e) => console.log("we-nerr", e.message));
const x = new Worker("process.exit(7);", { eval: true });
x.on("exit", (c) => console.log("we-code", c === 7));
x.on("error", (e) => console.log("we-xerr", e.message));
"#,
    );
    assert!(out.contains("we-badfile true"), "out: {out}");
    assert!(out.contains("we-file true"), "out: {out}");
    assert!(out.contains("we-throw true"), "out: {out}");
    assert!(out.contains("we-texit true"), "out: {out}");
    assert!(out.contains("we-miss true"), "out: {out}");
    assert!(out.contains("we-mexit true"), "out: {out}");
    assert!(out.contains("we-e2 true"), "out: {out}");
    assert!(out.contains("we-term2 true"), "out: {out}");
    assert!(out.contains("we-novalue true"), "out: {out}");
    assert!(out.contains("we-code true"), "out: {out}");
    assert!(!out.contains("we-ferr"), "out: {out}");
    assert!(!out.contains("we-e2err"), "out: {out}");
    assert!(!out.contains("we-nerr"), "out: {out}");
    assert!(!out.contains("we-xerr"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn worker_transfer_buffer_types() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { MessageChannel } from "node:worker_threads";
const { port1, port2 } = new MessageChannel();
const seen = [];
port2.on("message", (m) => { seen.push(m); });
const ab = new Uint8Array([1, 2, 3]).buffer;
port1.postMessage(ab, [ab]);
console.log("w9i-detach", ab.byteLength === 0);
const ab2 = new Uint8Array([4, 5]).buffer;
port1.postMessage(ab2);
console.log("w9i-copy", ab2.byteLength === 2);
const sub = new Uint8Array([1, 2, 3, 4]).subarray(1, 3);
port1.postMessage({ sub });
port1.postMessage({ bi: 5n, u: undefined, m: new Map([[1, 2]]), s: new Set([3]), d: new Date(0), ta: new Uint8Array([9]), dv: new DataView(new Uint8Array([7, 8]).buffer) });
const t = (n, f) => { try { f(); console.log(n, "NO-THROW"); } catch (e) { console.log(n, e.name); } };
t("w9i-baditem", () => port1.postMessage({ x: 1 }, [{ x: 1 }]));
t("w9i-dup", () => port1.postMessage("x", [ab2, ab2]));
t("w9i-detached", () => port1.postMessage(ab));
t("w9i-fn", () => port1.postMessage(() => {}));
// 循环/共享保留（M5 升级：先序 ref，真机口径；此前抛错记档作废）。
const circ = { n: 1 }; circ.me = circ; circ.arr = [circ];
port1.postMessage({ c: circ });
const shared = { v: 7 };
port1.postMessage({ a: shared, b: shared, m: new Map([["k", shared]]) });
setTimeout(() => {
  const [det, c2, subMsg, ty, circMsg, sharedMsg] = seen;
  console.log("w9i-gotxfer", det instanceof ArrayBuffer, det.byteLength === 3);
  console.log("w9i-gotbuf", c2 instanceof ArrayBuffer, c2.byteLength === 2, new Uint8Array(c2)[0] === 4);
  console.log("w9i-sub", ty !== undefined && subMsg.sub instanceof Uint8Array, subMsg.sub.length === 2, subMsg.sub[0] === 2, subMsg.sub.byteOffset === 1);
  console.log("w9i-types", typeof ty.bi === "bigint", ("u" in ty) && ty.u === undefined, ty.m instanceof Map && ty.m.get(1) === 2, ty.s instanceof Set && ty.s.has(3), ty.d instanceof Date && ty.d.getTime() === 0, ty.ta instanceof Uint8Array && ty.ta[0] === 9, ty.dv instanceof DataView && ty.dv.getUint8(1) === 8);
  console.log("w9i-circular", circMsg.c.n === 1, circMsg.c.me === circMsg.c, circMsg.c.arr[0] === circMsg.c, circMsg.c.arr !== circ.arr);
  console.log("w9i-shared", sharedMsg.a.v === 7, sharedMsg.a === sharedMsg.b, sharedMsg.m.get("k") === sharedMsg.a);
  port1.close();
  port2.close();
}, 200);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "w9i-detach true",
        "w9i-copy true",
        "w9i-baditem DataCloneError",
        "w9i-dup DataCloneError",
        "w9i-detached DataCloneError",
        "w9i-fn DataCloneError",
        "w9i-gotxfer true true",
        "w9i-gotbuf true true true",
        "w9i-sub true true true true",
        "w9i-types true true true true true true true",
        "w9i-circular true true true true",
        "w9i-shared true true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_transfer_port_migration() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { MessageChannel, MessagePort } from "node:worker_threads";
const { port1, port2 } = new MessageChannel();
const { port1: a1, port2: a2 } = new MessageChannel();
let a1got = [];
a1.on("message", (m) => { a1got.push(m); });
port1.postMessage({ p: a2 }, [a2]);
// 源端 neutered：后用静默，a1 收不到。
a2.postMessage("lost");
port2.on("message", (m) => {
  const ok = m.p instanceof MessagePort;
  console.log("w9i-mig", ok);
  if (!ok) return;
  globalThis.__mig = m.p;
  m.p.on("message", (x) => console.log("w9i-migmsg", x === "to-migrated"));
  m.p.postMessage("to-a1");
  a1.postMessage("to-migrated");
});
const t = (n, f) => { try { f(); console.log(n, "NO-THROW"); } catch (e) { console.log(n, e.name); } };
t("w9i-portnotransfer", () => port1.postMessage({ p: a1 }));
setTimeout(() => {
  console.log("w9i-neuter", !a1got.includes("lost"), a1got.includes("to-a1"));
  if (globalThis.__mig) globalThis.__mig.close();
  port1.close();
  port2.close();
  a1.close();
}, 300);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "w9i-mig true",
        "w9i-migmsg true",
        "w9i-portnotransfer DataCloneError",
        "w9i-neuter true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_transfer_cross_thread_and_broadcast() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { MessageChannel, BroadcastChannel, Worker } from "node:worker_threads";
// 同会话 BC：自收排除、关者止收。
const b1 = new BroadcastChannel("w9i-bc");
const b2 = new BroadcastChannel("w9i-bc");
const b3 = new BroadcastChannel("w9i-other");
const seen = [];
b2.onmessage = (e) => { seen.push(["b2", e.data]); };
b1.onmessage = () => { seen.push(["b1", "SELF"]); };
b3.onmessage = (e) => { seen.push(["b3", e.data]); };
b1.postMessage("hello");
await new Promise((r) => setTimeout(r, 100));
b2.close();
b1.postMessage("after");
await new Promise((r) => setTimeout(r, 100));
// 跨线程：端口经 workerData 迁移 + BC 跨线程扇出。
const { port1, port2 } = new MessageChannel();
const back = [];
port1.on("message", (m) => { back.push(m); });
const w = new Worker(
  "import { parentPort, workerData, BroadcastChannel } from 'node:worker_threads';" +
  "const bc = new BroadcastChannel('w9i-x');" +
  "workerData.p.on('message', (m) => { parentPort.postMessage('w saw ' + m); bc.postMessage('from-worker'); });" +
  "workerData.p.postMessage('hi-main');",
  { eval: true, workerData: { n: 41n, p: port2 }, transferList: [port2] }
);
const xseen = [];
const xb = new BroadcastChannel("w9i-x");
xb.onmessage = (e) => { xseen.push(e.data); };
w.on("message", (m) => { back.push("W:" + m); });
w.on("error", (e) => { back.push("ERR" + e.message); });
port1.postMessage("hi-worker");
setTimeout(() => {
  console.log("w9i-bc", JSON.stringify(seen) === JSON.stringify([["b2", "hello"]]));
  console.log("w9i-xfer", back.includes("hi-main"), back.includes("W:w saw hi-worker"));
  console.log("w9i-xbc", xseen.includes("from-worker"));
  w.terminate().then(() => {
    b1.close();
    b3.close();
    xb.close();
    port1.close();
  });
}, 600);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in ["w9i-bc true", "w9i-xfer true true", "w9i-xbc true"] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_stdio_surface() {
    // Worker 标出流（node 真机：stdout/stderr 缺省即 Readable 管道，stdin 仅
    // stdin:true 具现；旧 null-stdio 断言随实测翻转）：pipe/unpipe 继承形 +
    // 数据流动 + 退出收尾（消费后 end/close，readableEnded 置位）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Worker } from "node:worker_threads";
const a = new Worker("import { parentPort } from 'node:worker_threads'; parentPort.postMessage(1);", { eval: true });
console.log("pipe-stdio", a.stdout !== null, a.stderr !== null, a.stdin === null);
console.log("shape-stdio", typeof a.stdout.pipe, typeof a.stdout.unpipe, typeof a.stdout.setEncoding);
a.on("exit", () => {});
const b = new Worker("console.log('BMSG');", { eval: true, stdout: true, stderr: true, stdin: true });
console.log("has-stdio", b.stdout !== null, b.stderr !== null, b.stdin !== null);
let seen = "";
b.stdout.setEncoding("utf8");
b.stdout.on("data", (c) => { seen += c; });
let ended = 0;
b.stdout.once("end", () => { ended++; });
b.stdout.once("close", () => { ended++; });
b.on("exit", (c) => setTimeout(() => console.log("stdio-exit", c === 0, ended === 2, b.stdout.readableEnded, seen.includes("BMSG")), 50));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in ["pipe-stdio true true true", "shape-stdio function function function", "has-stdio true true true", "stdio-exit true true true true"] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_error_shape_and_event_faces() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "main.mjs",
        r#"
import { Worker, MessageChannel, MessagePort, BroadcastChannel, workerData, threadName, parentPort } from "node:worker_threads";
import assert from "node:assert";

// ── worker 分支：threadName 导出 + 抛原始值（number/string 各一 worker）──
if (workerData?.throwKind === "num") {
  parentPort.postMessage("armed");
  throw 42;
}
if (workerData?.throwKind === "str") {
  parentPort.postMessage("armed");
  throw "boom";
}

// 1) MessageEvent 全局：默认面 + init 转换（WebIDL USVString/DOMString）。
assert.strictEqual(typeof MessageEvent, "function");
const ev = new MessageEvent("message", { data: 2, origin: 1, lastEventId: 0 });
console.log("ev-msg", `${ev.type}|${ev.data}|${ev.origin}|${ev.lastEventId}|${ev.source}|${JSON.stringify(ev.ports)}` === "message|2|1|0|null|[]");
console.log("ev-msg-inst", new MessageEvent("m") instanceof Event);
let t1 = false;
try { new MessageEvent("m", { source: 1 }); } catch (e) { t1 = /Expected eventInitDict\.source \("1"\) to be an instance of MessagePort\./.test(e.message); }
let t2 = false;
try { new MessageEvent("m", { ports: 0 }); } catch (e) { t2 = /eventInitDict\.ports \(0\) is not iterable\./.test(e.message); }
let t3 = false;
try { new MessageEvent("m", { ports: [null] }); } catch (e) { t3 = /Expected eventInitDict\.ports\[0\] \("null"\) to be an instance of MessagePort\./.test(e.message); }
console.log("ev-msg-err", t1, t2, t3);

// 2) MessagePort EventTarget 双面：自定义类型 CustomEvent(detail)；EE 裸值不变；
//    onmessage 收真 MessageEvent（data/target/ports）。
{
  const { port1, port2 } = new MessageChannel();
  let etType = "", etDetail = "", eeVal = "";
  port2.addEventListener("foo", (e) => { etType = e.type; etDetail = e.detail; });
  port2.on("foo", (v) => { eeVal = v; });
  port2.emit("foo", "bar");
  console.log("port-ev", `${etType}|${etDetail}|${eeVal}` === "foo|bar|bar");
  // removeEventListener 摘净
  const fn = () => { etType = "BAD"; };
  port2.addEventListener("foo", fn);
  port2.removeEventListener("foo", fn);
  port2.emit("foo", "x");
  console.log("port-rm", etType === "foo");
  const got = await new Promise((res) => {
    port1.onmessage = (m) => res(m);
    port2.postMessage(4);
  });
  console.log("port-om", got instanceof MessageEvent, got.data === 4, got.target === port1, Array.isArray(got.ports) && got.ports.length === 0);
  port1.close(); port2.close();
}

// 3) BroadcastChannel：message 事件收 MessageEvent(data)；缺参/已关报错。
{
  const bc1 = new BroadcastChannel("ch");
  const bc2 = new BroadcastChannel("ch");
  const got = await new Promise((res) => {
    bc1.addEventListener("message", (e) => res(e));
    bc2.postMessage("hello");
  });
  console.log("bc-msg", got instanceof MessageEvent, got.data === "hello");
  bc1.close(); bc2.close();
  const bcX = new BroadcastChannel("ch");
  bcX.close(); bcX.close();
  let threw1 = "";
  try { bcX.postMessage(null); } catch (e) { threw1 = e.message; }
  let threw2 = "";
  const bcY = new BroadcastChannel("ch");
  try { bcY.postMessage(); } catch (e) { threw2 = e.message; }
  bcY.close();
  console.log("bc-err", threw1 === "BroadcastChannel is closed", threw2 === 'The "message" argument must be specified');
}

// 4) threadName（属性 + 退出置 null）+ resourceLimits 缺省 {}。
{
  const w = new Worker(new URL(import.meta.url).pathname, { name: "tname", workerData: { throwKind: "num" } });
  console.log("w-name", w.threadName === "tname", JSON.stringify(w.resourceLimits) === "{}");
  const errs = [];
  w.on("error", (e) => errs.push(e));
  w.on("exit", (c) => {
    console.log("w-prim", c === 1, errs.length === 1, typeof errs[0] === "number", errs[0] === 42);
    console.log("w-null", w.threadName === null);
    // string 原始值
    const w2 = new Worker(new URL(import.meta.url).pathname, { workerData: { throwKind: "str" } });
    const errs2 = [];
    w2.on("error", (e) => errs2.push(e));
    w2.on("exit", (c2) => {
      console.log("w-str", c2 === 1, errs2.length === 1, errs2[0] === "boom");
      run5();
    });
  });
}

// 5) 缺主模块：error 事件文案 node 形（Cannot find module '<abs>'）。
function run5() {
  const missing = new URL("file:///no/such/worker-does-not-exist.js");
  const w3 = new Worker(missing);
  w3.on("error", (e) => console.log("missing-mod", /Cannot find module .+worker-does-not-exist\.js/.test(e.message)));
  w3.on("exit", () => console.log("END"));
}
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in [
        "ev-msg true",
        "ev-msg-inst true",
        "ev-msg-err true true true",
        "port-ev true",
        "port-rm true",
        "port-om true true true true",
        "bc-msg true true",
        "bc-err true true",
        "w-name true true",
        "w-prim true true true true",
        "w-null true",
        "w-str true true true",
        "missing-mod true",
        "END",
    ] {
        assert!(stdout.lines().any(|l| l == line), "missing line: {line}\nout: {stdout}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_typed_view_and_sab_envelope() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "view.mjs",
        r#"
import { MessageChannel, MessagePort } from "node:worker_threads";
import assert from "node:assert";

// 边界：BPE>1 的 typed array 跨端（byteLength → 元素数折算；修前 Int32/Float64
// 全 OOB 静默丢消息）+ 子视图偏移 + DataView + SAB 品牌副本。
const { port1, port2 } = new MessageChannel();
port2.on("message", (m) => {
  port2.postMessage(m);
});
const results = await new Promise((res) => {
  const out = [];
  let step = 0;
  port1.on("message", (m) => {
    out.push(m);
    step++;
    if (step === 4) res(out);
  });
  port1.postMessage(new Int32Array([1, 2, 3, 4]));
  const f = new Float64Array([1.5, -2.5]);
  port1.postMessage(f);
  const ab = new ArrayBuffer(16);
  port1.postMessage(new Uint8Array(ab, 4, 8));
  port1.postMessage(new DataView(new ArrayBuffer(8)));
});
console.log(
  "view-i32",
  results[0] instanceof Int32Array, JSON.stringify([...results[0]]) === "[1,2,3,4]",
);
console.log("view-f64", results[1] instanceof Float64Array, JSON.stringify([...results[1]]) === "[1.5,-2.5]");
console.log("view-sub", results[2] instanceof Uint8Array, results[2].byteOffset === 4, results[2].byteLength === 8, results[2].buffer.byteLength === 16);
console.log("view-dv", results[3] instanceof DataView, results[3].byteLength === 8);

// SAB：品牌 roundtrip（副本语义；真共享内存跨线程底座记档）。
if (typeof SharedArrayBuffer === "function") {
  const sab = new SharedArrayBuffer(8);
  new Uint8Array(sab).set([7, 7]);
  const got = await new Promise((res) => {
    port1.on("message", (m) => { res(m); });
    port1.postMessage(sab);
  });
  console.log("sab", got instanceof SharedArrayBuffer, got.byteLength === 8, new Uint8Array(got)[0] === 7);
} else {
  console.log("sab skip");
}
port1.close(); port2.close();
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in [
        "view-i32 true true",
        "view-f64 true true",
        "view-sub true true true true",
        "view-dv true true",
        "sab true true true",
    ] {
        assert!(stdout.lines().any(|l| l == line), "missing line: {line}\nout: {stdout}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_terminate_interrupt_busy_loop() {
    // 10f：terminate 打断忙 JS 循环（interrupt 机制）——timer 回调内 while(true)、
    // 微任务自递归、nextTick 环、postMessage 洪泛、worker 内 busy 循环退出码 1
    // 且不发 error 事件。标签互不为子串（§4.42）：tmr-term/mt-term/tick-term/
    // flood-term/vm-term。正常+报错+边界。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("vm-busy.mjs")
        .write_str(r#"import vm from "node:vm"; while (true) vm.runInNewContext("");"#)
        .unwrap();
    let out = run_node_file(
        &dir,
        "term.mjs",
        r#"
import { Worker } from "node:worker_threads";
import assert from "node:assert";
// timer 回调内死循环：message → terminate → interrupt → exit(1)
{
  const w = new Worker(`
    const { parentPort } = require('worker_threads');
    setTimeout(() => { parentPort.postMessage({}); while (true); });
  `, { eval: true });
  w.on("message", () => w.terminate());
  w.on("error", () => console.log("tmr-term-err", true));
  w.on("exit", (code) => console.log("tmr-term", code === 1));
}
// 微任务环：terminate 打断（exit 1）
{
  const w = new Worker(`
    function loop() { Promise.resolve().then(loop); } loop();
    require('worker_threads').parentPort.postMessage('up');
  `, { eval: true });
  w.once("message", () => setImmediate(() => w.terminate()));
  w.on("error", () => console.log("mt-term-err", true));
  w.on("exit", (code) => console.log("mt-term", code === 1));
}
// nextTick 环：terminate 定时打点后打断
{
  const w = new Worker(`
    require('worker_threads').parentPort.postMessage('0');
    process.nextTick(() => { while (1); });
  `, { eval: true });
  w.on("message", () => setTimeout(() => w.terminate().then(() => {}), 1));
  w.on("error", () => console.log("tick-term-err", true));
  w.on("exit", (code) => console.log("tick-term", code === 1));
}
// postMessage 洪泛：首条消息即 terminate
{
  const w = new Worker(`
    const p = require('worker_threads').parentPort;
    while (true) p.postMessage({});
  `, { eval: true });
  w.once("message", () => w.terminate());
  w.on("error", () => console.log("flood-term-err", true));
  w.on("exit", (code) => console.log("flood-term", code === 1));
}
// vm 忙循环：worker 文件 while + runInNewContext，无 error 事件、exit 1
{
  const busyPath = new URL("vm-busy.mjs", import.meta.url).pathname;
  const w = new Worker(busyPath);
  w.on("error", () => console.log("vm-term-err", true));
  w.on("exit", (code) => console.log("vm-term", code === 1));
  setTimeout(() => w.terminate(), 50);
}
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "tmr-term true",
        "mt-term true",
        "tick-term true",
        "flood-term true",
        "vm-term true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    assert!(!out.contains("-err true"), "worker error event fired (must not):\n{out}");
    dir.close().unwrap();
}

#[test]
fn worker_bc_surface_and_env_snapshot() {
    // 10f：BroadcastChannel 校验面（name/postMessage 缺参边界、显式 undefined
    // 合法、Symbol 转换错、同步收信、inspect 形、ref/unref 品牌门）+ worker
    // env 快照隔离（子线程写不回主线程）+ threadId 退出后 -1。
    // 标签互不为子串（§4.42）：bc-sync/bc-inspect/bc-brand/env-iso/env-key。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "bc.mjs",
        r#"
import { BroadcastChannel, Worker, receiveMessageOnPort, threadId } from "node:worker_threads";
import assert from "node:assert";
import { inspect } from "node:util";
// 显式 undefined 合法（name "undefined"）
{
  const bc = new BroadcastChannel(undefined);
  console.log("bc-undef", bc.name === "undefined");
  bc.close();
}
// postMessage(undefined) 合法；缺参抛 MISSING_ARGS
{
  const bc = new BroadcastChannel("pm-undef");
  bc.postMessage(undefined);
  bc.close();
  assert.throws(() => bc.postMessage(), (e) => e.code === "ERR_MISSING_ARGS");
  console.log("bc-pm-args", true);
}
// 同步收信（无监听到达 → receiveMessageOnPort 拉取；二次即 undefined）
{
  const b1 = new BroadcastChannel("sync-ch");
  const b2 = new BroadcastChannel("sync-ch");
  b1.postMessage("ping");
  console.log("bc-sync", receiveMessageOnPort(b2).message === "ping", receiveMessageOnPort(b2) === undefined);
  b1.close(); b2.close();
}
// inspect 形（active true→false）+ 非真实 active 属性
{
  const bc = new BroadcastChannel("insp-ch");
  console.log("bc-inspect", inspect(bc.ref()) === "BroadcastChannel { name: 'insp-ch', active: true }");
  bc.close();
  console.log("bc-inspect-closed", inspect(bc.ref()) === "BroadcastChannel { name: 'insp-ch', active: false }");
}
// ref/unref/close/postMessage 品牌门（ERR_INVALID_THIS）
{
  let n = 0;
  for (const m of ["close", "postMessage", "ref", "unref"]) {
    try {
      Reflect.apply(BroadcastChannel.prototype[m], [], {});
      console.log("bc-brand-bad", m);
    } catch (e) { if (e.code === "ERR_INVALID_THIS") n++; }
  }
  console.log("bc-brand", n === 4);
}
// env 快照隔离 + worker threadId 退出后 -1
{
  process.env.WJS10F = "main";
  const w = new Worker(`
    const { parentPort } = require('worker_threads');
    const inherited = process.env.WJS10F;
    process.env.WJS10F = "worker-only";
    process.env.WJS10F_ONLY = "set";
    const assert = require('assert');
    assert.strictEqual(inherited, "main");
    parentPort.postMessage(process.env.WJS10F_ONLY);
  `, { eval: true });
  w.on("message", (m) => {
    console.log("env-iso", m === "set", process.env.WJS10F === "main", process.env.WJS10F_ONLY === undefined);
  });
  w.on("exit", (code) => {
    console.log("env-key", code === 0, w.threadId === -1);
  });
}
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in [
        "bc-undef true",
        "bc-pm-args true",
        "bc-sync true true",
        "bc-inspect true",
        "bc-inspect-closed true",
        "bc-brand true",
        "env-iso true true true",
        "env-key true true",
    ] {
        assert!(stdout.lines().any(|l| l == line), "missing line: {line}\nout: {stdout}");
    }
    assert!(!stdout.contains("bc-brand-bad"), "out: {stdout}");
    dir.close().unwrap();
}

#[test]
fn worker_message_port_faces() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { MessageChannel, MessagePort, Worker, markAsUntransferable } from "node:worker_threads";
import util from "node:util";
// 原型整形（message-port 套件逐字）
console.log("mp-proto", JSON.stringify(Object.getOwnPropertyNames(MessagePort.prototype).sort()) === JSON.stringify(["close","constructor","hasRef","onmessage","onmessageerror","postMessage","ref","start","unref"]));
// 构造门（message-port-constructor 套件）
const t = (n, f, code) => { try { f(); console.log(n, "NO-THROW"); } catch (e) { console.log(n, e.code === code); } };
t("mp-call", () => MessagePort(), "ERR_CONSTRUCT_CALL_INVALID");
t("mp-new", () => new MessagePort(), "ERR_CONSTRUCT_CALL_INVALID");
t("mp-newx", () => new MessagePort("x"), "ERR_CONSTRUCT_CALL_INVALID");
t("mc-call", () => MessageChannel(), "ERR_CONSTRUCT_CALL_REQUIRED");
// close 回调异步 + 顺序 + 双关（close 套件）
const { port1, port2 } = new MessageChannel();
const order = [];
port1.on("close", () => order.push("event"));
port1.close(() => order.push("cb"));
order.push("sync");
let fired = 0;
port1.on("close", () => fired++);
setTimeout(() => {
  console.log("mp-close-order", JSON.stringify(order) === JSON.stringify(["sync","event","cb"]), fired === 1);
  // transfer 归一（message-port 套件）
  const c = new MessageChannel();
  c.port1.postMessage(4, {});
  c.port1.postMessage(5, { transfer: undefined });
  t("mp-bare0", () => c.port1.postMessage(5, 0), "ERR_INVALID_ARG_TYPE");
  t("mp-optnull", () => c.port1.postMessage(5, { transfer: null }), "ERR_INVALID_ARG_TYPE");
  // 自杀式投递（target 套件）
  const s = new MessageChannel();
  const ab = new ArrayBuffer(10);
  let warned = false;
  process.on("warning", (w) => { warned = w.message.includes("communication channel was lost"); });
  s.port2.on("message", () => console.log("mp-suicide-leak BAD"));
  s.port2.postMessage(null, [s.port1, ab]);
  // transfer-self 文案 + untransferable（self/untransferable 套件）
  try { s.port1.postMessage(null, [s.port1]); console.log("mp-self NO-THROW"); }
  catch (e) { console.log("mp-self", e.constructor.name === "DOMException" && e.name === "DataCloneError" && e.code === 25 && e.message === "Transfer list contains source port"); }
  const u1 = new MessageChannel(); const u2 = new MessageChannel();
  markAsUntransferable(u2.port1);
  try { u1.port1.postMessage(u2.port1, [u2.port1]); console.log("mp-unxfer NO-THROW"); }
  catch (e) { console.log("mp-unxfer", e.constructor.name === "DOMException" && e.name === "DataCloneError" && e.code === 25); }
  // ports 透传（ports 块）
  const p1 = new MessageChannel(); const p2 = new MessageChannel();
  p1.port1.postMessage({ port: p2.port2 }, [p2.port2]);
  p1.port2.addEventListener("message", (ev) => {
    console.log("mp-ports", ev.ports.length === 1, ev.ports[0] instanceof MessagePort, ev.target === p1.port2);
    // 对端关闭透出（channel 块 2）
    const d = new MessageChannel();
    let dc = 0;
    d.port1.on("close", () => dc++);
    d.port2.on("close", () => dc++);
    d.port2.close();
    setTimeout(() => {
      console.log("mp-peerclose", dc === 2, ab.byteLength === 0, warned);
      console.log("mp-inspect", util.inspect(s.port1).includes("active: false"), util.inspect(p1.port1).includes("active: true"));
      console.log("mp-hasref", (() => { const h = new MessageChannel(); const f0 = h.port1.hasRef(); h.port1.on("message", () => {}); const f1 = h.port1.hasRef(); h.port1.close(); h.port2.close(); return f0 === false && f1 === true; })());
      for (const q of [c.port1,c.port2,s.port1,s.port2,u1.port1,u1.port2,u2.port1,u2.port2,p1.port1,p1.port2,p2.port1,p2.port2,d.port1,d.port2]) { try { q.close(); } catch {} }
      // process worker 事件（worker-event 套件）
      const w = new Worker("1+1", { eval: true });
      w.on("online", () => { console.log("mp-worker-ev", w.threadId > 0); w.terminate(); });
    }, 100);
  });
}, 50);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in [
        "mp-proto true",
        "mp-call true",
        "mp-new true",
        "mp-newx true",
        "mc-call true",
        "mp-close-order true true",
        "mp-bare0 true",
        "mp-optnull true",
        "mp-self true",
        "mp-unxfer true",
        "mp-ports true true true",
        "mp-peerclose true true true",
        "mp-inspect true true",
        "mp-hasref true",
        "mp-worker-ev true",
    ] {
        assert!(stdout.lines().any(|l| l == line), "missing line: {line}\nout: {stdout}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_thread_msg_routes() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(&dir, "p.mjs", r#"
import { Worker, postMessageToThread, threadId } from "node:worker_threads";
import { once } from "node:events";
const w = new Worker("const wt=require('node:worker_threads');wt.parentPort.on('message',()=>{});wt.parentPort.postMessage('ready');process.on('workerMessage',(v,s)=>{wt.parentPort.postMessage(['echo',v,s]);wt.postMessageToThread(0,'hi-main');});", { eval: true });
await once(w, "message");
process.on("workerMessage", (v, s) => console.log("tmRouteBack", v === "hi-main" && s === w.threadId));
w.on("message", (m) => { if (Array.isArray(m) && m[0] === "echo") console.log("tmRouteOk", m[1].n === 3 && m[2] === threadId); });
await postMessageToThread(w.threadId, { n: 3 }).then((v) => console.log("tmRouteRes", v === undefined));
await new Promise((r) => setTimeout(r, 500)); await w.terminate();
"#,
    );
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(["tmRouteBack true", "tmRouteRes true", "tmRouteOk true"].iter().all(|l| s.lines().any(|x| x == *l)), "out: {s}");
    dir.close().unwrap();
}

#[test]
fn worker_thread_msg_errors() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(&dir, "p.mjs", r#"
import { Worker, postMessageToThread, threadId } from "node:worker_threads";
import { once } from "node:events";
const t = async (tag, f, code, msg) => { try { await f(); console.log(tag, "NO-THROW"); } catch (e) { console.log(tag, e.code === code && e.message === msg); } };
await t("tmErrSelf", () => postMessageToThread(threadId, 1), "ERR_WORKER_MESSAGING_SAME_THREAD", "Cannot sent a message to the same thread");
await t("tmErrUnknown", () => postMessageToThread(2 ** 40, 1), "ERR_WORKER_MESSAGING_FAILED", "Cannot find the destination thread or listener");
await t("tmErrTimeType", () => postMessageToThread(999, 1, [], "x"), "ERR_INVALID_ARG_TYPE", "The \"timeout\" argument must be of type number. Received type string ('x')");
await t("tmErrTimeRange", () => postMessageToThread(999, 1, [], -1), "ERR_OUT_OF_RANGE", "The value of \"timeout\" is out of range. It must be >= 0. Received -1");
const w = new Worker("const wt=require('node:worker_threads');wt.parentPort.on('message',()=>{});wt.parentPort.postMessage('ready');", { eval: true });
await once(w, "message");
await t("tmErrNoListen", () => postMessageToThread(w.threadId, 1), "ERR_WORKER_MESSAGING_FAILED", "Cannot find the destination thread or listener");
const w2 = new Worker("process.on('workerMessage',()=>{throw new Error('KABOOM');});const wt=require('node:worker_threads');wt.parentPort.on('message',()=>{});wt.parentPort.postMessage('ready');", { eval: true });
await once(w2, "message"); await t("tmErrThrow", () => postMessageToThread(w2.threadId, 1), "ERR_WORKER_MESSAGING_ERRORED", "The destination thread threw an error while processing the message");
await w.terminate(); await w2.terminate();
"#,
    );
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(["tmErrSelf true", "tmErrUnknown true", "tmErrTimeType true", "tmErrTimeRange true", "tmErrNoListen true", "tmErrThrow true"].iter().all(|l| s.lines().any(|x| x == *l)), "out: {s}");
    dir.close().unwrap();
}

#[test]
fn worker_thread_msg_transfer_edge() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(&dir, "p.mjs", r#"
import { Worker, postMessageToThread, MessageChannel } from "node:worker_threads";
import { once } from "node:events";
const w = new Worker("const wt=require('node:worker_threads');wt.parentPort.on('message',()=>{});wt.parentPort.postMessage('ready');process.on('workerMessage',(v)=>{ const p=v&&v.port; if(p) p.on('message',()=>{p.postMessage('pong');}); });", { eval: true });
await once(w, "message");
const { port1, port2 } = new MessageChannel();
await postMessageToThread(w.threadId, { port: port2 }, [port2]);
port1.on("message", (m) => { console.log("tmEdgePort", m === "pong"); port1.close(); });
port1.postMessage("ping");
const tok = async (tag, f) => { try { console.log(tag, (await f()) === undefined); } catch { console.log(tag, false); } };
const terr = async (tag, f, code) => { try { await f(); console.log(tag, "NO-THROW"); } catch (e) { console.log(tag, e.code === code || e.name === code); } };
await tok("tmEdgeShort", () => postMessageToThread(w.threadId, 1, 2000)); const wb = new Worker("while(true){}", { eval: true }); await terr("tmEdgeZero", () => postMessageToThread(wb.threadId, 1, 0), "ERR_WORKER_MESSAGING_TIMEOUT"); await wb.terminate(); await terr("tmEdgeClone", () => postMessageToThread(w.threadId, () => {}, 0), "DataCloneError");
await new Promise((r) => setTimeout(r, 300)); await w.terminate();
"#,
    );
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(["tmEdgePort true", "tmEdgeShort true", "tmEdgeZero true", "tmEdgeClone true"].iter().all(|l| s.lines().any(|x| x == *l)), "out: {s}");
    dir.close().unwrap();
}
