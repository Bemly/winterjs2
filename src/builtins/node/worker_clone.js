import { EventEmitter } from "node:events";
import { codes } from "node:internal/errors";
import { inspect as __wjs2Inspect } from "node:util";
import { Readable as __wjs2Readable, Writable as __wjs2Writable } from "node:stream";

// node util.inspect 的字符串形（单引号——BroadcastChannel inspect 定制用）。
function __inspectQuote(s) {
  return __wjs2Inspect(String(s), { quotes: "single" });
}

const __uncloneable = new WeakSet();
export function markAsUncloneable(obj) {
  if ((typeof obj === "object" && obj !== null) || typeof obj === "function") {
    __uncloneable.add(obj);
  }
}

// 10f：node 26 具名面（mark-as-untransferable 套件）——标记 AB 不可转移，
// 转移即 DataCloneError(25) 且**不 detach**；`isMarkedAsUntransferable` 查询。
const __untransferable = new WeakSet();
export function markAsUntransferable(obj) {
  if ((typeof obj === "object" && obj !== null) || typeof obj === "function") {
    __untransferable.add(obj);
  }
}
export function isMarkedAsUntransferable(obj) {
  return __untransferable.has(obj);
}

// 10f：markAsUncloneable 同步影响全局 structuredClone（uncloneable 套件
// structuredClone 块；真机：标记物任何位置出现即 DataCloneError 25）。
// 模块求值期一次性包装（worker 会话各求值一次，幂等守卫）。
if (typeof globalThis.structuredClone === "function" && !globalThis.structuredClone.__wjs2UncloneAware) {
  const __origSC = globalThis.structuredClone;
  const __scWrapped = function (value, options) {
    const seen = new Set();
    const walk = (v) => {
      if (v !== null && (typeof v === "object" || typeof v === "function")) {
        if (seen.has(v)) return;
        seen.add(v);
        // transferable 本性豁免（与 __packValue 同口径；AB 标记后照常克隆）。
        if (!ArrayBuffer.isView(v) && !__isSAB(v) && !(v instanceof ArrayBuffer) && __uncloneable.has(v)) {
          __dataCloneErr("object could not be cloned.");
        }
        if (Array.isArray(v)) {
          for (const x of v) walk(x);
        } else if (v instanceof Map) {
          for (const [k2, v2] of v) { walk(k2); walk(v2); }
        } else if (v instanceof Set) {
          for (const x of v) walk(x);
        } else {
          for (const k of Object.keys(v)) walk(v[k]);
        }
      }
    };
    walk(value);
    return __origSC.call(this, value, options);
  };
  __scWrapped.__wjs2UncloneAware = true;
  try {
    globalThis.structuredClone = __scWrapped;
  } catch { /* 只读面跳过 */ }
}

function __dataCloneErr(message) {
  // 10f 对拍：真 DataCloneError = DOMException（constructor.name/code 25/instanceof
  // Error 三面都要对，transfer-self 套件逐项断言）；消息为全量原文。
  throw new globalThis.DOMException(message, "DataCloneError");
}

// ── 9i-2 线信封 v2（仍是单 JSON 串，Rust 通道零改动）─────────────────────
// 自定义打包走 JSON（BigInt/undefined/Date/Map/Set/ArrayBuffer/视图/
// MessagePort 全保留；循环/共享引用保留同一性——容器先序 id，重访即 `ref`
// marker（M5 vitest 牵引：任务图天生带环；此前抛错/多份拷贝， wire 兼容：
// 无环消息形状逐字节不变）；函数/symbol/MarkAsUncloneable 物件任何位置出现
// 即 DataCloneError；transfer 标记内联 + 顶层信封带 nonce，接收侧 reviver 认领。
// ArrayBuffer/视图：transfer 即 detach（源归零，真机同款），否则拷贝字节；
// SAB 只拷贝（变普通 AB，记档）；分离中（detached）出现即 DataCloneError。
// MessagePort：仅 transfer 可投递（offer/accept 邀约槽 + 源 neutered 静默），
// 同会话与跨会话统一路径（跨会话经源表项转发器多一跳，对端无感知）。
// 函数/symbol/markAsUncloneable 物件任何位置出现即 DataCloneError。

let __wireSeq = 0;
const __VIEW_CTORS = {
  Int8Array, Uint8Array, Uint8ClampedArray, Int16Array, Uint16Array,
  Int32Array, Uint32Array, Float32Array, Float64Array,
  BigInt64Array, BigUint64Array, DataView,
};

function __isSAB(v) {
  return typeof SharedArrayBuffer !== "undefined" && (v instanceof SharedArrayBuffer);
}
// 本引擎 structuredClone 不支持 BigInt（实测 DataCloneError），故可克隆性由
// 下方 walk 全权判定；此处仅列 walk 须显式拒绝的内置（structuredClone 口径）。
function __denyClone(v) {
  if (v instanceof Promise) return true;
  if (typeof WeakMap !== "undefined" && v instanceof WeakMap) return true;
  if (typeof WeakSet !== "undefined" && v instanceof WeakSet) return true;
  if (typeof WeakRef !== "undefined" && v instanceof WeakRef) return true;
  if (typeof FinalizationRegistry !== "undefined" && v instanceof FinalizationRegistry) return true;
  return false;
}
function __b64encode(u8) {
  return Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength).toString("base64");
}
function __b64decode(b64) {
  return new Uint8Array(Buffer.from(String(b64), "base64"));
}
function __detachBuf(ab) {
  if (typeof ab.transfer === "function") ab.transfer();
  else structuredClone(ab, { transfer: [ab] });
}

