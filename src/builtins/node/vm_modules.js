// ── 9i-1 模块系（真机口径：node --experimental-vm-modules 实测，见模块头注）──
// SourceTextModule：Rust 侧编译/link/evaluate（linker 图：JS 壳 BFS 调 linker
// 收集 (referrer, spec) → dep 边，native 换装会话 hook 经
// `LoadRequestedModules` 接线 + `ModuleLink`；零导入走旧单步路）；
// SyntheticModule 纯 JS（link 依赖位不可接线——引擎墙，见 __isSynthetic 注）。
import { inspect as __vsInspect } from "node:util";

// 动态导入落定帮忙（native 经 globalThis 取用；merge-note #2）。
// 回调/async 求值 promise 落定后调对应 done native（票据 + 值 + 是否失败）。
// 回调抛错不吞：done 侧按 rejection 结算（真机口径）。
globalThis.__wjs2_vm_dyn_then = (resultPromise, ticket) => {
  resultPromise.then(
    (v) => __wjs2_vm_dyn_done(String(ticket), v, false),
    (e) => __wjs2_vm_dyn_done(String(ticket), e, true),
  );
};
globalThis.__wjs2_vm_dyn_evalThen = (evalPromise, ticket) => {
  evalPromise.then(
    (v) => __wjs2_vm_dyn_eval_done(String(ticket), v, false),
    (e) => __wjs2_vm_dyn_eval_done(String(ticket), e, true),
  );
};

// 模块标识按 context 独立计数（无 context 共用默认计数，26.8.1 实测）。
// SourceText/Synthetic 共用同一计数器（真机同源，实测）。
const __vmModSeqs = new Map();
function __vmModSeqFor(key) {
  const n = __vmModSeqs.get(key) ?? 0;
  __vmModSeqs.set(key, n + 1);
  return n;
}

// 实例字段统一落为不可枚举（真机 `Object.keys(m)` 为空；inspect 走 custom）。
function __vmHideFields(m) {
  for (const k of Object.keys(m)) {
    Object.defineProperty(m, k, { enumerable: false });
  }
}

// 请求缓存键（linkRequests 键一致组用；phase 不入键，真机 ModuleCacheKey 口径）。
function __vmReqKey(r) {
  const attrs = r.attributes || {};
  const keys = Object.keys(attrs).sort();
  return r.specifier + "\n" + keys.map((k) => k + "=" + attrs[k]).join("\n");
}

// linker 第三参 extra（attributes/assert 同一空原型对象，真机实测）。
function __vmLinkExtra(r) {
  const attributes = Object.create(null);
  Object.assign(attributes, r.attributes || {});
  return { attributes, assert: attributes };
}

function __isModuleLive(m) {
  return (typeof m === "object" && m !== null) || typeof m === "function";
}

// 异步图传递闭包（hasAsyncGraph 面；link/instantiate 落定时机算一次，复用
// 时读缓存。`__tla` 为本模块静态标记；`__linkReqs` 为 linkRequests 绑定；
// `__tlaClosure` 为已 linker 链接者的记忆；环用 seen 集截断）。
function __vmTlaOf(m, seen) {
  if (!m || typeof m.__tla !== "boolean" || seen.has(m.__id)) return false;
  seen.add(m.__id);
  if (m.__tla === true || m.__tlaClosure === true) return true;
  const reqs = m.__linkReqs;
  if (Array.isArray(reqs)) {
    for (const d of reqs) {
      if (d && __vmTlaOf(d, seen)) return true;
    }
  }
  return false;
}

