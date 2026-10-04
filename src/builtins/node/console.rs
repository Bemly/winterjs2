//! `node:console`（Node `lib/console.js` 模块面，MIT；M5 vitest 牵引补齐）。
//!
//! 忠实面：具名导出全表（Console/assert/clear/count/countReset/debug/dir/
//! dirxml/error/group/groupCollapsed/groupEnd/info/log/table/time/timeEnd/
//! timeLog/trace/warn/context/createTask/profile/profileEnd/timeStamp）——
//! 无流方法直绑全局 console（同行为）；`Console` 类（`new Console(stdout[,
//! stderr][, ignoreErrors])` / `new Console({stdout, stderr})`，输出经
//! `node:util` 的 `format` 格式化后写流）。
//!
//! 偏差（记档）：
//! - `table` 只做 `format` 落盘（无列对齐渲染）；`profile/profileEnd/timeStamp`
//!   为无操作（真机同为 inspector 旁路，返回 undefined）；`createTask` 只回
//!   `{ run(f, ...a) }`（无 async_hooks 链路）；`context()` 回新 `Console`
//!   （真机回原生 console 上下文对象）；实例流校验宽松（无 `.write` 即回落
//!   全局 console，真机 TypeError）；实例 `dir` 不读 inspect options。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/console.js (module face; see module docs for deviations).
// Console 类与全局正名/别名由 prelude bootstrap 落定（真机口径：
// require('console') === globalThis.console）；本模块只做导出面装配。
import { format, inspect } from 'node:util';

const g = globalThis.console;
const bind = (name) => (...args) => g[name](...args);

function Console(stdout, stderr, ignoreErrors) {
  // 真机口径：无 new 亦可调用（回 new 实例）；方法构造器内绑定实例
  //（forEach(c.log) 点名）；全局 console 即 instanceof（custom instanceof）。
  if (!(this instanceof Console)) return new Console(stdout, stderr, ignoreErrors);
  {
    // Node 双形态：Console(stream[, stderr]) / Console({ stdout, stderr })。
    let groupIndentation;
    if (stdout !== null && typeof stdout === "object" && typeof stdout.write !== "function") {
      ignoreErrors = stderr;
      groupIndentation = stdout.groupIndentation;
      // inspectOptions 非对象即 ARG_TYPE（真机 validateObject 口径 +
      // common.invalidArgTypeHelper 文案）。
      if (stdout.inspectOptions !== undefined &&
          (typeof stdout.inspectOptions !== "object" || stdout.inspectOptions === null)) {
        const __iv = stdout.inspectOptions;
        const __recv = __iv == null ? ` Received ${__iv}`
          : ` Received type ${typeof __iv} (${inspect(__iv)})`;
        const e = new TypeError(
          `The "options.inspectOptions" property must be of type object.${__recv}`);
        e.code = "ERR_INVALID_ARG_TYPE";
        throw e;
      }
      stderr = stdout.stderr;
      stdout = stdout.stdout;
    }
    // 流校验（真机口径：stderr 缺省跟 stdout；无 write 即
    // ERR_CONSOLE_WRITABLE_STREAM，面 stdout/stderr）。
    if (stderr === undefined) stderr = stdout;
    if (!stdout || typeof stdout.write !== "function") {
      const e = new TypeError("Console expects a writable stream instance for stdout");
      e.code = "ERR_CONSOLE_WRITABLE_STREAM";
      throw e;
    }
    if (!stderr || typeof stderr.write !== "function") {
      const e = new TypeError("Console expects a writable stream instance for stderr");
      e.code = "ERR_CONSOLE_WRITABLE_STREAM";
      throw e;
    }
    this._stdout = stdout ?? null;
    this._stderr = stderr ?? null;
    this._ignoreErrors = ignoreErrors;
    this._counts = new Map();
    this._times = new Map();
    // 组缩进（node 口径：实例级 indent 层数 × groupIndentation 空格，缺省 2；
    // group(label) 先按当前层打印 label 再进一层，groupEnd 退一层）。
    this._indent = 0;
    this._groupIndent = groupIndentation === undefined ? 2 : Number(groupIndentation);
    for (const __m of __CONSOLE_METHODS) {
      // 动态查 this（子类重写优先）；包箭头（天然不可 new，真机口径）+
      // 显式正名（箭头成员赋值不推断）。
      if (typeof this[__m] === "function" && !Object.prototype.hasOwnProperty.call(this, __m)) {
        const __t = this[__m];
        const __self = this;
        const __f = (...__a) => __t.apply(__self, __a);
        try {
          Object.defineProperty(__f, "name", { value: __m, configurable: true });
        } catch {}
        this[__m] = __f;
      }
    }
  }
}
const __CONSOLE_METHODS = ["log", "info", "debug", "warn", "error", "dir",
  "dirxml", "trace", "table", "assert", "count", "countReset", "time",
  "timeLog", "timeEnd", "group", "groupCollapsed", "groupEnd", "clear",
  "profile", "profileEnd", "timeStamp"];