// 10f 对拍：postMessage 第二参归一（message-port 套件逐字）——
// null/undefined 即无 transfer；Array 即列表；可迭代（非 string）即列表；
// 其余对象即 `{ transfer }` 选项形（`{}`/无 transfer 键即无）；其他标量即
// 'Optional transferList argument must be an iterable'。
function __transferListErr(isOpts) {
  const err = new TypeError(isOpts
    ? "Optional options.transfer argument must be an iterable"
    : "Optional transferList argument must be an iterable");
  err.code = "ERR_INVALID_ARG_TYPE";
  return err;
}
function __asTransferArray(t, isOpts) {
  if (t === undefined || t === null) {
    if (isOpts) throw __transferListErr(true);
    return [];
  }
  if (typeof t === "string") throw __transferListErr(isOpts);
  if (typeof t?.[Symbol.iterator] === "function") {
    try {
      return [...t];
    } catch {
      throw __transferListErr(isOpts);
    }
  }
  throw __transferListErr(isOpts);
}
function __resolveTransfer(arg) {
  if (arg === undefined || arg === null) return [];
  if (Array.isArray(arg)) return arg;
  if (typeof arg === "string") throw __transferListErr(false);
  if (typeof arg === "object" || typeof arg === "function") {
    if (typeof arg[Symbol.iterator] === "function") return __asTransferArray(arg, false);
    // 选项形：transfer 缺省/undefined 即无；其余须可迭代。
    const t = arg.transfer;
    if (t === undefined) return [];
    return __asTransferArray(t, true);
  }
  throw __transferListErr(false);
}

function __normTransfer(transfer, srcPort) {
  if (transfer === undefined) return [];
  if (!Array.isArray(transfer)) return []; // 防御：调用方已归一（见 __resolveTransfer）
  const out = [];
  const seen = new Set();
  for (const t of transfer) {
    if (seen.has(t)) {
      // 10f 对拍：node 逐类型文案（transfer-duplicate 套件逐字）——端口与
      // ArrayBuffer 各有专用文案；视图/SAB/其他杂项走下方的 invalid value。
      __dataCloneErr(t instanceof MessagePort
        ? "Transfer list contains duplicate MessagePort"
        : "Transfer list contains duplicate ArrayBuffer");
    }
    seen.add(t);
    if (t instanceof MessagePort) {
      // 10f 对拍：源端口/已关闭端口两形 node 文案逐字（transfer-self/
      // transfer-closed 套件断 code 25 + constructor.name DOMException）。
      // markAsUntransferable 标记的端口拒转移（真机 "Cannot transfer object
      // of unsupported type."，transfer-untransferable 套件）。
      if (__untransferable.has(t)) __dataCloneErr("Cannot transfer object of unsupported type.");
      if (t === srcPort) __dataCloneErr("Transfer list contains source port");
      if (t.__neutered || t.__closed) {
        __dataCloneErr("MessagePort in transfer list is already detached");
      }
      out.push({ kind: "port", obj: t });
    } else if (ArrayBuffer.isView(t)) {
      // 10f 对拍：视图/SAB 一律非 transferable（真机逐字，view 系列探针）——
      // transfer 视图须传其 buffer（`[v.buffer]`），条目本身恒非法。
      __dataCloneErr("Found invalid value in transferList.");
    } else if (__isSAB(t)) {
      // SAB 共享免转移，列入 transfer 即非法（真机逐字）。
      __dataCloneErr("Found invalid value in transferList.");
    } else if (t instanceof ArrayBuffer) {
      // 10f：markAsUntransferable 标记的 AB 拒转移（不 detach——套件断
      // byteLength 不变；code 25/DataCloneError 由 __dataCloneErr 统一）。
      if (__untransferable.has(t)) __dataCloneErr("ArrayBuffer could not be cloned.");
      if (t.detached) __dataCloneErr("Detached buffer could not be cloned.");
      if (globalThis.__wjs2_bufPooled?.has(t)) __dataCloneErr("Pooled buffer could not be cloned.");
      out.push({ kind: "buf", obj: t });
    } else {
      // transfer-guards 套件（10f）：net.Socket/net.Server 在 transfer list 即
      // ERR_WORKER_HANDLE_NOT_TRANSFERABLE（node kTransferList 断言族；net 侧
      // 构造时经 globalThis.__wjs2_netXfer 登记，文案 errors.js 逐字）。
      const __xt = (globalThis.__wjs2_netXfer && typeof globalThis.__wjs2_netXfer.get === "function")
        ? globalThis.__wjs2_netXfer.get(t) : undefined;
      if (__xt !== undefined) {
        const e = new Error(`${__xt} cannot be transferred in its current state; it must be a freshly created or accepted handle that has not started reading and has no pending writes`);
        e.code = "ERR_WORKER_HANDLE_NOT_TRANSFERABLE";
        throw e;
      }
      // 10f 对拍：数字/普通对象等杂项条目恒非法（真机逐字 DataCloneError 25）。
      __dataCloneErr("Found invalid value in transferList.");
    }
  }
  return out;
}

