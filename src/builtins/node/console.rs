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
//!   全局 console，真机 TypeError）。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
 // Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/console.js (module face; see module docs for deviations).
// Console 类与全局正名/别名由 prelude bootstrap 落定（真机口径：
// require('console') === globalThis.console）；本模块只做导出面装配。
import { formatWithOptions, inspect } from 'node:util';
import { validateInteger, validateObject, validateOneOf } from 'node:internal/validators';
import { formatTime } from 'node:internal/debuglog';

const g = globalThis.console;
const bind = (name) => (...args) => g[name](...args);

function Console(stdout, stderr, ignoreErrors) {
  // 真机口径：无 new 亦可调用（回 new 实例）；方法构造器内绑定实例
  //（forEach(c.log) 点名）；全局 console 即 instanceof（custom instanceof）。
  if (!(this instanceof Console)) return new Console(stdout, stderr, ignoreErrors);
  {
    // Node 双形态：Console(stream[, stderr]) / Console({ stdout, stderr,
    // ignoreErrors, colorMode, inspectOptions, groupIndentation })。
    // 位置形态无 colorMode（恒 'auto'）；options 形态逐项解构（真机口径）。
    let groupIndentation;
    let colorMode = 'auto';
    let colorModeRaw;
    let inspectOptions;
    if (stdout !== null && typeof stdout === "object" && typeof stdout.write !== "function") {
      const __opts = stdout;
      stdout = __opts.stdout;
      stderr = __opts.stderr;
      // options 形态取 ignoreErrors 本体（真机解构口径；旧实现误读第二位置参数）。
      ignoreErrors = __opts.ignoreErrors;
      colorModeRaw = __opts.colorMode;
      if (colorModeRaw !== undefined) colorMode = colorModeRaw;
      inspectOptions = __opts.inspectOptions;
      groupIndentation = __opts.groupIndentation;
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
    // 选项校验（真机逐字：colorMode 白名单；groupIndentation 整数 0..1000；
    // inspectOptions 非对象即 ARG_TYPE，colors 与显式 colorMode 互斥）。
    validateOneOf(colorMode, 'colorMode', ['auto', true, false]);
    if (groupIndentation !== undefined) {
      validateInteger(groupIndentation, 'groupIndentation', 0, 1000);
    }
    let __optionsMap = null;
    if (inspectOptions !== undefined) {
      validateObject(inspectOptions, 'options.inspectOptions');
      const __isMap = (v) => {
        try { return Object.prototype.toString.call(v) === '[object Map]'; }
        catch { return false; }
      };
      __optionsMap = __isMap(inspectOptions) ? inspectOptions : new Map([
        [stdout, inspectOptions],
        [stderr, inspectOptions],
      ]);
      // 真机口径：任一流选项带 colors 即与显式 colorMode 冲突（raw 判定，
      // colorMode 缺省 'auto' 不触发）。
      for (const __o of __optionsMap.values()) {
        if (__o != null && (typeof __o === 'object' || typeof __o === 'function') &&
            __o.colors !== undefined && colorModeRaw !== undefined) {
          const e = new TypeError('Option "options.inspectOptions.color" cannot be used in ' +
            'combination with option "colorMode"');
          e.code = 'ERR_INCOMPATIBLE_OPTION_PAIR';
          throw e;
        }
      }
    }
  this._stdout = stdout ?? null;
  this._stderr = stderr ?? null;
  // 真机口径：ignoreErrors 缺省 true（`new Console({stdout})` 即吞写错；
  // node lib/internal/console/constructor.js `ignoreErrors = true`）。
  // 显式值按 Boolean 归一（`new Console(s, s, 0)` 即不吞）。
  this._ignoreErrors = ignoreErrors === undefined ? true : Boolean(ignoreErrors);
    this._counts = new Map();
    this._times = new Map();
    // 组缩进（node 口径：实例级 indent 层数 × groupIndentation 空格，缺省 2；
    // group(label) 先按当前层打印 label 再进一层，groupEnd 退一层）。
    this._indent = 0;
    this._groupIndent = groupIndentation === undefined ? 2 : groupIndentation;
    // 着色与 per-stream 检查选项（node kColorMode/kGetInspectOptions 口径）。
    this._colorMode = colorMode;
    this._inspectOptionsMap = __optionsMap;
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
  // 真机 kWriteToConsole 口径：先各 `\n` 后补缩进再整体前缀（空行亦垫空格）。
  if (!this._indent) return text;
  const p = " ".repeat(this._indent * this._groupIndent);
  text = String(text);
  if (text.includes("\n")) text = text.split("\n").join(`\n${p}`);
  return p + text;
};
// 着色判定（node internal/util/colors.shouldColorize 同形：FORCE_COLOR 走
// 进程 TTY 色深，否则 isTTY 且无 getColorDepth 即真、有则 >2）。
function __shouldColorize(stream) {
  try {
    if (typeof process !== 'undefined' && process.env &&
        process.env.FORCE_COLOR !== undefined) {
      let __d;
      try {
        const __t = process.stdout;
        __d = (__t && typeof __t.getColorDepth === 'function') ? __t.getColorDepth() : undefined;
      } catch {}
      if (__d !== undefined) return __d > 2;
    }
  } catch {}
  try {
    if (!stream || !stream.isTTY) return false;
    if (typeof stream.getColorDepth === 'function') {
      try { return stream.getColorDepth() > 2; } catch { return false; }
    }
    return true;
  } catch { return false; }
}
// per-stream 检查选项（node kGetInspectOptions 同形：colorMode auto 即按流
// 判定；Map 形态按流取（miss 即无色）；colors 缺省回填当次判定色）。
Console.prototype._inspectOpts = function(stream) {
  let color = this._colorMode;
  if (color === 'auto') color = __shouldColorize(stream);
  const __map = this._inspectOptionsMap;
  if (__map) {
    let o;
    try { o = __map.get(stream); } catch { o = undefined; }
    if (o !== undefined && o !== null && (typeof o === 'object' || typeof o === 'function')) {
      if (o.colors === undefined) {
        try { o.colors = color; } catch {}
      }
      return o;
    }
  }
  return color ? { colors: true } : {};
};
// 单参字符串 fast path（真机 kFormatForStdout 同形；余走 formatWithOptions）。
Console.prototype._fmtOut = function(args) {
  if (args.length === 1 && typeof args[0] === 'string') return args[0];
  return formatWithOptions(this._inspectOpts(this._stdout), ...args);
};
Console.prototype._fmtErr = function(args) {
  if (args.length === 1 && typeof args[0] === 'string') return args[0];
  return formatWithOptions(this._inspectOpts(this._stderr), ...args);
};
// 栈溢出判定（node internal/errors.isStackOverflowError 同形：与本引擎实爆的
// RangeError 按 name+message 比对；引擎无关——SpiderMonkey 文案非 V8 形）。
// 捕获只做一次（惰性，首个写错时才触发；console 高频路径零开销）。
let __overflowName, __overflowMessage, __overflowCached = false;
function __isStackOverflow(e) {
  if (!__overflowCached) {
    __overflowCached = true;
    try { const __f = () => __f(); __f(); }
    catch (err) { __overflowName = err?.name; __overflowMessage = err?.message; }
  }
  return !!e && (typeof e === "object" || typeof e === "function") &&
    e.name === __overflowName && e.message === __overflowMessage;
}
function __noop() {}
// 写错 errorHandler（node createWriteErrorHandler 同形：异步 cb 报错且尚未
// 派发 error 时补一次性 noop 监听，把随后的 'error' 派发吞掉；成功回调用
// null（本仓流与 node 一致）即 `err != null` 为假，不做任何事）。
function __makeWriteErrorHandler(instance, useStdout) {
  return (err) => {
    if (err === null || err === undefined) return;
    let stream = null;
    try { stream = useStdout ? instance._stdout : instance._stderr; } catch {}
    if (!stream || !stream._writableState || stream._writableState.errorEmitted) return;
    try {
      if (typeof stream.listenerCount === "function" && stream.listenerCount("error") === 0 &&
          typeof stream.once === "function") stream.once("error", __noop);
    } catch {}
  };
}
Console.prototype._w = function(stream, text, fb) {
  if (stream !== null && typeof stream.write === "function") {
    // 真机口径（kWriteToConsole）：ignoreErrors=false 即裸写透传一切错；
    // 否则同步写错一律吞（唯栈溢出 RangeError 穿透），异步写错走 errorHandler
    // 吞（noop 监听吃掉随后的 'error' 派发）；sync/async/setImmediate 三形见
    // test-console-{async,sync}-write-error，primitive throw 见 log-throw 件。
    if (!this._ignoreErrors) {
      stream.write(text + "\n");
      return;
    }
    const useStdout = stream === this._stdout;
    if (useStdout) this._stdoutErrorHandler ??= __makeWriteErrorHandler(this, true);
    else this._stderrErrorHandler ??= __makeWriteErrorHandler(this, false);
    const handler = useStdout ? this._stdoutErrorHandler : this._stderrErrorHandler;
    try {
      // 有 error 监听才补 noop（真机同形，先挂后写；有监听即用户自理）。
      if (typeof stream.listenerCount === "function" && stream.listenerCount("error") === 0 &&
          typeof stream.once === "function") stream.once("error", __noop);
      stream.write(text + "\n", handler);
    } catch (e) {
      // 真机口径：console 系调试工具，吞错是常态，但栈空间耗尽必须穿透
      //（test-console-no-swallow-stack-overflow 点名 RangeError）。
      // 引擎差：本仓 SpiderMonkey 栈溢出为 `InternalError: too much recursion`，
      // V8/JSC 为 `RangeError: Maximum call stack size exceeded`；node 断言按
      // RangeError，此处转类（message/stack 原样保留；用户伪造同名同文案与
      // 真机 isStackOverflowError 同理穿透，保真）。余错仍吞。
      if (__isStackOverflow(e)) {
        if (e.name !== "RangeError") {
          const __re = new RangeError(e.message);
          try { __re.stack = e.stack; } catch {}
          throw __re;
        }
        throw e;
      }
    } finally {
      try {
        if (typeof stream.removeListener === "function") stream.removeListener("error", __noop);
      } catch {}
    }
    return;
  }
  // 无可用流回落全局（真机 TypeError，宽松记档；回落方向与方法一致）。
  fb(text);
};
Console.prototype._out = function(...args) { const t = this._pad(this._fmtOut(args)); this._w(this._stdout, t, (s) => g.log(s)); };
Console.prototype._err = function(...args) { const t = this._pad(this._fmtErr(args)); this._w(this._stderr, t, (s) => g.error(s)); };
Console.prototype.log = function(...args) { this._out(...args); };
Console.prototype.info = function(...args) { this._out(...args); };
Console.prototype.debug = function(...args) { this._out(...args); };
Console.prototype.warn = function(...args) { this._err(...args); };
Console.prototype.error = function(...args) { this._err(...args); };
Console.prototype.dir = function(obj, options) {
  // 真机口径：customInspect 恒关 + per-stream 选项 + 调用方 options 覆盖。
  const t = this._pad(inspect(obj, {
    customInspect: false,
    ...this._inspectOpts(this._stdout),
    ...options,
  }));
  this._w(this._stdout, t, (s) => g.log(s));
};
Console.prototype.dirxml = function(...args) { this._out(...args); };
Console.prototype.trace = function(...args) { this._err("Trace:", ...args); };
Console.prototype.table = function(data) { this._out(data); };
Console.prototype.assert = function(value, ...args) {
  if (!value) this._err("Assertion failed:", ...args);
};
Console.prototype.count = function(label = "default") {
  // 真机口径（constructor.js count 原文）：入参 `${}` 归一（Symbol 抛 TypeError），
  // 缺标从 1 起（`?? 1` 与原文 `undefined → 1 else ++` 同构）。
  label = `${label}`;
  let n = this._counts.get(label);
  if (n === undefined) n = 1;
  else n++;
  this._counts.set(label, n);
  this._out(`${label}: ${n}`);
};
Console.prototype.countReset = function(label = "default") {
  // 真机口径（constructor.js countReset 原文）：不预归一；缺标走
  // `process.emitWarning("Count for '...' does not exist")`，命中删 `${label}`。
  if (!this._counts.has(label)) {
    process.emitWarning(`Count for '${label}' does not exist`);
    return;
  }
  this._counts.delete(`${label}`);
};
Console.prototype.time = function(label = "default") {
  // 真机口径（debuglog time 原文）：先 `${}` 归一（Symbol 抛 TypeError），
  // 已存在即警告并保留原计时（不重置）；存 `process.hrtime()` 元组。
  label = `${label}`;
  if (this._times.has(label)) {
    process.emitWarning(`Label '${label}' already exists for console.time()`);
    return;
  }
  this._times.set(label, process.hrtime());
};
Console.prototype.timeLog = function(label = "default", ...args) {
  // 真机口径（debuglog timeLogImpl 原文）：归一→缺标警告→hrtime 差值→formatTime→
  // `log('%s: %s', label, formatted, ...args)`（余参续接）。
  label = `${label}`;
  const t = this._times.get(label);
  if (t === undefined) {
    process.emitWarning(`No such label '${label}' for console.timeLog()`);
    return;
  }
  const d = process.hrtime(t);
  const f = formatTime(d[0] * 1000 + d[1] / 1e6);
  if (args.length === 0) this._out(`${label}: ${f}`);
  else this._out(`${label}: ${f}`, ...args);
};
Console.prototype.timeEnd = function(label = "default") {
  // 真机口径（debuglog timeEnd 原文）：归一→缺标警告→输出→删表（无死链）。
  label = `${label}`;
  const t = this._times.get(label);
  this._times.delete(label);
  if (t === undefined) {
    process.emitWarning(`No such label '${label}' for console.timeEnd()`);
    return;
  }
  const d = process.hrtime(t);
  const f = formatTime(d[0] * 1000 + d[1] / 1e6);
  this._out(`${label}: ${f}`);
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
