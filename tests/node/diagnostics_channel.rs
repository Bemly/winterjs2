//! tests/node/diagnostics_channel.rs — 对齐 src/builtins/node/diagnostics_channel.rs（node:diagnostics_channel）。

use crate::common::*;
use assert_fs::prelude::*;

#[test]
fn diagnostics_channel_surface() {
    // test-diagnostics-channel.js 命名子集
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("d.mjs");
    file.write_str(
        r#"import dc, { channel, hasSubscribers, Channel } from "node:diagnostics_channel";
import { AsyncLocalStorage } from "node:async_hooks";
console.log("idle", hasSubscribers("t-ch"), channel("t-ch").hasSubscribers);
const seen = [];
const listener = (msg, name) => seen.push([name, msg]);
dc.subscribe("t-ch", listener);
console.log("active", hasSubscribers("t-ch"), channel("t-ch") instanceof Channel);
channel("t-ch").publish({ n: 1 });
console.log("got", JSON.stringify(seen));
console.log("unsub-wrong", dc.unsubscribe("t-ch", () => {}), dc.unsubscribe("t-ch", listener), hasSubscribers("t-ch"));
// bindStore + runStores
const als = new AsyncLocalStorage();
const ch = channel("s-ch");
ch.bindStore(als, (d) => ({ w: d }));
ch.runStores(7, () => console.log("store", als.getStore()?.w));
console.log("outside", als.getStore());
console.log("unbound", ch.unbindStore(als), ch.hasSubscribers);
// TracingChannel 全窗口
const { tracingChannel } = dc;
const tc = tracingChannel("tr-x");
const ev = [];
tc.subscribe({ start: () => ev.push("s"), end: (c) => ev.push(`e:${c.result}`), error: () => ev.push("err") });
console.log("sync", tc.traceSync(() => "R"), ev.join(","));
tc.tracePromise(async () => "P").then((v) => console.log("promise", v));
// 边界
try { channel(42); } catch (e) { console.log("e1", e.code); }
try { dc.subscribe("t-ch2", "nope"); } catch (e) { console.log("e2", e.message.includes("must be of type function")); }
"#,
    )
    .unwrap();
    let out = winterjs2().args(["--run", file.path().to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("idle false false") && out.contains("active true true"), "out: {out}");
    assert!(out.contains(r#"got [["t-ch",{"n":1}]]"#), "out: {out}");
    assert!(out.contains("unsub-wrong false true false"), "out: {out}");
    assert!(out.contains("store 7") && out.contains("outside undefined"), "out: {out}");
    assert!(out.contains("unbound true false"), "out: {out}");
    assert!(out.contains("sync R s,e:R"), "out: {out}");
    assert!(out.contains("promise P"), "out: {out}");
    assert!(out.contains("e1 ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("e2 true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn diagnostics_channel_cjs_strict_udp_faces() {
    // base19d 真回归两件（diag/bind-store + udp，未验证落地，本件钉死）：
    // 正常：strict-CJS 首行指令经单括号壳保持（runStores 回调 this===undefined）/
    //   sloppy-CJS 装箱 globalThis（对照）/ udp.socket 构造期同步 publish；
    // 报错：unbindStore 未绑定回 false；
    // 边界：同名 channel 两侧同一对象。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("strict.cjs")
        .write_str(
            r#"'use strict';
const assert = require("node:assert");
const dc = require("node:diagnostics_channel");
const { AsyncLocalStorage } = require("node:async_hooks");
const ch = dc.channel("t-strict");
const store = new AsyncLocalStorage();
ch.bindStore(store, (d) => ({ d }));
ch.runStores({ n: 1 }, function() {
  assert.strictEqual(this, undefined);
  assert.deepStrictEqual(store.getStore(), { d: { n: 1 } });
});
assert.strictEqual(store.getStore(), undefined);
assert.strictEqual(ch.unbindStore(store), true);
assert.strictEqual(ch.unbindStore(store), false);
console.log("strict-ok");
"#,
        )
        .unwrap();
    dir.child("sloppy.cjs")
        .write_str(
            r#"const assert = require("node:assert");
const dc = require("node:diagnostics_channel");
const ch = dc.channel("t-sloppy");
ch.runStores(7, function() {
  assert.strictEqual(this, globalThis);
});
console.log("sloppy-ok");
"#,
        )
        .unwrap();
    dir.child("udp.cjs")
        .write_str(
            r#"const assert = require("node:assert");
const dgram = require("node:dgram");
const dc = require("node:diagnostics_channel");
assert.strictEqual(dc.channel("udp.socket"), dc.channel("udp.socket"));
let n = 0;
dc.channel("udp.socket").subscribe(({ socket }) => {
  n++;
  assert.ok(socket instanceof dgram.Socket);
});
const s = dgram.createSocket("udp4");
assert.strictEqual(n, 1);
s.close();
console.log("udp-ok");
"#,
        )
        .unwrap();
    for (name, marker) in [("strict.cjs", "strict-ok"), ("sloppy.cjs", "sloppy-ok"), ("udp.cjs", "udp-ok")] {
        let path = dir.child(name).path().to_path_buf();
        let out = winterjs2().args(["--run", path.to_str().unwrap()]).output().unwrap();
        assert!(out.status.success(), "{name} stderr: {}", String::from_utf8_lossy(&out.stderr));
        assert!(
            String::from_utf8(out.stdout).unwrap().lines().any(|l| l == marker),
            "{name} missing {marker}"
        );
    }
    dir.close().unwrap();
}

#[test]
fn diagnostics_channel_parity_fixes() {
    // 10f 对拍牵引回归（test-diagnostics-channel-* 套件门）：
    // 正常：订阅者抛错走 uncaughtException（非 unhandled rejection）/
    //   transform 抛错同路由/enterWith 进入 + withStoreScope 恢复；
    // 报错：tracingChannel(0) ERR_INVALID_ARG_TYPE；
    // 边界：tracingChannel({}) V8 文案 TypeError。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("d2.mjs");
    file.write_str(
        r#"import dc, { channel, tracingChannel } from "node:diagnostics_channel";
import { AsyncLocalStorage } from "node:async_hooks";
const seen = [];
process.on("uncaughtException", (e) => seen.push(e.message));
// 订阅者抛错 → uncaughtException（mustCall 式计数）
channel("e-sub").subscribe(() => { throw new Error("sub-boom"); });
channel("e-sub").publish({});
// transform 抛错 → uncaughtException，store 不进作用域
const als = new AsyncLocalStorage();
channel("e-tr").bindStore(als, () => { throw new Error("tr-boom"); });
channel("e-tr").runStores({}, () => {});
// enterWith + withStoreScope 恢复
const st = new AsyncLocalStorage();
st.enterWith("initial");
console.log("enter", st.getStore());
{
  const ch = channel("scope-x");
  ch.subscribe(() => {});
  ch.bindStore(st, (d) => d);
  using scope = ch.withStoreScope("scoped");
  console.log("scoped", st.getStore());
}
console.log("restored", st.getStore());
// 参数校验
try { tracingChannel(0); console.log("t0 BAD"); }
catch (e) { console.log("t0", e.code === "ERR_INVALID_ARG_TYPE"); }
try { tracingChannel({}); console.log("t1 BAD"); }
catch (e) { console.log("t1", /Cannot convert undefined or null to object/.test(e.message)); }
setTimeout(() => console.log("uncaught", JSON.stringify(seen)), 20);
"#,
    )
    .unwrap();
    let out = winterjs2().args(["--run", file.path().to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "enter initial",
        "scoped scoped",
        "restored initial",
        "t0 true",
        "t1 true",
        r#"uncaught ["sub-boom","tr-boom"]"#,
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}
