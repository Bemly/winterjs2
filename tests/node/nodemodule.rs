//! tests/node/nodemodule.rs — 对齐 src/builtins/node/nodemodule.rs（node:module createRequire）。

use crate::common::*;
use assert_fs::prelude::*;

#[test]
fn module_create_require() {
    // 正常：createRequire(file URL) 读 CJS/JSON/内建 + resolve；
    // Module.createRequire 同口径；builtinModules 双形/isBuiltin/sync 无操作。
    // 真机实测（node 26.8.1）：createRequire(42) 抛 ERR_INVALID_ARG_VALUE（旧 TYPE 断言翻转）。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("helper.cjs").write_str("module.exports = { v: 41 };\n").unwrap();
    dir.child("data.json").write_str("{\"n\": 7}\n").unwrap();
    let file = dir.child("m.mjs");
    file.write_str(
        r#"
import { createRequire, builtinModules, isBuiltin, Module } from "node:module";
const req = createRequire(import.meta.url);
console.log("cr-cjs", req("./helper.cjs").v);
console.log("cr-json", req("./data.json").n);
console.log("cr-builtin", typeof req("node:path").join);
console.log("cr-resolve", req.resolve("./helper.cjs").endsWith("helper.cjs"));
const req2 = Module.createRequire(import.meta.url);
console.log("mod-cr", req2("./helper.cjs").v);
console.log("bl", builtinModules.includes("node:module") && builtinModules.includes("module") && builtinModules.includes("sys") && isBuiltin("fs") && isBuiltin("node:fs") && isBuiltin("sys") && !isBuiltin("node:nope"));
console.log("sync", Module.syncBuiltinESMExports() === undefined);
try { req("./nope-missing-xyz.cjs"); } catch (e) { console.log("miss", String(e.message).includes("Cannot find module")); }
try { createRequire(42); } catch (e) { console.log("badbase", e.code); }
try { Module.register(); } catch (e) { console.log("reg", e.code); }
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
        "cr-cjs 41",
        "cr-json 7",
        "cr-builtin function",
        "cr-resolve true",
        "mod-cr 41",
        "bl true",
        "sync true",
        "miss true",
        "badbase ERR_INVALID_ARG_VALUE",
        "reg ERR_METHOD_NOT_IMPLEMENTED",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn module_surface_faces() {
    // 正常：Module.wrap/wrapper/_extensions/_cache/_pathCache/globalPaths/_stat/
    // _nodeModulePaths/_resolveLookupPaths/_initPaths/load/_compile + require.cache/
    // extensions 同一视图 + createRequire/require 参数校验 + MODULE_NOT_FOUND 真码。
    // 报错：非法 base/id/空串/缺失模块；边界：dotfile/空 NODE_PATH/包装改写生效。
    // UNSAFE-BOUNDARY 覆盖：throw_named/coded（TypeError 名+code 校验）+
    // cjs_live/cache 四 natives 非法参数不崩（panic 路径）。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("helper.cjs").write_str("module.exports = { v: 5 };\n").unwrap();
    dir.child("loadme.cjs").write_str("module.exports = { loaded: 1 };\n").unwrap();
    let file = dir.child("m.mjs");
    file.write_str(
        r#"
import { createRequire, Module } from "node:module";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
const req = createRequire(import.meta.url);
// wrap/wrapper（默认两片，改写对后续生效由 require 域覆盖，此处只验形态）。
console.log("wrap", typeof Module.wrap === "function" && Module.wrap("1").includes("1"));
console.log("wrapper", Array.isArray(Module.wrapper) && Module.wrapper.length === 2);
// _extensions 即 require.extensions（同一对象）。
console.log("ext-same", Module._extensions === req.extensions);
// _cache 即 require.cache（同一视图，get 陷阱可读）。
const key = req.resolve("./helper.cjs");
req("./helper.cjs");
console.log("cache-hit", req.cache[key] !== undefined && req.cache[key].exports.v === 5);
// _pathCache 可重赋（装饰口径）。
Module._pathCache = { __proto__: null };
console.log("pathcache", typeof Module._pathCache === "object");
// globalPaths 数组 + _initPaths 重算（NODE_PATH 空项丢弃）。
process.env.NODE_PATH = `/tmp/nope-xyz${path.delimiter}${path.delimiter}`;
Module._initPaths();
console.log("globalpaths", Array.isArray(Module.globalPaths) && !Module.globalPaths.includes(""));
// _stat（目录 1/文件 0/缺失负数）。
const d = fs.mkdtempSync(path.join(os.tmpdir(), "wjs-stat-"));
fs.writeFileSync(path.join(d, "f.js"), "1");
console.log("stat", Module._stat(d) === 1 && Module._stat(path.join(d, "f.js")) === 0 && Module._stat(path.join(d, "nope")) < 0);
// _nodeModulePaths（根与跳过 node_modules 套娃）。
console.log("nmp-root", JSON.stringify(Module._nodeModulePaths("/")) === '["/node_modules"]');
console.log("nmp-skip", !Module._nodeModulePaths("/a/node_modules/b").includes("/a/node_modules/node_modules"));
// _resolveLookupPaths（内建 null/相对无父 ['.']/裸名全局表）。
console.log("rlp-builtin", Module._resolveLookupPaths("fs", null) === null);
console.log("rlp-rel", JSON.stringify(Module._resolveLookupPaths("./x", null)) === '["."]');
console.log("rlp-bare", Module._resolveLookupPaths("lodash", null) === null || Array.isArray(Module._resolveLookupPaths("lodash", null)));
// load（实例方法，未加载断言 + filename/paths 回填 + loaded 置位）。
const m = new Module("test-id", null);
m.load(path.join(new URL(import.meta.url).pathname, "..", "loadme.cjs"));
console.log("load", m.exports.loaded === 1 && m.loaded === true);
// _compile（内存码进给定模块）。
const m2 = new Module("c2", null);
m2._compile("module.exports = { c: 9 };", path.join(d, "c2.js"));
console.log("compile", m2.exports.c === 9);
// createRequire 报错（非 file: URL/相对/对象 → ERR_INVALID_ARG_VALUE 真文案）。
for (const bad of ["https://example.com/x", "../", {}]) {
  try { createRequire(bad); console.log("NO-ERR", JSON.stringify(bad)); }
  catch (e) { console.log("cr-bad", e.code === "ERR_INVALID_ARG_VALUE"); }
}
// require 报错（数字 boolean → ERR_INVALID_ARG_TYPE 前缀；空串 → VALUE；缺失 → MODULE_NOT_FOUND 真码）。
for (const v of [1, false]) {
  try { req(v); console.log("NO-ERR"); }
  catch (e) { console.log("req-type", e.code === "ERR_INVALID_ARG_TYPE" && e.message.startsWith('The "id" argument must be of type string. Received')); }
}
try { req(""); } catch (e) { console.log("req-empty", e.code === "ERR_INVALID_ARG_VALUE"); }
try { req("./nope-missing-xyz.cjs"); } catch (e) { console.log("req-miss", e.code === "MODULE_NOT_FOUND" && e.message.startsWith("Cannot find module")); }
// UNSAFE-BOUNDARY panic 路径（非法参数不崩，抛 TypeError）。
for (const [fn, args] of [
  ["__wjs2_cjs_live", [1, 2]],
  ["__wjs2_require_cache_delete", [1]],
  ["__wjs2_require_cache_has", [1]],
  ["__wjs2_require_cache_get", [1]],
  ["__wjs2_require_cache_set", [1, 2]],
]) {
  try { globalThis[fn](...args); console.log("NO-ERR", fn); }
  catch (e) { console.log("panic-ok", fn, e.constructor.name === "Error" || e.constructor.name === "TypeError"); }
}
// TypeError 名校验（throw_named/coded 经 require 报错面）。
try { req(null); } catch (e) { console.log("typename", e.constructor.name === "TypeError"); }
console.log("surface-ok");
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
        "wrap true",
        "wrapper true",
        "ext-same true",
        "cache-hit true",
        "pathcache true",
        "globalpaths true",
        "stat true",
        "nmp-root true",
        "nmp-skip true",
        "rlp-builtin true",
        "rlp-rel true",
        "rlp-bare true",
        "load true",
        "compile true",
        "cr-bad true",
        "cr-bad true",
        "cr-bad true",
        "req-type true",
        "req-type true",
        "req-empty true",
        "req-miss true",
        "typename true",
        "surface-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    // panic-ok 行（5 件，函数名各异；JS 打印布尔真即不崩）。
    for name in [
        "__wjs2_cjs_live",
        "__wjs2_require_cache_delete",
        "__wjs2_require_cache_has",
        "__wjs2_require_cache_get",
        "__wjs2_require_cache_set",
    ] {
        assert!(
            out.lines().any(|l| l == format!("panic-ok {name} true")),
            "missing panic-ok {name}\nout: {out}"
        );
    }
    dir.close().unwrap();
}
