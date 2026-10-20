
// ── 9i-2 BroadcastChannel（同名跨会话扇出；发者自收排除，关者止收）────────
// 形态偏差记档：基座沿用 EventEmitter（与本仓 MessagePort 一致；真机为
// EventTarget，`String(bc)` 形状不同）；`message` 事件载荷为裸值（真机为
// MessageEvent），`onmessage` 回调收 `{ data }` 兼容形；无 `close` 事件。

// 10f 对拍：BroadcastChannel.prototype 方法族 this 品牌门（broadcastchannel
// 套件 Reflect.get/apply 面，node ERR_INVALID_THIS 口径）。
const __bcInvalidThis = () => {
  const err = new TypeError('Value of "this" must be of type BroadcastChannel');
  err.code = "ERR_INVALID_THIS";
  return err;
};
const __bcState = new WeakSet();

export class BroadcastChannel extends EventEmitter {
  constructor(name) {
    super();
    // node broadcastchannel 套件口径：仅无参即 ERR_MISSING_ARGS；显式
    // undefined 合法（name "undefined"）；Symbol 走 `${name}` ToString 即抛
    //（本引擎转换错文案与 V8 不同，此处按真机原文补齐——套件正则点名）。
    if (arguments.length === 0) {
      const err = new TypeError(`The "name" argument must be specified`);
      err.code = "ERR_MISSING_ARGS";
      throw err;
    }
    if (typeof name === "symbol") {
      throw new TypeError("Cannot convert a Symbol value to a string");
    }
    name = `${name}`;
    const sub = String(__wjs2_bc_sub(name));
    if (sub === "") {
      throw new Error("OperationError: BroadcastChannel is not initialized");
    }
    __bcState.add(this);
    this.__name = name;
    this.__sub = sub;
    this.__closed = false;
    this.__onmessage = null;
    // 无监听到达的广播排队（receiveMessageOnPort 同步收信——broadcastchannel
    // 套件 `receiveMessageOnPort(bc2).message` 点名；node 底层即 MessagePort）。
    this.__queue = [];
    this.__flushScheduled = false;
    this.__ev = this.__ev.bind(this);
    this.on("newListener", (ev) => { if (ev === "message") __wjs2_bc_flags(sub, "listen"); });
    // newListener 在入表前触发（§4.47）：延迟一轮再刷队。
    this.on("newListener", (ev) => { if (ev === "message") queueMicrotask(() => this.__maybeFlush()); });
    this.on("removeListener", (ev) => {
      if (ev === "message" && this.listenerCount("message") === 0) __wjs2_bc_flags(sub, "unlisten");
    });
    __wjs2_bc_attach(sub, this);
  }
  __maybeFlush() {
    if (this.__flushScheduled || this.__queue.length === 0) return;
    if (this.listenerCount("message") === 0) return;
    this.__flushScheduled = true;
    queueMicrotask(() => {
      this.__flushScheduled = false;
      this.__flushQueue();
    });
  }
  __flushQueue() {
    while (this.__queue.length > 0) {
      // 逐条查关（wpt：onmessage 内 close 阻断同端口已排队任务）+ 无监听停刷
      //（端口 once 陷阱同款 4.326；新监听经 newListener 续刷）。
      if (this.__closed) return;
      if (this.listenerCount("message") === 0) return;
      const raw = this.__queue.shift();
      let value;
      try {
        value = __fromWire(String(raw));
      } catch (err) {
        __workerEmitSafe(this, "messageerror", err instanceof Error ? err : new Error("worker message is not valid JSON"));
        continue;
      }
      __workerEmitSafe(this, "message", value);
    }
  }
  get name() {
    if (!__bcState.has(this)) throw __bcInvalidThis();
    return this.__name;
  }
  get onmessage() {
    if (!__bcState.has(this)) throw __bcInvalidThis();
    return this.__onmessage;
  }
  set onmessage(fn) {
    if (!__bcState.has(this)) throw __bcInvalidThis();
    if (this.__onmessage !== null) this.removeListener("message", this.__onmessageWrap);
    this.__onmessage = (typeof fn === "function") ? fn : null;
    if (this.__onmessage !== null) {
      // 真机口径：onmessage 收真 MessageEvent（data/target，broadcastchannel
      // 套件逐项）；EE 'message' 载荷裸值不变。
      this.__onmessageWrap = (value) => {
        try {
          this.__onmessage(new globalThis.MessageEvent("message", { data: value, __wjs2Target: this }));
        } catch { /* 忽略 */ }
      };
      this.on("message", this.__onmessageWrap);
    } else {
      this.__onmessageWrap = null;
    }
  }
  // EventTarget 双面（同 MessagePort：message 系 MessageEvent，自定义 CustomEvent）。
  addEventListener(type, fn) {
    if (typeof fn !== "function") return;
    const t = String(type);
    const isMsg = t === "message" || t === "messageerror";
    const per = (this.__etWrap ??= new WeakMap());
    let byType = per.get(fn);
    if (!byType) { byType = new Map(); per.set(fn, byType); }
    if (byType.has(t)) return;
    const wrap = (value) => {
      const ev = isMsg
        ? new globalThis.MessageEvent(t, { data: value, __wjs2Target: this })
        : new globalThis.CustomEvent(t, { detail: value });
      fn.call(this, ev);
    };
    byType.set(t, wrap);
    this.on(t, wrap);
  }
  removeEventListener(type, fn) {
    const wrap = this.__etWrap?.get(fn)?.get(String(type));
    if (wrap) {
      this.off(String(type), wrap);
      this.__etWrap.get(fn).delete(String(type));
    }
  }
  __ev(kind, payload) {
    if (kind !== "message") return;
    if (this.__closed) return;
    // 无监听即排队（同步收信面）；监听到达由 newListener 延迟刷（§4.47）。
    if (this.listenerCount("message") === 0) {
      this.__queue.push(payload);
      return;
    }
    let value;
    try {
      value = __fromWire(String(payload));
    } catch (e) {
      this.emit("messageerror", e instanceof Error ? e : new Error("worker message is not valid JSON"));
      return;
    }
    this.emit("message", value);
  }
  postMessage(value) {
    if (!__bcState.has(this)) throw __bcInvalidThis();
    // node broadcastchannel 套件口径：仅无参即抛；显式 undefined 合法（可投递）。
    if (arguments.length === 0) {
      const err = new TypeError(`The "message" argument must be specified`);
      err.code = "ERR_MISSING_ARGS";
      throw err;
    }
    if (this.__closed) throw new Error("BroadcastChannel is closed");
    __wjs2_bc_pub(this.name, this.__sub, __toWire(value));
  }
  close() {
    if (!__bcState.has(this)) throw __bcInvalidThis();
    if (this.__closed) return;
    this.__closed = true;
    __wjs2_bc_unsub(this.__sub);
  }
  ref() {
    if (!__bcState.has(this)) throw __bcInvalidThis();
    __wjs2_bc_flags(this.__sub, "ref");
    return this;
  }
  unref() {
    if (!__bcState.has(this)) throw __bcInvalidThis();
    __wjs2_bc_flags(this.__sub, "unref");
    return this;
  }
  // node 口径：inspect 形 "BroadcastChannel { name: 'x', active: true|false }"
  //（broadcastchannel 套件逐字；active 非真实属性——inspect 定制面）。
  [Symbol.for("nodejs.util.inspect.custom")]() {
    return `BroadcastChannel { name: ${__inspectQuote(this.__name)}, active: ${!this.__closed} }`;
  }
}

