//! DOM 事件（Event 家族/EventTarget/AbortSignal-Controller）（prelude 分域；拼接顺序见 mod.rs）。
pub const EVENTS_JS: &str = r#"
// ---- M5: 全局 Event / EventTarget / CustomEvent（Node 平坦派发口径）----
// Node 的 EventTarget 不实现捕获/冒泡 propagation path（官方文档明言）：
// capture 选项仅为 removeEventListener 匹配保留；listener 收函数或 {handleEvent}。
// 事件状态走共享 WeakMap（Event 与 EventTarget 跨类要读写字段，# 私有够不着；
// 与既有 __wjs2_abortState 同风格，前缀避免污染全局面）。
const __wjs2_eventState = new WeakMap();
const __wjs2_etState = new WeakMap();
// events/listenerCount/getEventListeners 侧读桥（EventTarget 直接注册可见；
// 无表/已摘即 []；仅内部用，不属公开面）。
globalThis.__wjs2_etListeners = (target, type) => {
  try {
    const st = __wjs2_etState.get(target);
    if (!st) return [];
    const list = st.get(String(type)) ?? [];
    return list.filter((e) => !e.removed).map((e) => e.listener);
  } catch { return []; }
};
// events.setMaxListeners(EventTarget 形)落点（max 缺省 10；warned 随设复位）。
const __wjs2_etMax = new WeakMap();
globalThis.__wjs2_etSetMax = (target, n) => {
  try {
    const cur = __wjs2_etMax.get(target) ?? {};
    cur.max = n; cur.warned = false;
    __wjs2_etMax.set(target, cur);
  } catch {}
};
globalThis.Event = class Event {
  constructor(type, options = {}) {
    if (arguments.length === 0) throw new TypeError('The "type" argument must be specified');
    // eventtarget 套件逐字：options 非空非对象即 ARG_TYPE（webidl validateObject 口径）。
    if (options !== undefined && options !== null && (typeof options !== "object" && typeof options !== "function")) {
      const recv = typeof options === "string" ? `'${options}'` : String(options);
      const e = new TypeError(`The "options" argument must be of type object. Received type ${typeof options} (${recv})`);
      e.code = "ERR_INVALID_ARG_TYPE"; throw e;
    }
    const o = options ?? {};
    __wjs2_eventState.set(this, {
      type: `${type}`,
      bubbles: !!o.bubbles,
      cancelable: !!o.cancelable,
      composed: !!o.composed,
      defaultPrevented: false,
      stopped: false,
      immediate: false,
      dispatching: false,
      timeStamp: Date.now(),
      target: null,
      currentTarget: null,
      phase: 0,
      path: null,
    });
  }
  get type() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).type; }
  get bubbles() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).bubbles; }
  get cancelable() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).cancelable; }
  get composed() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).composed; }
  get timeStamp() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).timeStamp; }
  get defaultPrevented() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).defaultPrevented; }
  get target() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).target; }
  get currentTarget() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).currentTarget; }
  get srcElement() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).target; }
  get isTrusted() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return false; }
  // eventtarget 套件：派发期 composedPath 为 [target]（平坦派发无捕获冒泡），
  // 余时 []（真机逐项）。
  composedPath() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).path ?? []; }
  // returnValue（cancelable+prevented 才 false）、eventPhase 恒 0、
  // cancelBubble 读写 stopped 旗、initEvent 重置三元、阶段静态值。
  get returnValue() {
    if (!__wjs2_isEvent(this)) __wjs2_needEvent();
    const s = __wjs2_eventState.get(this);
    return !(s.cancelable && s.defaultPrevented);
  }
  // eventtarget 套件：派发期 AT_TARGET(2)，余时 0（Node 无捕获冒泡）。
  get eventPhase() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return __wjs2_eventState.get(this).phase; }
  get cancelBubble() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return !!__wjs2_eventState.get(this).stopped; }
  set cancelBubble(v) { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); __wjs2_eventState.get(this).stopped = !!v; }
  initEvent(type, bubbles = false, cancelable = false) {
    if (!__wjs2_isEvent(this)) __wjs2_needEvent();
    const s = __wjs2_eventState.get(this);
    if (s.dispatching) return;
    s.type = String(type);
    s.bubbles = !!bubbles;
    s.cancelable = !!cancelable;
  }
  // events-customevent 套件：depth<0 即类名（余者含类名的精简形；本仓
  // inspect 无 undefined 回落，遂不返 undefined）。
  [Symbol.for("nodejs.util.inspect.custom")](depth) {
    const name = (this.constructor && this.constructor.name) || "Event";
    if (depth < 0) return name;
    try {
      return `${name} { type: '${__wjs2_eventState.get(this).type}' }`;
    } catch { return name; }
  }
  get [Symbol.toStringTag]() { return "Event"; }
  preventDefault() {
    if (!__wjs2_isEvent(this)) __wjs2_needEvent();
    const s = __wjs2_eventState.get(this);
    // passive 监听中的 preventDefault 是 no-op（passive 套件点名
    // defaultPrevented 保持 false 且 dispatch 返回 false）。
    if (s.cancelable && !s.passiveNow) s.defaultPrevented = true;
  }
  stopPropagation() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); __wjs2_eventState.get(this).stopped = true; }
  stopImmediatePropagation() {
    if (!__wjs2_isEvent(this)) __wjs2_needEvent();
    const s = __wjs2_eventState.get(this);
    s.stopped = true;
    s.immediate = true;
  }
  static NONE = 0;
  static CAPTURING_PHASE = 1;
  static AT_TARGET = 2;
  static BUBBLING_PHASE = 3;
};
globalThis.CustomEvent = class CustomEvent extends Event {
  #detail;
  constructor(type, options = {}) {
    if (arguments.length === 0) throw new TypeError('The "type" argument must be specified');
    super(type, options);
    this.#detail = (options ?? {}).detail ?? null;
  }
  get detail() {
    if (!__wjs2_isEvent(this)) __wjs2_needEvent();
    try { return this.#detail; } catch {
      const e = new TypeError('Value of "this" must be of type CustomEvent');
      e.code = "ERR_INVALID_THIS"; throw e;
    }
  }
  get [Symbol.toStringTag]() { return "CustomEvent"; }
};
// undici webidl 口径的值回显（MessageEvent 校验文案；真机逐形实测）：
// instanceOf 消息 = `"` + inspect(v, {quotes:'double'}) + `"`（"str" 形串自带
// 双引号故现 `""str""`；数字/容器仅外包一对）；not-iterable 用裸 inspect。
// 覆盖套件点名的形状（标量/空容器/数组/类实例），完整 inspect 面在 util。
const __wjs2_insp = (v) => {
  if (v === null) return "null";
  if (v === undefined) return "undefined";
  const t = typeof v;
  if (t === "string") {
    const body = v.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    return `"${body}"`;
  }
  if (t === "number" || t === "boolean" || t === "bigint") return String(v);
  if (t === "symbol") return v.toString();
  if (t === "function") return `[Function: ${v.name || "(anonymous)"}]`;
  if (Array.isArray(v)) return `[ ${v.map((x) => __wjs2_insp(x)).join(", ")} ]`;
  const n = v.constructor && v.constructor.name && v.constructor.name !== "Object" ? v.constructor.name : null;
  const keys = Object.keys(v);
  const body = keys.length === 0 ? "" : ` ${keys.map((k) => `${k}: ${__wjs2_insp(v[k])}`).join(", ")} `;
  return n ? `${n} {${body}}` : `{${body}}`;
};
const __wjs2_inspQuoted = (v) => `"${__wjs2_insp(v)}"`;
// Rust 侧取 symbol 描述（ToString 对 symbol 抛 TypeError；JS 侧 toString 合法）。
globalThis.__wjs2_symToString = (v) => (typeof v === "symbol") ? v.toString() : null;
// invalidArgTypeHelper 近似（eventtarget 套件逐字）：null/undef 原样，
// 函数名形，对象实例形，余下 inspect 截断（本仓 __wjs2_insp 口径）。
const __wjs2_evArgHelper = (v) => {
  if (v === null || v === undefined) return ` Received ${String(v)}`;
  if (typeof v === "function") return ` Received function ${v.name || "(anonymous)"}`;
  if (typeof v === "string") return ` Received type string ('${v}')`;
  if (typeof v === "object") {
    const n = v.constructor && v.constructor.name;
    if (typeof n === "string" && n !== "") return ` Received an instance of ${n}`;
  }
  let s = __wjs2_insp(v);
  if (s.length > 28) s = `${s.slice(0, 25)}...`;
  return ` Received type ${typeof v} (${s})`;
};
const __wjs2_evMissingArgs = (name) => {
  const e = new TypeError(`The "${name}" argument must be specified`);
  e.code = "ERR_MISSING_ARGS"; return e;
};
// 品牌检查（eventtarget-brandcheck 套件：ERR_INVALID_THIS 真机逐字）。
const __wjs2_isEvent = (v) => {
  try { return __wjs2_eventState.has(v); } catch { return false; }
};
const __wjs2_isET = (v) => {
  try { return __wjs2_etState.has(v); } catch { return false; }
};
const __wjs2_needEvent = () => {
  const e = new TypeError('Value of "this" must be of type Event');
  e.code = "ERR_INVALID_THIS"; throw e;
};
const __wjs2_needET = () => {
  const e = new TypeError('Value of "this" must be of type EventTarget');
  e.code = "ERR_INVALID_THIS"; throw e;
};
// 10f：全局 MessageEvent（node 26 主/worker 线程均全局；message-port/
// message-event 套件逐项对拍）。source/ports 须 MessagePort 实例——品牌经
// worker 模块求值期登记的 `__wjs2_MessagePort` 隐藏槽判定（主线程无全局
// MessagePort；求值前无从有端口，非 null source 即 TypeError 正确）。
globalThis.MessageEvent = class MessageEvent extends Event {
  #data; #origin; #lastEventId; #source; #ports;
  constructor(type, init = {}) {
    if (arguments.length === 0) throw new TypeError('The "type" argument must be specified');
    if (arguments.length === 0) throw new TypeError("MessageEvent requires at least 1 argument, but only 0 were passed");
    super(type, init);
    const o = init ?? {};
    this.#data = o.data ?? null;
    this.#origin = String(o.origin ?? "");
    this.#lastEventId = String(o.lastEventId ?? "");
    const src = o.source ?? null;
    if (src !== null) {
      const M = globalThis.__wjs2_MessagePort;
      if (!M || !(src instanceof M)) {
        throw new TypeError(`MessageEvent constructor: Expected eventInitDict.source (${__wjs2_inspQuoted(src)}) to be an instance of MessagePort.`);
      }
    }
    this.#source = src;
    let ports = o.ports;
    if (ports !== undefined && ports !== null) {
      if (typeof ports[Symbol.iterator] !== "function") {
        throw new TypeError(`MessageEvent constructor: eventInitDict.ports (${__wjs2_insp(ports)}) is not iterable.`);
      }
      const list = [...ports];
      for (let i = 0; i < list.length; i++) {
        const M2 = globalThis.__wjs2_MessagePort;
        if (!M2 || !(list[i] instanceof M2)) {
          throw new TypeError(`MessageEvent constructor: Expected eventInitDict.ports[${i}] (${__wjs2_inspQuoted(list[i])}) to be an instance of MessagePort.`);
        }
      }
      this.#ports = list;
    } else {
      this.#ports = [];
    }
    // 内部派发目标（`__wjs2Target` 不属 WebIDL 字典面，仅宿主 port 桥使用）。
    const tgt = o.__wjs2Target;
    if (tgt) __wjs2_eventState.get(this).target = tgt;
  }
  get data() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#data; }
  get origin() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#origin; }
  get lastEventId() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#lastEventId; }
  get source() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#source; }
  get ports() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#ports; }
  get [Symbol.toStringTag]() { return "MessageEvent"; }
};
// WebSocket CloseEvent（import-websocket 套件：node:http 重导出与全局同一性）。
// WebIDL 口径：code/reason/wasClean 只读，缺省 0/""/false。
globalThis.CloseEvent = class CloseEvent extends Event {
  #code; #reason; #wasClean;
  constructor(type, init = {}) {
    if (arguments.length === 0) throw new TypeError('The "type" argument must be specified');
    if (arguments.length === 0) throw new TypeError("CloseEvent requires at least 1 argument, but only 0 were passed");
    super(type, init);
    const o = init ?? {};
    this.#code = o.code ?? 0;
    this.#reason = String(o.reason ?? "");
    this.#wasClean = o.wasClean ?? false;
  }
  get code() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#code; }
  get reason() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#reason; }
  get wasClean() { if (!__wjs2_isEvent(this)) __wjs2_needEvent(); return this.#wasClean; }
  get [Symbol.toStringTag]() { return "CloseEvent"; }
};
globalThis.EventTarget = class EventTarget {
  constructor() {
    __wjs2_etState.set(this, new Map());
  }
  get [Symbol.toStringTag]() { return "EventTarget"; }
  addEventListener(type, listener, options = {}) {
    // eventtarget-brandcheck 套件：品牌先于元数（node 真机序）。
    if (!__wjs2_isET(this)) __wjs2_needET();
    if (arguments.length < 2) throw __wjs2_evMissingArgs("listener");
    // eventtarget 套件：Symbol type 即 TypeError（余者 String 化，真机逐项）。
    if (typeof type === "symbol") throw new TypeError("Cannot convert a Symbol value to a string");
    const o = typeof options === "boolean" ? { capture: options } : (options ?? {});
    // WebIDL 扁平化先行（passive/capture/once/signal 取值 → 用户 getter 可
    // 观察；null 回调仍须读 passive——passive 套件点名，DOM 口径 "callback
    // null 即 return" 在扁平化之后）。
    const capture = !!o.capture;
    const passive = !!o.passive;
    const once = !!o.once;
    const hasSignal = o.signal !== undefined;
    const signal = hasSignal ? o.signal : null;
    if (listener === null || listener === undefined) {
      // eventtarget 套件：null/undefined 不抛，发参数类型 Warning（真机逐项）。
      try {
        const w = new Error(`addEventListener called with ${String(listener)} which has no effect.`);
        w.name = "AddEventListenerArgumentTypeWarning";
        w.target = this;
        w.type = String(type);
        if (typeof process !== "undefined" && process !== null && typeof process.emitWarning === "function") {
          process.emitWarning(w);
        }
      } catch {}
      return;
    }
    if (typeof listener !== "function" && (typeof listener !== "object" || listener === null)) {
      const e = new TypeError(`The "listener" argument must be an instance of EventListener.${__wjs2_evArgHelper(listener)}`);
      e.code = "ERR_INVALID_ARG_TYPE"; throw e;
    }
    // handleEvent 查找递延到派发时（this-of-listener 套件：后加 handleEvent
    // 仍须调用；函数自带 handleEvent 属性仍走函数调用）。
    if (hasSignal) {
      // node 口径：signal 须 AbortSignal 形（null 亦抛；signal 套件点名码）。
      const bad = !signal || (typeof signal !== "object" && typeof signal !== "function") ||
        typeof signal.aborted !== "boolean" || typeof signal.addEventListener !== "function";
      if (bad) {
        const recv = signal === null ? "null"
          : (typeof signal === "object" ? `an instance of ${signal.constructor?.name ?? "Object"}` : (typeof signal === "string" ? `'${signal}'` : String(signal)));
        const e = new TypeError(`The "options.signal" property must be an instance of AbortSignal. Received ${recv}`);
        e.code = "ERR_INVALID_ARG_TYPE";
        throw e;
      }
      if (signal.aborted) return;
    }
    // Proxy 目标无表决不抛（mustNotMutate 包裹的 signal 形 addEventListener；
    // 监听记代理身份下，触发侧 miss 即 benign——abort 竞速由 aborted 轮询门覆盖）。
    let st = __wjs2_etState.get(this);
    if (!st) { st = new Map(); __wjs2_etState.set(this, st); }
    const key = String(type);
    const list = st.get(key) ?? [];
    if (list.some((e) => e.listener === listener && e.capture === capture)) return;
    const entry = { listener, once, capture, passive, signal, removed: false };
    list.push(entry);
    st.set(key, list);
    // eventtarget-memoryleakwarning 套件：超 max 即 Warning（单 target 一次）。
    try {
      const mm = __wjs2_etMax.get(this) ?? {};
      const max = mm.max !== undefined ? mm.max : 10;
      if (max > 0 && list.length > max && !mm.warned) {
        mm.warned = true;
        __wjs2_etMax.set(this, mm);
        const insp = (this.constructor && this.constructor.name) || "EventTarget";
        const w = new Error(`Possible EventTarget memory leak detected. ${list.length} ${key} listeners added to ${insp}. MaxListeners is ${max}. Use events.setMaxListeners() to increase limit`);
        w.name = "MaxListenersExceededWarning";
        w.target = this;
        w.type = key;
        w.count = list.length;
        if (typeof process !== "undefined" && process !== null && typeof process.emitWarning === "function") {
          process.emitWarning(w);
        }
      }
    } catch {}
    if (signal) signal.addEventListener("abort", () => this.removeEventListener(key, listener, options), { once: true });
  }
  removeEventListener(type, listener, options = {}) {
    // eventtarget-brandcheck 套件：品牌先于元数（node 真机序）。
    if (!__wjs2_isET(this)) __wjs2_needET();
    if (arguments.length < 2) throw __wjs2_evMissingArgs("listener");
    if (typeof type === "symbol") throw new TypeError("Cannot convert a Symbol value to a string");
    const o = typeof options === "boolean" ? { capture: options } : (options ?? {});
    const st = __wjs2_etState.get(this);
    if (!st) return;
    const list = st.get(String(type));
    if (!list) return;
    const i = list.findIndex((e) => e.listener === listener && e.capture === !!o.capture && !e.removed);
    if (i >= 0) {
      list[i].removed = true;
      list.splice(i, 1);
    }
  }
  dispatchEvent(event) {
    // eventtarget-brandcheck 套件：品牌先于元数（node 真机序）。
    if (!__wjs2_etState.has(this)) {
      const e = new TypeError('Value of "this" must be of type EventTarget');
      e.code = "ERR_INVALID_THIS"; throw e;
    }
    if (arguments.length < 1) throw __wjs2_evMissingArgs("event");
    if (!(event instanceof Event)) {
      const e = new TypeError(`The "event" argument must be an instance of Event.${__wjs2_evArgHelper(event)}`);
      e.code = "ERR_INVALID_ARG_TYPE"; throw e;
    }
    const es = __wjs2_eventState.get(event);
    // eventtarget 套件：在途事件重派发即 ERR_EVENT_RECURSION（同/异 target 皆然）。
    if (es.dispatching === true) {
      const e = new Error(`The event "${es.type}" is already being dispatched`);
      e.code = "ERR_EVENT_RECURSION"; throw e;
    }
    const st = __wjs2_etState.get(this);
    if (!st) throw new TypeError("dispatchEvent called on non-EventTarget");
    es.target = this;
    es.dispatching = true;
    const __prevPhase = es.phase;
    es.phase = 2;
    const __prevPath = es.path;
    es.path = [this];
    const list = (st.get(es.type) ?? []).slice();
    // eventtarget 套件：监听异常不中断后续（同步抛/异步拒一律 uncaughtException，
    // 无 error 事件、无 unhandledRejection，真机逐项）。
    const __reportErr = (e) => {
      queueMicrotask(() => {
        try {
          if (typeof process !== "undefined" && process !== null && typeof process.emit === "function") {
            process.emit("uncaughtException", e, "uncaughtException");
          } else {
            throw e;
          }
        } catch {}
      });
    };
    const __isThenable = (r) => r !== null && (typeof r === "object" || typeof r === "function") && typeof r.then === "function";
    try {
      for (const entry of list) {
        if (es.immediate || entry.removed) continue;
        if (entry.signal?.aborted) continue;
        // node 口径：once 到点内部摘除，不走 public removeEventListener
        //（原生引擎不可观察；经 public 会误触用户覆写，abort 套件钉住）。
        if (entry.once) {
          entry.removed = true;
          const lst = st.get(es.type);
          if (lst !== undefined) {
            const i = lst.indexOf(entry);
            if (i >= 0) lst.splice(i, 1);
          }
        }
        es.currentTarget = this;
        // passive 窗内派发（preventDefault 递延门消费后复位；嵌套派发按栈恢复）。
        const prevPassive = es.passiveNow;
        es.passiveNow = !!entry.passive;
        try {
          try {
            if (typeof entry.listener === "function") {
              const __r = entry.listener.call(this, event);
              if (__isThenable(__r)) __r.then(undefined, __reportErr);
            } else {
              const h = entry.listener.handleEvent;
              if (typeof h === "function") {
                const __r = h.call(entry.listener, event);
                if (__isThenable(__r)) __r.then(undefined, __reportErr);
              }
            }
          } catch (__e) { __reportErr(__e); }
        } finally {
          es.passiveNow = prevPassive;
        }
      }
    } finally {
      es.dispatching = false;
      es.currentTarget = null;
      es.phase = __prevPhase;
      es.path = __prevPath;
    }
    return !(es.cancelable && es.defaultPrevented);
  }
};
// eventtarget 套件：原型三方法须可枚举（真机同；class 默认不可枚举）。
for (const __k of ["addEventListener", "removeEventListener", "dispatchEvent"]) {
  try {
    const __d = Object.getOwnPropertyDescriptor(globalThis.EventTarget.prototype, __k);
    if (__d) Object.defineProperty(globalThis.EventTarget.prototype, __k, { ...__d, enumerable: true });
  } catch {}
}

"#;