// inspect 名牌原型（`util.inspect` 按 `constructor.name` 打头；真机三字段体）。
const __vmInspectProtos = new Map();
function __vmInspectTag(name) {
  let p = __vmInspectProtos.get(name);
  if (p === undefined) {
    const ctor = function () {};
    Object.defineProperty(ctor, "name", { value: name });
    p = ctor.prototype;
    __vmInspectProtos.set(name, p);
  }
  return p;
}
function __vmInspectBody(m, name, depth, options) {
  // this 凭据与 getter 同口径（checkGettersErrors 点名 custom 借调）。
  if ((typeof m !== "object" && typeof m !== "function") || m === null || !__vmModLive.has(m)) {
    const err = new TypeError('Value of "this" must be of type Module');
    err.code = "ERR_INVALID_THIS";
    throw err;
  }
  if (typeof depth === "number" && depth < 0) return `[${name}]`;
  // 真机断行计入 `Name ` 前缀（plain 体 70 字符、单行 87：默认 80 即断；
  // 本仓 inspect 只计体，需此处复刻阈值——util 面动则删此分支）。
  const bl = (options && typeof options.breakLength === "number") ? options.breakLength : 80;
  const body = { status: m.__status, identifier: m.__identifier, context: m.__context };
  let single = "";
  try {
    single = `${name} ${__vsInspect(body, { ...(options || {}), colors: false, breakLength: 1e9 })}`;
  } catch { single = ""; }
  if (single !== "" && single.length <= bl) {
    Object.setPrototypeOf(body, __vmInspectTag(name));
    return __vsInspect(body, options);
  }
  const nested = (typeof depth === "number") ? { ...(options || {}), depth: depth - 1 } : options;
  return `${name} {\n  status: ${__vsInspect(m.__status, nested)},\n  identifier: ${__vsInspect(m.__identifier, nested)},\n  context: ${__vsInspect(m.__context, nested)}\n}`;
}