Object.defineProperty(Console, Symbol.hasInstance, {
  value(instance) {
    if (instance === globalThis.console) return true;
    return Function.prototype[Symbol.hasInstance].call(this, instance);
  },
  configurable: true,
});
Console.prototype._pad = function(text) {
  if (!this._indent) return text;
  const p = " ".repeat(this._indent * this._groupIndent);
  return String(text).split("\n").map((l) => l ? p + l : l).join("\n");
};
Console.prototype._w = function(stream, text, fb) {
  if (stream !== null && typeof stream.write === "function") {
    try {
      // R3b：复用写回调（真机 errorHandler 同形；同 cb 合批即单 TickObject，
      // samecb-singletick 套件点名一次 init）。
      stream.write(text + "\n", this._wCb ??= () => {});
    } catch (e) {
      // 真机口径：ignoreErrors=false 即透传原错（instance 套件点名 /^Error: out$/）。
      if (!this._ignoreErrors) throw e;
    }
    return;
  }
  // 无可用流回落全局（真机 TypeError，宽松记档；回落方向与方法一致）。
  fb(text);
};
Console.prototype._out = function(...args) { const t = this._pad(format(...args)); this._w(this._stdout, t, (s) => g.log(s)); };
Console.prototype._err = function(...args) { const t = this._pad(format(...args)); this._w(this._stderr, t, (s) => g.error(s)); };
Console.prototype.log = function(...args) { this._out(...args); };
Console.prototype.info = function(...args) { this._out(...args); };
Console.prototype.debug = function(...args) { this._out(...args); };
Console.prototype.warn = function(...args) { this._err(...args); };
Console.prototype.error = function(...args) { this._err(...args); };
Console.prototype.dir = function(obj) { this._out(obj); };
Console.prototype.dirxml = function(...args) { this._out(...args); };
Console.prototype.trace = function(...args) { this._err("Trace:", ...args); };
Console.prototype.table = function(data) { this._out(data); };
Console.prototype.assert = function(value, ...args) {
  if (!value) this._err("Assertion failed:", ...args);
};
Console.prototype.count = function(label = "default") {
  const n = (this._counts.get(label) ?? 0) + 1;
  this._counts.set(label, n);
  this._out(`${label}: ${n}`);
};
Console.prototype.countReset = function(label = "default") { this._counts.delete(label); };
Console.prototype.time = function(label = "default") { this._times.set(label, Date.now()); };
Console.prototype.timeLog = function(label = "default", ...args) {
  const t = this._times.get(label);
  if (t === undefined) this._err(`Timer '${label}' does not exist`);
  else this._out(`${label}: ${Date.now() - t}ms`, ...args);
};
Console.prototype.timeEnd = function(label = "default") {
  const t = this._times.get(label);
  this._times.delete(label);
  if (t === undefined) this._err(`Timer '${label}' does not exist`);
  else this._out(`${label}: ${Date.now() - t}ms`);
};
Console.prototype.group = function(...args) { if (args.length > 0) this._out(...args); this._indent++; };
Console.prototype.groupCollapsed = function(...args) { if (args.length > 0) this._out(...args); this._indent++; };
Console.prototype.groupEnd = function() { if (this._indent > 0) this._indent--; };
Console.prototype.clear = function() {};
Console.prototype.profile = function() {};
Console.prototype.profileEnd = function() {};
Console.prototype.timeStamp = function() {};

function context() { return Object.create(g); }
function createTask() { return { run(f, ...args) { return f(...args); } }; }
function profile() {}
function profileEnd() {}
function timeStamp() {}

const __api = {
  Console,
  assert: bind("assert"),
  clear: bind("clear"),
  context,
  count: bind("count"),
  countReset: bind("countReset"),
  createTask,
  debug: bind("debug"),
  dir: bind("dir"),
  dirxml: (...args) => g.dir(...args),
  error: bind("error"),
  group: bind("group"),
  groupCollapsed: (...args) => g.group(...args),
  groupEnd: bind("groupEnd"),
  info: bind("info"),
  log: bind("log"),
  profile,
  profileEnd,
  table: (...args) => g.log(...args),
  time: bind("time"),
  timeEnd: bind("timeEnd"),
  timeLog: bind("timeLog"),
  timeStamp,
  trace: bind("trace"),
  warn: bind("warn"),
};
// 真机口径：require('console') === globalThis.console（instance 套件点名）。
// 附带把模块面钉到侧表（require prelude 的 context()/Console 惰性 getter
// 经 require 拿到的只是 default（即全局本体），直接读 .Console/.context
// 会自循环；侧表绕开）。
try {
  Object.defineProperty(g, "__wjs2_consoleMod", {
    value: { Console, context },
    configurable: true,
    writable: true,
  });
} catch {}
try {
  Object.defineProperty(g, "Console", { value: Console, configurable: true, writable: true });
} catch {}
export default g;
export {
  Console,
  context,
  createTask,
  profile,
  profileEnd,
  timeStamp,
};
export const assert = __api.assert;
export const clear = __api.clear;
export const count = __api.count;
export const countReset = __api.countReset;
export const debug = __api.debug;
export const dir = __api.dir;
export const dirxml = __api.dirxml;
export const error = __api.error;
export const group = __api.group;
export const groupCollapsed = __api.groupCollapsed;
export const groupEnd = __api.groupEnd;
export const info = __api.info;
export const log = __api.log;
export const table = __api.table;
export const time = __api.time;
export const timeEnd = __api.timeEnd;
export const timeLog = __api.timeLog;
export const trace = __api.trace;
export const warn = __api.warn;
"#;
