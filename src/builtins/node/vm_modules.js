// ── 9i-1 模块系（真机口径：node --experimental-vm-modules 实测，见模块头注）──
// SourceTextModule：Rust 侧编译/link/evaluate（零导入全链；带导入 link 即
// ERR_VM_MODULE_LINK_FAILURE，linker 切片后续）；SyntheticModule 纯 JS。

let __vmModSeq = 0;

function __modErr(code, message) {
  const err = new Error(message);
  err.code = code;
  throw err;
}

export class Module {
  link(linker) {
    if (typeof linker !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "linker" argument must be of type function. Received ${linker === null ? "null" : typeof linker}`);
    }
    return this.__doLink(linker);
  }
  evaluate() {
    return this.__doEvaluate();
  }
}

export class SourceTextModule extends Module {
  constructor(code, options = {}) {
    super();
    if (typeof code !== "string") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "code" argument must be of type string. Received type ${typeof code}`);
    }
    if (options === null || (typeof options !== "object" && typeof options !== "function")) {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options" argument must be of type object. Received ${options === null ? "null" : typeof options}`);
    }
    const identifier = __normStr(options.identifier, "identifier", `vm:module(${__vmModSeq++})`);
    let ctxId;
    if (options.context !== undefined) {
      ctxId = __validateCtx(options.context);
      this.__context = options.context;
    } else {
      ctxId = __vmCall(() => __wjs2_vm_create());
      const holder = {};
      __vmStdKeys(holder).id = ctxId;
      __vmSnapshot(ctxId, holder);
      __vmAutoRelease(holder, ctxId);
      this.__context = undefined;
    }
    this.__ctxId = ctxId;
    this.__identifier = identifier;
    this.__status = "unlinked";
    this.__error = null;
    this.__ns = undefined;
    // importModuleDynamically/initializeImportMeta 接受忽略（v1 未接线，记档）。
    this.__id = __vmCall(() => __wjs2_vm_compile_mod(ctxId, identifier, code));
    __vmModAutoRelease(this, this.__id);
  }
  get status() { return this.__status; }
  get identifier() { return this.__identifier; }
  get context() { return this.__context; }
  get dependencySpecifiers() {
    return JSON.parse(__vmCall(() => __wjs2_vm_mod_deps(this.__id)));
  }
  get namespace() {
    if (this.__status !== "evaluated") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be evaluated");
    }
    return this.__ns;
  }
  get error() {
    if (this.__status !== "errored") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be errored");
    }
    return this.__error;
  }
  __doLink(linker) {
    if (this.__status !== "unlinked") {
      return Promise.reject(Object.assign(new Error("Module status must be unlinked"), { code: "ERR_VM_MODULE_STATUS" }));
    }
    void linker;
    return Promise.resolve().then(() => {
      __vmCall(() => __wjs2_vm_link(this.__id));
      this.__status = "linked";
    });
  }
  __doEvaluate() {
    if (this.__status !== "linked" && this.__status !== "evaluated" && this.__status !== "errored") {
      return Promise.reject(Object.assign(new Error("Module status must be linked"), { code: "ERR_VM_MODULE_STATUS" }));
    }
    if (this.__status === "evaluated") return Promise.resolve(undefined);
    return Promise.resolve().then(() => {
      // native 同步抛错（参数/link 前置）即失败落定；完成值可能是跨域 promise
      // （§4.57：`instanceof Promise` 跨 compartment 恒 false，必须按 thenable 认领，
      // 否则落定被丢弃、报错变 unhandled rejection）。
      const r = __vmCall(() => __wjs2_vm_evaluate(this.__id));
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
        return r.then(done, failed);
      }
      try {
        return done();
      } catch (e) {
        return failed(e);
      }
    });
  }
}

export class SyntheticModule extends Module {
  constructor(exportNames, evaluateCallback, options = {}) {
    super();
    if (!Array.isArray(exportNames)) {
      __modErr("ERR_INVALID_ARG_TYPE", `The "exportNames" argument must be of type array. Received type ${typeof exportNames}`);
    }
    if (typeof evaluateCallback !== "function") {
      __modErr("ERR_INVALID_ARG_TYPE", `The "evaluateCallback" argument must be of type function. Received type ${typeof evaluateCallback}`);
    }
    if (options === null || (typeof options !== "object" && typeof options !== "function")) {
      __modErr("ERR_INVALID_ARG_TYPE", `The "options" argument must be of type object. Received ${options === null ? "null" : typeof options}`);
    }
    this.__exports = {};
    for (const n of exportNames) this.__exports[String(n)] = undefined;
    this.__cb = evaluateCallback;
    this.__identifier = __normStr(options.identifier, "identifier", `vm:module(${__vmModSeq++})`);
    this.__status = "linked";
    this.__error = null;
    this.__ns = undefined;
  }
  get status() { return this.__status; }
  get identifier() { return this.__identifier; }
  get dependencySpecifiers() { return undefined; }
  get namespace() {
    if (this.__status !== "evaluated") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be evaluated");
    }
    return this.__ns;
  }
  get error() {
    if (this.__status !== "errored") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must be errored");
    }
    return this.__error;
  }
  setExport(name, value) {
    if (this.__status === "evaluated") {
      __modErr("ERR_VM_MODULE_STATUS", "Module status must not be evaluated");
    }
    this.__exports[String(name)] = value;
  }
  // Synthetic 的 linker 可选（真机口径：无参 link 即过）。
  link(linker) {
    if (linker !== undefined) return super.link(linker);
    return this.__doLink(undefined);
  }
  __doLink(linker) {
    void linker;
    // Synthetic 出生即 linked；重复 link 照真机保持 linked（无操作成功）。
    return Promise.resolve(undefined);
  }
  __doEvaluate() {
    if (this.__status === "evaluated") return Promise.resolve(undefined);
    return Promise.resolve().then(() => {
      try {
        this.__cb(this.__exports);
      } catch (e) {
        this.__status = "errored";
        this.__error = e;
        throw e;
      }
      this.__status = "evaluated";
      this.__ns = Object.freeze({ ...this.__exports });
      return undefined;
    });
  }
}

const __api = {
  Script, createContext, createScript, runInContext, runInNewContext,
  runInThisContext, isContext, compileFunction, measureMemory, constants,
  Module, SourceTextModule, SyntheticModule,
};
export default __api;