function __packValue(v, st) {
  const ti = (v !== null && (typeof v === "object" || typeof v === "function")) ? st.tmap.get(v) : undefined;
  if (ti !== undefined) {
    st.seenTransfer.add(ti.obj);
    if (ti.kind === "port") {
      // 预邀约复用（__toWire 清单序单 offer；同对象多槽共享同一 nonce）。
      st.neuterPorts.push({ o: ti.obj, live: true });
      return { __wjs2_xfer: st.nonce, k: "port", id: ti.nonce };
    }
    if (ti.kind === "buf") {
      const bytes = __b64encode(new Uint8Array(ti.obj));
      __detachBuf(ti.obj);
      return { __wjs2_xfer: st.nonce, k: "buf", b: bytes };
    }
    // view：整 underlying buffer 字节 + 偏移复原（transfer 即整块 detach）。
    const b = ti.obj.buffer;
    const bytes = __b64encode(new Uint8Array(b));
    __detachBuf(b);
    return { __wjs2_xfer: st.nonce, k: "view", t: ti.obj.constructor.name, b: bytes, o: ti.obj.byteOffset, n: ti.obj.byteLength };
  }
  if (typeof v === "function" || typeof v === "symbol") __dataCloneErr(`${String(v)} could not be cloned.`);
  if ((typeof v === "object" && v !== null) || typeof v === "function") {
    // 10f：markAsUncloneable 不影响 ArrayBuffer/视图/SAB（transferable 本性；
    // uncloneable 套件 AB 直通块）。
    if (!ArrayBuffer.isView(v) && !__isSAB(v) && !(v instanceof ArrayBuffer) && __uncloneable.has(v)) {
      __dataCloneErr("object could not be cloned.");
    }
  }
  if ((typeof v === "object" && v !== null) && __denyClone(v)) __dataCloneErr("object could not be cloned.");
  if (typeof v === "bigint") return { __wjs2_xfer: st.nonce, k: "big", v: String(v) };
  if (v === undefined) return { __wjs2_xfer: st.nonce, k: "undef" };
  if (v instanceof Date) return { __wjs2_xfer: st.nonce, k: "date", v: v.toISOString() };
  if (v instanceof Map) {
    // 容器先序 id：首访编号，祖先/共享重访即 `ref`（解码侧同序注册，恒后向引用）。
    const hit = st.path.get(v);
    if (hit !== undefined) return { __wjs2_xfer: st.nonce, k: "ref", id: hit };
    st.path.set(v, st.nextId++);
    return { __wjs2_xfer: st.nonce, k: "map", v: [...v].map(([k2, v2]) => [__packValue(k2, st), __packValue(v2, st)]) };
  }
  if (v instanceof Set) {
    const hit = st.path.get(v);
    if (hit !== undefined) return { __wjs2_xfer: st.nonce, k: "ref", id: hit };
    st.path.set(v, st.nextId++);
    return { __wjs2_xfer: st.nonce, k: "set", v: [...v].map((x) => __packValue(x, st)) };
  }
  if (v instanceof MessagePort) {
    // 10f 对拍：裸端口（无论顶层/嵌套）一律长文案（真机逐字；broadcastchannel
    // 套件 /Object that needs transfer was found/ 仍命中——长串含短串）。
    __dataCloneErr("Object that needs transfer was found in message but not listed in transferList");
  }
  if (typeof SharedArrayBuffer === "function" && v instanceof SharedArrayBuffer) {
    // 10f 对拍：node postMessage(SAB) = 品牌保真的**副本**（真共享内存需跨线程
    // 底座，记档）；无 SAB 全局时不可达（typeof 守卫，§4.58）。
    return { __wjs2_xfer: st.nonce, k: "sab", b: __b64encode(new Uint8Array(v)) };
  }
  if (v instanceof ArrayBuffer) {
    if (v.detached) __dataCloneErr("Detached buffer could not be cloned.");
    return { __wjs2_xfer: st.nonce, k: "buf", b: __b64encode(new Uint8Array(v)) };
  }
  if (ArrayBuffer.isView(v)) {
    const b = v.buffer;
    if (b.detached) __dataCloneErr("Detached buffer could not be cloned.");
    return { __wjs2_xfer: st.nonce, k: "view", t: v.constructor.name, b: __b64encode(new Uint8Array(b)), o: v.byteOffset, n: v.byteLength };
  }
  // 10f：Blob/File 字节 roundtrip（uncloneable 套件 Blob 块；真机 data+type
  // 保真，File 另保 name/lastModified；字节经 prelude 内部 WeakMap 同步取）。
  if (typeof Blob !== "undefined" && v instanceof Blob) {
    const bytes = globalThis.__wjs2_blobBytes?.get(v);
    if (!(bytes instanceof Uint8Array)) __dataCloneErr("object could not be cloned.");
    const base = { __wjs2_xfer: st.nonce, k: "blob", t: v.type, b: __b64encode(bytes) };
    if (typeof File !== "undefined" && v instanceof File) {
      base.f = v.name;
      base.m = v.lastModified;
    }
    return base;
  }
  if (Array.isArray(v)) {
    const hit = st.path.get(v);
    if (hit !== undefined) return { __wjs2_xfer: st.nonce, k: "ref", id: hit };
    st.path.set(v, st.nextId++);
    return v.map((x) => __packValue(x, st));
  }
  if (v !== null && typeof v === "object") {
    const hit = st.path.get(v);
    if (hit !== undefined) return { __wjs2_xfer: st.nonce, k: "ref", id: hit };
    st.path.set(v, st.nextId++);
    const out = {};
    st.depth++;
    for (const k of Object.keys(v)) out[k] = __packValue(v[k], st);
    st.depth--;
    return out;
  }
  return v;
}

