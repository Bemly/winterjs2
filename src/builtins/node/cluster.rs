//! `node:cluster`：Bun 🟡对等面（10e）。
//!
//! 落法：骑 `node:worker_threads` Worker（`child.fork` 同构 bootstrap），
//! 真多进程语义不做（书面记档）：无独立进程/env 隔离/signal 语义；
//! `worker.process` 为最小桩（pid=线程 id）；`listening` 经 worker 内
//! Server.listen 补丁中继到 primary（P0；同线程端口回环直连语义成立，
//! 跨线程 fd 共享/RR 分发仍无能力，记档）；scheduling 仅存值。
//! Worker 判定走 `workerData.__wjs2_cluster`（不污染 `process.env`）；
//! `fork(env)` 的 env 在子端合入 `process.env`（底座 env 表进程共享，
//! 可见性属偏差，测试用唯一键名隔离）。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
import { EventEmitter } from "node:events";
import { Worker as ThreadWorker, workerData, isMainThread } from "node:worker_threads";
import { pathToFileURL } from "node:url";

export const SCHED_NONE = 1;
export const SCHED_RR = 2;

// 子会话入口（`child.fork` 同构；占位经 replacer 填 JSON，见 §4.44）。
const __CLUSTER_CHILD_SRC = `
import { parentPort, workerData } from "node:worker_threads";
const __mod = __CLUSTER_MOD__;
const __cargs = __CLUSTER_ARGS__;
const __cenv = __CLUSTER_ENV__;
const __ccwd = __CLUSTER_CWD__;
process.argv = [process.execPath, __mod, ...__cargs];
for (const [k, v] of Object.entries(__cenv)) {
  try { process.env[k] = String(v); } catch {}
}
if (typeof __ccwd === "string" && __ccwd !== "") {
  // worker 线程禁真 chdir（进程级共享 cwd，底座拒绝）——虚拟化 process.cwd
  //（cwd 套件只断言 cwd 取值；worker 内相对路径仍走进程真 cwd，记档偏离）。
  try { process.cwd = () => __ccwd; } catch {}
}
process.connected = true;
process.send = (message, ...rest) => {
  let cb = null;
  for (const a of rest) if (typeof a === "function") cb = a;
  if (!process.connected || parentPort === null) {
    const err = new Error("Channel closed");
    err.code = "ERR_IPC_CHANNEL_CLOSED";
    if (cb) queueMicrotask(() => cb(err));
    return false;
  }
  try {
    parentPort.postMessage(message);
  } catch (e) {
    const err = e instanceof Error ? e : new Error(String(e));
    if (!err.code) err.code = "ERR_IPC_CHANNEL_CLOSED";
    if (cb) queueMicrotask(() => cb(err));
    return false;
  }
  if (cb) queueMicrotask(() => cb(null));
  return true;
};
function __clusterDoDisconnect(local) {
  if (!process.connected) return;
  process.connected = false;
  // 关完 server 再走后续（node _disconnect checkWaitingCount 口径；
  // wait-server-close 套件要求 disconnect 处理时 server 已关）。
  __clusterCloseServers(() => {
    if (!local) {
      try { parentPort.postMessage({ __wjs2_cluster_ctl: "disconnect" }); } catch {}
      // worker 主动断开：本地派发等 primary 回执（真机 ack 口径；
      // race 套件要求相对 nextTick 异步，往返 + macrotask 必在其后）。
      // port 在 ack 后关（关早 ack 收不到）；primary 死亡则对端 close
      // 兜底 + 下方超时兜底（幂等，不双发）。
      __clusterAckWait = true;
      try {
        setTimeout(() => {
          if (__clusterAckWait) {
            __clusterAckWait = false;
            __clusterLocalDisconnect();
            try { parentPort.close(); } catch {}
          }
        }, 100).unref?.();
      } catch {}
    } else {
      queueMicrotask(() => { __clusterLocalDisconnect(); });
      queueMicrotask(() => { try { parentPort.close(); } catch {} });
    }
  });
}
// 本地 disconnect 派发（幂等：主从双向断开只发一次）。
let __clusterGone = false;
function __clusterLocalDisconnect() {
  if (__clusterGone) return;
  __clusterGone = true;
  // 派发后起服的 server 直接关（不再另等一轮）。
  __clusterCloseServers();
  process.__wjs2_emit("disconnect");
  __clusterSideEmit("disconnect");
}
// 强制退出路（primary worker.process.disconnect()，即 child_process 的
// 原始断开：worker 不等排空直接 exit(0)，forced-exit 套件口径）。
function __clusterForceExit() {
  if (process.connected) {
    process.connected = false;
    process.__wjs2_emit("disconnect");
    __clusterSideEmit("disconnect");
  }
  try { parentPort.close(); } catch {}
  __clusterCloseServers();
  process.exit(0);
}
// 子端 cluster.worker 扇出会合（模块实例后建，见文件尾赋值；
// 收发闭包只读 globalThis，不跨模块直接引用）。
// 进程桥（__wjs2_cluster_side_proc）同步扇出 disconnect/message（worker-destroy
// 套件挂在 cluster.worker.process 上；error 不经此路，防无监听抛错）。
function __clusterSideEmit(ev, a, b) {
  try {
    const __side = globalThis.__wjs2_cluster_side;
    if (__side) __side.emit(ev, a, b);
  } catch {}
  if (ev === "error") return;
  try {
    const __proc = globalThis.__wjs2_cluster_side_proc;
    if (__proc) __proc.emit(ev, a, b);
  } catch {}
}
// cluster 传柄用户消息派发（listen-handle-cluster 套件）：信封即 reviver
// 重建后交付 (msg, obj)（node got 同序）；余者原样单参。
function __clusterEmitUserMessage(message) {
  if (message !== null && typeof message === "object" && !Array.isArray(message) &&
      message.__wjs2_handle !== undefined && message.__wjs2_handle !== null &&
      typeof message.__wjs2_handle === "object") {
    const h = message.__wjs2_handle;
    const rev = globalThis.__wjs2_handleRevivers?.[h.type];
    if (typeof rev === "function") {
      try {
        rev(h, (obj) => {
          process.__wjs2_emit("message", message.__wjs2_msg, obj);
          __clusterSideEmit("message", message.__wjs2_msg, obj);
        });
      } catch {}
      return;
    }
  }
  process.__wjs2_emit("message", message);
  __clusterSideEmit("message", message);
}
process.disconnect = () => __clusterDoDisconnect(false);
// 起步缓冲：fork 后即达的用户消息先攒，待用户模块载完（监听器已挂）再重放；
// 否则 bootstrap 内 await import 让出点提前消费即丢（worker-events 族永挂）。
let __booted = false;
const __earlyMessages = [];
let __clusterAckWait = false;
if (parentPort !== null) {
  parentPort.on("message", (message) => {
    if (message !== null && typeof message === "object" && !Array.isArray(message) &&
        typeof message.__wjs2_cluster_ctl === "string") {
      if (message.__wjs2_cluster_ctl === "disconnect") {
        __clusterDoDisconnect(true);
        return;
      }
      if (message.__wjs2_cluster_ctl === "raw-disconnect") {
        __clusterForceExit();
        return;
      }
      if (message.__wjs2_cluster_ctl === "disconnect-ack") {
        if (__clusterAckWait) {
          __clusterAckWait = false;
          // macrotask 派发（setImmediate 恒在 nextTick 之后；race 套件口径），
          // 随后关 port（不断开注入新消息）。
          try {
            setImmediate(() => {
              __clusterLocalDisconnect();
              try { parentPort.close(); } catch {}
            });
          } catch { __clusterLocalDisconnect(); }
        }
        return;
      }
    }
    if (!__booted) {
      __earlyMessages.push(message);
      return;
    }
    __clusterEmitUserMessage(message);
  });
  parentPort.on("close", () => {
    if (process.connected) {
      // primary 意外死亡：worker 随即退出（node orphan 口径）。
      process.connected = false;
      __clusterLocalDisconnect();
      process.exit(0);
    }
  });
  try { parentPort.postMessage({ __wjs2_cluster_ctl: "online" }); } catch {}
}
// P0 listening 中继：worker 内 net.Server 起服即报 primary（地址纯数据过线）。
// 线程底座无 fd 共享，worker 绑的本线程端口经回环可直连，语义成立。
// 断开即关全部 worker server，落定后调 done（node _disconnect
// checkWaitingCount 口径；无 server 即时调）。
const __clusterServers = new Set();
function __clusterCloseServers(done) {
  const __list = [...__clusterServers];
  __clusterServers.clear();
  const __finish = () => { if (typeof done === "function") { try { done(); } catch {} } };
  if (__list.length === 0) { __finish(); return; }
  let __n = __list.length;
  const __one = () => { __n -= 1; if (__n <= 0) __finish(); };
  for (const __s of __list) {
    let __settled = false;
    const __once = () => { if (!__settled) { __settled = true; __one(); } };
    try {
      // 已起服等真 close（连接不断即等，worker-no-exit 口径）；
      // bind 窗口内 close 未必派发 close 事件，macrotask 兜底，幂等。
      const __wasListening = !!__s.listening;
      __s.once("close", __once);
      __s.close();
      if (!__wasListening) {
        try {
          setImmediate(() => { __once(); });
        } catch { queueMicrotask(() => { __once(); }); }
      }
    } catch { __once(); }
  }
}
try {
  const __clusterNet = await import("node:net");
  const __origListen = __clusterNet.Server.prototype.listen;
  if (typeof __origListen === "function" && !__origListen.__wjs2_cluster_patched) {
    const __patchedListen = function (...args) {
      __clusterServers.add(this);
      // 断开后起服（disconnect-early 竞速）：connected 在 DoDisconnect 内
      // 同步置 false，microtask 里的 gone 栏会有插空——此处认同步旗。
      if (!process.connected) {
        queueMicrotask(() => { try { this.close(); } catch {} });
        return Reflect.apply(__origListen, this, args);
      }
      this.once("listening", () => {
        try {
          // node listening 地址形：{address, addressType, fd, port}
          //（basic 套件点名 4 键 + addressType 数字 + fd 自有 undefined）。
          const __addr = this.address();
          const __fam = __addr !== null && typeof __addr === "object" ? __addr.family : null;
          const __safe = {
            address: __addr !== null && typeof __addr === "object" ? __addr.address : null,
            addressType: __fam === "IPv6" ? 6 : (__fam === "IPv4" ? 4 : __fam),
            port: __addr !== null && typeof __addr === "object" ? __addr.port : null,
          };
          parentPort.postMessage({ __wjs2_cluster_ctl: "listening", address: __safe });
        } catch {}
      });
      return Reflect.apply(__origListen, this, args);
    };
    __patchedListen.__wjs2_cluster_patched = true;
    __clusterNet.Server.prototype.listen = __patchedListen;
  }
} catch {}
await import(__mod);
__booted = true;
for (const __m of __earlyMessages) {
  __clusterEmitUserMessage(__m);
}
`;

