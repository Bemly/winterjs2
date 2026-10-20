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
const mm = await vm.measureMemory({ mode: "summary" });
console.log("v-mm", typeof mm.total.jsMemoryEstimate === "number", Array.isArray(mm.total.jsMemoryRange));
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
    assert!(out.contains("v-mm true true"), "out: {out}");
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
// 真机口径：评估后 setExport 仍可（活绑定更新命名空间，26.8.1 对拍）。
await s.setExport("x", 1);
console.log("modb-synset", s.namespace.x === 1);
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
    assert!(out.contains("modb-relink THROW ERR_VM_MODULE_ALREADY_LINKED"), "out: {out}");
    assert!(out.contains("modb-errearly THROW ERR_VM_MODULE_STATUS"), "out: {out}");
    assert!(out.contains("modb-evthrow THROW"), "out: {out}");
    assert!(out.contains("modb-est true true"), "out: {out}");
    assert!(out.contains("modb-deps true"), "out: {out}");
    assert!(out.contains("modb-linkimports THROW"), "out: {out}");
    assert!(out.contains("modb-syn0 true true"), "out: {out}");
    assert!(out.contains("modb-syn1 true true"), "out: {out}");
    assert!(out.contains("modb-synset true"), "out: {out}");
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

#[test]
fn vm_source_map_and_memory_surface() {
    // sourceMapURL 魔术注释口径 + measureMemory 形状/校验（真机 26.8.1 对拍）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 正常：末行魔术注释取值。
const s1 = new vm.Script("var x = 1;\n//# sourceMappingURL=b.json");
console.log("map-ok", s1.sourceMapURL === "b.json");
// 报错沿：畸形注释/空值即 undefined。
const s2 = new vm.Script("var x = 1;\n// sourceMappingURL=b.json");
const s3 = new vm.Script("//# sourceMappingURL=");
console.log("map-bad", s2.sourceMapURL === undefined, s3.sourceMapURL === undefined);
// 边界：字符串字面量内的魔术串不算。
const s4 = new vm.Script("var s = \"//# sourceMappingURL=q.json\";");
console.log("map-str", s4.sourceMapURL === undefined);
// 正常：summary / detailed 形状。
const sum = await vm.measureMemory({ mode: "summary", execution: "eager" });
const det = await vm.measureMemory({ mode: "detailed" });
console.log("mm-sum", typeof sum.total.jsMemoryEstimate === "number", Array.isArray(sum.total.jsMemoryRange));
console.log("mm-det", typeof det.total.jsMemoryEstimate === "number", typeof det.current.jsMemoryEstimate === "number", Array.isArray(det.other));
// 报错：非法参数同步抛码。
const bad = (f) => { try { f(); return "no"; } catch (e) { return e.code || "(nocode)"; } };
console.log("mm-bad", bad(() => vm.measureMemory(null)), bad(() => vm.measureMemory("summary")), bad(() => vm.measureMemory({ mode: "random" })), bad(() => vm.measureMemory({ execution: "random" })));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "map-ok true",
        "map-bad true true",
        "map-str true",
        "mm-sum true true",
        "mm-det true true true",
        "mm-bad ERR_INVALID_ARG_TYPE ERR_INVALID_ARG_TYPE ERR_INVALID_ARG_VALUE ERR_INVALID_ARG_VALUE",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_error_stack_display() {
    // vm 抛错栈 displayErrors 形态：首行 filename:line + caret + 空行 + 首帧。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 正常：文件名首行 + 源码行 + 首帧。
try { vm.runInNewContext("throw new Error(\"foo\");", {}, { filename: "t.vm" }); } catch (e) {
  const lines = e.stack.split("\n");
  console.log("stk", lines[0] === "t.vm:1", lines[1] === "throw new Error(\"foo\");", lines[4] === "Error: foo", lines[5].trim() === "at t.vm:1:7");
}
// 报错沿：空 message 首行裸名。
try { vm.runInNewContext("throw new Error()", {}, { filename: "t.vm" }); } catch (e) {
  console.log("stk-empty", e.stack.split("\n")[4] === "Error");
}
// 边界：行列偏移显示期相加。
const ctx = vm.createContext({});
try { vm.runInContext(" throw new Error()", ctx, { filename: "e.js", lineOffset: 32, columnOffset: 123 }); } catch (e) {
  const lines = e.stack.split("\n");
  console.log("stk-off", lines[0] === "e.js:33", /^ \^/.test(lines[2]), /e\.js:33:131/.test(lines[5]));
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
        "stk true true true true",
        "stk-empty true",
        "stk-off true true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_script_timeout_surface() {
    // 超时中断三件（M3 中断回调 + 看门重发 + 孤儿排空）：快路正常、
    // 同步死循环码文案逐字、嵌套取最早到期、微任务逃逸同步中止、
    // 模块求值超时（rejected-completion 转码 + 认领）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 正常：快路（大超时内完成）。
console.log("to-fast", vm.runInNewContext("40 + 1", {}, { timeout: 1000 }) === 41);
// 报错：同步死循环（码 + 文案逐字 + 快速返回）。
const t0 = Date.now();
try { vm.runInNewContext("while(true){}", {}, { timeout: 50 }); console.log("to-sync", "NO-THROW"); }
catch (e) { console.log("to-sync", e.code === "ERR_SCRIPT_EXECUTION_TIMEOUT", e.message === "Script execution timed out after 50ms", Date.now() - t0 < 5000); }
// 边界：嵌套内层短（最早到期者文案）。
try { vm.runInNewContext("vm.runInNewContext('while(true){}', {}, { timeout: 10 })", { vm }, { timeout: 5000 }); console.log("to-nest", "NO-THROW"); }
catch (e) { console.log("to-nest", e.code === "ERR_SCRIPT_EXECUTION_TIMEOUT", e.message === "Script execution timed out after 10ms"); }
// 边界：微任务逃逸（同步 + 排空段双杀，无 unhandled fatal）。
try { vm.runInNewContext("Promise.resolve().then(() => { while(true){} }); while(true){}", {}, { timeout: 50 }); console.log("to-esc", "NO-THROW"); }
catch (e) { console.log("to-esc", e.code === "ERR_SCRIPT_EXECUTION_TIMEOUT", e.message === "Script execution timed out after 50ms"); }
// 模块：求值超时（rejected completion 转码；状态 errored）。
const m = new vm.SourceTextModule("while(true){}", { context: vm.createContext({}) });
await m.link(() => {});
try { await m.evaluate({ timeout: 50 }); console.log("to-mod", "NO-THROW"); }
catch (e) { console.log("to-mod", e.code === "ERR_SCRIPT_EXECUTION_TIMEOUT", e.message === "Script execution timed out after 50ms", m.status === "errored"); }
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "to-fast true",
        "to-sync true true true",
        "to-nest true true",
        "to-esc true true",
        "to-mod true true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_sync_descriptor_fidelity() {
    // partial-update/proxy-query 口径：vm 内 defineProperty 非默认旗保形回写
    // （单项改写不丢其余旗、跨轮保持）；linked 模块命名空间预取（未初始化访问
    // ReferenceError）；__modErr 码类→构造器映射（ARG_TYPE→TypeError）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const ctx = vm.createContext({});
vm.runInContext(`
  globalThis.a1 = 1;
  Object.defineProperty(globalThis, 'a1', { enumerable: false });
  Object.defineProperty(globalThis, 'a2', { value: 2, enumerable: false, configurable: true });
  Object.defineProperty(globalThis, 'a2', { enumerable: true });
  globalThis.b1 = 1;
  Object.defineProperty(globalThis, 'b1', { writable: false });
`, ctx);
const d1 = Object.getOwnPropertyDescriptor(ctx, "a1");
console.log("df-a1", ctx.a1 === 1, d1.enumerable === false);
const d2 = Object.getOwnPropertyDescriptor(ctx, "a2");
console.log("df-a2", ctx.a2 === 2, d2.enumerable === true);
const d3 = Object.getOwnPropertyDescriptor(ctx, "b1");
console.log("df-b1", ctx.b1 === 1, d3.writable === false);
console.log("df-reread", vm.runInContext("a1", ctx) === 1, vm.runInContext("Object.getOwnPropertyDescriptor(globalThis, 'b1').writable", ctx) === false);
// 模块命名空间 linked 可见（未初始化导出访问 ReferenceError；求值后取值）。
const m = new vm.SourceTextModule("export const q = 40 + 1;");
m.linkRequests([]);
m.instantiate();
let nsName = "";
try { m.namespace.q; } catch (e) { nsName = e.name; }
console.log("df-ns", m.status === "linked", nsName === "ReferenceError", Reflect.ownKeys(m.namespace).includes("q"));
await m.evaluate();
console.log("df-nseval", m.namespace.q === 41);
// __modErr 码类映射。
const bad = (f) => { try { f(); return "NO"; } catch (e) { return `${e.constructor.name}:${e.code || ""}`; } };
console.log("df-err", bad(() => new vm.SourceTextModule("1", { context: "x" })), bad(() => { const q = new vm.SourceTextModule("1"); q.linkRequests("x"); }));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "df-a1 true true",
        "df-a2 true true",
        "df-b1 true true",
        "df-reread true true",
        "df-ns true true true",
        "df-nseval true",
        "df-err TypeError:ERR_INVALID_ARG_TYPE TypeError:ERR_INVALID_ARG_TYPE",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_module_this_and_link_guards() {
    // 借调守卫（原型/非实例一律 ERR_INVALID_THIS TypeError；函数 options 拒收）
    // + 链接失败落 errored（status/error 记账；后继求值拒因）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const bad = (f) => { try { f(); return "NO"; } catch (e) { return `${e.constructor.name}:${e.code || ""}`; } };
console.log("guard-proto", bad(() => vm.Module.prototype.link()), bad(() => vm.Module.prototype.status));
console.log("guard-stm", bad(() => vm.SourceTextModule.prototype.namespace));
console.log("guard-fn", bad(() => new vm.SourceTextModule("", () => {})));
const m = new vm.SourceTextModule("import 'x';");
try { await m.link(() => ({})); } catch {}
console.log("link-err", m.status === "errored", m.error && m.error.code);
await m.evaluate().then(() => console.log("ev", "RES"), (e) => console.log("ev", e.code || "(nocode)"));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "guard-proto TypeError:ERR_INVALID_THIS TypeError:ERR_INVALID_THIS",
        "guard-stm TypeError:ERR_INVALID_THIS",
        "guard-fn TypeError:ERR_INVALID_ARG_TYPE",
        "link-err true ERR_VM_MODULE_LINK_FAILURE",
        "ev ERR_VM_MODULE_LINK_FAILURE",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_module_linking_surface() {
    // linkRequests/instantiate/moduleRequests/标识按 context 独立 + cachedData 存根。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 正常：空导入 linkRequests + instantiate + 求值。
const m = new vm.SourceTextModule("export const a = 40 + 1;");
m.linkRequests([]);
m.instantiate();
console.log("link-st", m.status === "linked");
console.log("link-req", JSON.stringify(m.moduleRequests) === "[]", m.dependencySpecifiers === m.dependencySpecifiers);
await m.link(() => {}).then(() => console.log("link-relink", "NO-THROW"), (e) => console.log("link-relink", e.code));
await m.evaluate();
console.log("link-ev", m.status === "evaluated", m.namespace.a === 41);
// 报错：非数组 linkRequests；未 linkRequests 即 instantiate；重复 instantiate。
const bad = (f) => { try { f(); return "NO-THROW"; } catch (e) { return e.code || "(nocode)"; } };
console.log("link-bad", bad(() => new vm.SourceTextModule("1").linkRequests("x")), bad(() => { const q = new vm.SourceTextModule("1"); q.instantiate(); }));
const q2 = new vm.SourceTextModule("1");
q2.linkRequests([]);
q2.instantiate();
console.log("link-bad2", bad(() => q2.instantiate()));
// 边界：标识按 context 独立计数 + cachedData 匹配/失配。
const c1 = vm.createContext({});
const c2 = vm.createContext({});
const a1 = new vm.SourceTextModule("1", { context: c1 });
const a2 = new vm.SourceTextModule("2", { context: c1 });
const a3 = new vm.SourceTextModule("3", { context: c2 });
console.log("link-id", a1.identifier === "vm:module(0)", a2.identifier === "vm:module(1)", a3.identifier === "vm:module(0)");
const cm = new vm.SourceTextModule("const a = 1");
const cd = cm.createCachedData();
console.log("link-cd", Buffer.isBuffer(cd), cm.cachedDataRejected === undefined);
const cm2 = new vm.SourceTextModule("const a = 1", { cachedData: cd });
console.log("link-cd2", cm2.cachedDataRejected === false);
console.log("link-cd3", bad(() => new vm.SourceTextModule("other", { cachedData: cd })));
// 正常：求值中重入求值即 STATUS（SourceText + Synthetic）。
let inner = "";
const em = new vm.SourceTextModule("globalThis.__probeEv(); export const a = 1;");
em.linkRequests([]);
em.instantiate();
globalThis.__probeEv = () => { em.evaluate().then(() => { inner = "RES"; }, (e) => { inner = e.code || "(nocode)"; }); };
await em.evaluate();
console.log("link-reev", inner === "ERR_VM_MODULE_STATUS");
const sm = new vm.SyntheticModule(["a"], () => { sm.evaluate().then(() => {}, (e) => { inner = e.code || "(nocode2)"; }); });
sm.evaluate();
await Promise.resolve();
console.log("link-reev2", inner === "ERR_VM_MODULE_STATUS");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "link-st true",
        "link-req true true",
        "link-relink ERR_VM_MODULE_ALREADY_LINKED",
        "link-ev true true",
        "link-bad ERR_INVALID_ARG_TYPE ERR_VM_MODULE_LINK_FAILURE",
        "link-bad2 ERR_VM_MODULE_STATUS",
        "link-id true true true",
        "link-cd true true",
        "link-cd2 true",
        "link-cd3 ERR_VM_MODULE_CACHED_DATA_REJECTED",
        "link-reev true",
        "link-reev2 true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_codegen_restriction_surface() {
    // codeGeneration 校验 + strings/wasm 禁用遮蔽。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 正常：默认全开；显式 true 全开。
vm.runInNewContext("eval('1')", {});
const c0 = vm.createContext({}, { codeGeneration: { strings: true, wasm: true } });
console.log("cg-open", vm.runInContext("typeof eval", c0) === "function");
// 正常：strings:false 时 eval 抛 ctx 域 EvalError。
const c1 = vm.createContext({}, { codeGeneration: { strings: false } });
const CtxEvalError = vm.runInContext("EvalError", c1);
try { vm.runInContext("eval('x')", c1); console.log("cg-str", "NO-THROW"); }
catch (e) { console.log("cg-str", e instanceof CtxEvalError); }
// 正常：wasm:false 时 Module 抛 ctx 域 CompileError。
const wb = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
const c2 = vm.createContext({ WASM_BYTES: wb }, { codeGeneration: { wasm: false } });
const CtxCompileError = vm.runInContext("WebAssembly.CompileError", c2);
try { vm.runInContext("new WebAssembly.Module(WASM_BYTES)", c2); console.log("cg-wasm", "NO-THROW"); }
catch (e) { console.log("cg-wasm", e instanceof CtxCompileError, typeof vm.runInContext("WebAssembly.CompileError", c2)); }
// 报错：非布尔开关/非对象整个项。
const bad = (f) => { try { f(); return "NO-THROW"; } catch (e) { return e.code || "(nocode)"; } };
console.log("cg-bad", bad(() => vm.createContext({}, { codeGeneration: { strings: 0 } })), bad(() => vm.createContext({}, { codeGeneration: 1 })), bad(() => vm.runInNewContext("1", {}, { contextCodeGeneration: { wasm: 1 } })));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "cg-open true",
        "cg-str true",
        "cg-wasm true function",
        "cg-bad ERR_INVALID_ARG_TYPE ERR_INVALID_ARG_TYPE ERR_INVALID_ARG_TYPE",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_module_settle_surface() {
    // 同步落定（成功/失败身份）+ TLA 异步 + 落定前命名空间 + promise 内态显示。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
import { inspect } from "node:util";
// 正常：同步成功即落定（inspect 同步展值），重求值同值。
const m = new vm.SourceTextModule("export const a = 40 + 1;");
m.linkRequests([]);
m.instantiate();
const p = m.evaluate();
console.log("settle-sync", inspect(p) === "Promise { undefined }", m.namespace.a === 41);
const p2 = m.evaluate();
console.log("settle-reev", inspect(p2) === "Promise { undefined }");
// 报错沿：同步失败因身份一致 + 重求值同因拒绝。
const e = new vm.SourceTextModule("throw new Error('boom');");
e.linkRequests([]);
e.instantiate();
const pe = e.evaluate();
pe.then(undefined, () => {});
console.log("settle-err", inspect(pe).includes("rejected"), e.error instanceof Error, e.error.message === "boom");
const pe2 = e.evaluate();
console.log("settle-err2", pe2 !== pe, await pe2.then(() => "RES", (x) => x === e.error));
// 边界：TLA 异步落定；落定前命名空间访问即 ReferenceError。
const t = new vm.SourceTextModule("await Promise.resolve(); export const a = 1;");
t.linkRequests([]);
t.instantiate();
const pt = t.evaluate();
let nsName = "";
try { t.namespace.a; } catch (x) { nsName = x.name; }
console.log("settle-tla", inspect(pt) === "Promise { <pending> }", nsName === "ReferenceError");
await pt;
console.log("settle-tla2", t.status === "evaluated", t.namespace.a === 1);
// promise 内态显示三件。
console.log("settle-insp", inspect(Promise.resolve(42)) === "Promise { 42 }", inspect(new Promise(() => {})) === "Promise { <pending> }");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "settle-sync true true",
        "settle-reev true",
        "settle-err true true true",
        "settle-err2 true true",
        "settle-tla true true",
        "settle-tla2 true true",
        "settle-insp true true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing: {line}\nout: {out}");
    }
    dir.close().unwrap();
}