function __unpackValue(v, st) {
  if (Array.isArray(v)) {
    // 与打包侧同先序注册（先占位再填子项，祖先后向引用恒可解）。
    const a = [];
    st.refs[st.nextId++] = a;
    for (let i = 0; i < v.length; i++) a[i] = __unpackValue(v[i], st);
    return a;
  }
  if (v !== null && typeof v === "object") {
    if (v.__wjs2_xfer === st.nonce) {
      switch (v.k) {
        case "ref": return st.refs[v.id];
        case "big": return BigInt(v.v);
        case "undef": return undefined;
        case "date": return new Date(v.v);
        case "map": {
          const m = new Map();
          st.refs[st.nextId++] = m;
          for (const [k2, v2] of v.v) m.set(__unpackValue(k2, st), __unpackValue(v2, st));
          return m;
        }
        case "set": {
          const s = new Set();
          st.refs[st.nextId++] = s;
          for (const x of v.v) s.add(__unpackValue(x, st));
          return s;
        }
        case "buf": return __b64decode(v.b).buffer;
        case "blob": {
          const u8b = __b64decode(v.b);
          if (typeof v.f === "string" && typeof File !== "undefined") {
            return new File([u8b], v.f, { type: String(v.t || ""), lastModified: Number(v.m) || 0 });
          }
          return new Blob([u8b], { type: String(v.t || "") });
        }
        case "sab": {
          // 10f 对拍：SharedArrayBuffer 品牌 roundtrip（副本语义，真共享记档）。
          const u8s = __b64decode(v.b);
          const sab = new SharedArrayBuffer(u8s.length);
          new Uint8Array(sab).set(u8s);
          return sab;
        }
        case "view": {
          const u8 = __b64decode(v.b);
          const Ctor = __VIEW_CTORS[v.t] || Uint8Array;
          const o = Number(v.o) || 0;
          // 10f 修：wire n 是 byteLength——typed array 第三参是**元素数**，
          // 按 BPE 折算（此前 Int32/Float64 系全 OOB，workerData 求值中断连带
          // 整模块 class 声明区 TDZ 级联）。
          const bpe = Ctor.BYTES_PER_ELEMENT || 1;
          const n = Number(v.n);
          if (Ctor === DataView) return new DataView(u8.buffer, o, Number.isFinite(n) ? n : undefined);
          return new Ctor(u8.buffer, o, Number.isFinite(n) ? n / bpe : undefined);
        }
        case "port": {
          // 信封优先承接（见 __fromWirePorts：同 nonce 多槽共享同一端口）；
          // 回落直承（同步收信等无信封上下文路径）。
          const prev = st.portMap?.get(String(v.id));
          if (prev !== undefined) return prev;
          const local = String(__wjs2_port_accept(String(v.id)));
          if (local === "") __dataCloneErr("MessagePort in transfer list is already detached");
          return new MessagePort(local, __kPortInternal);
        }
        default: break;
      }
    }
    const out = {};
    st.refs[st.nextId++] = out;
    for (const k of Object.keys(v)) out[k] = __unpackValue(v[k], st);
    return out;
  }
  return v;
}

function __toWire(value, transferOrOptions, srcPort) {
  return __toWireList(value, __normTransfer(__resolveTransfer(transferOrOptions), srcPort), srcPort);
}
// 组装（入参为已归一清单；MessagePort.postMessage 单次 resolve 后直调，
 // 避免生成器等一次性可迭代对象二次消费）。
