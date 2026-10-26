//! tests/node/child/medium.rs — internal 别名/disconnect/detached/unref/silent（对齐 child M 轮）。

use crate::common::*;
use assert_fs::prelude::*;

#[test]
fn child_internal_faces() {
    // 正常：internal/child_process 可解（getValidStdio 为函数，
    // ChildProcess 与公开面同一对象，spawnSync 可重赋值）；
    // 报错：getValidStdio 非法输入 ARG_VALUE；
    // 边界：mock 恢复后原实现可用（mock 面见 node 套件）。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("p.mjs");
    file.write_str(
        r#"
import internalCp, { getValidStdio } from "node:internal/child_process";
import { ChildProcess, spawnSync } from "node:child_process";
console.log("RESOLVE", typeof getValidStdio === "function");
console.log("SAME-CLASS", internalCp.ChildProcess === ChildProcess);
console.log("MOCKABLE", (() => { const o = internalCp.spawnSync; internalCp.spawnSync = o; return internalCp.spawnSync === o; })());
try { getValidStdio("foo"); console.log("NO-THROW"); }
catch (e) { console.log("THROW", e.code); }
const r = getValidStdio([], false);
console.log("SHAPE", Array.isArray(r.stdio) && r.stdio.length === 3 && "ipc" in r && "ipcFd" in r);
console.log("SPAWN-INTERNAL", typeof spawnSync === "function");
"#,
    )
    .unwrap();
    let out = winterjs2()
        .arg("--expose-internals")
        .arg("--run")
        .arg(file.path())
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    for line in [
        "RESOLVE true",
        "SAME-CLASS true",
        "MOCKABLE true",
        "THROW ERR_INVALID_ARG_VALUE",
        "SHAPE true",
        "SPAWN-INTERNAL true",
    ] {
        assert!(stdout.contains(line), "{line} 缺失:\n{stdout}");
    }
}

#[test]
fn child_disconnect_faces() {
    // 正常：子 disconnect → 父 disconnect 事件（单次）；
    // 报错：关通道 send 异步报 ERR_IPC_CHANNEL_CLOSED（双真机 26.8.2/26.12
    // 同形；重 disconnect 有监听走事件、无监听抛——与真机一致）；
    // 边界：disconnect 后 send 回 false。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("echo-child.mjs")
        .write_str("process.on('message', (m) => { process.send({ echo: m }); });\n")
        .unwrap();
    let child_abs = dir.path().join("echo-child.mjs");
    let child_str = child_abs.to_string_lossy().into_owned();
    let file = dir.child("p.mjs");
    file.write_str(&format!(
        r#"
import {{ fork }} from "node:child_process";
const c = fork({child_str:?});
let disc = 0;
let errCode = "";
c.on("disconnect", () => {{ disc++; }});
c.on("error", (e) => {{ errCode = e.code; }});
c.on("message", () => {{
  c.disconnect();
  console.log("SEND-AFTER", c.send({{ late: 1 }}));
  try {{ c.disconnect(); console.log("REDISC-NO-THROW"); }}
  catch (e) {{ console.log("REDISC-THROW", e.code); }}
}});
c.send({{ hello: 1 }});
setTimeout(() => console.log("DISC-COUNT", disc, "ERR", errCode), 800);
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
    assert!(stdout.contains("SEND-AFTER false"), "send 未回 false:\n{stdout}");
    // 重 disconnect：有 error 监听走事件（真机同）， err 码断言见下。
    assert!(stdout.contains("REDISC-NO-THROW"), "重 disconnect 抛了:\n{stdout}");
    assert!(stdout.contains("DISC-COUNT 1"), "disconnect 非单次:\n{stdout}");
    assert!(stdout.contains("ERR ERR_IPC_CHANNEL_CLOSED"), "关通道 error 未到:\n{stdout}");
}

#[test]
fn child_detached_unref_faces() {
    // 正常：spawn detached + unref 后父即退（孙进程存活不断言，只断父不挂）；
    // 报错：无（unref 恒返回 this 链式）；
    // 边界：ref/unref 成对幂等（重复调不崩，退出码照旧）。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("p.mjs");
    file.write_str(
        r#"
import { spawn } from "node:child_process";
const c = spawn(process.execPath, ["--eval", "setInterval(() => {}, 1000)"], { detached: true, stdio: "ignore" });
console.log("UNREF-CHAIN", c.unref() === c);
console.log("REF-CHAIN", c.ref() === c);
c.unref();
c.unref();
console.log("PARENT-EXITING");
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
    assert!(stdout.contains("UNREF-CHAIN true"), "unref 链断:\n{stdout}");
    assert!(stdout.contains("REF-CHAIN true"), "ref 链断:\n{stdout}");
    assert!(stdout.contains("PARENT-EXITING"), "父未走完:\n{stdout}");
}

#[test]
fn child_silent_pipe_faces() {
    // 正常：silent fork 子输出进父端管道（data 可收）；
    // 报错：无；
    // 边界：pipe(end:false) 不关目标；非 silent 仍恒 null。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("echo-child.mjs")
        .write_str("process.stdout.write('s1');\nprocess.stderr.write('s2');\n")
        .unwrap();
    let child_abs = dir.path().join("echo-child.mjs");
    let child_str = child_abs.to_string_lossy().into_owned();
    let file = dir.child("p.mjs");
    file.write_str(&format!(
        r#"
import {{ fork }} from "node:child_process";
const c = fork({child_str:?}, [], {{ silent: true }});
let out = "", err = "";
c.stdout.on("data", (d) => {{ out += d; }});
c.stderr.on("data", (d) => {{ err += d; }});
c.on("exit", () => setTimeout(() => {{
  console.log("GOT", JSON.stringify(out), JSON.stringify(err));
}}, 400));
const c2 = fork({child_str:?}, []);
console.log("NONSILENT", c2.stdout === null, c2.stderr === null);
c2.on("exit", () => {{}});
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
    assert!(stdout.contains("GOT \"s1\" \"s2\""), "管道未收到:\n{stdout}");
    assert!(stdout.contains("NONSILENT true true"), "非 silent 非 null:\n{stdout}");
}
