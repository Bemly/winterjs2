//! tests/node/worker_argv.rs — Worker execArgv/argv 校验面（P0-small，对齐
//! node internal/worker.js；execArgv 矩阵按 node 26.8.1 真机探针近似）。

use crate::common::*;
use assert_fs::prelude::*;

#[test]
fn worker_exec_argv_and_argv_faces() {
    // 正常：允许旗放行 + eval/文件 argv 透传（worker 内断言，exit 码验收）；
    // 报错：execArgv 非数组/非法旗/NODE_OPTIONS 非法旗/argv 非数组；
    // 边界：argv falsy 缺省、混合类型串化。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("m.mjs");
    file.write_str(
        r#"
import { Worker } from "node:worker_threads";
for (const [opts, code] of [
  [{ execArgv: "hello" }, "ERR_INVALID_ARG_TYPE"],
  [{ execArgv: 6 }, "ERR_INVALID_ARG_TYPE"],
  [{ execArgv: ["--foo"] }, "ERR_WORKER_INVALID_EXEC_ARGV"],
  [{ execArgv: ["--title=blah"] }, "ERR_WORKER_INVALID_EXEC_ARGV"],
  [{ execArgv: ["--redirect-warnings"] }, "ERR_WORKER_INVALID_EXEC_ARGV"],
  [{ env: { NODE_OPTIONS: "--nonexistent-options" } }, "ERR_WORKER_INVALID_EXEC_ARGV"],
  [{ argv: "foo" }, "ERR_INVALID_ARG_TYPE"],
]) {
  try {
    new Worker("1", { eval: true, ...opts });
    console.log("NO-ERR", JSON.stringify(opts));
  } catch (e) { console.log("bad", e.code === code); }
}
const w1 = new Worker("1", { eval: true, execArgv: ["--no-warnings", "--pending-deprecation"] });
console.log("allow-sync", true);
const w2 = new Worker(
  "const assert = require('assert');" +
  "assert.deepStrictEqual(process.argv.slice(0, 2), [process.execPath, '[worker eval]']);",
  { eval: true },
);
const w3 = new Worker(
  "const assert = require('assert');" +
  "assert.deepStrictEqual(process.argv.slice(2), ['null', 'foo', '123']);",
  { eval: true, argv: [null, "foo", 123] },
);
// exit 监听构造期同步挂载（worker 同步退即发，滞后挂载即漏收，见上）。
const codes = await Promise.all(
  [w1, w2, w3].map((w) => new Promise((res) => w.on("exit", res))),
);
console.log("allow", codes[0] === 0);
console.log("eval-argv", codes[1] === 0);
console.log("user-argv", codes[2] === 0);
console.log("faces-ok");
"#,
    )
    .unwrap();
    let out = winterjs2()
        .arg("--run")
        .arg(file.path())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "bad true",
        "bad true",
        "bad true",
        "bad true",
        "bad true",
        "bad true",
        "bad true",
        "allow-sync true",
        "allow true",
        "eval-argv true",
        "user-argv true",
        "faces-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}
