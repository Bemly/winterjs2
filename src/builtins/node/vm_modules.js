// ── 9i-1 模块系（真机口径：node --experimental-vm-modules 实测，见模块头注）──
// SourceTextModule：Rust 侧编译/link/evaluate（零导入全链；带导入 link 即
// ERR_VM_MODULE_LINK_FAILURE，linker 切片后续）；SyntheticModule 纯 JS。

// 模块标识真机按 context 独立计数（无 context 共用默认计数，26.8.1 实测）。
const __vmModSeqs = new Map();
function __vmModSeqFor(key) {
  const n = __vmModSeqs.get(key) ?? 0;
  __vmModSeqs.set(key, n + 1);
  return n;
}

// invalidArgTypeHelper 近似（cachedData/exportNames 文案用，真机实测采样）。
function __recvArg(v) {
  if (v === undefined) return "undefined";
  if (v === null) return "null";
  if (Array.isArray(v)) return "an instance of Array";
  const t = typeof v;
  if (t === "string") return `type string ('${v}')`;
  if (t === "function") return `function ${v.name || "anonymous"}`;
  if (t === "object") return "an instance of Object";
  return `type ${t} (${String(v)})`;
}

// 存活模块实例集合（__assertThis 凭据：原型对象/`Object.create` 产物虽
// instanceof 通过但非构造实例，真机照样 ERR_INVALID_THIS）。
const __vmModLive = new WeakSet();

// 源码内容哈希（模块 cachedData 存根的匹配标记；V8 真缓存另案，记档）。
function __vmHashStr(s) {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h.toString(16);
}
function __vmModTag(code) {
  return `wjs2mod:${__vmHashStr(code)}`;
}

function __modErr(code, message) {
  // 真机码类→构造器映射（实测）：ARG_TYPE→TypeError、OUT_OF_RANGE→RangeError，
  // 其余 Error；只定构造器，码仍挂载（module-errors 点名 name）。
  let err;
  if (code === "ERR_INVALID_ARG_TYPE") err = new TypeError(message);
  else if (code === "ERR_OUT_OF_RANGE") err = new RangeError(message);
  else err = new Error(message);
  err.code = code;
  throw err;
}
function __modStatusErr(code, message) {
  const err = new Error(message);
  err.code = code;
  return Promise.reject(err);
}