function __toWireList(value, list, srcPort) {
  const st = { nonce: `w${++__wireSeq}x${Math.floor(Math.random() * 36 ** 6).toString(36)}`, tmap: new Map(), path: new Map(), nextId: 0, neuterPorts: [], seenTransfer: new Set(), depth: 0, portNonces: [] };
  for (const t of list) {
    if (t.kind === "port") {
      // 端口预邀约（清单序，单 offer 复用——同对象多槽共享同一 nonce；
      // 信封 `p` 携带供收端承接 `MessageEvent.ports` 用）。
      const nonce = String(__wjs2_port_offer(t.obj.__id));
      if (nonce === "") __dataCloneErr("MessagePort in transfer list is already detached");
      t.nonce = nonce;
      st.tmap.set(t.obj, { kind: "port", id: t.obj.__id, nonce, obj: t.obj });
      st.portNonces.push(nonce);
    } else st.tmap.set(t.obj, t);
  }
  // 可克隆性由 __packValue walk 全权判定（本引擎 structuredClone 不支持 BigInt，
  // 此处不再探路，见 __denyClone）。
  const tree = __packValue(value, st);
  // transfer 清单内但消息未引用者：照样生效（buffer detach、端口 neutered；
  // offer 槽由收端信封 `p` 承接，无需 withdraw——withdraw 只用于同步收信
  // 消费后无 ports 上下文的残留回收，见 __withdrawPorts）。
  for (const t of list) {
    if (st.seenTransfer.has(t.obj)) continue;
    if (t.kind === "port") {
      st.neuterPorts.push({ o: t.obj, live: false });
    } else if (t.kind === "buf") {
      __detachBuf(t.obj);
    } else if (t.kind === "view") {
      __detachBuf(t.obj.buffer);
    }
  }
  let json;
  try {
    // 顶层信封：nonce 随信封走（嵌套 marker 认领用；`p` 为端口 transfer 清单序
    // nonce 供收端承接 ports；单 JSON 串，通道零改动）。
    json = JSON.stringify({ __wjs2_env: st.nonce, d: tree, p: st.portNonces }) ?? "null";
  } catch {
    __dataCloneErr("object could not be cloned.");
  }
  for (const p of st.neuterPorts) {
    try { p.live ? __portNeuter(p.o) : __portNeuterDead(p.o); } catch { /* 忽略 */ }
  }
  return json;
}

// 10f：worker 未捕获的**原始值**（throw 42 / "boom" / Symbol.for('a') 等）经
// Rust 捕获点打包 `__wjs2_prim:{json}` 信封（Error::Script kind=None 且非对象
// 异常时 message 即信封），error 事件按类还原（error-primitive 套件逐类型断
// 同一性；注册 Symbol 经 Symbol.for 还原即跨线程同一）。
function __wjs2_primFromText(json) {
  let o;
  try { o = JSON.parse(json); } catch { return undefined; }
  switch (o.t) {
    case "num": return Number(o.v);
    case "str": return String(o.v);
    case "bool": return !!o.v;
    case "big": return BigInt(o.v);
    case "nil": return null;
    case "undef": return undefined;
    case "sym": return Symbol.for(o.v);
    default: return undefined;
  }
}

function __fromWire(json) {
  let raw;
  try {
    raw = JSON.parse(String(json));
  } catch {
    const err = new Error("worker message is not valid JSON");
    err.name = "MessageError";
    throw err;
  }
  if (raw !== null && typeof raw === "object" && typeof raw.__wjs2_env === "string" && "d" in raw) {
    return __unpackValue(raw.d, { nonce: raw.__wjs2_env, refs: [], nextId: 0 });
  }
  // v1 载荷（本二进制内不产生；防御性直通，无 marker 可误认）。
  return raw;
}

// 10f：端口面收信（`MessageEvent.ports` 用）——先按信封 `p` 清单序承接全部
// transfer 端口，再 walk（同 nonce 多槽共享同一端口）；返回 `{ message, ports }`。
function __fromWirePorts(json) {
  const raw = JSON.parse(String(json));
  const ports = [];
  const portMap = new Map();
  if (Array.isArray(raw?.p)) {
    for (const nonce of raw.p) {
      const local = String(__wjs2_port_accept(String(nonce)));
      if (local === "") __dataCloneErr("MessagePort in transfer list is already detached");
      const port = new MessagePort(local, __kPortInternal);
      portMap.set(String(nonce), port);
      ports.push(port);
    }
  }
  const message = __unpackValue(raw.d, { nonce: raw.__wjs2_env, refs: [], nextId: 0, portMap });
  return { message, ports };
}

// 同步收信（`receiveMessageOnPort` 只回 `{ message }`，无 ports 上下文）：
// 信封残留 nonce 撤邀约（已被行内承接者消费即 no-op），防 offer 槽泄漏。
function __withdrawPorts(json) {
  let raw;
  try {
    raw = JSON.parse(String(json));
  } catch {
    return;
  }
  if (Array.isArray(raw?.p)) {
    for (const nonce of raw.p) {
      try { __wjs2_port_withdraw(String(nonce)); } catch { /* 忽略 */ }
    }
  }
}

