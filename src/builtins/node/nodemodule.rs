//! `node:module`（Node `lib/module.js` 桥接；plan 9j 起解 vite `-r dev` 挡路）。
//!
//! 忠实面：`createRequire(filename)`（path｜file URL｜URL 对象 → 显式 base 的
//! require，`require.resolve` 同规则；file URL/绝对路径/裸名，非 file: URL 与
//! 相对路径抛 `ERR_INVALID_ARG_VALUE` 真文案）/`createRequireFromPath`/
//! `builtinModules`/`isBuiltin`/`Module` 类全表面（`load`/`_compile`/`wrap`/
//! `wrapper`/`_extensions`/`_cache`/`_pathCache`/`globalPaths`/ statics）/
//! `Module.prototype.require`（以自身 filename 为 base）/
//! `Module.registerHooks()` resolve 面（load 钩子保持未实现，见下）。
//!
//! 偏差（记档）：
//! - `require.cache`/`Module._cache` 同一视图：定制钩子加载走 JS 背书 Map，
//!   native 路径走 loader 注册表；`delete` 双清（native 失效经
//!   `__wjs2_require_cache_delete`）。native 条目不枚举（`Object.keys`
//!   仅见 JS 侧；node 全枚举，差集记档）。
//! - `require.extensions` 即 `Module._extensions`（同一对象；默认空，native
//!   默认引擎兜底；最长后缀匹配；node 默认带 .js/.json/.node 处理器——我方
//!   默认走 native，`delete extensions['.js']` 为无操作，差集记档）。
//! - CJS 包装沿用 curried 壳（与 native `__wjs2_cjs_compile` 同形；
//!   `Module.wrapper` 默认即该壳两片，改写即对后续编译生效）。
//!   行号：壳换行数不变即行号恒等（默认一片一换行，与现行一致）。
//! - `Module.registerHooks()` load 钩子与异步 `Module.register()` 抛
//!   `ERR_METHOD_NOT_IMPLEMENTED`（无消费方；resolve 链见 prelude）。
//! - `stripTypeScriptTypes` 不导出（TS 由 loader 原生处理）。
//! - 错误对象沿全局 `require` 口径（`MODULE_NOT_FOUND` 真码见下；参数校验用
//!   `ERR_INVALID_ARG_TYPE` 真码）。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/module.js (minimal bridge; see module docs for deviations).
import errors from 'node:internal/errors';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, resolve, sep, delimiter, isAbsolute } from 'node:path';
import { readFileSync } from 'node:fs';

const {
  codes: {
    ERR_INVALID_ARG_TYPE,
    ERR_INVALID_ARG_VALUE: { HideStackFramesError: ERR_INVALID_ARG_VALUE },
    ERR_METHOD_NOT_IMPLEMENTED,
  },
} = errors;

// ── CJS 包装（与 native `__wjs2_cjs_compile` 壳字节恒等；行号恒等） ──
// 默认两片：头含单换行（用户代码物理行号 = 报错行号，现行口径）。
// `Module.wrapper` 改写即对后续编译生效（native 编译期现读）。
// 单大括号：用户代码 `'use strict'` 须落在最内箭头函数体顶部才成指令
// （双大括号即内层块，指令退化为普通串语句，严格模式丢失，bind-store 回归）。
const kCjsShellHead = '((exports) => (require) => (module) => (__filename) => (__dirname) => {\n';
const kCjsShellTail = '\n})';

// ── Module 类活状态（native 编译期现读；改写即对后续编译生效） ──
// wrapper 默认即 native curried 壳两片（字节恒等，行号恒等）。
let cjsWrapperLive = [kCjsShellHead, kCjsShellTail];
// 扩展名表（空表起步；默认引擎走 native；`require.extensions` 即此对象）。
let cjsExtensionsLive = { __proto__: null };
// 路径缓存（可整体重赋；解析器不 consult，纯装饰口径）。
let cjsPathCacheLive = { __proto__: null };
// 全局路径（`_initPaths` 重算；`NODE_PATH` 口径）。
const cjsGlobalPaths = [];

// 活对象进 state（native 编译/解析期现读；改写即对后续生效）。
__wjs2_cjs_live(cjsWrapperLive, cjsExtensionsLive);