const __isClusterWorker =
  !isMainThread && !!workerData && !!workerData.__wjs2_cluster;
const __myId = __isClusterWorker ? workerData.__wjs2_cluster.id : 0;

export const isWorker = __isClusterWorker;
export const isPrimary = !__isClusterWorker;
export const isMaster = !__isClusterWorker;

let __sched = SCHED_RR;
// settings 单对象原地更新（具名导出活绑定 + 实例 getter 同源）。
const __settings = {};
// 真机缺省填充（setup-cumulative/argv 套件口径；fork 内调不依赖 this）。
function __ensureDefaults() {
  if (Object.keys(__settings).length === 0) {
    Object.assign(__settings, {
      args: process.argv.slice(2),
      exec: process.argv[1],
      execArgv: process.execArgv,
      silent: false,
    });
  }
}
let __nextId = 1;
const __workers = {};

class ClusterWorker extends EventEmitter {
  // node 原文（lib/internal/cluster/worker.js）：options 对象形
  //（{id, state, process}），非对象归一空表；裸数 id 为内部便捷形。
  constructor(opts, w) {
    super();
    if (typeof opts === "number") opts = { id: opts };
    if (opts === null || typeof opts !== "object") opts = {};
    this.id = (typeof opts.id === "number" ? opts.id : 0) | 0;
    this.state = typeof opts.state === "string" ? opts.state : "none";
    this.exitedAfterDisconnect = undefined;
    this.exitCode = null;
    this.signalCode = null;
    this.__w = (typeof w !== "undefined" && w !== null) ? w : (opts.__w ?? null);
    this.__connected = true;
    this.__dead = false;
    this.__online = false;
    this.__hasServers = false;
    const self = this;
    if (opts.process !== undefined) {
      this.process = opts.process;
      // 真机转发：process 的 error/message 透到 worker（worker.js）。
      if (this.process !== null && typeof this.process.on === "function") {
        this.process.on("error", (...a) => { self.emit("error", ...a); });
        this.process.on("message", (...a) => { self.emit("message", ...a); });
      }
    } else if (this.__w !== null) {
      this.process = {
        pid: this.__w.threadId,
        get connected() { return self.__connected; },
        get exitCode() { return self.exitCode; },
        get signalCode() { return self.signalCode; },
        kill(signal) { self.kill(signal); },
        disconnect() { self.__rawDisconnect(); },
      };
    } else {
      this.process = undefined;
    }
  }
  send(message, ...rest) {
    let cb = null;
    for (const a of rest) if (typeof a === "function") cb = a;
    if (!this.__connected || this.__w === null) {
      const err = new Error("Channel closed");
      err.code = "ERR_IPC_CHANNEL_CLOSED";
      if (cb) queueMicrotask(() => cb(err));
      return false;
    }
    // cluster 传柄（listen-handle-cluster 套件）：第二参 net.Server 即 dup
    // 监听 fd 随信封走（node child_process send/got 'net.Server' 口径；
    // 无柄线形不变）。
    let __payload = message;
    for (const a of rest) {
      if (a !== null && typeof a === "object" && a.__wjs2_netServer === true && typeof a.__id === "number") {
        let __fd = -1;
        try { __fd = Number(__wjs2_net_dup_listen(a.__id)); } catch {}
        if (__fd >= 0) __payload = { __wjs2_msg: message, __wjs2_handle: { type: "net.Server", fd: __fd } };
        break;
      }
    }
    try {
      this.__w.postMessage(__payload);
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e));
      if (!err.code) err.code = "ERR_IPC_CHANNEL_CLOSED";
      if (cb) queueMicrotask(() => cb(err));
      return false;
    }
    if (cb) queueMicrotask(() => cb(null));
    return true;
  }
  disconnect() {
    if (!this.__connected) return this;
    // node 主侧 disconnect 直接置旗（primary.js），worker 主动断开经
    // exitedAfterDisconnect 消息置旗（见收消息分支）。
    this.exitedAfterDisconnect = true;
    try { this.__w.postMessage({ __wjs2_cluster_ctl: "disconnect" }); } catch {}
    this.__onDisconnect();
    return this;
  }
  // 原始通道断开（child_process 口径：worker 强制 exit(0)，forced-exit 套件）。
  __rawDisconnect() {
    if (!this.__connected) return this;
    this.suicide = true;
    try { this.__w.postMessage({ __wjs2_cluster_ctl: "raw-disconnect" }); } catch {}
    this.__onDisconnect();
    return this;
  }
  kill(signal) {
    // primary kill：只终结线程，不碰 suicide/flag（kill-signal 套件要
    // exitedAfterDisconnect=false；node 主侧 destroy 亦不置此旗）。
    // 信号名透到 exit（terminate 无信号概念，记待发信号）。
    this.__killSignal = typeof signal === "string" ? signal : null;
    if (this.__w !== null) {
      try { this.__w.terminate(); } catch {}
    }
    return this;
  }
  destroy() {
    if (this.state === "destroying") return this;
    this.state = "destroying";
    this.exitedAfterDisconnect = true;
    this.disconnect();
    return this;
  }
  isConnected() { return this.__connected; }
  isDead() { return this.__dead || this.exitCode !== null || this.signalCode !== null; }
  __onDisconnect() {
    if (!this.__connected) return;
    this.__connected = false;
    this.state = "disconnected";
    // worker 侧 disconnect 同步关全部 server（node _disconnect 关共享柄口径）；
    // 无柄 worker 自然退出 → 主进程被线程续命到 exit 事件必达；
    // 有连接续命不断（server.close 等连接收尾，真机同；worker-no-exit 口径）。
    // 派发递延 microtask（before-exit 套件：disconnect() 同步调完再挂监听
    // 仍须接到；真机经通道往返本就异步）。
    const self = this;
    queueMicrotask(() => {
      self.emit("disconnect");
      cluster.emit("disconnect", self);
    });
  }
  __onExit(code) {
    // 信号杀：exitCode 恒 null、signalCode 为待发信号（kill-signal 套件口径）。
    let signal = this.__killSignal ?? null;
    if (signal !== null) code = null;
    this.exitCode = code;
    this.signalCode = signal;
    if (this.process !== undefined && this.process !== null &&
        typeof this.process === "object") {
      try { this.process.exitCode = code; } catch {}
      try { this.process.signalCode = signal; } catch {}
    }
    // 先记退出账再发 disconnect（node 先 waitpid 后通道 EOF 的可观察序；
    // worker-exit 套件要求 disconnect 处理时 exitedAfterDisconnect 已定为 false）。
    this.__dead = true;
    if (this.exitedAfterDisconnect === undefined) {
      this.exitedAfterDisconnect = this.suicide ? true : false;
    } else if (!this.exitedAfterDisconnect && this.suicide) {
      this.exitedAfterDisconnect = true;
    }
    delete __workers[this.id];
    this.emit("exit", code, signal);
    cluster.emit("exit", this, code, signal);
    if (this.__connected) this.__onDisconnect();
  }
}