// 内部构造令牌（用户侧任何 `MessagePort(..)`/`new MessagePort(..)` 一律
// ERR_CONSTRUCT_CALL_INVALID，真机逐字；见 message-port-constructor 套件）。
// 函数式（非 class）：class 的无 new 调用抛原生错带不上 code；原型只留
// node 同款 8 名（message-port 套件断言逐字），EventTarget 双面与内部管线
// 经实例闭包/模块函数实现，不进原型。
const __kPortInternal = {};
export function MessagePort(__id, __tok) {
  if (__tok !== __kPortInternal) throw new codes.ERR_CONSTRUCT_CALL_INVALID();
  EventEmitter.call(this);
  const self = this;
  self.__id = __id;
  self.__closed = false;
  self.__neutered = false;
  self.__peerClosed = false;
  self.__closeEmitted = false;
  self.__dropDead = false;
  self.__queue = [];
  self.__flushScheduled = false;
  self.__onmessage = null;
  self.__onmessageWrap = null;
  self.__onmessageerror = null;
  self.__onmessageerrorWrap = null;
  // Rust 分发经实例 `__ev` 属性查找（get_prop_value），实例闭包即可。
  self.__ev = (kind, payload) => __portEv(self, kind, payload);
  // EventTarget 双面（真机：'message'/'messageerror' 收 MessageEvent(data)，
  // 自定义类型经 emit 桥收 CustomEvent(detail)——message-port 套件逐项）。
  // 同 fn 跨类型各留各的包装（fn→Map(type→wrap)，remove 精确摘除）。
  // 实例闭包：原型只留 node 同款 8 名。
  self.__etWrap = new WeakMap();
  self.addEventListener = (type, fn) => {
    if (typeof fn !== "function") return;
    const t = String(type);
    const isMsg = t === "message" || t === "messageerror";
    let byType = self.__etWrap.get(fn);
    if (!byType) { byType = new Map(); self.__etWrap.set(fn, byType); }
    if (byType.has(t)) return;
    const wrap = (value) => {
      const ev = isMsg
        ? __portMessageEvent(self, t, value, self.__lastPorts ?? [])
        : new globalThis.CustomEvent(t, { detail: value });
      fn.call(self, ev);
    };
    byType.set(t, wrap);
    self.on(t, wrap);
  };
  self.removeEventListener = (type, fn) => {
    const wrap = self.__etWrap.get(fn)?.get(String(type));
    if (wrap) {
      self.off(String(type), wrap);
      self.__etWrap.get(fn).delete(String(type));
    }
  };
  // paused 口径：无 message 监听时只排队不刷；监听到达即开闸（Node 同款）。
  // 注意 newListener 在监听入表*之前*触发，故延迟一轮 microtask 再刷。
  self.on("newListener", (ev) => { if (ev === "message") queueMicrotask(() => __portMaybeFlush(self)); });
  // 监听门控计数：有 message 监听才续命事件循环（worker 空转即退，Node 口径）。
  // 真机：message 监听挂载即 ref（fresh hasRef false → 挂载后 true；
  // 显式 unref 后再挂载亦回 true，hasRef 探针）。
  self.on("newListener", (ev) => {
    if (ev === "message") {
      try { __wjs2_port_ref(__id); } catch { /* 忽略 */ }
      __wjs2_port_listen(__id);
    }
  });
  self.on("removeListener", (ev) => {
    if (ev === "message" && self.listenerCount("message") === 0) __wjs2_port_unlisten(__id);
  });
  __wjs2_port_attach(__id, self);
}
Object.setPrototypeOf(MessagePort.prototype, EventEmitter.prototype);
// 内部管线（原原型方法外迁，见上；原型只留 node 同款 8 名）。
function __portNeuter(port) {
  // 迁移后源端：静默（真机同款 no-op），不摘对端。
  if (port.__neutered) return;
  port.__neutered = true;
  port.__queue.length = 0;
}
function __portNeuterDead(port) {
  // 未引用转让（无承接者）：静默 + 后续到达丢弃。
  __portNeuter(port);
  port.__dropDead = true;
}
function __portMaybeFlush(port) {
  if (port.__flushScheduled || port.__queue.length === 0) return;
  if (port.listenerCount("message") === 0) return;
  port.__flushScheduled = true;
  queueMicrotask(() => {
    port.__flushScheduled = false;
    __portFlushQueue(port);
  });
}
function __portMessageEvent(port, type, value, ports) {
  return new globalThis.MessageEvent(type, { data: value, ports, __wjs2Target: port });
}
function __portFlushQueue(port) {
  while (port.__queue.length > 0) {
    // 停刷留队（node 逐 task 派发语义：pump 整批排空 inbox 时多条同批到达，
    // 同步连发会把 once() 摘除后的后继消息射进虚空；无监听即停，新监听经
    // newListener 续刷——message-before-close 套件双 once 逐条必达）。
    if (port.listenerCount("message") === 0) return;
    const raw = port.__queue.shift();
    let decoded;
    try {
      decoded = __fromWirePorts(raw);
    } catch (e) {
      port.emit("messageerror", e instanceof Error ? e : new Error("worker message is not valid JSON"));
      continue;
    }
    // EE 单参（node 真机：transfer 亦单参，once() 即 [value]；onmessage 的
    // MessageEvent.ports 经 __lastPorts 边信道同步递达，发射内读清）。
    port.__lastPorts = decoded.ports;
    try {
      port.emit("message", decoded.message);
    } finally {
      port.__lastPorts = null;
    }
  }
}
function __portEv(port, kind, payload) {
  if (kind === "peer-close") {
    // 对端关闭透出本端 'close'（channel 套件；真机：关闭端先收，随后对端；
    // 已关/已迁移只记态不重发；__peerClosed 压倒一切——此后 close() 完全静默）。
    if (port.__neutered) return;
    port.__peerClosed = true;
    if (port.__closed || port.__closeEmitted) return;
    port.__closeEmitted = true;
    queueMicrotask(() => { try { port.emit("close"); } catch { /* 忽略 */ } });
    return;
  }
  if (kind === "forwarded") {
    // 迁移升级到达：源端排队消息经现转发路由排空后静默摘除。
    const pending = port.__queue.splice(0);
    for (const wire of pending) {
      try { __wjs2_port_post(port.__id, String(wire)); } catch { /* 丢弃 */ }
    }
    try { __wjs2_port_detach(port.__id); } catch { /* 忽略 */ }
    return;
  }
  if (kind !== "message") return;
  if (port.__closed || port.__dropDead) return;
  port.__queue.push(payload);
  __portMaybeFlush(port);
}
// onmessage 兼容面（真机语义：赋值即隐式开始流动——经 addEventListener 走
// newListener 开闸；回调收真 MessageEvent（data/target/ports，真机逐项
// 实测），`message` EE 载荷裸值单参，ports 经 __lastPorts 边信道进事件）。
// 原型访问器（defineProperty：原型只留 node 同款 8 名）。
Object.defineProperty(MessagePort.prototype, "onmessage", {
  configurable: true,
  enumerable: false,
  get() { return this.__onmessage; },
  set(fn) {
    if (this.__onmessage) this.removeListener("message", this.__onmessageWrap);
    this.__onmessage = (typeof fn === "function") ? fn : null;
    if (this.__onmessage) {
      const self = this;
      this.__onmessageWrap = (value) => {
        const ports = self.__lastPorts ?? [];
        self.__onmessage(__portMessageEvent(self, "message", value, ports));
      };
      this.on("message", this.__onmessageWrap);
    } else {
      this.__onmessageWrap = null;
    }
  },
});
// onmessageerror 兼容面（同 onmessage，messageerror 事件载荷为错误值）。
Object.defineProperty(MessagePort.prototype, "onmessageerror", {
  configurable: true,
  enumerable: false,
  get() { return this.__onmessageerror; },
  set(fn) {
    if (this.__onmessageerror) this.removeListener("messageerror", this.__onmessageerrorWrap);
    this.__onmessageerror = (typeof fn === "function") ? fn : null;
    if (this.__onmessageerror) {
      const self = this;
      this.__onmessageerrorWrap = (value) => {
        self.__onmessageerror(__portMessageEvent(self, "messageerror", value));
      };
      this.on("messageerror", this.__onmessageerrorWrap);
    } else {
      this.__onmessageerrorWrap = null;
    }
  },
});
MessagePort.prototype.postMessage = function postMessage(value, transfer) {
  // 10f 对拍：校验恒先行——关闭/迁移发送端的非法 transfer 照抛（transfer-closed
  // 套件；真机探针），转移变更（detach/neuter）照常生效，仅消息体丢弃。
  // 10f：transfer 归一单次（生成器等一次性可迭代对象禁二次消费）——
  // 自杀判定与组装共享同一份清单；判定必须在转移前（转移后源端 neutered，
  // 再校验即误判 detached）。
  const list = __normTransfer(__resolveTransfer(transfer), this);
  const suicide = __isSuicideList(this, list);
  const wire = __toWireList(value, list, this);
  if (this.__closed || this.__neutered) {
    __withdrawPorts(wire);
    return;
  }
  // 自杀式投递（transfer 含纠缠对端）：warning + 通道丢失 + 消息丢弃
  //（target 套件；真机逐字；AB detach/端口 neuter 由 __toWire 照常完成，
  // 此处仅撤 offer 槽 + 发送端失活）。
  if (suicide) {
    try {
      globalThis.process?.emitWarning?.(
        "The target port was posted to itself, and the communication channel was lost",
        "Warning");
    } catch { /* 忽略 */ }
    __withdrawPorts(wire);
    // 发送端随通道失活（无 'close' 事件；后用静默，进程可退出）。
    this.__closed = true;
    this.__queue.length = 0;
    __wjs2_port_close(this.__id);
    return;
  }
  // 10f 对拍：本地 pair 经 Rust pending 表投递——`receiveMessageOnPort`
  // 同步可收（receive-message 套件），事件派发仍由 pump 逐轮驱动（node 的
  // task 级节奏；纯微任务链式 ping-pong 会饿死定时器，infinite-loop 实证）。
  __wjs2_port_post(this.__id, wire);
};
// 自杀判定（transfer 清单内含发送端的纠缠对端；target 套件）。
// 入参为已归一清单（调用方单次 resolve 后共享，避免可迭代对象二次消费）。
function __isSuicideList(srcPort, list) {
  for (const t of list) {
    if (t.kind !== "port") continue;
    try {
      if (__wjs2_port_is_peer(srcPort.__id, t.obj.__id) === true) return true;
    } catch { /* 忽略 */ }
  }
  return false;
}
MessagePort.prototype.start = function start() {};
MessagePort.prototype.close = function close(cb) {
  // close(cb) 口径（真机实测）：cb 经 once('close') 随 close 事件触发；
  // 非函数一律忽略（close(42) 不抛）；事件恒异步（sync-fired 探针）；
  // 已关闭再次 close 只调 cb 不重发事件（double-close 探针：事件恰 1 次）。
  // 时序铁律（teardown 崩溃根因）：Rust 侧 close（含 uncount + 对端通知）必须
  // 与 'close' 发射同 microtask——先 uncount 会让事件循环在发射前排空并开始
  // teardown，悬空发射触碰已销毁状态即 SEGV。
  const fn = typeof cb === "function" ? cb : undefined;
  // 对端已关压倒一切：完全静默（cb 亦不调；mixed 探针）。
  if (this.__peerClosed) return undefined;
  if (this.__closed || this.__neutered) {
    if (fn !== undefined) queueMicrotask(() => { fn(); });
    return undefined;
  }
  this.__closed = true;
  // 关不断流（node 口径：已排队消息照常派发，新到达才丢——close-while-receiving
  // 套件；`__portEv` message 臂凭 __closed 挡新，老队由 flush 排空）。
  if (fn !== undefined) this.once("close", fn);
  const self = this;
  queueMicrotask(() => {
    try {
      // 对端先到已发过即跳过（双关竞速只发一次）。
      if (!self.__closeEmitted) {
        self.__closeEmitted = true;
        self.emit("close");
      }
    } finally {
      __wjs2_port_close(self.__id);
    }
  });
  return undefined;
};
MessagePort.prototype.ref = function ref() {
  __wjs2_port_ref(this.__id);
  return this;
};
MessagePort.prototype.unref = function unref() {
  __wjs2_port_unref(this.__id);
  return this;
};
MessagePort.prototype.hasRef = function hasRef() {
  try {
    return Boolean(__wjs2_port_has_ref(this.__id));
  } catch {
    return true;
  }
};
// node 口径 inspect（inspect-during-init-hook 套件逐字 strictEqual +
// transfer-self 套件 includes 断言；完整 EventTarget 形为引擎边界记档，
// 只保 `active`/`refed` 开关语义）。
MessagePort.prototype[Symbol.for("nodejs.util.inspect.custom")] = function () {
  let nativeActive = true;
  try {
    nativeActive = __wjs2_port_is_active(this.__id) === true;
  } catch { /* 忽略 */ }
  let refed = true;
  try {
    refed = this.hasRef();
  } catch { /* 忽略 */ }
  return `MessagePort { active: ${!this.__closed && !this.__neutered && nativeActive}, refed: ${refed} }`;
};

