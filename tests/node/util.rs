//! tests/node/util.rs — 对齐 src/builtins/node/util.rs（node:util（含 parseEnv））。

use crate::common::*;
use crate::helpers::*;
use assert_fs::prelude::*;

#[test]
fn util_format_inspect_inherits() {
    // test-util-format.js / test-util-inspect.js 命名子集
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("u.mjs");
    file.write_str(
        r#"import util, { format, inspect, inherits, stripVTControlCharacters } from "node:util";
console.log(format("%s=%d", "a", 1.5), format("%%"), format("%j", { x: 1 }));
console.log(format("%i:%f", 3.9, "1.5"));
console.log(util.inspect({ a: 1, b: [1, 2], c: new Map([["k", 1]]) }));
console.log(util.inspect("it's"), inspect(new Date(0).toISOString() === inspect(new Date(0)) ? { s: "q'a" } : 0));
class Base { base() { return 1; } }
class Sub {}
inherits(Sub, Base);
console.log("inh", new Sub().base(), Sub.super_ === Base);
console.log("strip", stripVTControlCharacters("\u001b[31mred\u001b[39m"), stripVTControlCharacters("plain"));
console.log("promise-inp", util.inspect(Promise.resolve()));
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("a=1.5 %% {\"x\":1}"), "out: {out}");
    assert!(out.contains("3:1"), "out: {out}");
    assert!(
        out.contains("{ a: 1, b: [ 1, 2 ], c: Map(1) { 'k' => 1 } }"),
        "out: {out}"
    );
    assert!(out.contains("inh 1 true"), "out: {out}");
    assert!(out.contains("strip red plain"), "out: {out}");
    assert!(out.contains("Promise { undefined }"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn util_promisify_callbackify_deep_equal() {
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("p.mjs");
    file.write_str(
        r#"import { promisify, callbackify, isDeepStrictEqual } from "node:util";
import assert from "node:assert";
const pb = promisify((a, cb) => cb(null, a * 2));
console.log("prom", await pb(21));
// multi-value + customPromisifyArgs
const pr = promisify((cb) => cb(null, "bytesRead", "buffer"));
pr[Symbol.for("nodejs.util.promisify.custom")] // 不设 custom
console.log("prom-obj", JSON.stringify(await (async () => {
  const fn = promisify((cb) => cb(null, 1, 2));
  fn[Symbol("customPromisifyArgs")] = ["a", "b"];
  const fn2 = promisify((cb) => { fn2args(cb); }) ; return 0;
})()));
// promisify.custom 通道
const raw = (a, cb) => cb(null, a);
raw[Symbol.for("nodejs.util.promisify.custom")] = (a) => Promise.resolve(a + 100);
console.log("custom", await promisify(raw)(1));
// callbackify 正常 + falsy rejection
const cf = callbackify(async (n) => n + 1);
const [err, val] = await new Promise((res) => cf(1, (e, v) => res([e, v])));
console.log("cb", err, val);
const cf2 = callbackify(async () => { throw null; });
await new Promise((res) => cf2((e) => res(console.log("falsy", e?.code, e instanceof Error))));
// isDeepStrictEqual 语义（test-assert 依赖同款）
console.log("eq", isDeepStrictEqual({ a: [1, { b: 2 }] }, { a: [1, { b: 2 }] }));
console.log("neq0", isDeepStrictEqual(1, "1"), isDeepStrictEqual({ a: 1 }, { a: 1, b: undefined }));
console.log("nan0", isDeepStrictEqual(NaN, NaN), isDeepStrictEqual(0, -0));
console.log("map", isDeepStrictEqual(new Map([[1, "a"]]), new Map([[1, "a"]])));
console.log("proto", isDeepStrictEqual(Object.create(null, { x: { value: 1, enumerable: true } }), { x: 1 }));
// 边界：非函数入参
try { promisify(42); } catch (e) { console.log("err1", e.code, e.message.includes("must be of type function")); }
try { callbackify("x"); } catch (e) { console.log("err2", e.code); }
assert.strictEqual(typeof promisify.custom, "symbol");
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("prom 42"), "out: {out}");
    assert!(out.contains("custom 101"), "out: {out}");
    assert!(out.contains("cb null 2"), "out: {out}");
    assert!(
        out.contains("falsy ERR_FALSY_VALUE_REJECTION true"),
        "out: {out}"
    );
    assert!(out.contains("eq true"), "out: {out}");
    assert!(out.contains("neq0 false false"), "out: {out}");
    assert!(out.contains("nan0 true false"), "out: {out}");
    assert!(
        out.contains("map true") && out.contains("proto false"),
        "out: {out}"
    );
    assert!(out.contains("err1 ERR_INVALID_ARG_TYPE true"), "out: {out}");
    assert!(out.contains("err2 ERR_INVALID_ARG_TYPE"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn util_parse_env() {
    // 真机差分钉住（node 26.8.2 四组探针全同）：引号/注释/export/重复/排序/多行。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("pe.mjs");
    file.write_str(
        r##"
import { parseEnv } from "node:util";
console.log(JSON.stringify(parseEnv("B=2\nA=1")));
console.log(JSON.stringify(parseEnv("# c\nexport C=3\nD='a#b'\nE=\"x\\nY\"\nF=v # t\nG=\"m\nn\"\nH=\"q\"q")));
console.log(JSON.stringify(parseEnv("X=one\nX=two\n=v\nnoeq")));
try { parseEnv(42); } catch (e) { console.log("pe-t", e.code); }
"##,
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
        "{\"A\":\"1\",\"B\":\"2\"}",
        "{\"C\":\"3\",\"D\":\"a#b\",\"E\":\"x\\nY\",\"F\":\"v\",\"G\":\"m\\nn\",\"H\":\"q\"}",
        "{\"X\":\"two\"}",
        "pe-t ERR_INVALID_ARG_TYPE",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn util_parse_env_backticks() {
    // 正常：backtick 与单引号同为原文（剥外层、不展 `\n`、内层异种引号保留、
    // 跨行吞行）；边界：空 backtick 对、闭合后尾文丢弃；
    // 报错：无（非法输入沿用 ARG_TYPE）；null 原型：__proto__ 键自有化。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r##"
import { parseEnv } from "node:util";
const o = parseEnv("A=`x`\nB=``\nC=`a\"b'c`\nD=`m\nn`\nE=`v` # t\n__proto__=p");
console.log("vals:" + JSON.stringify([o.A, o.B, o.C, o.D, o.E, o["__proto__"]]));
console.log("nullproto:" + (Object.getPrototypeOf(o) === null));
console.log("own:" + Object.prototype.hasOwnProperty.call(o, "__proto__"));
console.log("pollute:" + (({}).polluted === undefined));
"##,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "vals:[\"x\",\"\",\"a\\\"b'c\",\"m\\nn\",\"v\",\"p\"]",
        "nullproto:true",
        "own:true",
        "pollute:true",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn util_parse_args_mime_errno() {
    // 10a：parseArgs/MIMEType/getSystemError*（真机 26.8.2 探针结论转断言）。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("u10.mjs");
    file.write_str(
        r##"
import util, { parseArgs, MIMEType, getSystemErrorName, getSystemErrorMessage, getSystemErrorMap } from "node:util";
// parseArgs 正常
console.log("pa-base", JSON.stringify(parseArgs({ args: ["--port", "8080", "--verbose", "pos1"], options: { port: { type: "string" }, verbose: { type: "boolean" } }, allowPositionals: true })));
console.log("pa-def", JSON.stringify(parseArgs({ args: [], options: { p: { type: "string", default: "d" }, m: { type: "string", multiple: true, default: ["a"] } } })));
console.log("pa-multi", JSON.stringify(parseArgs({ args: ["--m", "1", "--m", "2"], options: { m: { type: "string", multiple: true } } }).values));
console.log("pa-short", JSON.stringify(parseArgs({ args: ["-ab", "-p8080"], options: { a: { type: "boolean", short: "a" }, b: { type: "boolean", short: "b" }, port: { type: "string", short: "p" } } }).values));
console.log("pa-dd", JSON.stringify(parseArgs({ args: ["--", "--a"], options: { a: { type: "boolean" } }, allowPositionals: true }).positionals));
console.log("pa-ns", JSON.stringify(parseArgs({ args: ["--x=1", "-z"], options: {}, strict: false })));
console.log("pa-tok", JSON.stringify(parseArgs({ args: ["--port=8080", "pos"], options: { port: { type: "string" } }, allowPositionals: true, tokens: true }).tokens));
// parseArgs 报错
for (const [tag, fn] of [
  ["pa-unknown", () => parseArgs({ args: ["--nope"], options: { a: { type: "boolean" } } })],
  ["pa-noval", () => parseArgs({ args: ["--a"], options: { a: { type: "string" } } })],
  ["pa-pos", () => parseArgs({ args: ["pos"], options: {} })],
  ["pa-boolval", () => parseArgs({ args: ["--v=false"], options: { v: { type: "boolean" } } })],
  ["pa-shortmissing", () => parseArgs({ args: ["-p"], options: { port: { type: "string", short: "p" } } })],
  ["pa-badtype", () => parseArgs({ args: [], options: { a: { type: "number" } } })],
  ["pa-badshort", () => parseArgs({ args: [], options: { a: { type: "boolean", short: "ab" } } })],
  ["pa-baddef", () => parseArgs({ args: [], options: { m: { type: "boolean", multiple: true, default: true } } })],
]) {
  try { fn(); console.log(tag, "NO-THROW"); }
  catch (e) { console.log(tag, e.code, e instanceof TypeError, e.message); }
}
// MIME 正常
const m = new MIMEType("Text/HTML; Charset=UTF-8; x=1");
console.log("mi-base", m.type, m.subtype, m.essence, m.params.get("charset"), String(m));
console.log("mi-set", (() => { m.type = "application"; m.subtype = "json"; return String(m); })());
console.log("mi-ess", (() => { m.essence = "text/plain"; return String(m); })());
console.log("mi-params", (() => { const p = new MIMEType("text/html; a=1; b=2").params; return [String(p.size), p.get("a"), p.has("c")].join(",") + " " + [...p.keys()].join(",") + " " + String(p); })());
console.log("mi-del", (() => { const p = new MIMEType("text/html; a=1").params; return p.delete("a") + "/" + p.delete("z"); })());
console.log("mi-quote", String(new MIMEType("text/html; a=\"b c\"")));
// MIME 报错
for (const [tag, fn] of [
  ["mi-t1", () => new MIMEType("nope")],
  ["mi-t2", () => new MIMEType("A B/c")],
  ["mi-t3", () => new MIMEType("a/")],
  ["mi-t4", () => { new MIMEType("text/html").subtype = "BAD TYPE"; }],
  ["mi-t5", () => new MIMEType("text/html").params.set("", "x")],
  ["mi-t6", () => new MIMEType(42)],
]) {
  try { fn(); console.log(tag, "NO-THROW"); }
  catch (e) { console.log(tag, e.code, e instanceof TypeError, e.message); }
}
// errno 正常 + 报错 + 边界
console.log("en-size", getSystemErrorMap().size);
console.log("en-name", getSystemErrorName(-2), getSystemErrorName(-9999));
console.log("en-msg", getSystemErrorMessage(-2), getSystemErrorMessage(-9999));
console.log("en-ident", getSystemErrorMap() === getSystemErrorMap());
console.log("en-def", util.getSystemErrorName(-13));
for (const [tag, fn] of [
  ["en-t0", () => getSystemErrorName(0)],
  ["en-tm", () => getSystemErrorMessage(0)],
  ["en-ts", () => getSystemErrorName("x")],
]) {
  try { fn(); console.log(tag, "NO-THROW"); }
  catch (e) { console.log(tag, e.code, e instanceof RangeError, e.message); }
}
"##,
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
        "pa-base {\"values\":{\"port\":\"8080\",\"verbose\":true},\"positionals\":[\"pos1\"]}",
        "pa-def {\"values\":{\"p\":\"d\",\"m\":[\"a\"]},\"positionals\":[]}",
        "pa-multi {\"m\":[\"1\",\"2\"]}",
        "pa-short {\"a\":true,\"b\":true,\"port\":\"8080\"}",
        "pa-dd [\"--a\"]",
        "pa-ns {\"values\":{\"x\":\"1\",\"z\":true},\"positionals\":[]}",
        "pa-tok [{\"kind\":\"option\",\"name\":\"port\",\"rawName\":\"--port\",\"index\":0,\"value\":\"8080\",\"inlineValue\":true},{\"kind\":\"positional\",\"index\":1,\"value\":\"pos\"}]",
        "pa-unknown ERR_PARSE_ARGS_UNKNOWN_OPTION true Unknown option '--nope'",
        "pa-noval ERR_PARSE_ARGS_INVALID_OPTION_VALUE true Option '--a <value>' argument missing",
        "pa-pos ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL true Unexpected argument 'pos'. This command does not take positional arguments",
        "pa-boolval ERR_PARSE_ARGS_INVALID_OPTION_VALUE true Option '--v' does not take an argument",
        "pa-shortmissing ERR_PARSE_ARGS_INVALID_OPTION_VALUE true Option '-p, --port <value>' argument missing",
        "pa-badtype ERR_INVALID_ARG_TYPE true The \"options.a.type\" property must be ('string|boolean'). Received type string ('number')",
        "pa-badshort ERR_INVALID_ARG_VALUE true The property 'options.a.short' must be a single character. Received 'ab'",
        "pa-baddef ERR_INVALID_ARG_TYPE true The \"options.m.default\" property must be an instance of Array. Received type boolean (true)",
        "mi-base text html text/html UTF-8 text/html;charset=UTF-8;x=1",
        "mi-set application/json;charset=UTF-8;x=1",
        "mi-ess application/json;charset=UTF-8;x=1",
        "mi-params undefined,1,false a,b a=1;b=2",
        "mi-del undefined/undefined",
        "mi-quote text/html;a=\"b c\"",
        "mi-t1 ERR_INVALID_MIME_SYNTAX true The MIME syntax for a type in \"nope\" is invalid",
        "mi-t2 ERR_INVALID_MIME_SYNTAX true The MIME syntax for a type in \"A B/c\" is invalid at 1",
        "mi-t3 ERR_INVALID_MIME_SYNTAX true The MIME syntax for a subtype in \"a/\" is invalid",
        "mi-t4 ERR_INVALID_MIME_SYNTAX true The MIME syntax for a subtype in \"BAD TYPE\" is invalid at 3",
        "mi-t5 ERR_INVALID_MIME_SYNTAX true The MIME syntax for a parameter name in \"\" is invalid",
        "mi-t6 ERR_INVALID_ARG_TYPE true The \"input\" argument must be of type string. Received type number (42)",
        "en-size 85",
        "en-name ENOENT Unknown system error -9999",
        "en-msg no such file or directory Unknown system error -9999",
        "en-ident false",
        "en-def EACCES",
        "en-t0 ERR_OUT_OF_RANGE true The value of \"err\" is out of range. It must be a negative integer. Received 0",
        "en-tm ERR_OUT_OF_RANGE true The value of \"err\" is out of range. It must be a negative integer. Received 0",
        "en-ts ERR_INVALID_ARG_TYPE false The \"err\" argument must be of type number. Received type string ('x')",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn sys_alias() {
    // 10a：`node:sys` 是 util 的废弃别名——import 与 require 同实例，无运行时警告。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("s.mjs");
    file.write_str(
        r#"import sysDefault, { format } from "node:sys";
import utilDefault from "node:util";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const sysReq = require("sys");
const utilReq = require("util");
console.log("fmt", sysDefault.format("%s", "ok"), format("%d", 7));
console.log("same-import", sysDefault === utilDefault);
console.log("same-require", sysReq === utilReq);
console.log("same-cross", sysDefault === utilReq);
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
        "fmt ok 7",
        "same-import true",
        "same-require true",
        "same-cross true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn util_getcallsites() {
    // 10f：util.getCallSites（SM 栈解析；test/common mustNotCall 前置）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import { getCallSites } from "node:util";
function inner() { return getCallSites(); }
function outer() { return inner(); }
const sites = outer();
console.log("len", sites.length >= 2);
console.log("frame0", sites[0].functionName, typeof sites[0].scriptName, typeof sites[0].lineNumber);
console.log("frame1", sites[1].functionName === "outer", sites[1].scriptName.endsWith("p.mjs"));
console.log("methods", typeof sites[0].getFileName, typeof sites[0].getLineNumber);
"#,
    );
    assert!(out.contains("len true"), "out: {out}");
    assert!(out.contains("frame0 inner string number"), "out: {out}");
    assert!(out.contains("frame1 true true"), "out: {out}");
    assert!(out.contains("methods function function"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn util_getcallsites_options() {
    // 正常：frameCount 截断、对象形态 options、scriptId string；
    // 报错：frameCount 非整数/越界 OOR、非法 options 体/第二参 ARG_TYPE、
    // sourceMap 非布尔 ARG_TYPE；边界：stackTraceLimit=0 不影响捕获、
    // host 帧（node:util 自身）恒不在 [0]。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { getCallSites } from "node:util";
function inner() { return getCallSites(); }
const t = (f) => { try { f(); return "NO-THROW"; } catch (e) { return e.code; } };
console.log("host0:" + (inner()[0].functionName === "inner"));
console.log("cap:" + (inner().length >= 1 && getCallSites(1).length === 1));
console.log("sid:" + (typeof inner()[0].scriptId === "string"));
console.log("bad36:" + t(() => getCallSites(3.6)));
console.log("bad201:" + t(() => getCallSites(201)));
console.log("badneg:" + t(() => getCallSites(-1)));
console.log("badarr:" + t(() => getCallSites([])));
console.log("badobj2:" + t(() => getCallSites({}, {})));
console.log("bad210:" + t(() => getCallSites(10, 10)));
console.log("badsm:" + t(() => getCallSites({ sourceMap: 1 })));
console.log("opts-ok:" + (getCallSites({}).length >= 1 && getCallSites(1, {}).length === 1));
const lim = Error.stackTraceLimit;
Error.stackTraceLimit = 0;
console.log("limit0:" + (getCallSites().length > 0));
Error.stackTraceLimit = lim;
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "host0:true",
        "cap:true",
        "sid:true",
        "bad36:ERR_OUT_OF_RANGE",
        "bad201:ERR_OUT_OF_RANGE",
        "badneg:ERR_OUT_OF_RANGE",
        "badarr:ERR_INVALID_ARG_TYPE",
        "badobj2:ERR_INVALID_ARG_TYPE",
        "bad210:ERR_INVALID_ARG_TYPE",
        "badsm:ERR_INVALID_ARG_TYPE",
        "opts-ok:true",
        "limit0:true",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn util_deep_separator_depth() {
    // 10f 套件点名修：skipPrototype 第三参 + 装箱槽判定 + numericSeparator + %s + depth。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import util from "node:util";
function A(v) { this.v = v; }
function B(v) { this.v = v; }
console.log("skip", util.isDeepStrictEqual(new A(1), new B(1)), util.isDeepStrictEqual(new A(1), new B(1), true));
const spoof = new Boolean(true);
Object.defineProperty(spoof, Symbol.toStringTag, { value: "String" });
Object.setPrototypeOf(spoof, String.prototype);
console.log("spoof", util.isDeepStrictEqual(spoof, new String("true")));
const t1 = new Uint8Array(2), t2 = new Uint8Array(2);
const s = Symbol();
t1[s] = 1; t2[s] = 2;
console.log("ta-keys", util.isDeepStrictEqual(t1, t2));
console.log("ta-u8buf", util.isDeepStrictEqual(new Uint8Array([1]), Buffer.from([1]), true));
util.inspect.defaultOptions.numericSeparator = true;
console.log("sep", util.inspect(1234567), util.format("%d", 1234567));
util.inspect.defaultOptions.numericSeparator = false;
console.log("s-custom", util.format("%s", { toString() { return "Foo"; } }));
console.log("s-plain", util.format("%s", { a: [1, 2, 3] }));
console.log("s-buf", util.format("%s", Buffer.from("hi")));
console.log("depth0", util.inspect({ a: [1] }, { depth: 0 }));
"#,
    );
    assert!(out.contains("skip false true"), "out: {out}");
    assert!(out.contains("spoof false"), "out: {out}");
    assert!(out.contains("ta-keys false"), "out: {out}");
    assert!(out.contains("ta-u8buf true"), "out: {out}");
    assert!(out.contains("sep 1_234_567 1_234_567"), "out: {out}");
    assert!(out.contains("s-custom Foo"), "out: {out}");
    assert!(out.contains("s-plain { a: [Array] }"), "out: {out}");
    assert!(out.contains("s-buf hi"), "out: {out}");
    assert!(out.contains("depth0 { a: [Array] }"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn inspect_control_escapes() {
    // P2-process R6 附带：inspect 控制字符转义与真机 meta 表一致
    //（\0 → \x00，\x07 → \x07，\v → \x0B，\x1b → \x1B；旧表误用 \0/\a/\v/\e）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = stdout_of(&mut winterjs2().args([
        "--eval",
        "const util = require('node:util');\
         console.log(JSON.stringify(util.inspect('a\\0b')));\
         console.log(JSON.stringify(util.inspect('\\x07')));\
         console.log(JSON.stringify(util.inspect('\\v')));\
         console.log(JSON.stringify(util.inspect('\\x1b')));",
    ]));
    for line in [
        "\"'a\\\\x00b'\"",
        "\"'\\\\x07'\"",
        "\"'\\\\x0B'\"",
        "\"'\\\\x1B'\"",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn util_mime_type_parse_and_params() {
    // 正常：构造/往返；报错：非法本质抛 ERR_INVALID_MIME_SYNTAX（\v\f/空格前导）、
    // 非串参抛 ERR_INVALID_ARG_TYPE；边界：parse 非法回 null（Symbol 透出）、
    // 重名参数首个胜出、非法值整项丢弃、quoted 转义。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { MIMEType } from 'node:util';
const t = (f) => { try { return String(f()); } catch (e) { return e.constructor.name + ':' + e.code; } };
console.log('basic:' + String(new MIMEType('text/html; charset=GBK')));
console.log('dup:' + String(MIMEType.parse('text/html;charset=gbk;charset=windows-1255')));
console.log('drop:' + String(MIMEType.parse('text/html;charset =gbk')));
console.log('lead-ws:' + String(MIMEType.parse('text/html; charset=gbk')));
console.log('quoted:' + String(MIMEType.parse('text/html;charset="gbk("')));
console.log('empty-val:' + String(MIMEType.parse('text/html;foo=;bar=baz')));
console.log('ess-vt:' + t(() => new MIMEType('\x0bx/html')));
console.log('ess-sp:' + t(() => new MIMEType('text /html')));
console.log('nonstring:' + t(() => new MIMEType(42)));
console.log('parse-null:' + (MIMEType.parse('not a mime') === null));
console.log('parse-sym:' + t(() => MIMEType.parse(Symbol())));
console.log('essence:' + new MIMEType('TEXT/HTML;Charset=GBK').essence);
"#,
    );
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains("basic:text/html;charset=GBK"), "out: {out}");
    assert!(out.contains("dup:text/html;charset=gbk"), "out: {out}");
    assert!(out.contains("drop:text/html"), "out: {out}");
    assert!(out.contains("lead-ws:text/html;charset=gbk"), "out: {out}");
    assert!(out.contains("quoted:text/html;charset=\"gbk(\""), "out: {out}");
    assert!(out.contains("empty-val:text/html;bar=baz"), "out: {out}");
    assert!(out.contains("ess-vt:TypeError:ERR_INVALID_MIME_SYNTAX"), "out: {out}");
    assert!(out.contains("ess-sp:TypeError:ERR_INVALID_MIME_SYNTAX"), "out: {out}");
    assert!(out.contains("nonstring:TypeError:ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("parse-null:true"), "out: {out}");
    assert!(out.contains("parse-sym:TypeError:undefined"), "out: {out}");
    assert!(out.contains("essence:text/html"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn util_small_validations() {
    // 正常：styleText 嵌套/hex/别名/none、convert-signal 取值、sleep 有效值
    // no-op、pendingDeprecate length 保形、promisify custom 匿名回填原名；
    // 报错：styleText 非法格式 ARG_VALUE + 非法 stream ARG_TYPE、signal 非法值
    // ARG_VALUE、sleep 非数 ARG_TYPE/越界 OOR；边界：validateStream:false 跳
    // 过 stream 校验、DEP0174 警告照发。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r##"
import util, { styleText, convertProcessSignalToExitCode, deprecate, promisify } from "node:util";
import internalUtil from "node:internal/util";
const t = (f) => { try { f(); return "NO-THROW"; } catch (e) { return e.constructor.name + ":" + e.code; } };
console.log("style-nest:" + JSON.stringify(styleText("red", "A" + styleText("blue", "B", { validateStream: false }) + "C", { validateStream: false })));
console.log("style-hex:" + JSON.stringify(styleText("#ff0000", "T", { validateStream: false })));
console.log("style-alias:" + (styleText("grey", "x", { validateStream: false }) === styleText("gray", "x", { validateStream: false })));
console.log("style-none:" + JSON.stringify(styleText("none", "test", { validateStream: false })));
console.log("style-badfmt:" + t(() => styleText("invalid", "text")));
console.log("style-badstream:" + t(() => styleText("red", "text", { stream: {} })));
console.log("style-novalidate:" + JSON.stringify(styleText("red", "text", { stream: {}, validateStream: false })));
console.log("signal-ok:" + convertProcessSignalToExitCode("SIGTERM"));
console.log("signal-bad:" + t(() => convertProcessSignalToExitCode("INVALID")));
console.log("signal-badtype:" + t(() => convertProcessSignalToExitCode(123)));
console.log("sleep-ok:" + (internalUtil.sleep(5), "no-op"));
console.log("sleep-argtype:" + t(() => internalUtil.sleep("x")));
console.log("sleep-oor:" + t(() => internalUtil.sleep(-1)));
console.log("pending-len:" + (internalUtil.pendingDeprecate(function (a, b) {}).length === 2));
function origFn() {}
origFn[promisify.custom] = () => {};
// 真机口径：custom 原样返回不改名，匿名 custom 名即 ''（test-util-promisify-custom-names
// 轮撤除回填 fallback；各域 custom 已在定义处正名）。
console.log("prom-name:" + JSON.stringify(promisify(origFn).name));
let depCode = "";
process.on("warning", (w) => { depCode = w.code; });
const p = promisify(async () => 1);
p().catch(() => {});
await new Promise((r) => setTimeout(r, 50));
console.log("dep0174:" + depCode);
"##,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "style-nest:\"\\u001b[31mA\\u001b[34mB\\u001b[31mC\\u001b[39m\"",
        "style-hex:\"\\u001b[38;2;255;0;0mT\\u001b[39m\"",
        "style-alias:true",
        "style-none:\"test\"",
        "style-badfmt:TypeError:ERR_INVALID_ARG_VALUE",
        "style-badstream:TypeError:ERR_INVALID_ARG_TYPE",
        "style-novalidate:\"\\u001b[31mtext\\u001b[39m\"",
        "signal-ok:143",
        "signal-bad:TypeError:ERR_INVALID_ARG_VALUE",
        "signal-badtype:TypeError:ERR_INVALID_ARG_VALUE",
        "sleep-ok:no-op",
        "sleep-argtype:TypeError:ERR_INVALID_ARG_TYPE",
        "sleep-oor:RangeError:ERR_OUT_OF_RANGE",
        "pending-len:true",
        "prom-name:\"\"",
        "dep0174:DEP0174",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn util_promisify_custom_names_faces() {
    // 正常：test-util-promisify-custom-names.mjs 全量名断言（exists/question/
    // finished/eos 别名/pipeline/setImmediate/setTimeout/exec/execFile/connect，
    // 真机实测钉值）+ custom 恒等（promisify(finished)===stream/promises.finished）；
    // 报错：custom 非函数抛 ERR_INVALID_ARG_TYPE（真机口径）；
    // 边界：反复 promisify 同一对象恒等、原函数名不被改写（旧 fallback 曾全局
    // 改名污染 custom，回归钉死）。
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("p.mjs");
    file.write_str(
        r#"import { promisify } from "node:util";
import assert from "node:assert";
import fs from "node:fs";
import readline from "node:readline";
import stream from "node:stream";
import timers from "node:timers";
import child_process from "node:child_process";
import * as promises from "node:stream/promises";
import http2 from "node:http2";
const names = [
  ["exists", promisify(fs.exists).name],
  ["question", promisify(readline.Interface.prototype.question).name],
  ["finished", promisify(stream.finished).name],
  ["pipeline", promisify(stream.pipeline).name],
  ["setImmediate", promisify(timers.setImmediate).name],
  ["setTimeout", promisify(timers.setTimeout).name],
  ["exec", promisify(child_process.exec).name],
  ["execFile", promisify(child_process.execFile).name],
  ["connect", promisify(http2.connect).name],
];
for (const [want, got] of names) assert.strictEqual(got, want);
console.log("names ok");
assert.strictEqual(promisify(stream.finished), promises.finished);
assert.strictEqual(promisify(stream.finished), promisify(stream.finished));
console.log("identity ok");
console.log("unmutated", stream.finished.name, promises.finished.name);
try { promisify(Object.assign(function f(cb) { cb(null); }, { [promisify.custom]: 42 })); }
catch (e) { console.log("badcustom", e.code); }
console.log("exec-arity", child_process.execFile.name, child_process.exec.name);
"#,
    )
    .unwrap();
    let out = winterjs2()
        .args(["--run", file.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "names ok",
        "identity ok",
        "unmutated eos finished",
        "badcustom ERR_INVALID_ARG_TYPE",
        "exec-arity execFile exec",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}
