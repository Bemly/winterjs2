//! `require()` 附属 prelude（CJS `NODE_PRELUDE` 尾部拼装；`require` 本体为裸 native）。
//! 由 `require.rs` 按方法族拆出（纯搬移，`pub use` 原位重导出；字节恒等见拆分提交）。

/// `require` 附属 prelude（`NODE_PRELUDE` 尾部拼装；`require` 本体为裸 native）。
pub const REQUIRE_PRELUDE: &str = r#"
globalThis.__wjs2_make_module = (filename) => {
  const m = {
    exports: {},
    id: String(filename),
    filename: String(filename),
    paths: [],
    loaded: false,
    children: [],
    _compile: function (code, filenameArg) {
      __wjs2_cjs_compile(this, String(code), filenameArg != null ? String(filenameArg) : String(this.filename));
    },
  };
  // `parent` 访问器（node 口径：真值存 `__wjs2_parent` 背书字段；读写经 DEP0144
  // 门控，仅 --pending-deprecation 下警告；native 建链写背书字段不触发）。
  Object.defineProperty(m, 'parent', {
    configurable: true,
    enumerable: true,
    get() {
      __wjs2_dep_warn0144();
      return this.__wjs2_parent ?? null;
    },
    set(v) {
      __wjs2_dep_warn0144();
      this.__wjs2_parent = v;
    },
  });
  return m;
};
function __wjs2_dep_warn0144() {
  try {
    const argv = globalThis.process && globalThis.process.execArgv;
    if (Array.isArray(argv) && argv.includes('--pending-deprecation')) {
      globalThis.process.emitWarning(
        'module.parent is deprecated due to accuracy issues. Please use require.main to find program entry point instead.',
        { type: 'DeprecationWarning', code: 'DEP0144' },
      );
    }
  } catch { /* 警告通道失败不掩盖主流程 */ }
}
globalThis.__wjs2_make_base_require = (base) => (id) => __wjs2_require_from(base, String(id));
globalThis.__wjs2_require_main = (url) => globalThis.require(String(url));
// 全局 console 格式化（node 口径：`util.format`——%s/%d/%i/%f/%j/%o/%O/%c/%%、对象 inspect）。
// Phase 1 原生 sink 只 ToString（`[object Object]`、`%s` 原样），2026-09-25 补齐。
// 纯原始值且首参无 `%` 走快路径（不加载 node:util，启动/热路径零开销）。
// 2026-09-28 §7-②全量收尾：assert/trace 按 internal/console/constructor.js 原文
// 语义包装备案 + 补 8 缺失方法（table/dirxml/groupCollapsed/context/createTask/
// profile/profileEnd/timeStamp）+ 惰性 `Console` 类。table 沿 node:console 模块面
// 既有偏离（format 落盘、无列对齐）；countReset 无标签警告沿模块面偏离；trace 帧行
// 保留引擎口径（SM `fn@file:line` vs V8 `at` 形，首行 `Trace: msg` 对齐）。
{
  let fmt = null;
  const getFmt = () => (fmt ??= globalThis.require("node:util"));
  const prim = (x) => x === null || x === undefined || typeof x === "string" ||
    typeof x === "boolean" || (typeof x === "number" && !Object.is(x, -0));
  // 全局输出路由 + diagnostics 发布（console 域 count/clear/stdio-setters/
  // diagnostics-channels 套件口径；真机 global console 即绑流实例）：
  // - `_stdout`/`_stderr` 可写（node 懒加载面；缺省跟安装期捕获的进程流引用，
  //   补丁 `.write` 同对象即时可见，整体替换进程流则仍走旧引用——与真机同）。
  // - 未改道（无自定义流 + 进程写函数仍是原函数）走原生 raw（零行为变化，
  //   保留 REPL TTY CRLF 路径与启动零开销）；改道走流写 + 同步错吞
  //   （ignoreErrors 缺省真，唯真栈溢出穿透，探针与 node:console 模块面同式）。
  // - log/info/debug/warn/error 调前先向 `console.<k>` channel 发布原参数。
  const __defStdout = (() => { try { return process.stdout; } catch { return null; } })();
  const __defStderr = (() => { try { return process.stderr; } catch { return null; } })();
  let __origOutWrite = null;
  let __origErrWrite = null;
  try { __origOutWrite = process.stdout.write; } catch {}
  try { __origErrWrite = process.stderr.write; } catch {}
  let __diagCache = null;
  const __getDiag = () => (__diagCache ??= globalThis.require("node:diagnostics_channel"));
  let __ofName, __ofMsg, __ofCached = false;
  function __isStackOverflow(e) {
    if (!__ofCached) {
      __ofCached = true;
      try { const __f = () => __f(); __f(); }
      catch (err) { __ofName = err?.name; __ofMsg = err?.message; }
    }
    return !!e && (typeof e === "object" || typeof e === "function") &&
      e.name === __ofName && e.message === __ofMsg;
  }
  function __writeRouted(useStdout, text) {
    let stream = null;
    try {
      const custom = useStdout ? console._stdout : console._stderr;
      if (custom !== undefined && custom !== null) stream = custom;
      else {
        const cur = useStdout ? __defStdout : __defStderr;
        if (!cur) return false;
        if (cur.write !== (useStdout ? __origOutWrite : __origErrWrite)) stream = cur;
        else return false;
      }
    } catch { return false; }
    try {
      stream.write(text + "\n");
    } catch (e) {
      if (__isStackOverflow(e)) {
        if (e.name !== "RangeError") {
          const r = new RangeError(e.message);
          try { r.stack = e.stack; } catch {}
          throw r;
        }
        throw e;
      }
    }
    return true;
  }
  for (const [k, useStdout, chName] of [
    ["log", true, "console.log"], ["info", true, "console.info"],
    ["debug", true, "console.debug"], ["warn", false, "console.warn"],
    ["error", false, "console.error"],
  ]) {
    const raw = console[k];
    if (typeof raw !== "function") continue;
    console[k] = function (...args) {
      if (new.target) throw new TypeError(`console.${k} is not a constructor`);
      try {
        const ch = __getDiag().channel(chName);
        if (ch.hasSubscribers) ch.publish(args);
      } catch {}
      let text;
      if (args.every(prim) && !(typeof args[0] === "string" && args[0].includes("%"))) {
        text = args.map((a) => String(a)).join(" ");
      } else {
        text = getFmt().format(...args);
      }
      if (__writeRouted(useStdout, text)) return;
      return raw.call(this, text);
    };
  }
  const rawDir = console.dir;
  if (typeof rawDir === "function") {
    console.dir = function (obj, options) {
      if (new.target) throw new TypeError("console.dir is not a constructor");
      return rawDir.call(this, getFmt().inspect(obj, { customInspect: false, ...options }));
    };
  }
  // assert(expression, ...args)：constructor.js 原文——首参字符串即前缀
  // `Assertion failed: `，否则 unshift；再经 warn（二次格式化）。
  console.assert = function (expression, ...args) {
    if (new.target) throw new TypeError("console.assert is not a constructor");
    if (!expression) {
      if (args.length > 0 && typeof args[0] === "string") {
        args[0] = `Assertion failed: ${args[0]}`;
      } else {
        args.unshift("Assertion failed");
      }
      return this.warn(...args);
    }
  };
  // trace(...args)：message 经 stderr 格式化 + 栈（captureStackTrace 本引擎
  // 不合成首行，故自拼首行；空消息时 V8 省略 `: ` 即裸 `Trace`）→ error。
  console.trace = function (...args) {
    if (new.target) throw new TypeError("console.trace is not a constructor");
    const msg = getFmt().format(...args);
    let frames = "";
    try {
      const e = new Error(msg);
      if (typeof Error.captureStackTrace === "function") {
        try { Error.captureStackTrace(e, console.trace); } catch {}
      }
      const s = String(e.stack ?? "");
      if (s !== "" && s !== msg) frames = `\n${s}`;
    } catch {}
    const head = msg === "" ? "Trace" : `Trace: ${msg}`;
    return this.error(`${head}${frames}`);
  };
  // 计时/计数面（constructor.js + debuglog time 原文口径，JS 本体实现）：
  // 真机全局 console 即 Console 实例（`_times` 可见 + 缺标/复标走 warning）。
  // Rust 原生只 ToString/存 Instant（time 覆盖不告警、无 _times、恒 ms），
  // 此处 JS 面接管五方法（构造守卫内含）：归一→查表→warning/formatTime，
  // 与 node:console 模块面同一语义（Map 代 SafeMap：套件只用 get/size）。
  const __times = new Map();
  const __counts = new Map();
  let __fmtTime = null;
  const __getFormatTime = () => (
    __fmtTime ??= globalThis.require("node:internal/debuglog").formatTime
  );
  console._times = __times;
  console.time = function time(label = "default") {
    if (new.target) throw new TypeError("console.time is not a constructor");
    label = `${label}`;
    if (__times.has(label)) {
      process.emitWarning(`Label '${label}' already exists for console.time()`);
      return;
    }
    __times.set(label, process.hrtime());
  };
  console.timeLog = function timeLog(label = "default", ...args) {
    if (new.target) throw new TypeError("console.timeLog is not a constructor");
    label = `${label}`;
    const t = __times.get(label);
    if (t === undefined) {
      process.emitWarning(`No such label '${label}' for console.timeLog()`);
      return;
    }
    const d = process.hrtime(t);
    const f = __getFormatTime()(d[0] * 1000 + d[1] / 1e6);
    if (args.length === 0) console.log(`${label}: ${f}`);
    else console.log("%s: %s", label, f, ...args);
  };
  console.timeEnd = function timeEnd(label = "default") {
    if (new.target) throw new TypeError("console.timeEnd is not a constructor");
    label = `${label}`;
    const t = __times.get(label);
    __times.delete(label);
    if (t === undefined) {
      process.emitWarning(`No such label '${label}' for console.timeEnd()`);
      return;
    }
    const d = process.hrtime(t);
    const f = __getFormatTime()(d[0] * 1000 + d[1] / 1e6);
    console.log(`${label}: ${f}`);
  };
  console.count = function count(label = "default") {
    if (new.target) throw new TypeError("console.count is not a constructor");
    label = `${label}`;
    let n = __counts.get(label);
    if (n === undefined) n = 1;
    else n++;
    __counts.set(label, n);
    console.log(`${label}: ${n}`);
  };
  console.countReset = function countReset(label = "default") {
    if (new.target) throw new TypeError("console.countReset is not a constructor");
    if (!__counts.has(label)) {
      process.emitWarning(`Count for '${label}' does not exist`);
      return;
    }
    __counts.delete(`${label}`);
  };
  // 其余原生方法包一层构造守卫（真机 console 方法皆不可 new；原生直挂可构造）。
  // 置别名前，使 groupCollapsed 取到守卫版 group（同体）。
  for (const __gk of ["group", "groupEnd"]) {
    const __raw = console[__gk];
    if (typeof __raw !== "function") continue;
    console[__gk] = function (...__ga) {
      if (new.target) throw new TypeError(`console.${__gk} is not a constructor`);
      return __raw.apply(this, __ga);
    };
  }
  // clear（constructor.js 原文口径）：仅 _stdout 是 TTY 且 TERM 非 dumb 时
  // 写 `\x1b[1;1H\x1b[0J`（cursorTo(0,0)+clearScreenDown 合并；经路由写，
  // hijack 补丁即时可见），否则静默。
  console.clear = function clear() {
    if (new.target) throw new TypeError("console.clear is not a constructor");
    let s = null;
    try { s = console._stdout ?? __defStdout; } catch {}
    if (s && s.isTTY && process.env.TERM !== "dumb") {
      try { s.write("\u001b[1;1H\u001b[0J"); } catch {}
    }
  };
  // 别名（constructor.js 末：dirxml=log、groupCollapsed=group 同函数对象）。
  console.dirxml = console.log;
  console.groupCollapsed = console.group;
  // table：沿模块面偏离（format 落盘；非对象直 log，与 constructor.js 同分支）。
  console.table = function (data) {
    if (new.target) throw new TypeError("console.table is not a constructor");
    return this.log(data);
  };
  // inspector 旁路 stubs（与 node:console 模块面同形；真机为 native）。
  console.profile = function () {};
  console.profileEnd = function () {};
  console.timeStamp = function () {};
  console.createTask = function () {
    return { run(f, ...args) { return f(...args); } };
  };
  // context()/Console 经 node:console 模块面惰性取（与模块同一类/同一语义；
  // 不在启动期加载，首调才进模块；经侧表 __wjs2_consoleMod 取——require 回
  // 的是 default（即全局本体），直接读 .Console/.context 会自循环）。
  const getConsoleMod = () => (
    globalThis.require("node:console"), globalThis.console.__wjs2_consoleMod
  );
  console.context = function () {
    return getConsoleMod().context();
  };
  Object.defineProperty(console, "Console", {
    configurable: true,
    get() { return getConsoleMod().Console; },
  });
  // 正名 + 别名（真机口径；本 shim 在 prelude 之后包装，名/别名落此处才稳）：
  // log/info/debug 分体（各发自家 diagnostics channel；methods 套件容忍别名，
  // 但 diagnostics-channels 套件要求分 channel）、warn/error 分体（同理）；
  // 仅 dirxml=log、groupCollapsed=group 保持同体（无 channel 面）。
  // 包装函数匿名（name ''）由下循环正名，methods 套件点名 .name。
  console.dirxml = console.log;
  console.groupCollapsed = console.group;
  for (const __cn of ["log", "warn", "trace", "dir", "assert", "count",
      "countReset", "time", "timeLog", "timeEnd", "group", "groupEnd",
      "clear", "table", "debug", "info", "error"]) {
    try {
      const __fn = console[__cn];
      if (typeof __fn === "function" && !__fn.name) {
        Object.defineProperty(__fn, "name", { value: __cn, configurable: true });
      }
    } catch {}
  }
}
// 直挂原生（禁 JS 闭包包装）：describe_scripted_caller 的最内层帧须是调用方
// 文件——闭包帧（本 prelude）会盖掉它，相对 require.resolve 即丢 base
// （jsdom api.js 实测：caller=__wjs2_node_prelude.js）。
globalThis.require.resolve = __wjs2_require_resolve;
Object.defineProperty(globalThis.require, "main", {
  configurable: true,
  get() {
    const u = __wjs2_require_main_url();
    if (u === undefined) return undefined;
    return { filename: u, id: u, paths: [] };
  },
});
// require.cache/extensions 共享视图（node:module 活对象；未加载前 undefined，
// 与现行一致；getter 不改调用帧，caller 定位无损）。
Object.defineProperty(globalThis.require, "cache", {
  configurable: true,
  enumerable: true,
  get() { return globalThis.__wjs2_cjs_cache; },
});
Object.defineProperty(globalThis.require, "extensions", {
  configurable: true,
  enumerable: true,
  get() { return globalThis.__wjs2_module_extensions; },
});
"#;