export function receiveMessageOnPort(port) {
  // 10f 对拍：BroadcastChannel 亦收（node BC 底层即 MessagePort，
  // broadcastchannel 套件 `receiveMessageOnPort(bc2)` 点名）。
  if (!(port instanceof MessagePort) && !(port instanceof BroadcastChannel)) {
    const err = new TypeError("The \"port\" argument must be a MessagePort instance");
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  // 10f：优先 Rust pending 表（postMessage 本地路由的同步收信口）；再退 JS
  // 队列（迁移/BC 排队/历史路径）。同步收信无 ports 上下文——残留 offer 槽
  // 撤回收（已被行内承接者消费即 no-op，见 __withdrawPorts）。
  if (port instanceof MessagePort) {
    const wire = __wjs2_port_try_recv(port.__id);
    if (wire !== "") {
      const message = __fromWire(wire);
      __withdrawPorts(wire);
      return { message };
    }
  }
  if (port instanceof BroadcastChannel) {
    const wire = __wjs2_bc_try_recv(port.__sub);
    if (wire !== "") return { message: __fromWire(wire) };
  }
  if (port.__queue.length === 0) return undefined;
  const wire = port.__queue.shift();
  const message = __fromWire(wire);
  __withdrawPorts(wire);
  return { message };
}

export function moveMessagePortToContext(port, context) {
  if (!(port instanceof MessagePort)) {
    const err = new TypeError("The \"port\" argument must be an instance of MessagePort");
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  // 10f 对拍：已关闭端口先抛（close 套件块；真机 ERR_CLOSED_MESSAGE_PORT）。
  // 上下文实搬（跨 compartment 纠缠）另案——此处直通（存量黑盒 th-move 钉）。
  if (port.__closed || port.__neutered) throw new codes.ERR_CLOSED_MESSAGE_PORT();
  void context;
  return port;
}

// 函数式（非 class）：无 new 调用须抛 node 文案
// ERR_CONSTRUCT_CALL_REQUIRED（class 原生错带不上 code）。
export function MessageChannel() {
  if (new.target === undefined) throw new codes.ERR_CONSTRUCT_CALL_REQUIRED();
  const [a, b] = String(__wjs2_port_pair()).split(" ");
  this.port1 = new MessagePort(a, __kPortInternal);
  this.port2 = new MessagePort(b, __kPortInternal);
}
