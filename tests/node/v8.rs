//! tests/node/v8.rs — 对齐 src/builtins/node/v8.rs（node:v8（兼 readline，单测））。

use crate::common::*;
use assert_fs::prelude::*;

#[test]
fn v8_readline_surface() {
    // v8：startupSnapshot 守卫（vite try 内调用）；readline：10c-2 起全面实现，
    // 旧宽松口径（question 抛未实现/非法入参带码）已退役，此处按新语义断言。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("vr.mjs");
    file.write_str(
        r#"
import v8, { startupSnapshot } from "node:v8";
console.log("v8snap", startupSnapshot.isBuildingSnapshot() === false, v8.startupSnapshot === startupSnapshot);
import rl, { createInterface, cursorTo, clearScreenDown, emitKeypressEvents } from "node:readline";
import { PassThrough } from "node:stream";
try { createInterface({ input: null, output: null }); } catch (e) { console.log("rl-null", e.constructor.name, e.code); }
const itf = createInterface({ input: new PassThrough(), output: new PassThrough() });
let closed = false;
itf.on("close", () => { closed = true; });
itf.setPrompt("> ");
console.log("rl-open", itf.getPrompt() === "> " && itf.closed === false);
itf.close();
console.log("rl-close", closed && itf.closed);
console.log("rl-cursor", cursorTo(null, 0, 0) === false && clearScreenDown(null) === false);
try { emitKeypressEvents(null); } catch (e) { console.log("rl-emitnull", e.code); }
itf.question("q?", () => {});
console.log("rl-q", "no-throw");
try { createInterface(42); } catch (e) { console.log("rl-bad", e.constructor.name, e.code); }
console.log("rl-def", typeof rl.createInterface === "function");
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
        "v8snap true true",
        "rl-null TypeError undefined",
        "rl-open true",
        "rl-close true",
        "rl-cursor true",
        "rl-emitnull ERR_INVALID_ARG_TYPE",
        "rl-q no-throw",
        "rl-bad TypeError undefined",
        "rl-def true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn v8_serialize_faces() {
    // serialize/deserialize 自洽往返（非 V8 线格式，跨引擎不互通；自家
    // 往返逐类断言）。正常：标量/串/Buffer/视图/ArrayBuffer/数组/对象；
    // 报错：函数不可序列化、非法入参；边界：空 Buffer、大整数、嵌套。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("vs.mjs");
    file.write_str(
        r#"
import v8 from "node:v8";
import assert from "node:assert";
const rt = (v, cmp) => {
  const d = v8.deserialize(v8.serialize(v));
  assert.deepStrictEqual(d, cmp ?? v);
  return d;
};
// 正常
rt(undefined); rt(null); rt(true); rt(3.14); rt("hi"); rt(10n);
assert.ok(Buffer.isBuffer(v8.serialize("x")));
rt(Buffer.alloc(0));
const ta = rt({ a: new Int32Array([1, -2]), b: new Uint16Array([3]) });
assert.ok(ta.a instanceof Int32Array && ta.b instanceof Uint16Array);
const ab = rt(new Uint8Array([9]).buffer);
assert.ok(ab instanceof ArrayBuffer && new Uint8Array(ab)[0] === 9);
rt([1, "a", null, [true]]);
rt({ n: { deep: [1n, "s"] } });
const dv = rt({ v: new DataView(new Uint8Array([7]).buffer) });
assert.ok(dv.v instanceof DataView && dv.v.getUint8(0) === 7);
console.log("rt-ok");
// 报错
assert.throws(() => v8.serialize(() => {}), TypeError);
assert.throws(() => v8.deserialize(42), TypeError);
assert.throws(() => v8.deserialize(v8.serialize(Symbol())), TypeError);
console.log("err-ok");
// 边界
rt("");
const big = rt("x".repeat(100000));
assert.strictEqual(big.length, 100000);
console.log("edge-ok");
"#,
    )
    .unwrap();
    let out = winterjs2().arg("--run").arg(file.path()).current_dir(dir.path()).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let out = String::from_utf8(out.stdout).unwrap();
    for line in ["rt-ok", "err-ok", "edge-ok"] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}
