//! tests/node/vm.rs — 对齐 src/builtins/node/vm.rs（node:vm）。

use crate::helpers::*;

#[test]
fn vm_context_spawns_and_isolates() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const sb = { a: 5 };
const r = vm.runInNewContext("b = a + 1; b", sb);
console.log("v-run", r === 6, sb.b === 6, typeof b === "undefined");
const c1 = vm.createContext({ x: 1 });
const c2 = vm.createContext({ x: 2 });
console.log("v-ctx", vm.isContext(c1), vm.isContext(c2), vm.isContext({}));
vm.runInContext("y = x * 10", c1);
vm.runInContext("y = x * 10", c2);
console.log("v-iso", c1.y === 10, c2.y === 20);
const s = new vm.Script("40 + 2");
console.log("v-script", s.runInNewContext() === 42, s.runInThisContext() === 42);
const f = vm.compileFunction("return a + b", ["a", "b"]);
console.log("v-cf", f(20, 22) === 42);
const o = vm.runInNewContext("({ z: 7 })", {});
console.log("v-ccw", o.z === 7, typeof o === "object");
const sb2 = {};
vm.runInNewContext("Promise.resolve(1).then(v => { globalThis.px = v; })", sb2);
console.log("v-micro", sb2.px === 1);
console.log("v-std", vm.runInNewContext("typeof Object") === "function", vm.runInNewContext("typeof console") === "undefined");
console.log("v-const", typeof vm.constants.USE_MAIN_CONTEXT_DEFAULT_LOADER, typeof vm.constants.DONT_CONTEXTIFY);
console.log("v-timeout", vm.runInNewContext("1 + 1", {}, { timeout: 100 }) === 2);
const mm = await vm.measureMemory().then(() => "no", (e) => e.code);
console.log("v-mm", mm === "ERR_CONTEXT_NOT_INITIALIZED");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("v-run true true true"), "out: {out}");
    assert!(out.contains("v-ctx true true false"), "out: {out}");
    assert!(out.contains("v-iso true true"), "out: {out}");
    assert!(out.contains("v-script true true"), "out: {out}");
    assert!(out.contains("v-cf true"), "out: {out}");
    assert!(out.contains("v-ccw true true"), "out: {out}");
    assert!(out.contains("v-micro true"), "out: {out}");
    assert!(out.contains("v-std true true"), "out: {out}");
    assert!(out.contains("v-const symbol symbol"), "out: {out}");
    assert!(out.contains("v-timeout true"), "out: {out}");
    assert!(out.contains("v-mm true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn vm_errors_boundary() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
try { new vm.Script("}{"); } catch (e) { console.log("w-ctor", e.constructor.name === "SyntaxError"); }
try { vm.compileFunction("}{"); } catch (e) { console.log("w-cf", e.constructor.name === "SyntaxError"); }
try { vm.runInNewContext("throw new RangeError('nope')"); } catch (e) { console.log("w-range", e.constructor.name === "RangeError", e.message === "nope"); }
try { vm.runInNewContext("throw 'strval'"); } catch (e) { console.log("w-str", typeof e === "string", e === "strval"); }
try { vm.runInNewContext("noSuchVar + 1"); } catch (e) { console.log("w-ref", e.constructor.name === "ReferenceError"); }
try { vm.runInContext("1", {}); } catch (e) { console.log("w-badctx", e.code === "ERR_INVALID_ARG_TYPE"); }
try { vm.runInNewContext("1", 42); } catch (e) { console.log("w-badsb", e.code === "ERR_INVALID_ARG_TYPE"); }
try { vm.isContext(42); } catch (e) { console.log("w-isctx", e.code === "ERR_INVALID_ARG_TYPE"); }
try { vm.runInNewContext("1", {}, { microtaskMode: "nope" }); } catch (e) { console.log("w-mmode", e.code === "ERR_INVALID_ARG_VALUE"); }
try { vm.runInNewContext("1", {}, { timeout: -1 }); } catch (e) { console.log("w-timeout", e.code === "ERR_OUT_OF_RANGE"); }
const pc = vm.createContext({ q: 41 });
const f2 = vm.compileFunction("return q + 1", [], { parsingContext: pc });
console.log("w-pc", f2() === 42);
const ce = vm.compileFunction("return ex + 1", [], { contextExtensions: [{ ex: 41 }] });
console.log("w-ext", ce() === 42);
const cached = new vm.Script("9", { cachedData: Buffer.alloc(0), produceCachedData: true });
console.log("w-cache", cached.runInNewContext() === 9, cached.cachedDataProduced === false);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("w-ctor true"), "out: {out}");
    assert!(out.contains("w-cf true"), "out: {out}");
    assert!(out.contains("w-range true true"), "out: {out}");
    assert!(out.contains("w-str true true"), "out: {out}");
    assert!(out.contains("w-ref true"), "out: {out}");
    assert!(out.contains("w-badctx true"), "out: {out}");
    assert!(out.contains("w-badsb true"), "out: {out}");
    assert!(out.contains("w-isctx true"), "out: {out}");
    assert!(out.contains("w-mmode true"), "out: {out}");
    assert!(out.contains("w-timeout true"), "out: {out}");
    assert!(out.contains("w-pc true"), "out: {out}");
    assert!(out.contains("w-ext true"), "out: {out}");
    assert!(out.contains("w-cache true true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn vm_source_module_chain() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const m = new vm.SourceTextModule("export const a = 40 + 1;");
console.log("mod-st0", m.status === "unlinked", m.identifier === "vm:module(0)", Array.isArray(m.dependencySpecifiers) && m.dependencySpecifiers.length === 0);
console.log("mod-inst", m instanceof vm.SourceTextModule, m instanceof vm.Module);
await m.link(() => {});
console.log("mod-st1", m.status === "linked");
const er = m.evaluate();
console.log("mod-evret", er instanceof Promise);
await er;
console.log("mod-st2", m.status === "evaluated", m.namespace.a === 41);
// 重复求值照真机成功（无操作）。
await m.evaluate();
console.log("mod-reev", m.status === "evaluated");
// 上下文隔离：同名种子不同值。
const c1 = vm.createContext({ seed: 3 });
const c2 = vm.createContext({ seed: 4 });
const m1 = new vm.SourceTextModule("export const v = seed * 2;", { context: c1, identifier: "m1" });
const m2 = new vm.SourceTextModule("export const v = seed * 2;", { context: c2, identifier: "m2" });
await m1.link(() => {});
await m2.link(() => {});
await m1.evaluate();
await m2.evaluate();
console.log("mod-iso", m1.namespace.v === 6, m2.namespace.v === 8, m1.identifier === "m1", m1.context === c1);
// 顶层 await 模块（异步求值认领路径）。
const t = new vm.SourceTextModule("export const v = await Promise.resolve(41);");
await t.link(() => {});
await t.evaluate();
console.log("mod-tla", t.status === "evaluated", t.namespace.v === 41);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "mod-st0 true true true",
        "mod-inst true true",
        "mod-st1 true",
        "mod-evret true",
        "mod-st2 true true",
        "mod-reev true",
        "mod-iso true true true true",
        "mod-tla true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_module_boundary() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const t = async (n, f) => { try { const r = await f(); console.log(n, "OK", r === undefined ? "undef" : "val"); } catch (e) { console.log(n, "THROW", e.code || "(nocode)"); } };
await t("modb-syntax", async () => new vm.SourceTextModule("export const q = ;"));
await t("modb-nonstr", async () => new vm.SourceTextModule(123));
await t("modb-badctx", async () => new vm.SourceTextModule("export const a = 1;", { context: {} }));
const m = new vm.SourceTextModule("export const a = 1;");
await t("modb-linknofn", async () => m.link());
await t("modb-nsearly", async () => m.namespace);
await t("modb-evunlinked", async () => m.evaluate());
await m.link(() => {});
await t("modb-relink", async () => m.link(() => {}));
await t("modb-errearly", async () => m.error);
const e = new vm.SourceTextModule("throw new Error('boom');");
await e.link(() => {});
await t("modb-evthrow", async () => e.evaluate());
console.log("modb-est", e.status === "errored", e.error && e.error.message === "boom");
const im = new vm.SourceTextModule("import {x} from './nope.js'; export const a = x;");
console.log("modb-deps", JSON.stringify(im.dependencySpecifiers) === JSON.stringify(["./nope.js"]));
await t("modb-linkimports", async () => im.link(() => {}));
const s = new vm.SyntheticModule(["x"], function () { this.setExport("x", 42); });
console.log("modb-syn0", s.status === "linked", s.dependencySpecifiers === undefined);
await s.link();
await s.evaluate();
console.log("modb-syn1", s.status === "evaluated", s.namespace.x === 42);
await t("modb-synset", async () => s.setExport("x", 1));
const se = new vm.SyntheticModule(["d"], function () { throw new Error("cbboom"); });
await se.link(() => {});
await t("modb-syncb", async () => se.evaluate());
console.log("modb-synest", se.status === "errored");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("modb-syntax THROW"), "out: {out}");
    assert!(out.contains("modb-nonstr THROW ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("modb-badctx THROW ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("modb-linknofn THROW ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("modb-nsearly THROW ERR_VM_MODULE_STATUS"), "out: {out}");
    assert!(out.contains("modb-evunlinked THROW ERR_VM_MODULE_STATUS"), "out: {out}");
    assert!(out.contains("modb-relink THROW ERR_VM_MODULE_STATUS"), "out: {out}");
    assert!(out.contains("modb-errearly THROW ERR_VM_MODULE_STATUS"), "out: {out}");
    assert!(out.contains("modb-evthrow THROW"), "out: {out}");
    assert!(out.contains("modb-est true true"), "out: {out}");
    assert!(out.contains("modb-deps true"), "out: {out}");
    assert!(out.contains("modb-linkimports THROW"), "out: {out}");
    assert!(out.contains("modb-syn0 true true"), "out: {out}");
    assert!(out.contains("modb-syn1 true true"), "out: {out}");
    assert!(out.contains("modb-synset THROW ERR_VM_MODULE_STATUS"), "out: {out}");
    assert!(out.contains("modb-syncb THROW"), "out: {out}");
    assert!(out.contains("modb-synest true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn vm_dont_contextify() {
    // Node 24+ DONT_CONTEXTIFY（真机 26.8.2 对拍）：新建独立 context 并返回其
    // global 本体——≠主 globalThis、isContext、runInContext("this")===返回值、
    // 写入不穿透主域、新域带 SAB/Atomics（jsdom 29 以此直装 DOM 全局）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const w = vm.createContext(vm.constants.DONT_CONTEXTIFY);
console.log("ident", w !== globalThis, vm.isContext(w));
console.log("this", vm.runInContext("this", w) === w, vm.runInContext("this", w) === globalThis);
w.__probe = 7;
console.log("iso", globalThis.__probe === undefined, vm.runInContext("__probe", w));
console.log("sab", typeof w.SharedArrayBuffer, typeof w.Atomics);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in ["ident true true", "this true false", "iso true 7", "sab function object"] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_sync_snapshot() {
    // 10f：创建快照 + sync-out 全键口径（不可枚举串键回写、symbol 只存在性、
    // 标准构造器未改不污染、改了回写；`undefined/NaN/Infinity` 只读常量永不碰）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 不可枚举串键：创建即 sync-in，vm 内可见；vm 内新建不可枚举串键 sync-out 回写。
const sb = {};
Object.defineProperty(sb, "h", { value: 41, writable: true, configurable: true });
const c = vm.createContext(sb);
console.log("hidden-in", vm.runInContext("h", c));
vm.runInContext('Object.defineProperty(this, "ni", { value: 7, configurable: true });', c);
console.log("hidden-out", c.ni, Object.getOwnPropertyNames(c).includes("ni"));
// 标准构造器未改不污染、改了回写；只读常量永不碰。
const sb2 = {};
const c2 = vm.createContext(sb2);
vm.runInContext("Array.__probe = 1", c2);
console.log("std", sb2.Array, "__probe" in sb2);
vm.runInContext("this.foo = 123", c2);
console.log("newkey", c2.foo);
// 种子覆盖回写、标准替换回写。
const sb3 = { keep: 1 };
const c3 = vm.createContext(sb3);
vm.runInContext("keep = 2; Array = 5", c3);
console.log("seed", sb3.keep, sb3.Array);
// DONT_CONTEXTIFY：簿记键 + 只读常量不抛。
const w = vm.createContext(vm.constants.DONT_CONTEXTIFY);
console.log("dont", vm.runInContext("1 + 1", w) === 2, typeof w.Object === "function");
"#,
    );
    for line in [
        "hidden-in 41",
        "hidden-out 7 true",
        "std undefined false",
        "newkey 123",
        "seed 2 5",
        "dont true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_sync_all_keys() {
    // 10f：UNSAFE-BOUNDARY panic 路径（坏 id → TypeError 包络；same 缺参 → TypeError）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const bad = (n, f) => { try { f(); console.log(n, "NO-THROW"); } catch (e) { console.log(n, e.constructor.name); } };
bad("keysall", () => __wjs2_vm_keys_all("999999"));
bad("keyscount", () => __wjs2_vm_keys_count("999999"));
bad("same-arity", () => __wjs2_vm_same(1));
console.log("same-ok", __wjs2_vm_same(1, 2) === false, __wjs2_vm_same(NaN, NaN) === true);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "keysall Error",
        "keyscount Error",
        "same-arity Error",
        "same-ok true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_rerun_with_new_globals() {
    // 10c-3：同 context 重复 runInContext（前轮新建的全局须可复用；
    // sync-in 的重定义走赋值回落，见 vm_set）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "r.mjs",
        r#"
import vm from "node:vm";
const a = vm.createContext({ x: 1 });
console.log("a", vm.runInContext("x + 1", a), vm.runInContext("x + 2", a));
const b = vm.createContext({});
vm.runInContext("function foo() { return 1; }", b);
console.log("fn", vm.runInContext("foo()", b));
const c = vm.createContext({});
vm.runInContext("var cv = 5", c);
console.log("vr", vm.runInContext("cv + 1", c));
const d = vm.createContext({ seed: 9 });
vm.runInContext("function f() { return seed * 2; }", d);
console.log("mix", vm.runInContext("f()", d), vm.runInContext("seed + 1", d));
"#,
    );
    for line in ["a 2 3", "fn 1", "vr 6", "mix 18 10"] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}


#[test]
fn vm_parity_sync_and_errors() {
    // 10f vm 对拍收口面：簿记不落沙箱键（ownkeys 族）、symbol 键/访问器同步、
    // 描述符保形（nonWritable + strict 赋值文案桥）、沙箱自指键（window）、
    // 错误原物透传（跨域 instanceof + 非对象 throw 原样 + ReferenceError 文案）。
    // 断言标签互不为子串、多布尔分参（§4.42）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "sync.mjs",
        r#"
import vm from "node:vm";

// ownkeys：簿记键不落沙箱（createContext 前后键集/符号数不变）
const sym1 = Symbol("s1");
const sb = { a: 1, [sym1]: true };
Object.defineProperty(sb, "b", { value: true, writable: false, enumerable: false, configurable: false });
const before = Reflect.ownKeys(sb).length;
const ctx = vm.createContext(sb);
console.log("keys-keep", before === Reflect.ownKeys(sb).length, Object.getOwnPropertySymbols(sb).length === 1);
// symbol 键同步进 vm global；非默认描述符（b 不可写不可枚举）保形
const vmSym = vm.runInContext("Reflect.ownKeys(this).some(k => typeof k === 'symbol')", ctx);
const vmB = vm.runInContext("const d = Object.getOwnPropertyDescriptor(this, 'b'); d.writable === false && d.enumerable === false", ctx);
console.log("keys-vm", vmSym, vmB);

// 描述符保形 + strict 赋值桥接文案（vm_set 只读静默跳过，值不被覆盖）
const ctx2 = vm.createContext({});
Object.defineProperty(ctx2, "nw", { value: 51, writable: false, enumerable: true });
let threw = "", msg = "";
try { vm.runInContext('"use strict"; nw = 0', ctx2); } catch (e) { threw = e.constructor.name; msg = e.message; }
console.log("nw-throw", threw === "TypeError", msg.startsWith("Cannot assign to read only property 'nw'"), vm.runInContext("nw", ctx2) === 51);

// symbol 访问器：经 vm global（CCW）读写都触发沙箱侧 get/set
const sAcc = Symbol("acc");
let stored = 0;
const ctx3 = vm.createContext({});
Object.defineProperty(ctx3, sAcc, { get: () => stored + 40, set: (v) => { stored = v; }, configurable: true });
const gp = vm.runInContext("this", ctx3);
gp[sAcc] = 7;
console.log("sym-acc", stored === 7, gp[sAcc] === 47);

// 自指键：ctx.window = ctx → vm 侧 this/window 同身份，且 sandbox.window 不被换
const ctx4 = vm.createContext();
ctx4.window = ctx4;
const tv = vm.runInContext("this", ctx4);
const wv = vm.runInContext("window", ctx4);
console.log("selfref", tv === wv, ctx4.window === ctx4);

// 错误透传：vm 域 SyntaxError 身份（instanceof 跨域）+ 非对象 throw 原样
const ctx5 = vm.createContext({});
vm.runInContext("Object.defineProperty(this, 'foo', { value: 1, configurable: false })", ctx5);
let realmSyn = false;
try { vm.runInContext("let foo = 2", ctx5); } catch (e) { realmSyn = e instanceof vm.runInContext("SyntaxError", ctx5); }
console.log("err-realm", realmSyn);
let prim = false;
try { vm.runInNewContext("throw 'sv'"); } catch (e) { prim = e === "sv"; }
console.log("err-prim", prim);
let refMsg = "";
try { vm.runInNewContext('"use strict"; zz = 1'); } catch (e) { refMsg = e.message; }
console.log("err-ref", refMsg === "zz is not defined");
"#,
    );
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    for line in [
        "keys-keep true true",
        "keys-vm true true",
        "nw-throw true true true",
        "sym-acc true true",
        "selfref true true",
        "err-realm true",
        "err-prim true",
        "err-ref true",
    ] {
        assert!(text.lines().any(|l| l == line), "missing: {line}\nout: {text}");
    }
    dir.close().unwrap();
}