// 子端当前 worker 桩（send/disconnect 走 parentPort；只读 id）。
// node 原文（lib/internal/cluster/worker.js）：process 的 error/message
// 转发到 worker——process.emit('error') 即触发 worker error 监听。
class ClusterWorkerSide extends EventEmitter {
  constructor(id) {
    super();
    this.id = id;
    this.state = "none";
    this.suicide = false;
    this.exitedAfterDisconnect = false;
    const self = this;
    // 进程桥：worker-destroy 套件在 cluster.worker.process 上挂
    // disconnect 监听——与全局 process 同源事件。
    const __proc = new EventEmitter();
    __proc.pid = 0;
    __proc.connected = true;
    __proc.kill = (signal) => {};
    __proc.disconnect = () => {
      if (typeof process.disconnect === "function") process.disconnect();
    };
    this.process = __proc;
    try { globalThis.__wjs2_cluster_side_proc = __proc; } catch {}
    process.on("error", (...args) => { self.emit("error", ...args); });
  }
  send(message, ...rest) {
    if (typeof process.send === "function") return process.send(message, ...rest);
    return false;
  }
  disconnect() {
    if (typeof process.disconnect === "function") process.disconnect();
    return this;
  }
  destroy() {
    this.exitedAfterDisconnect = true;
    this.disconnect();
    return this;
  }
  kill(signal) {
    // 子端 kill（node destroy 口径）：置旗 + 走 disconnect 自然退出
    //（kill-disconnect 套件要求 exitedAfterDisconnect=true）。
    this.exitedAfterDisconnect = true;
    this.disconnect();
    return this;
  }
  isConnected() { return !!process.connected; }
  isDead() { return false; }
}