// 定制扩展名最长后缀匹配（node `findLongestRegisteredExtension` 口径逐字：
// 基名内逐点左到右（首点跳过，即 dotfile 不认全名），首个已注册即最长；
// 无命中默认 '.js'）。
function longestRegisteredExtension(filename, extensions) {
  const slash = Math.max(filename.lastIndexOf('/'), filename.lastIndexOf('\\'));
  const name = slash === -1 ? filename : filename.slice(slash + 1);
  let startIndex = 0;
  for (;;) {
    const index = name.indexOf('.', startIndex);
    if (index === -1) break;
    startIndex = index + 1;
    if (index === 0) continue;
    const currentExtension = name.slice(index);
    if (Object.prototype.hasOwnProperty.call(extensions, currentExtension)) {
      return currentExtension;
    }
  }
  return '.js';
}

function normalizeBase(filename) {
  if (filename instanceof URL) filename = filename.href;
  if (typeof filename !== 'string') {
    throw new ERR_INVALID_ARG_TYPE('filename', ['string', 'URL'], filename);
  }
  return filename;
}

function makeRequire(base) {
  function require(id) {
    // node 口径：先校验类型（validateString + 空串），再转串（真机实测：
    // 数字/布尔/对象bulk先抛类型错，空串抛值错；String(id) 会抹掉类型信息故先判）。
    if (typeof id !== 'string') {
      throw new ERR_INVALID_ARG_TYPE('id', 'string', id);
    }
    if (id === '') {
      throw new ERR_INVALID_ARG_VALUE('id', id, 'must be a non-empty string');
    }
    const spec = id;
    const hooks = require.extensions;
    if (hooks && typeof hooks === "object" && !Array.isArray(hooks)) {
      let resolved = null;
      try { resolved = __wjs2_require_resolve_from(base, spec); } catch { resolved = null; }
      if (resolved !== null && resolved.startsWith("file://")) {
        if (Object.prototype.hasOwnProperty.call(require.cache, resolved)) {
          return require.cache[resolved].exports;
        }
        const fsPath = fileURLToPath(resolved);
        const hook = longestRegisteredExtension(fsPath, hooks);
        if (hook !== null && typeof hooks[hook] === "function") {
          const mod = new Module(fsPath, require.main ?? null);
          mod.filename = fsPath;
          mod.paths = Module._nodeModulePaths(dirname(fsPath));
          require.cache[resolved] = mod;
          hooks[hook](mod, fsPath);
          mod.loaded = true;
          return mod.exports;
        }
      }
    }
    return __wjs2_require_from(base, spec);
  }
  require.resolve = function resolve(id) {
    return __wjs2_require_resolve_from(base, String(id));
  };
  require.cache = cjsCacheLive;
  require.extensions = cjsExtensionsLive;
  require.main = globalThis.require?.main;
  return require;
}

export function createRequire(filename) {
  // node 口径：仅 file: URL 串/对象、绝对路径串；其余 ERR_INVALID_ARG_VALUE
  //（真文案逐字；URL 对象先判协议，normalizeBase 会抹掉类型信息故先判原文）。
  if (filename instanceof URL) {
    if (filename.protocol !== 'file:') {
      const err = new ERR_INVALID_ARG_VALUE('filename', 'file URL object, file URL string, or absolute path string', filename);
      err.message = `The argument 'filename' must be a file URL object, file URL string, or absolute path string. Received ${inspectValue(filename)}`;
      throw err;
    }
    return makeRequire(filename.href);
  }
  if (typeof filename === 'string') {
    if (filename.startsWith('file:')) return makeRequire(filename);
    if (isAbsolute(filename)) return makeRequire(filename);
  }
  const err = new ERR_INVALID_ARG_VALUE('filename', 'file URL object, file URL string, or absolute path string', filename);
  err.message = `The argument 'filename' must be a file URL object, file URL string, or absolute path string. Received ${inspectValue(filename)}`;
  throw err;
}
function inspectValue(v) {
  if (typeof v === 'string') return `'${v}'`;
  if (v !== null && typeof v === 'object' && Object.getPrototypeOf(v) === Object.prototype) {
    try { return JSON.stringify(v); } catch { /* fall through */ }
  }
  try { return String(v); } catch { return String(v); }
}

export function createRequireFromPath(path) {
  if (typeof path !== 'string') {
    throw new ERR_INVALID_ARG_TYPE('path', ['string'], path);
  }
  return makeRequire(path);
}

export const builtinModules = JSON.parse(__wjs2_builtin_modules());

export function isBuiltin(moduleName) {
  return builtinModules.includes(String(moduleName));
}

