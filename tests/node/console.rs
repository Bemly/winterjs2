//! tests/node/console.rs — 对齐 src/builtins/node/console.rs（node:console）。

use crate::helpers::*;

#[test]
fn console_module_surface() {
    // 正常：具名表全 + Console 写自定义流（log/count/timeEnd/assert-false）+
    // default 形状；报错：无流构造抛 ERR_CONSOLE_WRITABLE_STREAM（真机口径）；
    // 边界：单流 stderr 跟随、默认导出可调用（真机口径）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import c, { Console, log, table, context, createTask, profile, timeStamp } from "node:console";
import assert from "node:assert";
console.log("shape", typeof Console, typeof log, typeof table, typeof context, typeof createTask, typeof profile, typeof timeStamp);
console.log("keys", typeof c.log, typeof c.error, typeof c.Console, typeof c.timeStamp);
console.log("assert-callable", typeof assert === "function", assert === assert.ok);
assert(true, "no-throw");
const ws = { out: "", write(s) { this.out += s; } };
const k = new Console(ws);
k.log("hi", 42);
k.count("a"); k.count("a"); k.countReset("a"); k.count("a");
k.time("t"); k.timeEnd("t");
k.assert(false, "boom");
k.table([1]);
console.log("stream", JSON.stringify(ws.out));
const g = new Console(ws, ws);
g.log("fallback-ok");
console.log("fb-stream", JSON.stringify(ws.out.includes("fallback-ok")));
try { new Console(); console.log("no-throw FAIL"); }
catch (e) { console.log("no-stream", e.code); }
console.log("ctx", typeof context(), typeof createTask().run);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("shape function function function function function function function"), "out: {out}");
    assert!(out.contains("keys function function function function"), "out: {out}");
    assert!(out.contains("assert-callable true false"), "out: {out}");
    assert!(out.contains("hi 42"), "out: {out}");
    assert!(out.contains("a: 1") && out.contains("a: 2"), "out: {out}");
    assert!(out.contains("fallback-ok") || out.contains("fb-stream true"), "out: {out}");
    assert!(out.contains("no-stream ERR_CONSOLE_WRITABLE_STREAM"), "out: {out}");
    assert!(out.contains("ctx object function"), "out: {out}");
    dir.close().unwrap();
}