// invalidArgTypeHelper 近似（cachedData/exportNames 文案用，真机实测采样）。
function __recvArg(v) {
  if (v === undefined) return "undefined";
  if (v === null) return "null";
  if (Array.isArray(v)) return "an instance of Array";
  const t = typeof v;
  if (t === "string") return `type string ('${v}')`;
  if (t === "function") return `function ${v.name}`;
  if (t === "object") return "an instance of Object";
  if (t === "symbol") return `type symbol (${String(v)})`;
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
  // 真机码类→构造器映射（实测）：ARG_TYPE/ARG_VALUE→TypeError、
  // OUT_OF_RANGE→RangeError，其余 Error；只定构造器，码仍挂载（module-errors 点名 name）。
  let err;
  if (code === "ERR_INVALID_ARG_TYPE" || code === "ERR_INVALID_ARG_VALUE") err = new TypeError(message);
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
      __modErr("ERR_INVALID_ARG_TYPE", `The "linker" argument must be of type function. Received ${__recvArg(linker)}`);
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
    this.__reqCache = undefined;
    this.__modReqCache = undefined;
    // 顶层 await 静态标记（hasTopLevelAwait 面；async 图闭包在 link/instantiate 落定）。
    this.__tla = __vmCall(() => __wjs2_vm_mod_tla(code, identifier));
    this.__tlaClosure = false;
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
    // importModuleDynamically/initializeImportMeta 校验 + 动态导入注册
    //（merge-note #2：回调/meta/源码进 native 注册表，hook 投递用）。
    if (options.importModuleDynamically !== undefined && typeof options.importModuleDynamically !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options.importModuleDynamically" property must be of type function. Received ${__recvArg(options.importModuleDynamically)}`);
    }
    if (options.initializeImportMeta !== undefined && typeof options.initializeImportMeta !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options.initializeImportMeta" property must be of type function. Received ${__recvArg(options.initializeImportMeta)}`);
    }
    this.__id = __vmCall(() => __wjs2_vm_compile_mod(ctxId, identifier, code));
    __vmCall(() => __wjs2_vm_dyn_reg_mod(
      this.__id,
      ctxId,
      this,
      options.importModuleDynamically,
      options.initializeImportMeta,
      code,
    ));
    __vmModAutoRelease(this, this.__id);
    __vmHideFields(this);
  }
  get status() { this.__assertThis(); return this.__status; }
  get identifier() { this.__assertThis(); return this.__identifier; }
  get context() { this.__assertThis(); return this.__context; }
  get dependencySpecifiers() {
    this.__assertThis();
    if (this.__depCache === undefined) {
      this.__depCache = this.__requests().map((r) => r.specifier);
    }
    return this.__depCache;
  }
  // 静态请求表（native 全量 `{specifier,attributes,phase}`；首现序全键去重）。
  __requests() {
    if (this.__reqCache === undefined) {
      this.__reqCache = JSON.parse(__vmCall(() => __wjs2_vm_mod_deps(this.__id)));
    }
    return this.__reqCache;
  }
  get moduleRequests() {
    this.__assertThis();
    if (this.__modReqCache === undefined) {
      // 真机口径：空原型冻结对象（条目与 attributes 双冻；phase 源自引擎请求）。
      this.__modReqCache = this.__requests().map((r) => {
        const attributes = Object.create(null);
        Object.assign(attributes, r.attributes || {});
        const item = Object.create(null);
        item.specifier = r.specifier;
        item.attributes = Object.freeze(attributes);
        item.phase = r.phase;
        return Object.freeze(item);
      });
    }
    return this.__modReqCache;
  }
  // 顶层 await 探测（真机口径：仅本模块源码，构造期静态判定，不含依赖）。
  hasTopLevelAwait() {
    this.__assertThis();
    return this.__tla === true;
  }
  // 异步图探测（真机口径：本模块及传递依赖任一含 TLA；未实例化即抛）。
  hasAsyncGraph() {
    this.__assertThis();
    if (this.__status === "unlinked" || this.__status === "linking") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be instantiated");
    }
    return this.__tlaClosure === true;
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
  [__vsInspect.custom](depth, options) {
    return __vmInspectBody(this, "SourceTextModule", depth, options);
  }
  createCachedData() {
    if (this.__status === "evaluated" || this.__status === "errored") {
      __modErr("ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA", "Cannot create cached data for a module that has already been evaluated");
    }
    return Buffer.from(__vmModTag(this.__source), "utf8");
  }
  linkRequests(requests) {
    this.__assertThis();
    // 真机顺序：状态 → 数组形 → 长度 → 逐项（身份/同 ctx）→ 同键同模块。
    if (this.__status !== "unlinked") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be unlinked");
    }
    if (!Array.isArray(requests)) {
      __modErr("ERR_INVALID_ARG_TYPE", `The "modules" argument must be an instance of Array. Received ${__recvArg(requests)}`);
    }
    const reqs = this.__requests();
    if (requests.length !== reqs.length) {
      __modErr("ERR_MODULE_LINK_MISMATCH", `Expected ${reqs.length} modules, got ${requests.length}`);
    }
    const keyMod = new Map();
    for (let i = 0; i < reqs.length; i++) {
      const m = requests[i];
      if (!__isModuleLive(m) || !__vmModLive.has(m)) {
        __modErr("ERR_VM_MODULE_NOT_MODULE", "Provided module is not an instance of Module");
      }
      if (!(m instanceof SyntheticModule) && m.__ctxId !== this.__ctxId) {
        __modErr("ERR_VM_MODULE_DIFFERENT_CONTEXT", "Linked modules must use the same context");
      }
      const key = __vmReqKey(reqs[i]);
      if (keyMod.has(key)) {
        if (keyMod.get(key).mod !== m) {
          __modErr("ERR_MODULE_LINK_MISMATCH", `Module request 'ModuleCacheKey("${reqs[i].specifier}")' at index ${i} must be linked to the same module requested at index ${keyMod.get(key).index}`);
        }
      } else {
        keyMod.set(key, { mod: m, index: i });
      }
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
    // linkRequests 绑定 BFS：未链接且有请求的依赖必须自带 linkReqs，
    // 否则按真机点名该依赖（首请求 spec + identifier）。
    const seen = new Map();
    seen.set(this.__id, this);
    const modules = [this.__id];
    const edges = [];
    const queue = [this];
    while (queue.length > 0) {
      const ref = queue.shift();
      const reqs = ref.__requests();
      const bound = ref.__linkReqs;
      for (let i = 0; i < reqs.length; i++) {
        const m = bound[i];
        if (m.__status === "errored") {
          const err = new Error(`request for '${reqs[i].specifier}' resolved to an errored module`, { cause: m.__error });
          err.code = "ERR_VM_MODULE_LINK_FAILURE";
          throw err;
        }
        if (m instanceof SyntheticModule) {
          __modErr("ERR_VM_MODULE_LINK_FAILURE", `request for '${reqs[i].specifier}' resolved to a SyntheticModule which can not be linked by this engine (no named-synthetic record API; Gecko153 Modules.h only exposes CreateDefaultExportSyntheticModule)`);
        }
        edges.push([ref.__id, reqs[i].specifier, m.__id]);
        if (!seen.has(m.__id)) {
          seen.set(m.__id, m);
          // 已链接/已求值依赖沿用既有绑定，不遍历（与 link() 混用口径）；
          // 未链接且有请求者必须自带 linkReqs，否则按真机点名。
          if (m.__status === "unlinked") {
            const depReqs = m.__requests();
            if (depReqs.length > 0 && m.__linkReqs === undefined) {
              __modErr("ERR_VM_MODULE_LINK_FAILURE", `request for '${depReqs[0].specifier}' can not be resolved on module '${m.__identifier}' that is not linked`);
            }
            modules.push(m.__id);
            queue.push(m);
          }
        }
      }
    }
    __vmCall(() => __wjs2_vm_link(this.__id, JSON.stringify({ modules, edges })));
    for (const w of seen.values()) {
      if (w.__status === "unlinked" || w.__status === "linking") w.__status = "linked";
      // 连带落定的依赖同样结算异步图闭包（hasAsyncGraph 面；entry 与依赖同权）。
      if (w.__tlaClosure !== true) w.__tlaClosure = __vmTlaOf(w, new Set());
    }
    return undefined;
  }
  __linkValidateDep(dep, spec) {
    // linker 返回值校验（真机顺序：身份 → synthetic 墙 → 同 ctx → 已失败态）；
    // 失败一律记 entry errored（同步抛 linker 自身错误除外，V8 quirk）。
    if (!__isModuleLive(dep) || !__vmModLive.has(dep)) {
      __modErr("ERR_VM_MODULE_NOT_MODULE", "Provided module is not an instance of Module");
    }
    if (dep instanceof SyntheticModule) {
      __modErr("ERR_VM_MODULE_LINK_FAILURE", `request for '${spec}' resolved to a SyntheticModule which can not be linked by this engine (no named-synthetic record API; Gecko153 Modules.h only exposes CreateDefaultExportSyntheticModule)`);
    }
    if (dep.__ctxId !== this.__ctxId) {
      __modErr("ERR_VM_MODULE_DIFFERENT_CONTEXT", "Linked modules must use the same context");
    }
    if (dep.__status === "errored") {
      const err = new Error(`request for '${spec}' resolved to an errored module`, { cause: dep.__error });
      err.code = "ERR_VM_MODULE_LINK_FAILURE";
      throw err;
    }
    return dep;
  }
  async __doLink(linker) {
    if (this.__status !== "unlinked") {
      if (this.__status === "linked") {
        return __modStatusErr("ERR_VM_MODULE_ALREADY_LINKED", "Module has already been linked");
      }
      return __modStatusErr("ERR_VM_MODULE_STATUS", "Module status must be unlinked");
    }
    // linking 同步落位（真机 link 后 status 即 linking，实测）。
    this.__status = "linking";
    const reqs = this.__requests();
    if (reqs.length === 0) {
      return Promise.resolve().then(() => {
        try {
          __vmCall(() => __wjs2_vm_link(this.__id));
        } catch (e) {
          // 链接失败即 errored（真机口径）。
          this.__status = "errored";
          this.__error = e;
          throw e;
        }
        this.__status = "linked";
        // 零导入图即自身（TLA 闭包 = 自身标记）。
        this.__tlaClosure = this.__tla === true;
      });
    }
    // linker BFS（transitive；每 (referrer, 请求键) 调一次；异步 linker 顺序
    // await——shared-deps race 回归即靠此串行化；同步抛 linker 原错且状态不动）。
    const seen = new Map();
    seen.set(this.__id, this);
    const modules = [this.__id];
    const edges = [];
    const queue = [this];
    while (queue.length > 0) {
      const ref = queue.shift();
      const rreqs = ref.__requests();
      const called = new Set();
      for (const r of rreqs) {
        const key = __vmReqKey(r);
        if (called.has(key)) continue;
        called.add(key);
        let dep;
        try {
          dep = linker(r.specifier, ref, __vmLinkExtra(r));
        } catch (e) {
          throw e;
        }
        try {
          dep = await dep;
        } catch (e) {
          this.__status = "errored";
          this.__error = e;
          throw e;
        }
        try {
          this.__linkValidateDep(dep, r.specifier);
        } catch (e) {
          this.__status = "errored";
          this.__error = e;
          throw e;
        }
        edges.push([ref.__id, r.specifier, dep.__id]);
        if (!seen.has(dep.__id)) {
          seen.set(dep.__id, dep);
          // 已链接/已求值依赖不再为其请求调 linker（V8 复用既有绑定；
          // depth 链式口径），只记边供引擎接线。
          if (dep.__status !== "linked" && dep.__status !== "evaluated") {
            modules.push(dep.__id);
            queue.push(dep);
          }
        }
      }
    }
    try {
      __vmCall(() => __wjs2_vm_link(this.__id, JSON.stringify({ modules, edges })));
    } catch (e) {
      this.__status = "errored";
      this.__error = e;
      throw e;
    }
    for (const w of seen.values()) {
      if (w.__status === "unlinked" || w.__status === "linking") w.__status = "linked";
      // 连带落定的依赖同样结算异步图闭包（hasAsyncGraph 面；entry 与依赖同权）。
      if (w.__tlaClosure !== true) w.__tlaClosure = __vmTlaOf(w, new Set());
    }
    return undefined;
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
    // 沙箱同步（runIn* 同款快照语义，真机 contextify 为活绑定，此处以
    // syncIn/syncOut 对齐可观察面：求值前 sandbox→global，求值落定 global→sandbox。
    // 无显式 context（主域）即跳过；link 不执行代码不同步）。
    // 模块顶层声明落环境记录不在 globalThis 上，只有显式 global 写入才回写。
    const __syncSb = this.__context !== undefined ? this.__context : null;
    const __syncId = this.__context !== undefined ? this.__ctxId : null;
    if (__syncSb !== null) {
      try {
        __syncIn(__syncId, __syncSb);
      } catch (e) {
        this.__status = "errored";
        this.__error = e;
        return Promise.reject(e);
      }
    }
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
      if (__syncSb !== null) {
        __syncOut(__syncId, __syncSb);
      }
      return undefined;
    };
    const failed = (e) => {
      this.__status = "errored";
      this.__error = e;
      if (__syncSb !== null) {
        try {
          __syncOut(__syncId, __syncSb);
        } catch { /* 落定优先，同步失败不掩盖原错 */ }
      }
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
    // Synthetic 亦收 context（真机：同 SourceText 按 context 独立计数 + 身份透出）。
    let ctxKey = "";
    if (options.context !== undefined) {
      const ctxId = __validateCtx(options.context);
      ctxKey = String(ctxId);
      this.__context = options.context;
    } else {
      this.__context = undefined;
    }
    this.__ctxId = ctxKey === "" ? undefined : ctxKey;
    this.__identifier = __normStr(options.identifier, "identifier", `vm:module(${__vmModSeqFor(ctxKey)})`);
    this.__status = "linked";
    this.__error = null;
    // 命名空间活绑定：冻结外壳 + getter 直读 __exports（setExport 评估后仍生效，真机）。
    const ns = {};
    for (const n of seen) {
      Object.defineProperty(ns, n, { enumerable: true, configurable: true, get: () => this.__exports[n] });
    }
    this.__ns = Object.freeze(ns);
    // Synthetic 无 TLA（evaluateCallback 作普通函数跑）；包装 + 命名空间进
    // native 注册（回调返回 identity 比对用，merge-note #2）。
    this.__tla = false;
    this.__tlaClosure = false;
    __vmCall(() => __wjs2_vm_dyn_reg_ns(this, this.__ns));
    __vmHideFields(this);
  }
  get status() { this.__assertThis(); return this.__status; }
  get identifier() { this.__assertThis(); return this.__identifier; }
  get context() { this.__assertThis(); return this.__context; }
  get dependencySpecifiers() { this.__assertThis(); return undefined; }
  get namespace() {
    this.__assertThis();
    // 真机口径（lib/internal/vm/module.js）：synthetic 出生即 instantiated
    // （linked），命名空间自 linked 起可读（evaluated 前读到当前导出快照）。
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
    // 显式凭据（`this.__assertThis()` 在 null/裸对象上先炸属性访问/方法缺失，
    // 丢码；真机一律 ERR_INVALID_THIS，实测）。
    if (!__isModuleLive(this) || !__vmModLive.has(this)) {
      const err = new TypeError('Value of "this" must be of type Module');
      err.code = "ERR_INVALID_THIS";
      throw err;
    }
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
    // 求值步同步执行（真机：同步完成即同步落定）。回调返回的 thenable 不认领、
    // 不派发——用户侧挂载处理器即 handled；无人认领的拒绝交运行时既有
    // unhandled 机制（与 V8 微任务检查点语义一致；旧侧车 `throw e` 会衍生
    // 二次 unhandled，误报已 handled 情形，删）。
    this.__status = "evaluating";
    try {
      this.__cb(this.__nsHelper());
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
  [__vsInspect.custom](depth, options) {
    return __vmInspectBody(this, "SyntheticModule", depth, options);
  }
}

const __api = {
  Script, createContext, createScript, runInContext, runInNewContext,
  runInThisContext, isContext, compileFunction, measureMemory, constants,
  Module, SourceTextModule, SyntheticModule,
};
export default __api;