export const isMainThread = __wjs2_worker_is_main();
export const threadId = Number(__wjs2_worker_thread_id());
// 10f：本线程构造名（主线程 null；worker 无名亦 null——thread-name 套件）。
export const threadName = __wjs2_worker_name() || null;
const __parentId = String(__wjs2_worker_parent());
export const parentPort = (__parentId === "") ? null : new MessagePort(__parentId, __kPortInternal);
const __dataRaw = __wjs2_worker_data();
export const workerData = (__dataRaw === undefined || __dataRaw === "") ? null : __fromWire(String(__dataRaw));
export const resourceLimits = {};
export const SHARE_ENV = Symbol("SHARE_ENV");

// node worker bootstrap 口径：worker 内的进程操作族换 UNSUPPORTED_OPERATION 桩
// （unsupported-things 套件逐项点名：disabled 位/() 形/无 () 属性形三种）。
// 本模块在 worker 内首次求值时装上（worker 线程同一 prelude，__parentId 非空即 worker）。
if (__parentId !== "") {
  // 10f 对拍 + fork 兼容（§4.132 坑八）：fork 子进程（`__wjs2_forkChild`）有
  // IPC 通道——send/disconnect/channel/connected 是正道，不装桩；fork 的
  // 子端 src 随后覆写它们（getter 桩会让 strict 赋值直接炸，fork 全灭）。
  const __proc = globalThis.process;
  const __isFork = __wjs2_worker_is_fork() === true;
  const __throwUnsupported = (msg) => {
    const err = new Error(msg);
    err.code = "ERR_WORKER_UNSUPPORTED_OPERATION";
    throw err;
  };
  for (const fn of ["abort", "chdir", "setuid", "seteuid",
    "setgid", "setegid", "setgroups", "initgroups"]) {
    const stub = function () { __throwUnsupported(`process.${fn}() is not supported in workers`); };
    stub.disabled = true;
    try { __proc[fn] = stub; } catch { /* 只读面跳过 */ }
  }
  if (!__isFork) {
    for (const fn of ["send", "disconnect"]) {
      const stub = function () { __throwUnsupported(`process.${fn}() is not supported in workers`); };
      stub.disabled = true;
      try { __proc[fn] = stub; } catch { /* 只读面跳过 */ }
    }
    for (const prop of ["channel", "connected"]) {
      try {
        Object.defineProperty(__proc, prop, {
          get() { __throwUnsupported(`process.${prop} is not supported in workers`); },
          configurable: true,
        });
      } catch { /* 同上 */ }
    }
  }
  try {
    __proc.umask = (mask) => {
      if (mask !== undefined) {
        __throwUnsupported("Setting process.umask() is not supported in workers");
      }
      return __wjs2_umask();
    };
  } catch { /* 同上 */ }
  for (const k of ["_startProfilerIdleNotifier", "_stopProfilerIdleNotifier",
    "_debugProcess", "_debugPause", "_debugEnd"]) {
    try { delete __proc[k]; } catch { /* 同上 */ }
  }
}

