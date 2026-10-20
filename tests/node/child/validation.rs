//! tests/node/child/validation.rs — spawn/fork 选项校验（对齐 src/builtins/node/child_spawn.js）。

use crate::common::*;
use assert_fs::prelude::*;

#[test]
fn child_option_validation_faces() {
    // 正常：合法 serialization/timeout/stdio 不抛同步错（失败走异步 error 事件）；
    // 报错：serialization 非法值 ARG_VALUE 逐字文案、fork timeout 非 number
    // ARG_TYPE 越界 RANGE、fork stdio '33' ARG_VALUE、无 ipc 数组 IPC_REQUIRED、
    // 双 ipc ONE_PIPE；边界：timeout 上限 2^53-1 过、+1 RANGE。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("p.mjs");
    file.write_str(
        r#"
import { spawn, fork } from "node:child_process";
const show = (e) => console.log("THROW", e.code, JSON.stringify(e.message));
// serialization 非法四值（spawn 与 fork 同口径）。
for (const v of [null, 42, Infinity, "foo"]) {
  try { spawn(process.execPath, [], { serialization: v }); console.log("spawn-ser-NO-THROW", JSON.stringify(String(v))); }
  catch (e) { show(e); }
  try { fork("x.js", { serialization: v }); console.log("fork-ser-NO-THROW", JSON.stringify(String(v))); }
  catch (e) { show(e); }
}
// fork timeout 校验。
for (const v of ["badValue", {}]) {
  try { fork("x.js", { timeout: v }); console.log("fork-timeout-NO-THROW"); }
  catch (e) { show(e); }
}
for (const v of [1.5, -1, 9007199254740992]) {
  try { fork("x.js", { timeout: v }); console.log("fork-timeout-NO-THROW", String(v)); }
  catch (e) { show(e); }
}
// fork stdio 校验。
for (const v of ["33", "ipc"]) {
  try { fork("x.js", { stdio: v }); console.log("fork-stdio-NO-THROW", v); }
  catch (e) { show(e); }
}
try { fork("x.js", { stdio: ["pipe"] }); console.log("fork-stdio-arr-NO-THROW"); }
catch (e) { show(e); }
try { fork("x.js", { stdio: ["ipc", "ipc"] }); console.log("fork-stdio-2ipc-NO-THROW"); }
catch (e) { show(e); }
// 边界：合法值同步不抛（走异步 error 事件吞掉；errored fork 须 unref，
// 否则坏 worker 驻留事件循环——预存行为，非本轮回归）。
for (const o of [{ serialization: "json" }, { serialization: "advanced" }, { timeout: 0 }, { stdio: "pipe" }, { stdio: ["inherit", "inherit", "inherit", "ipc"] }]) {
  try { const c = fork("no-such-file-xyz.js", o); c.on("error", () => {}); c.unref(); console.log("FORK-OK", JSON.stringify(o)); }
  catch (e) { console.log("FORK-SYNC-THROW", JSON.stringify(o), e.code); }
}
try { const c = spawn("no-such-binary-xyz", [], { serialization: "advanced" }); c.on("error", () => {}); c.unref(); console.log("SPAWN-OK"); }
catch (e) { console.log("SPAWN-SYNC-THROW", e.code); }
"#,
    )
    .unwrap();
    let out = winterjs2()
        .arg("--run")
        .arg(file.path())
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    for v in ["null", "42", "Infinity", "'foo'"] {
        assert!(
            stdout.contains(&format!(
                "THROW ERR_INVALID_ARG_VALUE \"The property 'options.serialization' must be one of: undefined, 'json', 'advanced'. Received {v}\""
            )),
            "serialization {v} 文案缺失:\n{stdout}"
        );
    }
    assert!(
        stdout.contains("THROW ERR_INVALID_ARG_TYPE \"The \\\"timeout\\\" argument must be of type number."),
        "timeout 类型文案缺失:\n{stdout}"
    );
    assert!(
        stdout.contains("must be an integer. Received 1.5"),
        "timeout 整数文案缺失:\n{stdout}"
    );
    assert!(
        stdout.contains("It must be >= 0 && <= 9007199254740991. Received -1"),
        "timeout 范围文案缺失:\n{stdout}"
    );
    assert!(
        stdout.contains("THROW ERR_INVALID_ARG_VALUE \"The argument 'stdio' is invalid. Received '33'\""),
        "stdio 文案缺失:\n{stdout}"
    );
    assert!(
        stdout.contains("ERR_CHILD_PROCESS_IPC_REQUIRED"),
        "ipc-required 缺失:\n{stdout}"
    );
    assert!(
        stdout.contains("Child process can have only one IPC pipe"),
        "one-pipe 缺失:\n{stdout}"
    );
    assert!(!stdout.contains("NO-THROW"), "有应抛未抛:\n{stdout}");
    assert!(!stdout.contains("SYNC-THROW"), "合法值误抛:\n{stdout}");
    assert!(stdout.contains("FORK-OK") && stdout.contains("SPAWN-OK"), "合法面缺失:\n{stdout}");
}

#[test]
fn child_fork_timeout_kill_faces() {
    // 正常：fork timeout 到点按 killSignal 杀（exit(null, SIGTERM)/SIGKILL）；
    // 报错：覆盖 badValue 校验（validation faces 已逐字，此处不断言文案只断言码）；
    // 边界：自然先退则 timeout 不补发（exit 0 单发）。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("stay.mjs")
        .write_str("setInterval(() => {}, 1000);\n")
        .unwrap();
    dir.child("quick.mjs").write_str("process.exit(0);\n").unwrap();
    let stay_abs = dir.path().join("stay.mjs");
    let quick_abs = dir.path().join("quick.mjs");
    let file = dir.child("p.mjs");
    file.write_str(&format!(
        r#"
import {{ fork }} from "node:child_process";
const c1 = fork({stay_abs:?}, {{ timeout: 120 }});
c1.on("exit", (code, sig) => console.log("T-DEF", code, sig));
const c2 = fork({stay_abs:?}, {{ timeout: 120, killSignal: "SIGKILL" }});
c2.on("exit", (code, sig) => console.log("T-KILL", code, sig));
const c3 = fork({quick_abs:?}, {{ timeout: 5000 }});
c3.on("exit", (code, sig) => console.log("T-NATURAL", code, sig));
"#,
    ))
    .unwrap();
    let out = winterjs2()
        .arg("--run")
        .arg(file.path())
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("T-DEF null SIGTERM"), "默认 kill 缺失:\n{stdout}");
    assert!(stdout.contains("T-KILL null SIGKILL"), "SIGKILL 缺失:\n{stdout}");
    assert!(stdout.contains("T-NATURAL 0 null"), "自然退出缺失/被补发:\n{stdout}");
}
