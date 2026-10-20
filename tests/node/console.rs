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

#[test]
fn console_write_error_swallow() {
    // 正常：ignoreErrors 缺省 true，同步 throw/async 回调错/throw null 一律吞；
    // 报错：ignoreErrors=false 即透传同步原错；边界：真栈溢出穿透为 RangeError
    //（本仓引擎 InternalError 转类），伪造 RangeError 文案仍吞（真机同形）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Console } from "node:console";
import { Writable } from "node:stream";
const syncBoom = new Writable({ write(c, e, cb) { throw new Error("sync-boom"); } });
new Console({ stdout: syncBoom }).log("x");
console.log("sync-swallow ok");
const asyncBoom = new Writable({ write(c, e, cb) { setImmediate(() => cb(new Error("async-boom"))); } });
new Console(asyncBoom, asyncBoom, true).log("y");
setTimeout(() => console.log("async-swallow ok"), 50);
const primBoom = new Writable({ write() { throw null; } });
new Console({ stdout: primBoom }).log("z");
console.log("prim-swallow ok");
const strict = new Writable({ write() { throw new Error("out"); } });
try { new Console({ stdout: strict, ignoreErrors: false }).log("w"); console.log("strict FAIL"); }
catch (e) { console.log("strict-throw", e.message); }
const fakeRange = new Writable({ write() { throw new RangeError("custom-range"); } });
new Console({ stdout: fakeRange }).log("v");
console.log("fake-range-swallow ok");
const overflow = new Writable({ write: function ow(...a) { return ow(...a); } });
try { new Console({ stdout: overflow }).log("u"); console.log("overflow FAIL"); }
catch (e) { console.log("overflow-throw", e.name); }
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "sync-swallow ok",
        "async-swallow ok",
        "prim-swallow ok",
        "strict-throw out",
        "fake-range-swallow ok",
        "overflow-throw RangeError",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn console_options_surface() {
    // 正常：colorMode:true 非 TTY 强制上色、Map 形态 per-stream 选项、
    // dir 关 customInspect；报错：colorMode 白名单/互斥对/组缩进/选项体
    // 六条文案逐字（真机实测）；边界：auto 非 TTY 无色、组缩进自定义宽度。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Console } from "node:console";
import { Writable, PassThrough } from "node:stream";
const s = new Writable({ write(c, e, cb) { cb(); } });
const bad = [
  () => new Console({ stdout: s, ignoreErrors: false, colorMode: 0 }),
  () => new Console({ stdout: s, ignoreErrors: false, colorMode: "auto", inspectOptions: { colors: false } }),
  () => new Console({ stdout: s, groupIndentation: -1 }),
  () => new Console({ stdout: s, groupIndentation: 1.5 }),
  () => new Console({ stdout: s, groupIndentation: "x" }),
  () => new Console({ stdout: s, inspectOptions: 42 }),
];
const codes = [];
for (const t of bad) { try { t(); codes.push("NO-THROW"); } catch (e) { codes.push(`${e.code}|${e.message}`); } }
console.log("codes", JSON.stringify(codes));
let seen = "";
const s2 = new Writable({ write(c, e, cb) { seen += String(c); cb(); } });
new Console({ stdout: s2, colorMode: true }).log({ a: 2 });
console.log("forced", JSON.stringify(seen));
seen = "";
new Console({ stdout: s2, colorMode: "auto" }).log({ a: 2 });
console.log("auto-plain", JSON.stringify(seen));
const r = { [Symbol.for("nodejs.util.inspect.custom")]() { return "CUSTOM"; } };
seen = "";
new Console({ stdout: s2, colorMode: true }).dir(r);
console.log("dirraw", JSON.stringify(seen).includes("CUSTOM") ? "CUSTOM-LEAK" : "no-custom");
const so = new PassThrough().setEncoding("utf8");
const se = new PassThrough().setEncoding("utf8");
new Console({ stdout: so, stderr: se,
  inspectOptions: new Map([[so, { colors: true }], [se, { colors: false }]]) })
  .log("Hello", 42);
new Console({ stdout: so, stderr: se,
  inspectOptions: new Map([[so, { colors: true }], [se, { colors: false }]]) })
  .warn("Hello", 42);
console.log("per-stream", JSON.stringify(so.read()), JSON.stringify(se.read()));
seen = "";
const g = new Console({ stdout: s2, groupIndentation: 4 });
g.group("L");
g.log("deep");
console.log("grp", JSON.stringify(seen));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "ERR_INVALID_ARG_VALUE|The argument 'colorMode' must be one of: 'auto', true, false. Received 0",
        "ERR_INCOMPATIBLE_OPTION_PAIR|Option \\\"options.inspectOptions.color\\\" cannot be used",
        "ERR_OUT_OF_RANGE|The value of \\\"groupIndentation\\\" is out of range. It must be >= 0 && <= 1000. Received -1",
        "ERR_OUT_OF_RANGE|The value of \\\"groupIndentation\\\" is out of range. It must be an integer. Received 1.5",
        "ERR_INVALID_ARG_TYPE|The \\\"groupIndentation\\\" argument must be of type number. Received type string ('x')",
        "ERR_INVALID_ARG_TYPE|The \\\"options.inspectOptions\\\" property must be of type object. Received type number (42)",
        "forced \"{ a: \\u001b[33m2\\u001b[39m }\\n\"",
        "auto-plain \"{ a: 2 }\\n\"",
        "dirraw no-custom",
        "per-stream \"Hello \\u001b[33m42\\u001b[39m\\n\" \"Hello 42\\n\"",
        "grp \"L\\n    deep\\n\"",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn console_debuglog_format_time_faces() {
    // 正常：test-console-formatTime 8 断言逐字（node 真机实测同值）+
    // node:internal/util/debuglog 与裸 internal/util/debuglog 同一实现；
    // 报错：Symbol 入参 TypeError（真机同形）；边界：0/负数/亚毫秒舍入/
    // 分·时整界/NaN（真机实测钉值）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { formatTime } from "node:internal/util/debuglog";
import bare from "internal/util/debuglog";
import assert from "node:assert";
assert.strictEqual(formatTime(100.0096), "100.01ms");
assert.strictEqual(formatTime(100.0115), "100.011ms");
assert.strictEqual(formatTime(1500.04), "1.500s");
assert.strictEqual(formatTime(1000.056), "1.000s");
assert.strictEqual(formatTime(60300.3), "1:00.300 (m:ss.mmm)");
assert.strictEqual(formatTime(4000457.4), "1:06:40.457 (h:mm:ss.mmm)");
assert.strictEqual(formatTime(3601310.4), "1:00:01.310 (h:mm:ss.mmm)");
assert.strictEqual(formatTime(3213601017.6), "892:40:01.018 (h:mm:ss.mmm)");
console.log("suite8 ok");
console.log("bare", typeof bare.formatTime, bare.formatTime === formatTime);
console.log("edge", formatTime(0), formatTime(-5), formatTime(0.0004), formatTime(0.0005));
console.log("round", formatTime(999.9999), formatTime(60000), formatTime(3600000), formatTime(NaN));
try { formatTime(Symbol("x")); console.log("sym-no-throw"); }
catch (e) { console.log("sym-throw", e.constructor.name); }
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "suite8 ok",
        "bare function true",
        "edge 0ms -5ms 0ms 0.001ms",
        "round 1000ms 1:00.000 (m:ss.mmm) 1:00:00.000 (h:mm:ss.mmm) NaNms",
        "sym-throw TypeError",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn console_timer_count_warning_faces() {
    // 正常：实例 time/timeEnd 输出 `label: ms` 形 + timeLog 余参续接 +
    // count 递增/countReset 命中删 + dup 不重置 + `__proto__`/归一标签
    // （`[]`→`""`、`{}`→`[object Object]`、缺省→`default`）；
    // 报错：time/timeEnd/count 的 Symbol 入参抛 TypeError（真机同形）；
    // 边界：8 警告文本逐字（warning 事件收，真机 Warning 无 code）+
    // timeEnd 无死链（size 回落）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { Console } from "node:console";
import { Writable } from "node:stream";
let seen = "";
const ws = new Writable({ write(c, e, cb) { seen += String(c); cb(); } });
const warns = [];
process.on("warning", (w) => warns.push(`${w.name}|${w.code}|${w.message}`));
const k = new Console(ws);
k.time("t"); k.timeEnd("t");
k.time("log1"); k.timeLog("log1"); k.timeLog("log1", "test"); k.timeLog("log1", {}, [1, 2, 3]); k.timeEnd("log1");
k.count("a"); k.count("a"); k.countReset("a"); k.count("a");
k.time("__proto__"); k.timeEnd("__proto__");
k.time([]); k.timeEnd([]);
k.time({}); k.timeEnd({});
k.time(null); k.timeEnd(null);
k.time(undefined); k.timeEnd("default");
k.time("dup"); k.time("dup"); k.timeEnd("dup");
const size0 = k._times.size;
k.time("l1"); k.time("l2"); k.timeEnd("l1"); k.timeEnd("l2");
console.log("deadlinks", k._times.size === size0);
for (const l of seen.split("\n")) { if (l !== "") console.log("SL|" + l); }
for (const f of [() => k.time(Symbol("x")), () => k.timeEnd(Symbol("x")), () => k.count(Symbol("x"))]) {
  try { f(); console.log("sym-no-throw"); }
  catch (e) { console.log("sym-throw", e.constructor.name); }
}
try { k.countReset(Symbol("x")); console.log("sym-no-throw"); }
catch (e) { console.log("sym-throw", e.constructor.name); }
k.countReset("nope"); k.timeLog("nope2"); k.timeEnd("nope3");
k.countReset(); k.timeLog(); k.timeEnd(); k.time(); k.time();
await new Promise((r) => setTimeout(r, 100));
console.log("warns", JSON.stringify(warns));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "deadlinks true",
        "sym-throw TypeError",
        "sym-throw TypeError",
        "sym-throw TypeError",
        "sym-throw TypeError",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    for re in [
        "t: \\d+(\\.\\d{1,3})?(ms|s)",
        "log1: \\d+(\\.\\d{1,3})?(ms|s) test",
        "log1: \\d+(\\.\\d{1,3})?(ms|s) \\{\\} \\[ 1, 2, 3 \\]",
        "__proto__: \\d+(\\.\\d{1,3})?(ms|s)",
        ": \\d+(\\.\\d{1,3})?(ms|s)",
        "a: 1",
        "a: 2",
    ] {
        let ok = out
            .lines()
            .filter_map(|l| l.strip_prefix("SL|"))
            .any(|l| regex_lite_match(re, l));
        assert!(ok, "missing /{re}/, out: {out}");
    }
    for want in [
        "Warning|undefined|Count for 'nope' does not exist",
        "Warning|undefined|No such label 'nope2' for console.timeLog()",
        "Warning|undefined|No such label 'nope3' for console.timeEnd()",
        "Warning|undefined|Count for 'default' does not exist",
        "Warning|undefined|No such label 'default' for console.timeLog()",
        "Warning|undefined|No such label 'default' for console.timeEnd()",
        "Warning|undefined|Label 'default' already exists for console.time()",
        "Warning|undefined|Label 'dup' already exists for console.time()",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

fn regex_lite_match(re: &str, line: &str) -> bool {
    // 小守门：本文件只用 `\\d`/`(\\.\\d{1,3})?`/`(ms|s)` 三构式，逐段手配。
    let l = line.trim();
    let mut rest = l;
    let mut pat = re;
    while !pat.is_empty() {
        if let Some(p) = pat.strip_prefix("\\d+") {
            let n = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
            if n == 0 {
                return false;
            }
            rest = &rest[n..];
            pat = p;
        } else if let Some(p) = pat.strip_prefix("(\\.\\d{1,3})?") {
            if let Some(r) = rest.strip_prefix('.') {
                let n = r.bytes().take_while(|b| b.is_ascii_digit()).count();
                if n == 0 || n > 3 {
                    return false;
                }
                rest = &r[n..];
            }
            pat = p;
        } else if let Some(p) = pat.strip_prefix("(ms|s)") {
            if let Some(r) = rest.strip_prefix("ms") {
                rest = r;
            } else if let Some(r) = rest.strip_prefix('s') {
                rest = r;
            } else {
                return false;
            }
            pat = p;
        } else if let Some(p) = pat.strip_prefix("\\{\\}") {
            if let Some(r) = rest.strip_prefix("{}") {
                rest = r;
                pat = p;
            } else {
                return false;
            }
        } else if let Some(p) = pat.strip_prefix("\\[ 1, 2, 3 \\]") {
            if let Some(r) = rest.strip_prefix("[ 1, 2, 3 ]") {
                rest = r;
                pat = p;
            } else {
                return false;
            }
        } else {
            let ch = pat.chars().next().unwrap();
            if !rest.starts_with(ch) {
                return false;
            }
            rest = &rest[ch.len_utf8()..];
            pat = &pat[ch.len_utf8()..];
        }
    }
    rest.is_empty()
}

#[test]
fn console_global_timer_count_warning_faces() {
    // 全局面（prelude JS 本体，R-a~R-h 全局列）：time/timeEnd 输出形 +
    // timeLog 余参续接 + count 递增/countReset 命中删 + dup 不重置（同对象恒等）+
    // `_times` 可见（get/size）+ 归一标签；报错：Symbol 入参 TypeError；
    // 边界：8 警告文本逐字 + timeEnd 无死链。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
const warns = [];
process.on("warning", (w) => warns.push(`${w.name}|${w.code}|${w.message}`));
console.time("t"); console.timeEnd("t");
console.time("log1"); console.timeLog("log1"); console.timeLog("log1", "test"); console.timeLog("log1", {}, [1, 2, 3]); console.timeEnd("log1");
console.count("a"); console.count("a"); console.countReset("a"); console.count("a");
const d0 = (console.time("same"), console._times.get("same"));
console.time("same");
console.log("same-obj", console._times.get("same") === d0);
console.timeEnd("same");
const size0 = console._times.size;
console.time("l1"); console.time("l2"); console.timeEnd("l1"); console.timeEnd("l2");
console.log("deadlinks", console._times.size === size0);
console.log("times-map", typeof console._times.get === "function" && typeof console._times.size === "number");
for (const f of [() => console.time(Symbol("x")), () => console.timeEnd(Symbol("x")), () => console.count(Symbol("x"))]) {
  try { f(); console.log("sym-no-throw"); }
  catch (e) { console.log("sym-throw", e.constructor.name); }
}
try { console.countReset(Symbol("x")); console.log("sym-no-throw"); }
catch (e) { console.log("sym-throw", e.constructor.name); }
console.countReset("nope"); console.timeLog("nope2"); console.timeEnd("nope3");
await new Promise((r) => setTimeout(r, 100));
console.log("warns", JSON.stringify(warns));
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "same-obj true",
        "deadlinks true",
        "times-map true",
        "sym-throw TypeError",
        "sym-throw TypeError",
        "sym-throw TypeError",
        "sym-throw TypeError",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    for re in [
        "t: \\d+(\\.\\d{1,3})?(ms|s)",
        "log1: \\d+(\\.\\d{1,3})?(ms|s) test",
        "log1: \\d+(\\.\\d{1,3})?(ms|s) \\{\\} \\[ 1, 2, 3 \\]",
        "a: 1",
        "a: 2",
    ] {
        let ok = out.lines().any(|l| regex_lite_match(re, l.trim()));
        assert!(ok, "missing /{re}/, out: {out}");
    }
    for want in [
        "Warning|undefined|Count for 'nope' does not exist",
        "Warning|undefined|No such label 'nope2' for console.timeLog()",
        "Warning|undefined|No such label 'nope3' for console.timeEnd()",
        "Warning|undefined|Label 'same' already exists for console.time()",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn console_global_routing_faces() {
    // 正常：补丁 process.stdout.write 捕获 count/log 输出；
    // console._stdout/_stderr 设定后走自定义流；clear 在 isTTY 写转义、
    // 非 TTY 静默；diagnostics channel 收到原参数；
    // 报错：自定义流同步写错即吞（ignoreErrors 缺省真）；
    // 边界：恢复补丁/自定义流后走原生，输出仍正常。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_node_file(
        &dir,
        "p.mjs",
        r#"
import { channel } from "node:diagnostics_channel";
import { Writable } from "node:stream";
const tty0 = process.stdout.isTTY;
let buf = "";
const origWrite = process.stdout.write;
process.stdout.write = (s) => { buf += String(s); return true; };
console.count();
console.log("hi", 42);
process.stdout.write = origWrite;
console.log("captured", JSON.stringify(buf));
let s1 = "", s2 = "";
const w1 = new Writable({ write(c, e, cb) { s1 += String(c); cb(); } });
const w2 = new Writable({ write(c, e, cb) { s2 += String(c); cb(); } });
console._stdout = w1;
console._stderr = w2;
console.log("to-s1");
console.error("to-s2");
await new Promise((r) => setTimeout(r, 50));
console._stdout = undefined;
console._stderr = undefined;
console.log("routed", JSON.stringify(s1), JSON.stringify(s2));
const seen = [];
for (const m of ["log", "info", "debug", "warn", "error"]) {
  channel(`console.${m}`).subscribe((a) => seen.push(`${m}:${a.length}`));
}
console.log("a", 1);
console.info("b");
console.debug("c");
console.warn("d");
console.error("e");
await new Promise((r) => setTimeout(r, 50));
console.log("channels", JSON.stringify(seen));
process.stdout.isTTY = true;
let cbuf = "";
process.stdout.write = (s) => { cbuf += String(s); return true; };
console.clear();
process.stdout.write = origWrite;
let cbuf2 = "";
process.stdout.write = (s) => { cbuf2 += String(s); return true; };
process.stdout.isTTY = false;
console.clear();
process.stdout.write = origWrite;
process.stdout.isTTY = tty0;
const termDumb = process.env.TERM === "dumb";
console.log("clear-tty", JSON.stringify(cbuf), "clear-plain", JSON.stringify(cbuf2));
console.log("clear-expect", JSON.stringify(termDumb ? "" : "\u001b[1;1H\u001b[0J"));
const boom = new Writable({ write() { throw new Error("x-boom"); } });
console._stdout = boom;
console.log("swallowed");
console._stdout = undefined;
console.log("after-swallow ok");
"#,
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = String::from_utf8(out.stdout).unwrap();
    for want in [
        "captured \"default: 1\\nhi 42\\n\"",
        "routed \"to-s1\\n\" \"to-s2\\n\"",
        "channels [\"log:2\",\"info:1\",\"debug:1\",\"warn:1\",\"error:1\"]",
        "clear-plain \"\"",
        "after-swallow ok",
    ] {
        assert!(out.contains(want), "missing {want}, out: {out}");
    }
    // clear TTY 形随 TERM（dumb 即静默，真机同；套件 test-console-clear 同口径）。
    let expect = if out.contains("clear-expect \"\"") {
        "clear-tty \"\""
    } else {
        "clear-tty \"\\u001b[1;1H\\u001b[0J\""
    };
    assert!(out.contains(expect), "missing {expect}, out: {out}");
    dir.close().unwrap();
}