export class Module {
  constructor(id = '', parent) {
    this.id = String(id);
    this.filename = String(id);
    this.paths = [];
    this.exports = {};
    // `parent` 走原型访问器（DEP0144 门控，见下）；此处只写背书字段。
    this.__wjs2_parent = parent ?? null;
    this.loaded = false;
    this.children = [];
  }
  require(id) {
    // node 口径：同 createRequire 实例（先校验再转串；见 makeRequire）。
    if (typeof id !== 'string') {
      throw new ERR_INVALID_ARG_TYPE('id', 'string', id);
    }
    if (id === '') {
      throw new ERR_INVALID_ARG_VALUE('id', id, 'must be a non-empty string');
    }
    return __wjs2_require_from(this.filename, id);
  }
  // CJS 加载（node 口径：断言未加载 → filename/paths 回填 → 最长后缀分派 →
  // handler(module, filename) → loaded = true）。
  load(filename) {
    if (this.loaded) {
      throw new Error('Module.load: module already loaded');
    }
    this.filename ??= filename;
    this.paths ??= Module._nodeModulePaths(dirname(filename));
    const ext = longestRegisteredExtension(filename, Module._extensions);
    const custom = Object.prototype.hasOwnProperty.call(Module._extensions, ext)
      ? Module._extensions[ext]
      : undefined;
    if (typeof custom === 'function') {
      custom(this, filename);
    } else if (String(filename).endsWith('.json') || ext === '.json') {
      const text = readFileSync(filename, 'utf8');
      this.exports = JSON.parse(text);
    } else if (typeof custom !== 'undefined') {
      throw new TypeError(`Module.load: no handler for '${filename}'`);
    } else {
      const text = readFileSync(filename, 'utf8');
      this._compile(text, filename);
    }
    this.loaded = true;
  }
  _compile(content, filename) {
    __wjs2_cjs_compile(
      this,
      String(content),
      filename != null ? String(filename) : String(this.filename),
    );
  }
  // CJS 包装（当前 wrapper 模板；改写即对后续编译生效）。
  static wrap(script) {
    return `${Module.wrapper[0]}${script}${Module.wrapper[1]}`;
  }
  static get wrapper() {
    return cjsWrapperLive;
  }
  static set wrapper(v) {
    cjsWrapperLive = v;
    __wjs2_cjs_live(cjsWrapperLive, cjsExtensionsLive);
  }
  static get _extensions() {
    return cjsExtensionsLive;
  }
  static set _extensions(v) {
    cjsExtensionsLive = v;
    __wjs2_cjs_live(cjsWrapperLive, v);
  }
  static get _cache() {
    return cjsCacheLive;
  }
  static get _pathCache() {
    return cjsPathCacheLive;
  }
  static set _pathCache(v) {
    cjsPathCacheLive = v;
  }
  static get globalPaths() {
    return cjsGlobalPaths;
  }
  // 目录类型（node 口径：0 文件，1 目录，负数其它（含不存在）；抛错即负数）。
  static _stat(filename) {
    try {
      const st = JSON.parse(__wjs2_fs_stat(String(filename), true));
      if (st && st.isDirectory === true) return 1;
      if (st && st.isFile === true) return 0;
      return -1;
    } catch {
      return -1;
    }
  }
  // node_modules 查找表（node 口径：`path.resolve(from)` 后逐级向上；
  // 不做存在性判定，纯字符串走（套件含不存在路径，`_stat` 分支即错，记档）。
  static _nodeModulePaths(from) {
    const dir = resolve(String(from));
    const paths = [];
    const parts = dir.split(sep);
    for (let i = parts.length; i >= 0; i--) {
      const cur = i === 0 ? sep : parts.slice(0, i).join(sep) || sep;
      if (cur === sep && sep !== '/') continue;
      // node 口径：祖先本身已是 node_modules 即跳过（不套娃）。
      if (cur !== sep && cur.endsWith(sep + 'node_modules')) continue;
      paths.push(cur + (cur.endsWith(sep) ? '' : sep) + 'node_modules');
      if (cur === sep) break;
    }
    return paths;
  }
  // 解析查找表（node 口径逐字：内建返 null；非相对走 parent.paths+全局；
  // 相对且无可用父走 ['.']；有父走其目录）。
  static _resolveLookupPaths(request, parent) {
    if (typeof request === 'string' && builtinModules.includes(request)) {
      return null;
    }
    const c0 = typeof request === 'string' && request.length > 0 ? request[0] : '';
    const c1 = typeof request === 'string' && request.length > 1 ? request[1] : '';
    const isWin = typeof process !== 'undefined' && process.platform === 'win32';
    const isNonRelative = c0 !== '.' ||
      (request.length > 1 && c1 !== '.' && c1 !== '/' && (!isWin || c1 !== '\\'));
    if (isNonRelative) {
      let paths;
      if (parent && Array.isArray(parent.paths) && parent.paths.length > 0) {
        paths = cjsGlobalPaths.slice();
        for (let i = parent.paths.length - 1; i >= 0; i--) {
          paths.unshift(parent.paths[i]);
        }
      } else {
        paths = cjsGlobalPaths;
      }
      return paths.length > 0 ? paths : null;
    }
    if (!parent || !parent.id || !parent.filename) {
      return ['.'];
    }
    return [dirname(resolve(parent.filename))];
  }
  // 全局路径初始化（node 口径逐字：`$PREFIX/lib/node`（execPath 上两级）+
  // `$HOME/.node_modules` + `$HOME/.node_libraries` + `NODE_PATH` 切分（空项
  // 丢弃，NODE_PATH 段最优先）。
  static _initPaths() {
    const homeDir = typeof process !== 'undefined' && process.env
      ? process.env.HOME || ''
      : '';
    const nodePath = typeof process !== 'undefined' && process.env
      ? process.env.NODE_PATH || ''
      : '';
    const prefixDir = resolve(
      typeof process !== 'undefined' ? process.execPath : '/',
      '..',
      '..',
    );
    const paths = [resolve(prefixDir, 'lib', 'node')];
    if (homeDir !== '') {
      paths.unshift(resolve(homeDir, '.node_libraries'));
      paths.unshift(resolve(homeDir, '.node_modules'));
    }
    if (nodePath !== '') {
      const segs = nodePath.split(delimiter).filter((p) => p !== '');
      for (let i = segs.length - 1; i >= 0; i--) {
        paths.unshift(segs[i]);
      }
    }
    cjsGlobalPaths.length = 0;
    for (const p of paths) cjsGlobalPaths.push(p);
  }
  static createRequire(filename) {
    return createRequire(filename);
  }
  static createRequireFromPath(path) {
    return createRequireFromPath(path);
  }
  static isBuiltin(moduleName) {
    return isBuiltin(moduleName);
  }
  static get builtinModules() {
    return builtinModules;
  }
  static syncBuiltinESMExports() {
    return undefined;
  }
  static register() {
    throw new ERR_METHOD_NOT_IMPLEMENTED('Module.register');
  }
  // Node 22.15+ 的同步 ESM 定制钩子（进程内、当前线程；vite config 打包链的
  // import.meta.resolve 走此面）。resolve 钩子链在 __wjs2_module_resolve_chain
  // 消费（后注册者先跑，next = 链上已见部分，默认底座 = 本仓解析器）。
  // load 钩子未实现（无消费方，fail fast）；register()（异步 worker 形态）
  // 有 registerHooks 后 vite 不再到达，保持未实现。
  static registerHooks(specifiers) {
    if (typeof specifiers !== 'object' || specifiers === null) {
      throw new ERR_INVALID_ARG_TYPE('specifiers', ['object'], specifiers);
    }
    for (const key of ['resolve', 'load']) {
      const hook = specifiers[key];
      if (hook !== undefined && typeof hook !== 'function') {
        throw new ERR_INVALID_ARG_TYPE(`specifiers.${key}`, ['function', 'undefined'], hook);
      }
    }
    if (specifiers.load !== undefined) {
      throw new ERR_METHOD_NOT_IMPLEMENTED('Module.registerHooks: load hook');
    }
    const entry = { resolve: specifiers.resolve };
    globalThis.__wjs2_module_hooks.push(entry);
    return [
      function deregister() {
        const hooks = globalThis.__wjs2_module_hooks;
        const i = hooks.indexOf(entry);
        if (i !== -1) hooks.splice(i, 1);
      },
    ];
  }
}