export function setEnvironmentData(key, value) {
  if (typeof key !== "string") {
    const err = new TypeError(`The "key" argument must be of type string. Received type ${typeof key}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  __wjs2_worker_env_set(key, __toWire(value));
}
export function getEnvironmentData(key) {
  if (typeof key !== "string") {
    const err = new TypeError(`The "key" argument must be of type string. Received type ${typeof key}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  const raw = __wjs2_worker_env_get(key);
  if (raw === undefined) return undefined;
  return __fromWire(String(raw));
}

// node lib/internal/worker.js 口径：URL(data:)/URL(file:)/绝对串/./.. 串四路，
// 其余 ERR_WORKER_PATH（file:///data: 串带 Wrap 提示）；非法类型 ARG_TYPE 数组形。
function __workerFilePath(filename) {
  const isUrlLike = typeof filename === "object" && filename !== null && typeof filename.href === "string";
  if (isUrlLike && String(filename.href).startsWith("data:")) {
    // node 口径：data: URL 解码为源码走 eval（utf8/base64 双形）。
    const href = String(filename.href);
    const comma = href.indexOf(",");
    const meta = href.slice(5, comma === -1 ? href.length : comma);
    const payload = comma === -1 ? "" : href.slice(comma + 1);
    const decoded = meta.endsWith(";base64") ? atob(payload) : decodeURIComponent(payload);
    return { src: decoded, isEval: "1" };
  }
  if (isUrlLike) {
    const href = String(filename.href);
    if (href.startsWith("file://")) {
      try {
        return { src: decodeURIComponent(new URL(href).pathname), isEval: "0" };
      } catch { /* 落下走报错 */ }
    }
    // node 口径：URL 实例非 file: 即 ERR_INVALID_URL_SCHEME（unsupported-path 套件）。
    throw new codes.ERR_INVALID_URL_SCHEME("file");
  }
  if (typeof filename === "string") {
    if (filename.startsWith("/") || /^\.\.?[\\/]/.test(filename)) {
      return { src: filename, isEval: "0" };
    }
    const hint = (filename.startsWith("file://") ? " Wrap file:// URLs with `new URL`." : "") +
      (filename.startsWith("data:text/javascript") ? " Wrap data: URLs with `new URL`." : "");
    const err = new TypeError(
      `The worker script or module filename must be an absolute path or a relative path starting with './' or '../'.${hint} Received "${filename}"`);
    err.code = "ERR_WORKER_PATH";
    throw err;
  }
  throw new codes.ERR_INVALID_ARG_TYPE("filename", ["string", "URL"], filename);
}

export class Worker extends EventEmitter {
  // 子线程标出流（node 真机：stdout/stderr 恒 Readable 管道——子输出经 WStdio
  // 事件进父流并本地 TEE；stdin 仅 stdin:true 具现 Writable）。
  __stdin = null;
  __stdout = null;
  __stderr = null;
  constructor(filename, options = {}) {
    super();
    if (options === null || (typeof options !== "object" && typeof options !== "function")) {
      const err = new TypeError(`The "options" argument must be of type object.`);
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    // node lib/internal/worker.js 校验序：eval 门 → filename 路由 → env/name/execArgv。
    if (options.eval && typeof filename !== "string") {
      throw new codes.ERR_INVALID_ARG_VALUE("options.eval", options.eval, "must be false when 'filename' is not a string");
    }
    const { src, isEval } = options.eval
      ? { src: String(filename), isEval: "1" }
      : __workerFilePath(filename);
    // env（node internal/worker.js 逐字口径）：对象（含数组）逐值 `${v}` 化；
    // null/undefined 继承——本仓落成创建时快照（worker 隔离，10f process-env）；
    // SHARE_ENV 共享真 env（快照 wire 置空即原语义）；其余 ARG_TYPE（message
    // 由 errors.js 逐字渲染，套件断全文）。
    const envOpt = options.env;
    let envJson;
    if (typeof envOpt === "object" && envOpt !== null) {
      const envObj = {};
      for (const [k, v] of Object.entries(envOpt)) envObj[k] = `${v}`;
      envJson = JSON.stringify(envObj);
    } else if (envOpt == null) {
      envJson = JSON.stringify(process.env);
    } else if (envOpt === SHARE_ENV) {
      envJson = "";
    } else {
      throw new codes.ERR_INVALID_ARG_TYPE("options.env", ["object", "undefined", "null", "worker_threads.SHARE_ENV"], envOpt);
    }
    if (options.name !== undefined && typeof options.name !== "string") {
      throw new codes.ERR_INVALID_ARG_TYPE("options.name", "string", options.name);
    }
    if (options.execArgv !== undefined &&
        (!Array.isArray(options.execArgv) || options.execArgv.some((s) => typeof s !== "string"))) {
      throw new codes.ERR_INVALID_ARG_TYPE("options.execArgv", ["string[]"], options.execArgv);
    }
    // execArgv 内容校验（node internal/worker.js：C++ 侧 invalidExecArgv →
    // ERR_WORKER_INVALID_EXEC_ARGV；本仓按真机探针矩阵近似：允许集外即拒，
    // --redirect-warnings/--disable-warning 及须带值旗仅裸形拒；未知旗即拒）。
    // 真机探针基线 node 26.8.1（约 160 旗全量对拍，见 journal）。
    const __kWorkerExecArgvAllowed = new Set([
      "no-deprecation", "no-warnings", "trace-deprecation", "trace-warnings",
      "pending-deprecation", "throw-deprecation", "disable-warning", "trace-uncaught",
      "unhandled-rejections", "inspect", "inspect-brk", "inspect-publish-uid",
      "debug-port", "experimental-vm-modules", "experimental-strip-types",
      "no-experimental-require-module", "experimental-require-module",
      "preserve-symlinks", "preserve-symlinks-main", "no-network-family-autoselection",
      "enable-network-family-autoselection", "network-family-autoselection-attempt-timeout",
      "dns-result-order", "redirect-warnings", "trace-sync-io", "trace-tls",
      "trace-promises", "trace-exit", "trace-env", "trace-env-js-stack",
      "trace-env-native-stack", "frozen-intrinsics", "report-exclude-env",
      "report-exclude-network", "report-signal", "report-uncaught-exception",
      "heap-prof", "cpu-prof", "diagnostic-dir", "input-type", "print", "check",
      "test", "watch", "env-file", "permission", "allow-worker", "allow-addons",
      "allow-child-process", "allow-net", "allow-wasi", "allow-openssl-store",
      "async-context-frame", "no-async-context-frame", "no-addons", "disable-sigusr1",
      "use-system-ca", "no-use-system-ca", "max-http-header-size", "tls-min-v1.2",
      "tls-max-v1.3", "use-env-proxy", "no-global-search-paths", "force-context-aware",
      "test-udp-no-try-send", "permission-audit", "experimental-eventsource",
      "experimental-websocket", "no-experimental-websocket", "experimental-sqlite",
      "no-experimental-sqlite", "experimental-vfs", "experimental-worker-inspection",
      "experimental-network-inspection", "experimental-storage-inspection",
      "experimental-test-coverage", "experimental-addon-modules", "experimental-ffi",
      "experimental-import-meta-resolve", "experimental-print-required-tla",
      "experimental-quic", "experimental-stream-iter", "webstorage", "localstorage-file",
    ]);
    // 裸形（无 =值）即拒：装饰双形二旗 + 必须带值旗（node CLI 同款拒收）。
    const __kWorkerExecArgvDenyBare = new Set([
      "redirect-warnings", "disable-warning", "unhandled-rejections", "dns-result-order",
      "report-signal", "heapsnapshot-signal", "trace-event-categories",
      "trace-event-file-pattern",
    ]);
    // 预加载收集器（execArgv 校验期收集 --require/-r 值；NODE_OPTIONS 期置空）。
    let __preload = [];
    const __workerBadExecArgv = (list) => {
      const bad = [];
      // --require/-r/--import 带值旗（node 真机口径：长形支持 =值（须非空）与
      // 分离值（下个条目非 -开头即消费，空串亦消费、运行时再炸）；短形 -r 仅
      // 分离值（=形即拒）；缺值拒文案为 "<flag> requires an argument"）。
      // 命中值即记入预加载清单（--require/-r 按序执行；--import 仅放行，ESM
      // 预加载另案）。`__preload` 为共享收集器（execArgv + NODE_OPTIONS 同口径，
      // 后者只校验不执行）。
      const valueFlags = new Set(["--require", "-r", "--import"]);
      const consume = (i, head, eq, entry) => {
        if (eq !== -1 && head === "-r") { bad.push(entry); return i; }
        let value = eq === -1 ? null : entry.slice(eq + 1);
        if (value === null) {
          const next = list[i + 1];
          if (next === undefined || next.startsWith("-")) {
            bad.push(`${head} requires an argument`);
            return i;
          }
          value = next;
          i++;
        } else if (value === "") {
          // =形空值拒文案为完整合成串（真机："--require= requires an argument"）。
          bad.push(`${entry} requires an argument`);
          return i;
        }
        if (__preload !== null && (head === "--require" || head === "-r")) {
          __preload.push(value);
        }
        return i;
      };
      for (let i = 0; i < list.length; i++) {
        const entry = list[i];
        const eq = entry.indexOf("=");
        const head = eq === -1 ? entry : entry.slice(0, eq);
        if (valueFlags.has(head)) {
          i = consume(i, head, eq, entry);
          continue;
        }
        const base = head.replace(/^--/, "");
        if (!__kWorkerExecArgvAllowed.has(base) || (eq === -1 && __kWorkerExecArgvDenyBare.has(base))) {
          bad.push(entry);
        }
      }
      return bad;
    };
    if (options.execArgv !== undefined) {
      const badExec = __workerBadExecArgv(options.execArgv);
      if (badExec.length > 0) throw new codes.ERR_WORKER_INVALID_EXEC_ARGV(badExec);
    }
    const preloadJson = __preload.length > 0 ? JSON.stringify(__preload) : "";
    __preload = null;
    // env 内 NODE_OPTIONS 同口径校验（空白切分近似；仅串形校验，只校验不执行）。
    if (typeof envOpt === "object" && envOpt !== null && typeof envOpt.NODE_OPTIONS === "string" &&
        envOpt.NODE_OPTIONS !== "") {
      const badNodeOpts = __workerBadExecArgv(envOpt.NODE_OPTIONS.split(/\s+/).filter((s) => s !== ""));
      if (badNodeOpts.length > 0) {
        throw new codes.ERR_WORKER_INVALID_EXEC_ARGV(badNodeOpts, "invalid NODE_OPTIONS env variable");
      }
    }
    // argv（node internal/worker.js:215-218：truthy 即 validateArray + String 化；
    // falsy 即缺省；worker 侧 process.argv 透传，eval 形补 '[worker eval]'）。
    let argvJson = "";
    if (options.argv) {
      if (!Array.isArray(options.argv)) {
        throw new codes.ERR_INVALID_ARG_TYPE("options.argv", "Array", options.argv);
      }
      argvJson = JSON.stringify(options.argv.map(String));
    }
    let dataJson = "";
    if (options.workerData !== undefined) {
      dataJson = __toWire(options.workerData, options.transferList);
    }
    let ids;
    // 第 8/9 位：stdio 管道独占位（"1"/"2"/"12"）+ --require 预加载清单（JSON）。
    const stdioPipe = `${options.stdout ? "1" : ""}${options.stderr ? "2" : ""}`;
    ids = String(__wjs2_worker_spawn(src, isEval, dataJson, options.name ?? "", options.__wjs2_forkChild ? "1" : "0", envJson, argvJson, stdioPipe, preloadJson)).split(" ");
    this.__id = ids[0];
    this.__tid = Number(ids[1]);
    this.__exited = null;
    this.__name = options.name ?? null;
    this.__rl = options.resourceLimits ?? null;
    this.__termWaiters = [];
    this.__cpuWaiters = new Map();
    // 缺省双管（node 真机：stdout/stderr 恒 Readable，stdin 仅 stdin:true 才具现；
    // 旧"恒 null"断言已随实测翻转，见黑盒；stdout:true/stderr:true 即管道独占、
    // 本地不回显，独占位随 spawn 第 8 参进 worker 会话）。
    this.__stdin = options.stdin ? new __WorkerStdin(this.__id) : null;
    this.__stdout = new __WorkerStdio();
    this.__stderr = new __WorkerStdio();
    this.__ev = this.__ev.bind(this);
    __wjs2_worker_attach(this.__id, this);
    // 10f 对拍：新建 Worker 派发 process 'worker' 事件（worker-event 套件；
    // 真机异步，microtask 递延；参数为 Worker 实例本身）。
    const __self = this;
    queueMicrotask(() => {
      try {
        globalThis.process?.emit?.("worker", __self);
      } catch { /* 忽略 */ }
    });
  }
  __ev(kind, payload) {
    if (kind === "message") {
      let value;
      try {
        value = __fromWire(String(payload));
        // Worker 'message' 无 ports 面——残留 offer 槽撤回收（行内已承接即 no-op）。
        __withdrawPorts(String(payload));
      } catch (e) {
        __workerEmitSafe(this, "messageerror", e instanceof Error ? e : new Error("worker message is not valid JSON"));
        return;
      }
      __workerEmitSafe(this, "message", value);
    } else if (kind === "error") {
      // 10f 对拍：Rust 侧 "Kind: message" 前缀还原原错误类（SyntaxError 套件
      // 断 err.constructor === SyntaxError）；裸文本回 Error（uncaught-exception
      // 套件断 String(err) === 'Error: foo'）；`__wjs2_prim:` 信封还原原始值
      // （error-primitive 套件断 err === 42 / Symbol.for('a') 等）。
      const text = String(payload);
      if (text.startsWith("__wjs2_prim:")) {
        this.emit("error", __wjs2_primFromText(text.slice("__wjs2_prim:".length)));
        return;
      }
      const m = /^(SyntaxError|TypeError|RangeError|EvalError|ReferenceError|URIError|AggregateError): ([\s\S]*)$/.exec(text);
      __workerEmitSafe(this, "error", m ? new globalThis[m[1]](m[2]) : new Error(text));
    } else if (kind === "exit") {
      const code = Number(payload);
      this.__exited = code;
      if (this.__stdout) this.__stdout.__end();
      if (this.__stderr) this.__stderr.__end();
      const waiters = this.__termWaiters.splice(0);
      for (const w of waiters) {
        try { w(code); } catch { /* 忽略 */ }
      }
      __workerEmitSafe(this, "exit", code);
    } else if (kind === "online") {
      __workerEmitSafe(this, "online");
    } else if (kind === "stdout" || kind === "stderr") {
      // 子输出事件（WStdio；exit 前必达——同 inbox 保序，flush 套件点名）。
      const target = kind === "stdout" ? this.__stdout : this.__stderr;
      try { target?.__pushWire(String(payload)); } catch { /* 流已摘即丢 */ }
    } else if (kind === "cpu-result") {
      // CPU 采样回执（WCpuResult；nonce 对账结算，退出竞态丢弃）。
      let obj;
      try { obj = JSON.parse(String(payload)); } catch { return; }
      const w = this.__cpuWaiters.get(obj?.nonce);
      if (!w) return;
      this.__cpuWaiters.delete(obj.nonce);
      const cur = { user: Number(obj.user), system: Number(obj.system) };
      if (w.prev) w.resolve({ user: cur.user - w.prev.user, system: cur.system - w.prev.system });
      else w.resolve(cur);
    }
  }
  postMessage(value, transfer) {
    const wire = __toWire(value, transfer);
    __wjs2_worker_post(this.__id, wire);
  }
  terminate() {
    // 10f：terminate 即 ref（terminate-unrefed 套件——unref 后 terminate 仍须
    // 决议；退出即摘续命，ref 不泄漏）。
    try { this.ref(); } catch { /* 忽略 */ }
    try { __wjs2_worker_terminate(this.__id); } catch { /* 已退出即走下 */ }
    if (this.__exited !== null) return Promise.resolve(this.__exited);
    return new Promise((resolve) => { this.__termWaiters.push(resolve); });
  }
  ref() {
    __wjs2_worker_set_ref(this.__id, "1");
    return this;
  }
  unref() {
    __wjs2_worker_set_ref(this.__id, "0");
    return this;
  }
  get threadId() {
    // 10f 对拍：退出后 threadId 恒 -1（safe-getters 套件点名；真机口径）。
    return this.__exited === null ? this.__tid : -1;
  }
  // 10f 对拍：threadName 运行期 = options.name（无名 null），exit 后恒 null；
  // resourceLimits = 传入对象（缺省 {}，退出后 {}——resource-limits 套件）。
  get threadName() { return this.__exited === null ? (this.__name ?? null) : null; }
  get resourceLimits() { return this.__rl ?? {}; }
  get stdin() { return this.__stdin; }
  get stdout() { return this.__stdout; }
  get stderr() { return this.__stderr; }
  // CPU 用量（node internal/worker.js:565-594：prev 真值即对象 + 双 numeric
  // [0, 2^53-1] 校验，同步抛；采样走 worker 会话自采回执，promise 结算；
  // 已退出/无路由即 ERR_WORKER_NOT_RUNNING 拒——cpu-usage 套件）。
  cpuUsage(prev) {
    if (prev) {
      if (typeof prev !== "object" || prev === null) {
        throw new codes.ERR_INVALID_ARG_TYPE("prev", "object", prev);
      }
      for (const k of ["user", "system"]) {
        const v = prev[k];
        if (typeof v !== "number") {
          throw new codes.ERR_INVALID_ARG_TYPE(`prev.${k}`, "number", v);
        }
        if (!(v >= 0 && v <= 9007199254740991)) {
          throw new codes.ERR_OUT_OF_RANGE(`prev.${k}`, ">= 0 && <= 9007199254740991", v);
        }
      }
    }
    if (this.__exited !== null) {
      return Promise.reject(new codes.ERR_WORKER_NOT_RUNNING());
    }
    const nonce = `${this.__id}.${++__cpuSeq}`;
    return new Promise((resolve, reject) => {
      this.__cpuWaiters.set(nonce, { resolve, reject, prev: prev || null });
      let queued = false;
      try {
        queued = __wjs2_worker_cpu_sample(this.__id, nonce) === true;
      } catch {
        queued = false;
      }
      if (!queued) {
        this.__cpuWaiters.delete(nonce);
        reject(new codes.ERR_WORKER_NOT_RUNNING());
      }
    });
  }
}
// native 源发射安全垫（worker exit/message/error/online 监听抛错走
// uncaughtException + 进程存活，真机口径；同步透传即 fatal，
// exit-event-error 套件点名）。
function __workerEmitSafe(emitter, ev, ...args) {
  try {
    emitter.emit(ev, ...args);
  } catch (e) {
    setTimeout(() => { throw e; }, 0);
  }
}
// MessageEvent 的 source/ports 校验需要 MessagePort 品牌（主线程无全局
// MessagePort，模块求值期登记到隐藏槽；求值前无从有端口，校验恒 TypeError）。
globalThis.__wjs2_MessagePort = MessagePort;
// 10f：BroadcastChannel 是 Web 全局（node 主/worker 线程均全局，messaging
// 套件裸 `new BroadcastChannel(...)`；与 worker_threads 导出同一类）。
globalThis.BroadcastChannel = BroadcastChannel;
// 子输出流（node 真机 Readable：WStdio 事件经 __pushWire 进流；setEncoding/
// pipe/unpipe 全继承；`__end` 在 worker 退出时由分发调用——push(null) 收尾）。
class __WorkerStdio extends __wjs2Readable {
  constructor() {
    super({ read() {} });
    // 基类 readableEnded/destroyed 只读——终态记自有旗（旧面 readableEnded 照读基类）。
    this.__ended = false;
  }
  // WStdio wire（JSON：{s} 串直推 / {b} base64 转 Buffer 推；坏线即丢）。
  __pushWire(wire) {
    if (this.__ended) return;
    let obj;
    try { obj = JSON.parse(String(wire)); } catch { return; }
    if (obj && typeof obj === "object" && typeof obj.b === "string") {
      let buf;
      try { buf = Buffer.from(obj.b, "base64"); } catch { return; }
      this.push(buf);
    } else if (obj && typeof obj === "object" && typeof obj.s === "string") {
      this.push(obj.s);
    }
  }
  __end() {
    if (this.__ended) return;
    this.__ended = true;
    try { this.push(null); } catch { /* 已摘即止 */ }
  }
}

// 子输入流（stdin:true 具现；_write 经 WStdin 事件进 worker 会话）。
class __WorkerStdin extends __wjs2Writable {
  constructor(id) {
    super({
      write: (chunk, encoding, cb) => {
        let wire;
        try {
          if (typeof chunk === "string") wire = JSON.stringify({ s: chunk });
          else if (typeof Buffer !== "undefined" && Buffer.isBuffer(chunk)) {
            wire = JSON.stringify({ b: chunk.toString("base64") });
          } else if (chunk instanceof Uint8Array) {
            wire = JSON.stringify({ b: Buffer.from(chunk).toString("base64") });
          } else {
            wire = JSON.stringify({ s: String(chunk) });
          }
        } catch { wire = JSON.stringify({ s: "" }); }
        try { __wjs2_worker_stdin(id, wire); } catch { /* 对端已退即丢 */ }
        cb();
      },
    });
    this.__id = id;
  }
  // 显式 end 即对端 EOF（worker 侧 stdin 推 end；echo 管道收尾用）。
  _final(cb) {
    try { __wjs2_worker_stdin(this.__id, '{"end":1}'); } catch { /* 对端已退即丢 */ }
    cb();
  }
}

// ── 线程直投（postMessageToThread；node lib/internal/worker/messaging.js 口径）
// 路由：worker 起服 rendezvous 即登记进程级 threadId→收件箱（Rust 侧，与派生者
// 无关）；0 号目标直走发送方会话 worker_main_inbox（node 主线程接收特殊化）。
// 主线程发送序探针先行（node 主线程先查路由后序列化），worker 线程组装先行
// （node worker 侧 postMessage 即克隆，路由失败亦先抛 DataCloneError）；同线程
// SAME_THREAD；无路由/无监听 FAILED；监听抛错 ERRORED；timeout 竞速 TIMEOUT。
// transfer 走 __toWire 信封（offer/accept 表进程级，端口跨线程照常迁移）。
const __selfTid = threadId;
function __threadErr(code, message) {
  const err = new Error(message);
  err.code = code;
  return err;
}
const __threadPending = new Map();
let __threadSeq = 0;
// CPU 采样 nonce 序号（worker id 前缀，`__cpuWaiters` 对账用）。
let __cpuSeq = 0;
function __threadBridgeEv(kind, payload) {
  try {
    if (kind === "thread-ack") {
      const s = String(payload);
      const i = s.lastIndexOf(" ");
      const nonce = s.slice(0, i);
      const w = __threadPending.get(nonce);
      if (w === undefined) return "ok";
      __threadPending.delete(nonce);
      if (w.timer !== null) clearTimeout(w.timer);
      const code = s.slice(i + 1);
      if (code === "0") w.resolve();
      else if (code === "2") w.reject(__threadErr("ERR_WORKER_MESSAGING_ERRORED", "The destination thread threw an error while processing the message"));
      else w.reject(__threadErr("ERR_WORKER_MESSAGING_FAILED", "Cannot find the destination thread or listener"));
      return "ok";
    }
    if (kind !== "thread-msg") return "1";
    const s = String(payload);
    const i = s.indexOf("\n");
    const j = s.indexOf("\n", i + 1);
    const source = Number(s.slice(i + 1, j));
    const wire = s.slice(j + 1);
    let value;
    try {
      value = __fromWirePorts(wire).message;
      __withdrawPorts(wire);
    } catch {
      return "2";
    }
    try {
      return globalThis.process.emit("workerMessage", value, source) ? "0" : "1";
    } catch {
      return "2";
    }
  } catch {
    return "2";
  }
}
// 收发共用桥接（Rust 侧保留 id 登记，会话生命期常驻；模块求值期 attach 一次）。
__wjs2_thread_bridge_attach({ __ev: __threadBridgeEv });
export async function postMessageToThread(threadId, value, transferList, timeout) {
  // node 口径：第三参数字 + 无第四参即 timeout 简写。
  if (typeof transferList === "number" && timeout === undefined) {
    timeout = transferList;
    transferList = [];
  }
  if (timeout !== undefined) {
    if (typeof timeout !== "number") throw new codes.ERR_INVALID_ARG_TYPE("timeout", "number", timeout);
    if (Number.isNaN(timeout) || timeout < 0) throw new codes.ERR_OUT_OF_RANGE("timeout", ">= 0", timeout);
  }
  if (threadId === __selfTid) {
    throw __threadErr("ERR_WORKER_MESSAGING_SAME_THREAD", "Cannot sent a message to the same thread");
  }
  const dest = (typeof threadId === "number" && Number.isInteger(threadId) && threadId >= 0) ? String(threadId) : "";
  const nonce = `tm${__selfTid}.${++__threadSeq}.${Math.floor(Math.random() * 36 ** 6).toString(36)}`;
  if (isMainThread) {
    // 主线程序：先探针（无路由即 FAILED，不组装）。
    if (__wjs2_post_to_thread(dest, "", "", "") !== true) {
      throw __threadErr("ERR_WORKER_MESSAGING_FAILED", "Cannot find the destination thread or listener");
    }
  }
  const wire = __toWire(value, transferList);
  return new Promise((resolve, reject) => {
    const waiter = { resolve, reject, timer: null };
    __threadPending.set(nonce, waiter);
    if (timeout !== undefined && Number.isFinite(timeout)) {
      waiter.timer = setTimeout(() => {
        if (__threadPending.delete(nonce)) {
          reject(__threadErr("ERR_WORKER_MESSAGING_TIMEOUT", "Sending a message to another thread timed out"));
        }
      }, timeout);
    }
    let queued = false;
    try {
      queued = __wjs2_post_to_thread(dest, nonce, String(__selfTid), wire) === true;
    } catch {
      queued = false;
    }
    if (!queued) {
      __threadPending.delete(nonce);
      if (waiter.timer !== null) clearTimeout(waiter.timer);
      reject(__threadErr("ERR_WORKER_MESSAGING_FAILED", "Cannot find the destination thread or listener"));
    }
  });
}

const __api = {
  isMainThread, threadId, threadName, parentPort, workerData, resourceLimits, SHARE_ENV,
  MessagePort, MessageChannel, BroadcastChannel, Worker, markAsUncloneable,
  markAsUntransferable, isMarkedAsUntransferable,
  moveMessagePortToContext, receiveMessageOnPort,
  setEnvironmentData, getEnvironmentData, postMessageToThread,
};
export default __api;
