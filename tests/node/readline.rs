//! tests/node/readline.rs — 对齐 src/builtins/node/readline.rs（node:readline）
//! + readline_promises.rs（node:readline/promises）。

use crate::helpers::*;

#[test]
fn readline_lines_history() {
    // 行提交 + history（recent-first/去空去重/上限）+ question + prompt。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "l.mjs",
        r#"
import readline, { createInterface } from "node:readline";
import { PassThrough } from "node:stream";
const input = new PassThrough(), output = new PassThrough();
let shown = "";
output.on("data", (c) => (shown += c));
const rl = createInterface({ input, output, prompt: "P> ", historySize: 3 });
console.log("term", rl.terminal, JSON.stringify(rl.getPrompt()), JSON.stringify(rl.history));
const lines = [];
rl.on("line", (l) => lines.push(l));
rl.prompt();
rl.question("Q? ", (a) => console.log("answer", JSON.stringify(a)));
input.write("hello\n");
input.write("world\n");
setTimeout(() => {
  console.log("lines", JSON.stringify(lines));
  console.log("hist", JSON.stringify(rl.history));
  console.log("shown", JSON.stringify(shown));
  console.log("line", JSON.stringify(rl.line), rl.cursor);
  rl.close();
  console.log("closed", rl.closed);
}, 100);
"#,
    );
    for line in [
        "term false \"P> \" []",
        "answer \"hello\"",
        "lines [\"world\"]",
        "hist []",
        "shown \"P> Q? \"",
        "line \"\" undefined",
        "closed true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn readline_terminal_edit() {
    // 终端编辑 Emacs 子集 + history + question + prompt 回显。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "t.mjs",
        r#"
import { createInterface } from "node:readline";
import { PassThrough } from "node:stream";
const input = new PassThrough(), output = new PassThrough();
let shown = "";
output.on("data", (c) => (shown += c));
const rl = createInterface({ input, output, terminal: true, prompt: "> " });
console.log("term", rl.terminal, rl.cursor);
const lines = [];
rl.on("line", (l) => lines.push(l));
// 可打印插入 + 回车提交
input.write("helo");
input.write("\x7f"); // 删 o
input.write("lo");
input.write("\r");
setTimeout(() => {
  console.log("l1", JSON.stringify(lines), JSON.stringify(rl.line), rl.cursor);
  // C-a/C-f*2/C-k：行首 + 前进两格 + 杀至行尾得 "ab"
  input.write("abcdef");
  input.write("\x01");
  input.write("\x06");
  input.write("\x06");
  input.write("\x0b");
  input.write("\r");
  setTimeout(() => {
    console.log("l2", JSON.stringify(lines));
    // Up 历史 + C-u 整行杀
    input.write("first\r");
    setTimeout(() => {
      input.write("second\r");
      setTimeout(() => {
        input.write("\x1b[A"); // Up -> second
        input.write("\x15"); // C-u 杀掉
        input.write("third\r");
        setTimeout(() => {
          console.log("hist", JSON.stringify(rl.history));
          // question 走终端提交
          rl.question("Q? ", (a) => console.log("answer", JSON.stringify(a)));
          input.write("ans\r");
          setTimeout(() => {
            console.log("shown-has-prompt", shown.includes("> ") && shown.includes("Q? "));
            rl.close();
          }, 50);
        }, 50);
      }, 50);
    }, 50);
  }, 50);
}, 50);
"#,
    );
    for line in [
        "term true 0",
        "l1 [\"hello\"] \"\" 0",
        "l2 [\"hello\",\"ab\"]",
          "hist [\"third\",\"second\",\"first\",\"ab\",\"hello\"]",
        "answer \"ans\"",
        "shown-has-prompt true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn readline_keypress_validate() {
    // 按键形状 + 校验 + 迭代器 + pause/resume + 自关。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "k.mjs",
        r#"
import readline, { createInterface, emitKeypressEvents } from "node:readline";
import { PassThrough } from "node:stream";
// 按键形状（对拍表转断言）
{
  const i = new PassThrough();
  emitKeypressEvents(i);
  const got = [];
  i.on("keypress", (s, k) => got.push([s, k.name, k.ctrl, k.meta, k.shift, k.code]));
  i.write("a");
  i.write("\r");
  i.write("\x7f");
  i.write("\x1b[A");
  i.write("\x03");
  i.write("\x1bx");
  i.write("\n");
  setTimeout(() => {
    console.log("keys", JSON.stringify(got));
  }, 50);
}
// 校验：无码 TypeError
try { createInterface({}); } catch (e) { console.log("v-obj", e.constructor.name, e.code); }
try { createInterface(); } catch (e) { console.log("v-none", e.constructor.name, e.code); }
console.log("v-pos", typeof createInterface(new PassThrough(), new PassThrough()).question);
// split 包按键（ESC 跨包）
{
  const i = new PassThrough();
  emitKeypressEvents(i);
  i.on("keypress", (s, k) => console.log("split-key", k.name));
  i.write("\x1b");
  setTimeout(() => i.write("[B"), 20);
}
// 迭代器 + 自关 + pause
{
  const i = new PassThrough(), o = new PassThrough();
  const rl = createInterface({ input: i, output: o });
  (async () => {
    const got = [];
    for await (const l of rl) { got.push(l); if (got.length === 2) rl.close(); }
    console.log("iter", JSON.stringify(got));
  })();
  i.write("x\ny\n");
  const i2 = new PassThrough(), o2 = new PassThrough();
  const rl2 = createInterface({ input: i2, output: o2 });
  rl2.on("close", () => console.log("auto-close yes"));
  i2.end("last\n");
  const i3 = new PassThrough(), o3 = new PassThrough();
  const rl3 = createInterface({ input: i3, output: o3 });
  const ls = [];
  rl3.on("line", (l) => ls.push(l));
  rl3.pause();
  i3.write("a\n");
  setTimeout(() => {
    console.log("paused", JSON.stringify(ls), rl3.paused);
    rl3.resume();
    setTimeout(() => console.log("resumed", JSON.stringify(ls)), 30);
  }, 60);
}
setTimeout(() => console.log("end-ok"), 300);
"#,
    );
    for line in [
        "keys [[\"a\",\"a\",false,false,false,null],[\"\\r\",\"return\",false,false,false,null],[\"\",\"backspace\",false,false,false,null],[null,\"up\",false,false,false,\"[A\"],[\"\\u0003\",\"c\",true,false,false,null],[null,\"x\",false,true,false,null],[\"\\n\",\"enter\",false,false,false,null]]",
        "v-obj TypeError undefined",
        "v-none TypeError undefined",
        "v-pos function",
        "split-key down",
        "iter [\"x\",\"y\"]",
        "auto-close yes",
        "paused [] true",
        "resumed [\"a\"]",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn readline_iface_options_and_write() {
    // P2-repl：new Interface(options) 归一 + write 入流排空/关后码 + 多行历史倒序去重。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "w.mjs",
        r#"
import { Interface } from "node:readline";
import { PassThrough } from "node:stream";
const input = new PassThrough(), output = new PassThrough();
const rl = new Interface({ input, output, terminal: true, prompt: "T> ", historySize: 5, removeHistoryDuplicates: true });
console.log("opt", rl.terminal, JSON.stringify(rl.getPrompt()), rl.historySize, rl.removeHistoryDuplicates);
rl.line = "line1\nline2";
input.emit("keypress", "", { name: "enter" });
rl.line = "other";
input.emit("keypress", "", { name: "enter" });
rl.line = "line1\nline2";
input.emit("keypress", "", { name: "enter" });
console.log("hist", JSON.stringify(rl.history));
const i2 = new PassThrough(), o2 = new PassThrough();
const r2 = new Interface({ input: i2, output: o2 });
const got = [];
r2.on("line", (l) => got.push(l));
r2.write("a\nb\npartial");
console.log("drain", JSON.stringify(got));
r2.close();
try { r2.write("x"); console.log("closed FAIL"); }
catch (e) { console.log("closed", e.code); }
rl.close();
"#,
    );
    for line in [
        "opt true \"T> \" 5 true",
        "hist [\"line2\\rline1\",\"other\"]",
        "drain [\"a\",\"b\"]",
        "closed ERR_USE_AFTER_CLOSE",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    assert!(!out.contains("FAIL"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn readline_async_iterator_faces() {
    // 异步迭代器经 EventEmitter.on 接线（node internal/readline 口径）。
    // 正常：for-await 逐行 + watermarkData 符号属性；报错：无；边界：
    // EOF 残留行先发 line 再 close、无尾换行亦得行、背压暂停恢复。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import fs from "node:fs";
import readline from "node:readline";
import { Readable } from "node:stream";
// 正常：for-await + watermarkData
{
  const readable = new Readable({ read() {} });
  readable.push("line 1\nline 2\n");
  readable.push(null);
  const rli = readline.createInterface({ input: readable, crlfDelay: Infinity });
  const it = rli[Symbol.asyncIterator]();
  const wm = it[Symbol.for("nodejs.watermarkData")];
  console.log("wm", typeof wm, wm.high, wm.low);
  const got = [];
  for await (const l of rli) got.push(l);
  console.log("iter", JSON.stringify(got));
}
// 边界：无尾换行残留行
{
  fs.writeFileSync("t.txt", "alpha\nbeta-noeol");
  const rli = readline.createInterface({ input: fs.createReadStream("t.txt"), crlfDelay: Infinity });
  const got = [];
  for await (const l of rli) got.push(l);
  console.log("noeol", JSON.stringify(got));
}
// 边界：空文件零行即关
{
  fs.writeFileSync("e.txt", "");
  const rli = readline.createInterface({ input: fs.createReadStream("e.txt"), crlfDelay: Infinity });
  const got = [];
  for await (const l of rli) got.push(l);
  console.log("empty", JSON.stringify(got));
}
"#,
    );
    for line in [
        "wm object 1024 1",
        "iter [\"line 1\",\"line 2\"]",
        "noeol [\"alpha\",\"beta-noeol\"]",
        "empty []",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn readline_cursor_top_faces() {
    // 顶层光标件（node internal/readline/callbacks.js 口径）：
    // 正常：四件写序列 + 回 true；报错：非法回调 TypeError、NaN x/y 数错、
    // x 非数而 y 是数即 ERR_INVALID_CURSOR_POS；边界：null 流即回 true。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "c.mjs",
        r#"
import readline, { cursorTo, clearLine, clearScreenDown, moveCursor } from "node:readline";
import { PassThrough } from "node:stream";
const o = new PassThrough();
let shown = "";
o.on("data", (c) => (shown += c));
// 正常
console.log("cur", cursorTo(o, 1, 1), JSON.stringify(shown));
shown = "";
console.log("clr", clearLine(o, -1), clearLine(o, 1), clearLine(o, 0), JSON.stringify(shown));
shown = "";
console.log("csd", clearScreenDown(o), JSON.stringify(shown));
shown = "";
console.log("mv", moveCursor(o, 2, -1), JSON.stringify(shown));
// 报错
for (const fn of [
  () => cursorTo(o, 1, 1, null),
  () => clearLine(o, 0, null),
  () => clearScreenDown(o, null),
  () => moveCursor(o, 1, 1, null),
]) {
  try { fn(); console.log("cb FAIL"); }
  catch (e) { console.log("cb", e.code); }
}
for (const [x, y] of [[NaN, undefined], [1, NaN], ["a", undefined]]) {
  try { cursorTo(o, x, y); console.log("nan FAIL"); }
  catch (e) { console.log("nan", e.code); }
}
try { cursorTo(o, "a", 1); console.log("pos FAIL"); }
catch (e) { console.log("pos", e.code); }
// 边界：null 流 / 零位移（均回 true、无写）
console.log("null", cursorTo(null), clearLine(null, 0), clearScreenDown(null), moveCursor(null, 1, 1));
shown = "";
console.log("zero", moveCursor(o, 0, 0), JSON.stringify(shown));
"#,
    );
    for line in [
        "cur true \"\\u001b[2;2H\"",
        "clr true true true \"\\u001b[1K\\u001b[0K\\u001b[2K\"",
        "csd true \"\\u001b[0J\"",
        "mv true \"\\u001b[2C\\u001b[1A\"",
        "cb ERR_INVALID_ARG_TYPE",
        "cb ERR_INVALID_ARG_TYPE",
        "cb ERR_INVALID_ARG_TYPE",
        "cb ERR_INVALID_ARG_TYPE",
        "nan ERR_INVALID_ARG_VALUE",
        "nan ERR_INVALID_ARG_VALUE",
        "nan ERR_INVALID_ARG_VALUE",
        "pos ERR_INVALID_CURSOR_POS",
        "null true true true true",
        "zero true \"\"",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn readline_promises_readline_faces() {
    // node:readline/promises Readline（链式游标写器）：
    // 正常：链式 + commit 落盘 + rollback 弃置 + clearLine/moveCursor/cursorTo
    // 序列；报错：非 Writable 构造 TypeError、cursorTo 非数 TypeError、
    // clearLine 越界 RangeError、_write 同步抛即 commit 拒收；边界：autoCommit
    // 异步落盘、moveCursor(0,0) 空写。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import { Readline } from "node:readline/promises";
import { Writable } from "node:stream";
class T extends Writable {
  constructor() { super(); this.data = ""; }
  _write(c, e, cb) { this.data += c.toString(); cb(); }
}
const w = new T();
const r = new Readline(w);
console.log("chain", r === r.moveCursor(0, 0) && r === r.rollback());
await r.cursorTo(1).commit();
console.log("cur", JSON.stringify(w.data));
w.data = "";
await r.clearScreenDown().rollback();
console.log("rollback", JSON.stringify(w.data));
await r.clearLine(-1).commit();
console.log("clr", JSON.stringify(w.data));
w.data = "";
await r.moveCursor(1, 1).commit();
await r.cursorTo(1, 2).commit();
console.log("mv", JSON.stringify(w.data));
// 报错
for (const v of [undefined, null, 0, 1n, true, Symbol(), "", {}, () => {}]) {
  try { new Readline(v); console.log("ctor FAIL"); }
  catch (e) { if (e.code !== "ERR_INVALID_ARG_TYPE") console.log("ctor BAD " + e.code); }
}
console.log("ctor-ok");
for (const v of [undefined, "a", NaN, 1.1]) {
  try { r.cursorTo(v); console.log("ct FAIL"); }
  catch (e) { console.log("ct", e.code); }
}
try { r.clearLine(2); console.log("cl FAIL"); }
catch (e) { console.log("cl", e.code); }
{
  const bad = new (class extends Writable { _write() { throw new Error("boom"); } })();
  const rb = new Readline(bad);
  try { await rb.cursorTo(1).commit(); console.log("rej FAIL"); }
  catch (e) { console.log("rej", e.message); }
}
// 边界
{
  const w2 = new T();
  const ra = new Readline(w2, { autoCommit: true });
  ra.moveCursor(0, 0);
  ra.clearScreenDown();
  await new Promise((r2) => setTimeout(r2, 50));
  console.log("auto", JSON.stringify(w2.data));
}
"#,
    );
    for line in [
        "chain true",
        "cur \"\\u001b[2G\"",
        "rollback \"\"",
        "clr \"\\u001b[1K\"",
        "mv \"\\u001b[1C\\u001b[1B\\u001b[3;2H\"",
        "ctor-ok",
        "ct ERR_INVALID_ARG_TYPE",
        "ct ERR_INVALID_ARG_TYPE",
        "ct ERR_OUT_OF_RANGE",
        "ct ERR_OUT_OF_RANGE",
        "cl ERR_OUT_OF_RANGE",
        "rej boom",
        "auto \"\\u001b[0J\"",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    assert!(!out.contains("FAIL") && !out.contains("BAD"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn readline_history_events_and_validation() {
    // history 'history' 事件（live 数组，监听改动落盘不改当行）+ 构造校验：
    // 正常：shift 后当行照旧、rli.history 已空；报错：completer/history/
    // historySize/tabSize 非法码；边界：crlfDelay 箝制（0→100，Inf 透传）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "h.mjs",
        r#"
import readline, { createInterface } from "node:readline";
import { PassThrough } from "node:stream";
{
  const i = new PassThrough(), o = new PassThrough();
  const rli = createInterface({ input: i, output: o, terminal: true });
  rli.on("history", (h) => {
    console.log("hev", JSON.stringify(h), h === rli.history);
    h.shift();
  });
  rli.on("line", (l) => console.log("line", JSON.stringify(l), rli.history.length));
  i.write("foo\r");
  rli.close();
}
// 报错
const i0 = new PassThrough();
for (const [label, fn, code] of [
  ["comp", () => createInterface({ input: i0, completer: 1 }), "ERR_INVALID_ARG_VALUE"],
  ["hist", () => createInterface({ input: i0, history: {} }), "ERR_INVALID_ARG_TYPE"],
  ["size-neg", () => createInterface({ input: i0, historySize: -1 }), "ERR_OUT_OF_RANGE"],
  ["size-str", () => createInterface({ input: i0, historySize: "x" }), "ERR_INVALID_ARG_TYPE"],
  ["tab0", () => createInterface({ input: i0, tabSize: 0 }), "ERR_OUT_OF_RANGE"],
  ["tabstr", () => createInterface({ input: i0, tabSize: "4" }), "ERR_INVALID_ARG_TYPE"],
]) {
  try { fn(); console.log(label, "FAIL"); }
  catch (e) { console.log(label, e.code); }
}
// 边界
for (const [v, want] of [[undefined, 100], [0, 100], [50, 100], [5000, 5000], [Infinity, Infinity]]) {
  const r = createInterface({ input: new PassThrough(), crlfDelay: v });
  console.log("crlf", String(v), r.crlfDelay === want);
  r.close();
}
"#,
    );
    for line in [
        "hev [\"foo\"] true",
        "line \"foo\" 0",
        "comp ERR_INVALID_ARG_VALUE",
        "hist ERR_INVALID_ARG_TYPE",
        "size-neg ERR_OUT_OF_RANGE",
        "size-str ERR_INVALID_ARG_TYPE",
        "tab0 ERR_OUT_OF_RANGE",
        "tabstr ERR_INVALID_ARG_TYPE",
        "crlf undefined true",
        "crlf 0 true",
        "crlf 50 true",
        "crlf 5000 true",
        "crlf Infinity true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    assert!(!out.contains("FAIL"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn readline_tab_and_abort_faces() {
    // Tab 补全 + promises question 中止：
    // 正常：同步回值补前缀、连按二次展表；报错：reject 形 completer 落
    // "Tab completion error: Error: message"；边界：无 completer 时 Tab 作
    // 普通字符；已决 signal 即 AbortError（cause 透传）、中途 abort 结算、
    // C-c 结算 AbortError。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "q.mjs",
        r#"
import readline, { createInterface } from "node:readline";
import { Interface as PInterface } from "node:readline/promises";
import { PassThrough } from "node:stream";
// 同步补全：前缀直插 + 二次展表
{
  const i = new PassThrough(), o = new PassThrough();
  let shown = "";
  o.on("data", (c) => (shown += c));
  const rli = createInterface({
    input: i, output: o, terminal: true, prompt: "> ",
    completer: (line) => [["foobar", "foobaz"], line],
  });
  i.write("foo");
  i.write("\t");
  console.log("tab1", JSON.stringify(rli.line));
  i.write("\t");
  console.log("tab2-has-table", shown.includes("foobar") && shown.includes("foobaz"));
  rli.close();
}
// 报错形 completer
{
  const i = new PassThrough(), o = new PassThrough();
  let shown = "";
  o.on("data", (c) => (shown += c));
  const rli = createInterface({
    input: i, output: o, terminal: true,
    completer: () => Promise.reject(new Error("message")),
  });
  i.write("\t");
  await new Promise((r) => setTimeout(r, 50));
  console.log("taberr", shown.startsWith("Tab completion error: Error: message"));
  rli.close();
}
// 边界：无 completer 即普通字符
{
  const i = new PassThrough(), o = new PassThrough();
  const rli = createInterface({ input: i, output: o, terminal: true, prompt: "" });
  let got = null;
  rli.on("line", (l) => (got = l));
  i.write("a\tb\r");
  console.log("notab", JSON.stringify(got), JSON.stringify(rli.line));
  rli.close();
}
// promises question 中止三件
{
  const i = new PassThrough(), o = new PassThrough();
  const rli = new PInterface({ input: i, output: o, terminal: true });
  const s = AbortSignal.abort("boom");
  try { await rli.question("q?", { signal: s }); console.log("ab0 FAIL"); }
  catch (e) { console.log("ab0", e.name, e.cause); }
  rli.close();
}
{
  const i = new PassThrough(), o = new PassThrough();
  const rli = new PInterface({ input: i, output: o, terminal: true });
  const ac = new AbortController();
  let settled = "";
  rli.question("q?", { signal: ac.signal }).then(
    () => (settled = "resolved"),
    (e) => (settled = e.name + ":" + (e.cause?.message ?? e.cause)));
  ac.abort(new Error("stop"));
  await new Promise((r) => setTimeout(r, 50));
  console.log("ab1", settled);
  rli.close();
}
{
  const i = new PassThrough(), o = new PassThrough();
  const rli = new PInterface({ input: i, output: o, terminal: true });
  let settled = "";
  rli.question("q?").then(
    () => (settled = "resolved"),
    (e) => (settled = e.name));
  i.emit("keypress", "", { name: "c", ctrl: true });
  await new Promise((r) => setTimeout(r, 50));
  console.log("cc", settled, rli.closed);
  rli.close();
}
"#,
    );
    for line in [
        "tab1 \"fooba\"",
        "tab2-has-table true",
        "taberr true",
        "notab \"a\\tb\" \"\"",
        "ab0 AbortError boom",
        "ab1 AbortError:stop",
        "cc AbortError true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    assert!(!out.contains("FAIL"), "out: {out}");
    dir.close().unwrap();
}