// require.cache 背书视图（纯 native 背书：定制钩子 mod 与 plain 加载统一进
// native 注册表；JS 侧无影子表，`delete` 即 native 移除）。
// 键形：URL 串或绝对路径（路径转 file: URL 后查；非法键形走默认分支）。
function cjsCacheKey(p) {
  const s = String(p);
  if (s.startsWith('file:')) return s;
  try {
    return String(pathToFileURL(s));
  } catch {
    return s;
  }
}
const cjsCacheLive = new Proxy({ __proto__: null }, {
  get(t, p, r) {
    if (typeof p === 'string') {
      try {
        const m = __wjs2_require_cache_get(cjsCacheKey(p));
        if (m !== undefined) return m;
      } catch { /* native 未命中即回默认 */ }
    }
    return Reflect.get(t, p, r);
  },
  set(t, p, v, r) {
    if (typeof p === 'string' && v !== null &&
        (typeof v === 'object' || typeof v === 'function')) {
      try {
        __wjs2_require_cache_set(cjsCacheKey(p), v);
        return true;
      } catch { /* 回默认分支 */ }
    }
    return Reflect.set(t, p, v, r);
  },
  has(t, p) {
    if (typeof p === 'string') {
      try {
        if (__wjs2_require_cache_has(cjsCacheKey(p))) return true;
      } catch { /* 回默认分支 */ }
    }
    return Reflect.has(t, p);
  },
  // `hasOwnProperty` 经此陷阱（node `require.cache` 为普通对象，hasOwnProperty
  // 见 native 条目；`Object.keys` 仍只见 JS 侧——non-enumerable，差集记档）。
  getOwnPropertyDescriptor(t, p) {
    if (typeof p === 'string') {
      try {
        const m = __wjs2_require_cache_get(cjsCacheKey(p));
        if (m !== undefined) {
          return { configurable: true, enumerable: false, value: m, writable: true };
        }
      } catch { /* 回默认分支 */ }
    }
    return Reflect.getOwnPropertyDescriptor(t, p);
  },
  deleteProperty(t, p) {
    if (typeof p === 'string') {
      try {
        __wjs2_require_cache_delete(cjsCacheKey(p));
      } catch { /* 键形非法即只清默认分支 */ }
      return true;
    }
    return Reflect.deleteProperty(t, p);
  },
});