export class Module {
  constructor() {
    // 抽象基：直接构造即 TypeError（真机 `Module is not a constructor`）。
    if (new.target === Module) {
      throw new TypeError("Module is not a constructor");
    }
    __vmModLive.add(this);
  }
  __assertThis() {
    // 凭据式（真机口径）：原型对象/`Object.create` 产物、`Module.prototype`
    // 方法借调一律 ERR_INVALID_THIS（instanceof 不足为凭）。
    if ((typeof this !== "object" && typeof this !== "function") || this === null || !__vmModLive.has(this)) {
      const err = new TypeError('Value of "this" must be of type Module');
      err.code = "ERR_INVALID_THIS";
      throw err;
    }
  }
  // 基类访问器（真机基类即带；实例走子类覆写，此处仅作借调守卫）。
  get status() { this.__assertThis(); }
  get identifier() { this.__assertThis(); }
  get context() { this.__assertThis(); }
  get namespace() { this.__assertThis(); }
  get error() { this.__assertThis(); }
  link(linker) {
    this.__assertThis();
    if (typeof linker !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "linker" argument must be of type function. Received ${linker === null ? "null" : typeof linker}`);
    }
    return this.__doLink(linker);
  }
  evaluate(options) {
    this.__assertThis();
    let timeout = 0;
    if (options !== undefined) {
      if (options === null || (typeof options !== "object" && typeof options !== "function")) {
        __modErr("ERR_INVALID_ARG_TYPE", `The "options" argument must be of type object. Received ${__recv(options)}`);
      }
      if (options.timeout !== undefined) {
        __normTimeout(options.timeout, "timeout");
        timeout = options.timeout;
      }
      if (options.breakOnSigint !== undefined && typeof options.breakOnSigint !== "boolean") {
        __modErr("ERR_INVALID_ARG_TYPE", `The "options.breakOnSigint" property must be of type boolean. Received ${__recv(options.breakOnSigint)}`);
      }
    }
    return this.__doEvaluate(timeout);
  }
}

export class SourceTextModule extends Module {
  constructor(code, options = {}) {
    super();
    if (typeof code !== "string") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "code" argument must be of type string. Received type ${typeof code}`);
    }
    // 模块 options 真机用严格对象校验（函数不收；run 系宽松口径另案）。
    if (options === null || typeof options !== "object") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options" argument must be of type object. Received ${options === null ? "null" : `type ${typeof options}`}`);
    }
    let ctxKey = "";
    let ctxId;
    if (options.context !== undefined) {
      ctxId = __validateCtx(options.context);
      ctxKey = String(ctxId);
      this.__context = options.context;
    } else {
      // 无显式 context 即主域（真机口径）：不建新 global，标识走默认计数。
      ctxId = "main";
      this.__context = undefined;
    }
    const identifier = __normStr(options.identifier, "identifier", `vm:module(${__vmModSeqFor(ctxKey)})`);
    this.__ctxId = ctxId;
    this.__identifier = identifier;
    this.__status = "unlinked";
    this.__error = null;
    this.__ns = undefined;
    this.__source = code;
    this.__linkReqs = undefined;
    this.__depCache = undefined;
    this.__modReqCache = undefined;
    // cachedData 接受并记忆匹配结论（存根：内容哈希比对，见 createCachedData）。
    this.__cachedRejected = undefined;
    if (options.cachedData !== undefined) {
      const cd = options.cachedData;
      if (!(typeof ArrayBuffer.isView === "function" && ArrayBuffer.isView(cd)) && !(cd instanceof ArrayBuffer)) {
        __modErr("ERR_INVALID_ARG_TYPE", `The "options.cachedData" property must be an instance of Buffer, TypedArray, or DataView. Received ${__recvArg(cd)}`);
      }
      let tag = "";
      try { tag = Buffer.from(cd).toString("utf8"); } catch { tag = ""; }
      this.__cachedRejected = tag !== __vmModTag(code);
      if (this.__cachedRejected) {
        __modErr("ERR_VM_MODULE_CACHED_DATA_REJECTED", "Cached data rejected");
      }
    }
    // importModuleDynamically/initializeImportMeta 只校验（v1 未接线，记档）。
    if (options.importModuleDynamically !== undefined && typeof options.importModuleDynamically !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options.importModuleDynamically" property must be of type function. Received ${__recvArg(options.importModuleDynamically)}`);
    }
    if (options.initializeImportMeta !== undefined && typeof options.initializeImportMeta !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options.initializeImportMeta" property must be of type function. Received ${__recvArg(options.initializeImportMeta)}`);
    }
    this.__id = __vmCall(() => __wjs2_vm_compile_mod(ctxId, identifier, code));
    __vmModAutoRelease(this, this.__id);
  }
  get status() { this.__assertThis(); return this.__status; }
  get identifier() { this.__assertThis(); return this.__identifier; }
  get context() { this.__assertThis(); return this.__context; }
  get dependencySpecifiers() {
    this.__assertThis();
    if (this.__depCache === undefined) {
      this.__depCache = JSON.parse(__vmCall(() => __wjs2_vm_mod_deps(this.__id)));
    }
    return this.__depCache;
  }
  get moduleRequests() {
    this.__assertThis();
    if (this.__modReqCache === undefined) {
      this.__modReqCache = this.dependencySpecifiers.map((s) => ({ specifier: s, attributes: {}, phase: "evaluation" }));
    }
    return this.__modReqCache;
  }
  get namespace() {
    this.__assertThis();
    if (this.__status !== "evaluated" && this.__status !== "evaluating" && this.__status !== "linked") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must not be unlinked or linking");
    }
    // linked 预取（instantiate/link 落定时抓命名空间；未初始化导出访问
    // 按规范抛 ReferenceError，真机口径）。
    if (this.__ns === undefined) {
      this.__ns = __vmCall(() => __wjs2_vm_mod_ns(this.__id));
    }
    return this.__ns;
  }
  get error() {
    this.__assertThis();
    if (this.__status !== "errored") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be errored");
    }
    return this.__error;
  }
  get cachedDataRejected() { return this.__cachedRejected; }
  createCachedData() {
    if (this.__status === "evaluated" || this.__status === "errored") {
      __modErr("ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA", "Cannot create cached data for a module that has already been evaluated");
    }
    return Buffer.from(__vmModTag(this.__source), "utf8");
  }
  linkRequests(requests) {
    this.__assertThis();
    if (!Array.isArray(requests)) {
      __modErr("ERR_INVALID_ARG_TYPE", `The "modules" argument must be an instance of Array. Received ${__recvArg(requests)}`);
    }
    this.__linkReqs = requests;
    return undefined;
  }
  instantiate() {
    this.__assertThis();
    if (this.__status !== "unlinked") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be unlinked");
    }
    if (this.__linkReqs === undefined) {
      __modErr("ERR_VM_MODULE_LINK_FAILURE", "module is not linked");
    }
    __vmCall(() => __wjs2_vm_link(this.__id));
    this.__status = "linked";
    return undefined;
  }
  __doLink(linker) {
    if (this.__status !== "unlinked") {
      if (this.__status === "linking" || this.__status === "evaluating") {
        return __modStatusErr("ERR_VM_MODULE_STATUS", "Module status must be unlinked");
      }
      return __modStatusErr("ERR_VM_MODULE_ALREADY_LINKED", "Module has already been linked");
    }
    // linking 同步落位（真机 link 后 status 即 linking，实测）。
    this.__status = "linking";
    void linker;
    return Promise.resolve().then(() => {
      try {
        __vmCall(() => __wjs2_vm_link(this.__id));
      } catch (e) {
        // 链接失败即 errored（真机口径；v1 零导入外请求同样落 errored）。
        this.__status = "errored";
        this.__error = e;
        throw e;
      }
      this.__status = "linked";
    });
  }
  __doEvaluate(timeoutMs = 0) {
    if (this.__status !== "linked" && this.__status !== "evaluating" && this.__status !== "evaluated" && this.__status !== "errored") {
      return __modStatusErr("ERR_VM_MODULE_STATUS", "Module status must be one of linked, evaluated, or errored");
    }
    if (this.__status === "evaluated") return Promise.resolve(undefined);
    if (this.__status === "errored") return Promise.reject(this.__error);
    if (this.__status === "evaluating") {
      return __modStatusErr("ERR_VM_MODULE_STATUS", "Module status must be one of linked, evaluated, or errored");
    }
    this.__status = "evaluating";
    // native 原样返回 completion；同步落定经内态 API 判定（`__wjs2_promise_state`
    // 内解包，跨 compartment 安全；`export then` 病态模块记档）：
    // fulfilled→同步记账；rejected→认领因（挂空处理器消运行时 unhandled，
    // 不能让引擎孤儿 promise 触发 fatal）并同步失败；pending（TLA）→异步认领。
    let r;
    try {
      r = __vmCall(() => __wjs2_vm_evaluate(this.__id, String(timeoutMs)), 0, 0, timeoutMs);
    } catch (e) {
      this.__status = "errored";
      this.__error = e;
      return Promise.reject(e);
    }
    const done = () => {
      __vmCall(() => __wjs2_vm_mod_settled(this.__id));
      this.__status = "evaluated";
      this.__ns = __vmCall(() => __wjs2_vm_mod_ns(this.__id));
      return undefined;
    };
    const failed = (e) => {
      this.__status = "errored";
      this.__error = e;
      throw e;
    };
    if (r !== null && (typeof r === "object" || typeof r === "function") && typeof r.then === "function") {
      let st = -1;
      try { st = __wjs2_promise_state(r); } catch { st = -1; }
      if (st === 1) {
        try {
          done();
        } catch (e) {
          return failed(e);
        }
        return Promise.resolve(undefined);
      }
      if (st === 2) {
        let reason;
        try {
          reason = __wjs2_promise_result(r);
        } catch (e2) {
          return failed(e2);
        }
        try { r.then(undefined, () => {}); } catch { /* 已认领即够 */ }
        this.__status = "errored";
        this.__error = reason;
        return Promise.reject(reason);
      }
      // TLA/异步：落定前命名空间即存在（未初始化导出访问抛 ReferenceError，
      // 真机口径），先行认领；落定后 done() 重取同一对象。
      try {
        this.__ns = __vmCall(() => __wjs2_vm_mod_ns(this.__id));
      } catch { /* 未就绪即跳过，落定时重取 */ }
      return r.then(done, failed);
    }
    try {
      done();
    } catch (e) {
      return failed(e);
    }
    return Promise.resolve(undefined);
  }
}

