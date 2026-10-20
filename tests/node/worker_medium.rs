//! tests/node/worker_medium.rs — Worker P0-medium 面（stdio×4/message-port-close/
//! message-before-close/messaging-timeout/cpu-usage/exit-code 矩阵/
//! exit-event-error/cleanup-handles；node 定语义，黑盒三件）。

use crate::helpers::*;
use assert_fs::prelude::*;

#[test]
fn worker_message_before_close() {
    // 正常：transfer 端口 worker 侧连发两条 + close，200 轮双 once 逐条必达
    //（node test-worker-message-port-message-before-close，10000 轮此处 200 轮
    // 同构；pump 整批排空 + 同步连发曾丢第二条，见 4.326）。
    // 边界：同批三条 on 常驻全收；接收侧 close 后已排队照发（close-while-receiving）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Worker, MessageChannel } from "node:worker_threads";
import { once } from "node:events";
const worker = new Worker("require('worker_threads').parentPort.on('message', ({ port }) => { port.postMessage('firstMessage'); port.postMessage('lastMessage'); port.close(); });", { eval: true });
let ok = 0;
for (let i = 0; i < 200; i++) {
  const { port1, port2 } = new MessageChannel();
  worker.postMessage({ port: port2 }, [port2]);
  const a = await once(port1, "message");
  const b = await once(port1, "message");
  if (a[0] === "firstMessage" && b[0] === "lastMessage") ok++;
  port1.close(); port2.close();
}
console.log("mbc-loop", ok === 200);
// 同批三条 + on 常驻：同步连发不丢不错序。
const c = new MessageChannel();
const got = [];
c.port1.on("message", (m) => got.push(m));
c.port2.postMessage("m1"); c.port2.postMessage("m2"); c.port2.postMessage("m3");
setTimeout(() => {
  console.log("mbc-batch", JSON.stringify(got) === JSON.stringify(["m1", "m2", "m3"]));
  // 接收侧 close 后已排队照发（close-while-receiving 真机口径）。
  const d = new MessageChannel();
  let n = 0;
  d.port1.on("message", () => { n++; d.port1.close(); });
  d.port2.postMessage("foo"); d.port2.postMessage("bar");
  setTimeout(async () => {
    console.log("mbc-cwr", n === 2);
    for (const q of [c.port1, c.port2, d.port1, d.port2]) try { q.close(); } catch {}
    await worker.terminate();
  }, 100);
}, 100);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in ["mbc-loop true", "mbc-batch true", "mbc-cwr true"] {
        assert!(stdout.lines().any(|l| l == line), "missing line: {line}\nout: {stdout}");
    }
    dir.close().unwrap();
}

#[test]
fn worker_cpu_usage_faces() {
    // 正常：prev/curr 单调非减 + delta 差值（node test-worker-cpu-usage）；
    // 报错：非对象/坏字段抛 INVALID_ARG_TYPE/OUT_OF_RANGE（文案逐字对真机）；
    // 边界：NaN/undefined/null 视作无参；退出后采样拒 NOT_RUNNING。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Worker } from "node:worker_threads";
const worker = new Worker("const { parentPort } = require('worker_threads'); parentPort.on('message', () => {});", { eval: true });
const validate = (r) => typeof r === "object" && r !== null && r.user >= 0 && r.system >= 0 && Number.isFinite(r.user) && Number.isFinite(r.system);
for (const v of [NaN, undefined, null]) worker.cpuUsage(v);
console.log("cpu-nothrow", true);
for (const [v, code] of [[-1, "ERR_INVALID_ARG_TYPE"], [1.1, "ERR_INVALID_ARG_TYPE"], [{}, "ERR_INVALID_ARG_TYPE"], [[], "ERR_INVALID_ARG_TYPE"], [true, "ERR_INVALID_ARG_TYPE"], [Infinity, "ERR_INVALID_ARG_TYPE"], [{ user: -1, system: 1 }, "ERR_OUT_OF_RANGE"], [{ user: 1, system: -1 }, "ERR_OUT_OF_RANGE"]]) {
  try { await worker.cpuUsage(v); console.log("cpu-bad NO-THROW", JSON.stringify(String(v))); }
  catch (e) { console.log("cpu-bad", e.code === code); }
}
try { await worker.cpuUsage({}); console.log("cpu-empty NO-THROW"); }
catch (e) { console.log("cpu-empty", e.code === "ERR_INVALID_ARG_TYPE" && e.message === 'The "prev.user" property must be of type number. Received undefined'); }
try { await worker.cpuUsage({ user: -1, system: 1 }); console.log("cpu-range NO-THROW"); }
catch (e) { console.log("cpu-range", e.code === "ERR_OUT_OF_RANGE" && e.message === 'The value of "prev.user" is out of range. It must be >= 0 && <= 9007199254740991. Received -1'); }
worker.on("online", async () => {
  const prev = await worker.cpuUsage();
  const curr = await worker.cpuUsage();
  console.log("cpu-shape", validate(prev) && validate(curr));
  console.log("cpu-mono", curr.user >= prev.user && curr.system >= prev.system);
  console.log("cpu-delta", validate(await worker.cpuUsage(curr)));
  await worker.terminate();
});
worker.once("exit", async () => {
  try { await worker.cpuUsage(); console.log("cpu-exited NO-THROW"); }
  catch (e) { console.log("cpu-exited", e.code === "ERR_WORKER_NOT_RUNNING" && e.message === "Worker instance not running"); }
});
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in [
        "cpu-nothrow true",
        "cpu-shape true",
        "cpu-mono true",
        "cpu-delta true",
        "cpu-empty true",
        "cpu-range true",
        "cpu-exited true",
    ] {
        assert!(stdout.lines().any(|l| l == line), "missing line: {line}\nout: {stdout}");
    }
    assert_eq!(stdout.lines().filter(|l| *l == "cpu-bad true").count(), 8, "out: {stdout}");
    dir.close().unwrap();
}