// 全局 require 共享视图（prelude 懒取；node:module 未加载前为 undefined，
// 与现行一致；加载后即同一对象）。
globalThis.__wjs2_cjs_cache = cjsCacheLive;
globalThis.__wjs2_module_extensions = cjsExtensionsLive;

// 循环依赖警告代理（node `CircularRequirePrototypeWarningProxy` 口径：
// 仅普通对象（原型即 Object.prototype、非代理、无 __esModule）挂警告原型；
// 代理/异常体一律跳过（不触发其 trap，真机口径）；加载完成后摘除恢复。
const cjsCircularWarnProxy = new Proxy({}, {
  __proto__: null,
  get(target, prop) {
    if (prop in target || prop === '__esModule') return target[prop];
    try {
      process.emitWarning(
        `Accessing non-existent property '${String(prop)}' of module exports inside circular dependency`,
      );
    } catch { /* 警告通道失败不掩盖原访问 */ }
    return undefined;
  },
  getOwnPropertyDescriptor(target, prop) {
    if (Object.prototype.hasOwnProperty.call(target, prop) || prop === '__esModule') {
      return Object.getOwnPropertyDescriptor(target, prop);
    }
    try {
      process.emitWarning(
        `Accessing non-existent property '${String(prop)}' of module exports inside circular dependency`,
      );
    } catch { /* 同上 */ }
    return undefined;
  },
});
function __wjs2_cjs_circular(exports) {
  try {
    if (!exports || (typeof exports !== 'object' && typeof exports !== 'function')) return exports;
    if (Object.getPrototypeOf(exports) !== Object.prototype) return exports;
    if (exports.__esModule) return exports;
    Object.setPrototypeOf(exports, cjsCircularWarnProxy);
  } catch { /* 代理 trap 抛错即跳过（真机 isProxy 口径；我方 isProxy 恒 false 见 4.x） */ }
  return exports;
}
function __wjs2_cjs_circular_done(exports) {
  try {
    if (exports && Object.getPrototypeOf(exports) === cjsCircularWarnProxy) {
      Object.setPrototypeOf(exports, Object.prototype);
    }
  } catch { /* 同上 */ }
  return exports;
}
globalThis.__wjs2_cjs_circular = __wjs2_cjs_circular;
globalThis.__wjs2_cjs_circular_done = __wjs2_cjs_circular_done;

// `Module.prototype.parent` 访问器（node 口径：DEP0144 门控，背书字段直存）。
Object.defineProperty(Module.prototype, 'parent', {
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

Module.Module = Module;

// 启动即初始化全局路径（node initializeCJS 同款；`_resolveLookupPaths` 无父
// 时直接读此表）。
Module._initPaths();

export default Module;
"#;
