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

#[test]
fn worker_exit_event_faces() {
    // 正常：exit 监听抛错走 uncaughtException + 进程存活（node
    // test-worker-exit-event-error）；message/online 监听抛错同口径；
    // 边界：error 监听正常收货不受安全垫影响。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Worker } from "node:worker_threads";
const seen = [];
process.on("uncaughtException", (e) => seen.push(e.message));
new Worker("", { eval: true }).on("exit", () => { throw new Error("foo"); });
const w2 = new Worker("require('worker_threads').parentPort.postMessage(1);", { eval: true });
w2.on("message", () => { throw new Error("msg-boom"); });
w2.on("exit", () => {});
const w3 = new Worker("1", { eval: true });
w3.on("online", () => { throw new Error("online-boom"); });
w3.on("exit", () => {});
const w4 = new Worker("throw new Error('werr');", { eval: true });
w4.on("error", (e) => seen.push("err:" + e.message));
w4.on("exit", () => {});
setTimeout(() => console.log("exit-ev", JSON.stringify(seen.sort())), 800);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.lines().any(|l| l == r#"exit-ev ["err:werr","foo","msg-boom","online-boom"]"#),
        "out: {stdout}"
    );
    dir.close().unwrap();
}

#[test]
fn worker_stdio_faces() {
    // 正常：缺省共享（子输出进管道 + 父终端双可见）；stdout:true 管道独占
    //（父终端不可见，管道必达）；--require 预加载输出先行（'A\nB\n'）。
    // 报错：--require/-r/--import 缺值三形拒（文案 "<flag> requires an argument"）；
    // 边界：-r 禁 =形；分离空串消费后运行时炸（校验放行）。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("printA.js").write_str("console.log(\"A\");").unwrap();
    let preload = dir.child("printA.js").path().to_string_lossy().into_owned();
    let out = run_node_file(
        &dir,
        "p.mjs",
        &format!(
            r#"
import {{ Worker }} from "node:worker_threads";
// 共享：缺省子输出父终端可见。
const s = new Worker("console.log('SHARED-KID');", {{ eval: true }});
s.on("exit", () => {{}});
// 独占：stdout:true 父终端不可见、管道必达。
const x = new Worker("console.log('EXCL-KID');", {{ eval: true, stdout: true }});
let xseen = "";
x.stdout.setEncoding("utf8");
x.stdout.on("data", (c) => {{ xseen += c; }});
x.on("exit", () => setTimeout(() => console.log("excl-pipe", xseen.includes("EXCL-KID")), 50));
// 预加载：--require 输出先行。
const p = new Worker('console.log("B");', {{ execArgv: ["--require", {preload:?}], eval: true, stdout: true }});
p.on("exit", () => setTimeout(() => console.log("preload", JSON.stringify(p.stdout.read(p.stdout.readableLength)?.toString())), 50));
// 带值旗矩阵。
const t = (tag, argv, code, msg) => {{ try {{ const w = new Worker("1", {{ eval: true, execArgv: argv }}); w.on("exit", () => {{}}); w.on("error", () => {{}}); console.log(tag, "ALLOWED"); }} catch (e) {{ console.log(tag, e.code === code && (msg === undefined || e.message.includes(msg))); }} }};
t("req-bare", ["--require"], "ERR_WORKER_INVALID_EXEC_ARGV", "--require requires an argument");
t("req-empty", ["--require="], "ERR_WORKER_INVALID_EXEC_ARGV", "--require= requires an argument");
t("req-dash", ["--require", "--foo"], "ERR_WORKER_INVALID_EXEC_ARGV", "--require requires an argument");
t("r-bare", ["-r"], "ERR_WORKER_INVALID_EXEC_ARGV", "-r requires an argument");
t("r-eq", ["-r=x"], "ERR_WORKER_INVALID_EXEC_ARGV");
t("imp-bare", ["--import"], "ERR_WORKER_INVALID_EXEC_ARGV", "--import requires an argument");
t("req-sep", ["--require", {preload:?}], undefined);
t("req-eq", [`--require={preload}`], undefined);
"#
        ),
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in [
        "excl-pipe true",
        "preload \"A\\nB\\n\"",
        "req-bare true",
        "req-empty true",
        "req-dash true",
        "r-bare true",
        "r-eq true",
        "imp-bare true",
        "req-sep ALLOWED",
        "req-eq ALLOWED",
    ] {
        assert!(stdout.lines().any(|l| l == line), "missing line: {line}\nout: {stdout}");
    }
    // 共享可见 + 独占不可见（父终端捕获断言）。
    assert!(stdout.lines().any(|l| l.contains("SHARED-KID")), "shared missing:\n{stdout}");
    assert!(!stdout.lines().any(|l| l == "EXCL-KID"), "exclusive leaked:\n{stdout}");
    dir.close().unwrap();
}

#[test]
fn worker_exit_code_faces() {
    // 正常：exitCode/exit 监听/改码矩阵 12 格（node test-worker-exit-code，
    // 逐格断码；error 事件照发——werr 格点名）。
    // 报错：uncaught 文案透传（error 事件断 message）。
    // 边界：监听内再抛 worker 恒 1（主线程 7）；_fatalException 置空即 6。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Worker, parentPort } from "node:worker_threads";
const caseSrc = [
  [`process.exitCode = 42; process.on('exit', (c) => { if (process.exitCode !== 42 || c !== 42) { console.log('bad0'); process.exit(99); } });`, 42, null],
  [`process.exitCode = 99; process.on('exit', (c) => { if (process.exitCode !== 42 || c !== 42) { console.log('bad1'); process.exit(99); } }); process.exit(42);`, 42, null],
  [`process.exitCode = 99; process.on('exit', (c) => { if (c !== 1 || process.exitCode !== 1) { console.log('bad2'); process.exit(99); } }); throw new Error('ok');`, 1, "Error: ok"],
  [`process.exitCode = 95; process.on('exit', (c) => { if (process.exitCode !== 95 || c !== 95) { console.log('bad3'); process.exit(99); } process.exitCode = 99; });`, 99, null],
  [`const noop = () => {}; process.on('exit', (c) => { process.off('uncaughtException', noop); if (process.exitCode !== undefined || c !== 0) { console.log('bad4'); process.exit(99); } }); process.on('uncaughtException', noop); throw new Error('ok');`, 0, null],
  [`const f = () => { process.exitCode = 97; }; process.on('exit', (c) => { process.off('uncaughtException', f); if (process.exitCode !== 97 || c !== 97) { console.log('bad5'); process.exit(99); } }); process.on('uncaughtException', f); throw new Error('ok');`, 97, null],
  [`process.on('exit', (c) => { if (process.exitCode !== 1 || c !== 1) { console.log('bad6'); process.exit(99); } process.exitCode = 98; }); throw new Error('ok');`, 98, "Error: ok"],
  [`process.on('exit', (c) => { if (process.exitCode !== 1 || c !== 1) { console.log('bad7'); process.exit(99); } process.exitCode = 0; }); throw new Error('ok');`, 0, "Error: ok"],
  [`process.on('uncaughtException', () => { throw new Error('ok'); }); throw new Error('bad');`, 1, "Error: ok"],
  [`process.exitCode = 99; process.on('exit', (c) => { if (process.exitCode !== 0 || c !== 0) { console.log('bad9'); process.exit(99); } }); process.exit(0);`, 0, null],
  [`process._fatalException = undefined; throw new Error('ok');`, 6, null],
];
let pend = caseSrc.length;
caseSrc.forEach(([src, want, errWant], i) => {
  const w = new Worker(`const { parentPort } = require('worker_threads'); parentPort.once('message', () => { ${src} });`, { eval: true });
  if (errWant !== null) w.on("error", (e) => console.log("ew" + i, String(e) === errWant));
  else w.on("error", (e) => console.log("e-unexpected" + i, e.message));
  w.on("exit", (code) => { console.log("ex" + i, code === want); if (--pend === 0) console.log("done"); });
  w.postMessage(i);
});
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for i in 0..11 {
        assert!(
            stdout.lines().any(|l| l == format!("ex{i} true")),
            "missing ex{i}:\n{stdout}"
        );
    }
    for i in [2, 6, 7, 8] {
        assert!(
            stdout.lines().any(|l| l == format!("ew{i} true")),
            "missing ew{i}:\n{stdout}"
        );
    }
    assert!(stdout.lines().any(|l| l == "done"), "out: {stdout}");
    assert!(!stdout.contains("e-unexpected"), "out: {stdout}");
    dir.close().unwrap();
}