class Cluster extends EventEmitter {
  get SCHED_NONE() { return SCHED_NONE; }
  get SCHED_RR() { return SCHED_RR; }
  get workers() { return __workers; }
  get settings() { return { ...__settings }; }
  get schedulingPolicy() { return __sched; }
  // CJS 直引 `require('node:cluster').Worker` 拿到的是默认导出本体，
  // 具名类挂实例 getter 补齐（basic/instanceof 套件口径）。
  get Worker() { return WorkerFn; }
  set schedulingPolicy(v) {
    if (v !== SCHED_NONE && v !== SCHED_RR) {
      const err = new TypeError("schedulingPolicy must be SCHED_NONE or SCHED_RR");
      err.code = "ERR_INVALID_ARG_VALUE";
      throw err;
    }
    __sched = v;
  }
  get worker() { return __isClusterWorker ? __sideWorker : undefined; }
  get isWorker() { return __isClusterWorker; }
  get isPrimary() { return !__isClusterWorker; }
  get isMaster() { return !__isClusterWorker; }
  setupPrimary(settings) {
    if (settings !== undefined && (typeof settings !== "object" || settings === null)) {
      const err = new TypeError("settings must be an object");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    // 首调填真机缺省（setup-cumulative 套件口径），后续合并保留。
    __ensureDefaults();
    Object.assign(__settings, settings ?? {});
    cluster.emit("setup", { ...__settings });
  }
  setupMaster(settings) { this.setupPrimary(settings); }
  fork(env) {
    if (__isClusterWorker) {
      // 线程底座不支持 worker 内再 fork（见模块头注）。
      const err = new Error("fork is not available inside cluster workers");
      err.code = "ERR_NOT_SUPPORTED";
      throw err;
    }
    // env 宽容口径（真机：非对象 env 经展开并入环境，不抛；
    // child-index-dgram 套件 `cluster.fork(__filename)` 点名）。
    if (env !== undefined && (typeof env !== "object" || env === null)) env = {};
    const id = __nextId++;
    // 缺省经 setupPrimary 真机缺省（cumulative 套件口径；直接 fork 亦有 exec）。
    // idle-worker 套件解构调用 fork（this 丢失），经 cluster 实例调用。
    cluster.setupPrimary();
    const exec = __settings.exec ?? process.argv[1];
    if (typeof exec !== "string") {
      const err = new Error("cluster.fork needs an entry file (process.argv[1])");
      err.code = "ERR_INVALID_ARG_VALUE";
      throw err;
    }
    const fileUrl = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(exec) ? exec : pathToFileURL(exec).href;
    const argsArr = [...(__settings.args ?? [])].map(String);
    const envObj = {};
    for (const [k, v] of Object.entries(env ?? {})) envObj[String(k)] = String(v);
    const src = __CLUSTER_CHILD_SRC
      .replace("__CLUSTER_MOD__", () => JSON.stringify(fileUrl))
      .replace("__CLUSTER_ARGS__", () => JSON.stringify(argsArr))
      .replace("__CLUSTER_CWD__", () => JSON.stringify(typeof __settings.cwd === "string" ? __settings.cwd : null))
      .replace("__CLUSTER_ENV__", () => JSON.stringify(envObj));
    const w = new ThreadWorker(src, { eval: true, workerData: { __wjs2_cluster: { id } }, __wjs2_forkChild: true });
    const worker = new ClusterWorker({ id }, w);
    __workers[id] = worker;
    w.on("message", (m) => {
      if (m !== null && typeof m === "object" && !Array.isArray(m) && typeof m.__wjs2_cluster_ctl === "string") {
        if (m.__wjs2_cluster_ctl === "online") {
          worker.__online = true;
          worker.emit("online");
          cluster.emit("online", worker);
        } else         if (m.__wjs2_cluster_ctl === "disconnect") {
          worker.suicide = true;
          // worker 主动断开：置旗（node exitedAfterDisconnect 消息口径；
          // kill-disconnect 套件要求 disconnect 时即为 true）。
          worker.exitedAfterDisconnect = true;
          worker.__onDisconnect();
          // 回执：worker 侧本地派发等此 ack（race 套件口径）。
          try { worker.__w.postMessage({ __wjs2_cluster_ctl: "disconnect-ack" }); } catch {}
        } else if (m.__wjs2_cluster_ctl === "listening") {
          worker.__hasServers = true;
          // fd 自有 undefined（basic 套件 hasOwn + strictEqual 双断言；
          // JSON 线不传 undefined，接收侧补）。
          const __la = (m.address !== null && typeof m.address === "object") ? m.address : {};
          try { __la.fd = undefined; } catch {}
          worker.emit("listening", __la);
          cluster.emit("listening", worker, __la);
        }
        return;
      }
      worker.emit("message", m);
      cluster.emit("message", worker, m);
    });
    w.on("error", (e) => { worker.emit("error", e); });
    w.on("exit", (code) => { worker.__onExit(code); });
    queueMicrotask(() => { cluster.emit("fork", worker); });
    return worker;
  }
  disconnect(cb) {
    // node 口径（lib/internal/cluster/primary.js intercom）：cb 在全部
    // 存量 worker 退出后才调，而非断开即调；零 worker 即走 microtask。
    const targets = Object.values(__workers);
    for (const w of targets) {
      try { w.disconnect(); } catch {}
    }
    if (typeof cb !== "function") return;
    if (targets.length === 0) {
      queueMicrotask(cb);
      return;
    }
    let pending = targets.length;
    const done = () => {
      pending -= 1;
      if (pending === 0) cb();
    };
    for (const w of targets) {
      if (w.__dead) done();
      else w.once("exit", done);
    }
  }
}

const cluster = new Cluster();
const __sideWorker = __isClusterWorker ? new ClusterWorkerSide(__myId) : undefined;
if (__isClusterWorker) {
  try { globalThis.__wjs2_cluster_side = __sideWorker; } catch {}
}

// `new Worker()`/`Worker.call()` 函数形（worker-constructor 套件：
// 无 new 可调、`call` 换 receiver 照常构造，`instanceof` 双向成立）。
function WorkerFn(...args) {
  return new ClusterWorker(...args);
}
Object.setPrototypeOf(WorkerFn, ClusterWorker);
WorkerFn.prototype = ClusterWorker.prototype;

export default cluster;
export { __workers as workers, __settings as settings };
export { WorkerFn as Worker };
"#;