export class SyntheticModule extends Module {
  constructor(exportNames, evaluateCallback, options = {}) {
    super();
    if (!Array.isArray(exportNames)) {
      __modErr("ERR_INVALID_ARG_TYPE", `The "exportNames" argument must be an Array of unique strings. Received ${__recvArg(exportNames)}`);
    }
    if (typeof evaluateCallback !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "evaluateCallback" argument must be of type function. Received ${__recvArg(evaluateCallback)}`);
    }
    if (options === null || (typeof options !== "object" && typeof options !== "function")) {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options" argument must be of type object. Received ${__recvArg(options)}`);
    }
    this.__exports = {};
    const seen = new Set();
    for (const n of exportNames) {
      if (typeof n !== "string") {
        __modErr("ERR_INVALID_ARG_TYPE", `The "exportNames" argument must be an Array of unique strings. Received ${__recvArg(exportNames)}`);
      }
      if (seen.has(n)) {
        __modErr("ERR_INVALID_ARG_VALUE", `The property 'exportNames.${n}' is duplicated. Received '${n}'`);
      }
      seen.add(n);
      this.__exports[n] = undefined;
    }
    this.__cb = evaluateCallback;
    // Synthetic 无显式 context 概念：标识走默认计数（真机实测）。
    this.__identifier = __normStr(options.identifier, "identifier", `vm:module(${__vmModSeqFor("")})`);
    this.__status = "linked";
    this.__error = null;
    // 命名空间活绑定：冻结外壳 + getter 直读 __exports（setExport 评估后仍生效，真机）。
    const ns = {};
    for (const n of seen) {
      Object.defineProperty(ns, n, { enumerable: true, configurable: true, get: () => this.__exports[n] });
    }
    this.__ns = Object.freeze(ns);
  }
  get status() { this.__assertThis(); return this.__status; }
  get identifier() { this.__assertThis(); return this.__identifier; }
  get dependencySpecifiers() { this.__assertThis(); return undefined; }
  get namespace() {
    this.__assertThis();
    if (this.__status !== "evaluated" && this.__status !== "evaluating") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be evaluated");
    }
    return this.__ns;
  }
  get error() {
    this.__assertThis();
    if (this.__status !== "errored") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be errored");
    }
    return this.__error;
  }
  setExport(name, value) {
    this.__assertThis();
    if (typeof name !== "string") {
      const err = new TypeError(`The "name" argument must be of type string. Received ${__recvArg(name)}`);
      throw err;
    }
    if (!(name in this.__exports)) {
      throw new ReferenceError(`Export '${name}' does not exist`);
    }
    this.__exports[name] = value;
  }
  // Synthetic 的 linker 可选（真机口径：无参 link 即过）。
  link(linker) {
    this.__assertThis();
    if (linker !== undefined) return super.link(linker);
    return this.__doLink(undefined);
  }
  __doLink(linker) {
    void linker;
    // Synthetic 出生即 linked；重复 link 照真机保持 linked（无操作成功）。
    return Promise.resolve(undefined);
  }
  __doEvaluate(timeoutMs = 0) {
    if (this.__status === "evaluated") return Promise.resolve(undefined);
    if (this.__status === "errored") return Promise.reject(this.__error);
    if (this.__status === "evaluating") {
      return __modStatusErr("ERR_VM_MODULE_STATUS", "Module status must not be evaluating");
    }
    // 求值步同步执行（真机：同步完成即同步落定；异步回调的拒绝走
    // unhandledRejection——按监听有无分流：有监听则手动派发（运行时不发该
    // 事件、只做 fatal），无监听则抛回浮动 promise 交运行时 fatal，
    // 与真机无监听行为一致）。
    this.__status = "evaluating";
    try {
      const r = this.__cb(this.__nsHelper());
      if (r !== null && (typeof r === "object" || typeof r === "function") && typeof r.then === "function") {
        r.then(undefined, (e) => {
          let n = 0;
          try { n = (typeof process !== "undefined" && process.listenerCount) ? process.listenerCount("unhandledRejection") : 0; } catch { n = 0; }
          if (n > 0) {
            process.emit("unhandledRejection", e, r);
          } else {
            throw e;
          }
        });
      }
    } catch (e) {
      this.__status = "errored";
      this.__error = e;
      return Promise.reject(e);
    }
    this.__status = "evaluated";
    return Promise.resolve(undefined);
  }
  __nsHelper() {
    // 评估回调拿到的设置器（真机按导出名设值）。
    return { setExport: (k, v) => this.setExport(k, v) };
  }
}

const __api = {
  Script, createContext, createScript, runInContext, runInNewContext,
  runInThisContext, isContext, compileFunction, measureMemory, constants,
  Module, SourceTextModule, SyntheticModule,
};
export default __api;
