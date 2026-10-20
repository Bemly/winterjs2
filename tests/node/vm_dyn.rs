//! tests/node/vm_dyn.rs — 对齐 src/builtins/node/vm_dyn.rs（动态导入投递 +
//! import.meta 投递 + unhandledRejection 投递；merge-note #2/#3）。
//!
//! 覆盖：三路 referrer 回调（this/参数/恒等）、缺回调/非法返回/符号直通、
//! async 回调、命名空间直返、meta 默认/回调/抛错、事件投递与模式、
//! UNSAFE-BOUNDARY 入口守卫（坏 id/坏类型）。

use crate::common::*;
use crate::helpers::*;
use assert_fs::prelude::*;

#[test]
fn vm_module_dynamic_import_faces() {
    // 正常 + 报错 + 边界（多断言分参，标签禁子串，见 §4.42）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 正常：Script 回调（this/参数/属性原型/phase/恒等）。
const foo = new vm.SourceTextModule("export const a = 1;");
await foo.link(() => {});
await foo.evaluate();
let seen = null;
const s = new vm.Script('import("foo", { with: { k: "v" } })', {
  importModuleDynamically: function (spec, ref, attrs, phase) {
    seen = [this === undefined, spec, ref === s,
      Object.getPrototypeOf(attrs) === null, attrs.k, phase];
    return foo;
  },
});
console.log("dyn-script", (await s.runInThisContext()) === foo.namespace, JSON.stringify(seen));
// 正常：模块 async 回调 + 命名空间直返恒等。
const inner = new vm.SourceTextModule("export const q = 9;");
await inner.link(() => {});
await inner.evaluate();
const m = new vm.SourceTextModule("globalThis.__r = import(\"x\");", {
  importModuleDynamically: async () => inner.namespace,
});
await m.link(() => {});
await m.evaluate();
console.log("dyn-ns", (await globalThis.__r) === inner.namespace);
delete globalThis.__r;
// 正常：compileFunction + SyntheticModule（恒等；参数另行断言）。
const syn = new vm.SyntheticModule(["x"], function () { this.setExport("x", 42); });
await syn.link(() => {});
await syn.evaluate();
let cfSeen = null;
const f = vm.compileFunction("return import(\"y\")", [], {
  importModuleDynamically: (spec, ref) => { cfSeen = [spec, ref === f]; return syn; },
});
console.log("dyn-cf", (await f()) === syn.namespace, JSON.stringify(cfSeen));
// 报错：无回调即 CALLBACK_MISSING（Script/模块双路）。
const s2 = new vm.Script('import("z")');
await s2.runInThisContext().then(
  () => console.log("dyn-nocb", "NO-THROW"),
  (e) => console.log("dyn-nocb", e.constructor.name, e.code),
);
const m2 = new vm.SourceTextModule("globalThis.__r2 = import(\"z\"); globalThis.__r2.catch(() => {});");
await m2.link(() => {});
await m2.evaluate();
try { await globalThis.__r2; console.log("dyn-nocb2", "NO-THROW"); }
catch (e) { console.log("dyn-nocb2", e.constructor.name, e.code); }
delete globalThis.__r2;
// 报错：非法返回即 NOT_MODULE（5/undefined 双形）。
const bad = async (ret) => {
  const mm = new vm.SourceTextModule("globalThis.__r3 = import(\"w\"); globalThis.__r3.catch(() => {});", {
    importModuleDynamically: () => ret,
  });
  await mm.link(() => {});
  await mm.evaluate();
  try { await globalThis.__r3; return "NO-THROW"; }
  catch (e) { return `${e.constructor.name}:${e.code}`; }
  finally { delete globalThis.__r3; }
};
console.log("dyn-bad5", await bad(5));
console.log("dyn-badundef", await bad(undefined));
// 报错：构造器校验（Script/compileFunction 非函数即 TypeError）。
const badCtor = (f) => { try { f(); return "NO-THROW"; } catch (e) { return `${e.constructor.name}:${e.code}`; } };
console.log("dyn-ctor", badCtor(() => new vm.Script("1", { importModuleDynamically: false })));
console.log("dyn-cfctor", badCtor(() => vm.compileFunction("1", [], { importModuleDynamically: 42 })));
// 边界：USE_MAIN_CONTEXT_DEFAULT_LOADER 走主 loader（file URL 真解析；
  // 依赖与用例同目录，import.meta 定位，零仓库污染）。
import { writeFileSync } from "node:fs";
const depUrl = new URL("./dep.mjs", import.meta.url);
writeFileSync(depUrl, "export const v = 41;");
const sm = new vm.Script(`import(${JSON.stringify(depUrl.href)})`, {
  importModuleDynamically: vm.constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
});
console.log("dyn-mainloader", (await sm.runInThisContext()).v === 41);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "dyn-script true [true,\"foo\",true,true,\"v\",\"evaluation\"]",
        "dyn-ns true",
        "dyn-cf true [\"y\",true]",
        "dyn-nocb TypeError ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING",
        "dyn-nocb2 TypeError ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING",
        "dyn-bad5 Error:ERR_VM_MODULE_NOT_MODULE",
        "dyn-badundef Error:ERR_VM_MODULE_NOT_MODULE",
        "dyn-ctor TypeError:ERR_INVALID_ARG_TYPE",
        "dyn-cfctor TypeError:ERR_INVALID_ARG_TYPE",
        "dyn-mainloader true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_module_import_meta_faces() {
    // 正常 + 报错 + 边界。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
// 正常：回调拿 (meta, module)，this undefined，返值忽略，键集精确。
let seen = null;
const m = new vm.SourceTextModule("globalThis.__im = import.meta;", {
  initializeImportMeta: function (meta, mod) {
    seen = [this === undefined, mod === m, Object.getPrototypeOf(meta) === null,
      JSON.stringify(Reflect.ownKeys(meta))];
    meta.prop = 42;
    return "ignored";
  },
});
await m.link(() => {});
await m.evaluate();
console.log("meta-cb", JSON.stringify(seen), globalThis.__im.prop, JSON.stringify(Reflect.ownKeys(globalThis.__im)));
delete globalThis.__im;
// 正常：无回调即空 meta（null 原型，零键）。
const m2 = new vm.SourceTextModule("globalThis.__im2 = import.meta;");
await m2.link(() => {});
await m2.evaluate();
console.log("meta-default", typeof globalThis.__im2, Object.getPrototypeOf(globalThis.__im2) === null, JSON.stringify(Reflect.ownKeys(globalThis.__im2)));
delete globalThis.__im2;
// 报错：回调抛错即求值失败（原错透传）。
const m3 = new vm.SourceTextModule("globalThis.__im3 = import.meta;", {
  initializeImportMeta: () => { throw new RangeError("meta-boom"); },
});
await m3.link(() => {});
await m3.evaluate().then(
  () => console.log("meta-throw", "NO-THROW"),
  (e) => console.log("meta-throw", e.constructor.name, e.message),
);
// 边界：非法 init 值一律 TypeError（7 形全量）。
const vals = [null, {}, 0, Symbol.iterator, [], "s", false];
console.log("meta-bad", vals.map((v) => {
  try { new vm.SourceTextModule("", { initializeImportMeta: v }); return "NO"; }
  catch (e) { return `${e.constructor.name}:${e.code}`; }
}).join("|"));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "meta-cb [true,true,true,\"[]\"] 42 [\"prop\"]",
        "meta-default object true []",
        "meta-throw RangeError meta-boom",
        "meta-bad TypeError:ERR_INVALID_ARG_TYPE|TypeError:ERR_INVALID_ARG_TYPE|TypeError:ERR_INVALID_ARG_TYPE|TypeError:ERR_INVALID_ARG_TYPE|TypeError:ERR_INVALID_ARG_TYPE|TypeError:ERR_INVALID_ARG_TYPE|TypeError:ERR_INVALID_ARG_TYPE",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_unhandled_delivery_faces() {
    // 正常：监听拦到 (reason, promise)，不退出。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const seen = [];
process.on("unhandledRejection", (r, p) => seen.push([r && r.message, p instanceof Promise]));
vm.runInNewContext("Promise.reject(new Error(\"vm-boom\"));", {});
Promise.reject(new Error("main-boom"));
setTimeout(() => console.log("ev", JSON.stringify(seen.sort())), 50);
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(
        out.lines().any(|l| l == "ev [[\"main-boom\",true],[\"vm-boom\",false]]"),
        "out: {out}"
    );
    dir.close().unwrap();

    // 报错：无监听即 fatal（exit 1 + 文案）。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("q.mjs");
    file.write_str("Promise.reject(new Error(\"nope\"));").unwrap();
    let out = winterjs2().arg("--run").arg(file.path()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(err.contains("unhandled rejection"), "stderr: {err}");
    dir.close().unwrap();

    // 边界：warn 模式警告 + 继续（exit 0）；none 模式静默（exit 0）。
    for (flag, want_exit, want_warn) in [
        ("--unhandled-rejections=warn", 0, true),
        ("--unhandled-rejections=none", 0, false),
        ("--unhandled-rejections=strict", 1, false),
        ("--unhandled-rejections=warn-with-error-code", 1, true),
    ] {
        let dir = assert_fs::TempDir::new().unwrap();
        let file = dir.child("r.mjs");
        file.write_str("Promise.reject(new Error(\"m\"));").unwrap();
        let out = winterjs2()
            .arg(flag)
            .arg("--run")
            .arg(file.path())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(want_exit), "flag: {flag}");
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        // warn 系打印警告名；throw/strict/none 不打印（fatal 文案另含 "Error: m"，不做其断言）。
        assert_eq!(
            err.contains("UnhandledPromiseRejectionWarning"),
            want_warn,
            "flag: {flag} stderr: {err}"
        );
        dir.close().unwrap();
    }
}

#[test]
fn vm_referrer_ctx_faces() {
    // 正常：无 JS 栈的 eval 动态 import 走上下文回调（referrer 即 sandbox 本体）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const foo = new vm.SourceTextModule("export const a = 1;");
await foo.link(() => {});
await foo.evaluate();
const ctx = vm.createContext({}, {
  importModuleDynamically: (spec, wrap) => {
    console.log("ctx-cb", spec, wrap === ctx);
    return foo;
  },
});
const s = new vm.Script('Promise.resolve("import(\'foo\')").then(eval)', {
  importModuleDynamically: () => console.log("script-cb-NO"),
});
console.log("r1", (await s.runInContext(ctx)) === foo.namespace);
// 正常：模块路同理（Script 回调不触发）。
const m = new vm.SourceTextModule('globalThis.__r = Promise.resolve("import(\'foo\')").then(eval);', {
  context: ctx,
  importModuleDynamically: () => console.log("mod-cb-NO"),
});
await m.link(() => {});
await m.evaluate();
console.log("r2", (await ctx.__r) === foo.namespace);
delete ctx.__r;
// 报错：无上下文回调即 CALLBACK_MISSING（Script/模块双路）。
const ctx2 = vm.createContext({});
const s2 = new vm.Script('Promise.resolve("import(\'foo\')").then(eval)', {
  importModuleDynamically: () => {},
});
try { await s2.runInContext(ctx2); console.log("m1", "NO-THROW"); }
catch (e) { console.log("m1", e.constructor.name, e.code); }
const m2 = new vm.SourceTextModule('globalThis.__r2 = Promise.resolve("import(\'foo\')").then(eval);', {
  context: ctx2,
  importModuleDynamically: () => {},
});
await m2.link(() => {});
await m2.evaluate();
try { await ctx2.__r2; console.log("m2", "NO-THROW"); }
catch (e) { console.log("m2", e.constructor.name, e.code); }
delete ctx2.__r2;
// 边界：createContext 校验（非法回调即 TypeError）。
try { vm.createContext({}, { importModuleDynamically: 42 }); console.log("ctor", "NO-THROW"); }
catch (e) { console.log("ctor", e.constructor.name, e.code); }
console.log("done");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "ctx-cb foo true",
        "r1 true",
        "r2 true",
        "m1 TypeError ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING",
        "m2 TypeError ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING",
        "ctor TypeError ERR_INVALID_ARG_TYPE",
        "done",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_tla_faces() {
    // 正常：hasTopLevelAwait 仅看自身源码（依赖 TLA 不传染）。
    // 正常：hasAsyncGraph 看传递闭包（linkRequests/instantiate 与 linker 双路）。
    // 报错：未实例化即抛 STATUS。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const plain = new vm.SourceTextModule("export const a = 1;");
console.log("tla-plain", plain.hasTopLevelAwait());
const tla = new vm.SourceTextModule("export const a = 1;\nawait 0;");
console.log("tla-yes", tla.hasTopLevelAwait());
const fnonly = new vm.SourceTextModule("export async function f() { await 0; }");
console.log("tla-fn", fnonly.hasTopLevelAwait());
try { plain.hasAsyncGraph(); console.log("ag-unlinked", "NO-THROW"); }
catch (e) { console.log("ag-unlinked", e.constructor.name, e.code); }
plain.linkRequests([]);
console.log("tla-prelink", plain.hasTopLevelAwait());
const dep = new vm.SourceTextModule("export const q = 1;\nawait 0;");
dep.linkRequests([]);
const top = new vm.SourceTextModule("export { q } from 'd';");
top.linkRequests([dep]);
top.instantiate();
console.log("ag-dep", dep.hasAsyncGraph(), top.hasAsyncGraph(), top.hasTopLevelAwait());
console.log("done");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "tla-plain false",
        "tla-yes true",
        "tla-fn false",
        "ag-unlinked Error ERR_VM_MODULE_STATUS",
        "tla-prelink false",
        "ag-dep true true false",
        "done",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn vm_dyn_native_guards() {
    // UNSAFE-BOUNDARY 入口守卫（坏 id/坏类型一律 TypeError 包络，不 panic）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import vm from "node:vm";
const bad = (n, f) => { try { f(); console.log(n, "NO-THROW"); } catch (e) { console.log(n, e.constructor.name); } };
bad("regmod-badid", () => __wjs2_vm_dyn_reg_mod("zzz", "main", {}, undefined, undefined, ""));
bad("regfn-badfn", () => __wjs2_vm_dyn_reg_fn(42, "main", undefined, "0", ""));
bad("regns-badns", () => __wjs2_vm_dyn_reg_ns(1, 2));
bad("run-arity", () => __wjs2_vm_dyn_run("1"));
bad("runthis-arity", () => __wjs2_vm_dyn_run_this());
bad("done-badticket", () => __wjs2_vm_dyn_done("999999", undefined, false));
bad("evaldone-badticket", () => __wjs2_vm_dyn_eval_done("999999", undefined, true));
// 正常：空壳注册不多余抛错（undefined 回调即无回调）。
bad("regmod-ok", () => __wjs2_vm_dyn_reg_mod("999998", "main", {}, undefined, undefined, ""));
bad("regfn-ok", () => __wjs2_vm_dyn_reg_fn(async function fn() {}, "main", undefined, "0", ""));
console.log("guards-done");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "regmod-badid Error",
        "regfn-badfn Error",
        "regns-badns Error",
        "run-arity Error",
        "runthis-arity Error",
        "done-badticket Error",
        "evaldone-badticket Error",
        "regmod-ok NO-THROW",
        "regfn-ok NO-THROW",
        "guards-done",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}
