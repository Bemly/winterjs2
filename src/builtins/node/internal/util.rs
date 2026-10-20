//! `node:internal/util`（Node `lib/internal/util.js` 按需移植，MIT）。
//!
//! 移植 9a 依赖的小件：`kEmptyObject`/`spliceOne`/`normalizeEncoding`(+slowCases
//! 逐字)/`setOwnProperty`/`kObjLen`…（仅用到的）。其余（deprecate 侧/parseArgs/
//! lazy DOMGlobal collectors 等）随各模块按需后续追加。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node:internal/util (pieces needed by Phase 9a modules).
import { validateUint32 } from 'node:internal/validators';
import { getOptionValue } from 'node:internal/options';
function normalizeEncoding(enc) {
  if (enc == null || enc === 'utf8' || enc === 'utf-8') return 'utf8';
  return slowCases(enc);
}

function slowCases(enc) {
  switch (enc.length) {
    case 4:
      if (enc === 'UTF8') return 'utf8';
      if (enc === 'ucs2' || enc === 'UCS2') return 'utf16le';
      enc = enc.toLowerCase();
      if (enc === 'utf8') return 'utf8';
      if (enc === 'ucs2') return 'utf16le';
      break;
    case 3:
      if (enc === 'hex' || enc === 'HEX' || enc.toLowerCase() === 'hex')
        return 'hex';
      break;
    case 5:
      if (enc === 'ascii') return 'ascii';
      if (enc === 'ucs-2') return 'utf16le';
      if (enc === 'UTF-8') return 'utf8';
      if (enc === 'ASCII') return 'ascii';
      if (enc === 'UCS-2') return 'utf16le';
      enc = enc.toLowerCase();
      if (enc === 'utf-8') return 'utf8';
      if (enc === 'ascii') return 'ascii';
      if (enc === 'ucs-2') return 'utf16le';
      break;
    case 6:
      if (enc === 'base64') return 'base64';
      if (enc === 'latin1' || enc === 'binary') return 'latin1';
      if (enc === 'BASE64') return 'base64';
      if (enc === 'LATIN1' || enc === 'BINARY') return 'latin1';
      enc = enc.toLowerCase();
      if (enc === 'base64') return 'base64';
      if (enc === 'latin1' || enc === 'binary') return 'latin1';
      break;
    case 7:
      if (enc === 'utf16le' || enc === 'UTF16LE' || enc.toLowerCase() === 'utf16le')
        return 'utf16le';
      break;
    case 8:
      if (enc === 'utf-16le' || enc === 'UTF-16LE' || enc.toLowerCase() === 'utf-16le')
        return 'utf16le';
      break;
    case 9:
      if (enc === 'base64url' || enc === 'BASE64URL' || enc.toLowerCase() === 'base64url')
        return 'base64url';
      break;
    default:
      if (enc === '') return 'utf8';
  }
}

// As of V8 6.6, depending on the size of the array, this is anywhere
// between 1.5-10x faster than the two-arg version of Array#splice()
function spliceOne(list, index) {
  for (; index + 1 < list.length; index++)
    list[index] = list[index + 1];
  list.pop();
}

// Mimics obj[key] = value but ignoring potential prototype inheritance.
function setOwnProperty(obj, key, value) {
  return Object.defineProperty(obj, key, {
    __proto__: null, value, writable: true, enumerable: true, configurable: true,
  });
}

const kEmptyObject = Object.freeze({ __proto__: null });


// ── Phase 9b：streams 系所需（once/sleep/assignFunctionName 逐字，promisify 面）──
// Node internal/util 的 promisify 在 streams 域只取 { custom }（符号与 node:util 同注册表）。
const promisify = { custom: Symbol.for('nodejs.util.promisify.custom') };

function once(callback, { preserveReturnValue = false } = {}) {
  let called = false;
  let returnValue;
  return function(...args) {
    if (called) return returnValue;
    called = true;
    const result = Reflect.apply(callback, this, args);
    returnValue = preserveReturnValue ? result : undefined;
    return result;
  };
}

// Node _sleep 的阻塞实现（Atomics.wait）；运行时主线程禁用，恒 no-op（记档）。
// 校验先行（真机口径：validateUint32，非数 ARG_TYPE、非整数/越界 OOR）。
function sleep(msec) {
  validateUint32(msec, 'msec');
  // Sync sleep unavailable on main thread in winterjs2; no-op (deviation).
}

function assignFunctionName(name, fn, descriptor = {}) {
  if (typeof name !== 'string') {
    const symbolDescription = name.description;
    if (symbolDescription === undefined) {
      throw new Error('Attempted to name function after descriptionless Symbol');
    }
    name = `[${symbolDescription}]`;
  }
  return Object.defineProperty(fn, 'name', {
    __proto__: null,
    writable: false,
    enumerable: false,
    configurable: true,
    ...Object.getOwnPropertyDescriptor(fn, 'name'),
    ...descriptor,
    value: name,
  });
}

// R2-iter（node 原文口径）：错误判定 + 实验警告 + 懒 DOMException。
// isError 取 instanceof 近似（跨 realm 原生错记档，见 4.57）；
// lazyDOMException 直构全局 DOMException（无 messaging 绑定，记档）。
function isError(e) {
  return e instanceof Error;
}
const __experimentalWarned = new Set();
function emitExperimentalWarning(feature, messagePrefix, code, ctor) {
  if (__experimentalWarned.has(feature)) return;
  __experimentalWarned.add(feature);
  let msg = `${feature} is an experimental feature and might change at any time`;
  if (messagePrefix) {
    msg = messagePrefix + msg;
  }
  process.emitWarning(msg, 'ExperimentalWarning', code, ctor);
}
const lazyDOMException = (message, name) => new DOMException(String(message), String(name));

// 内部废弃包装（node internal/util.pendingDeprecate 口径）：仅
// `--pending-deprecation`（且非 `--no-deprecation`）下首调告警；length 保形。
const __pendingCodesWarned = new Set();
function __isPendingDeprecation() {
  try {
    return !!getOptionValue('--pending-deprecation') &&
      !getOptionValue('--no-deprecation');
  } catch { return false; }
}
function pendingDeprecate(fn, msg, code) {
  let warned = false;
  function deprecated(...args) {
    if (!warned && __isPendingDeprecation()) {
      warned = true;
      if (code !== undefined) {
        if (!__pendingCodesWarned.has(code)) {
          __pendingCodesWarned.add(code);
          process.emitWarning(msg, 'DeprecationWarning', code, deprecated);
        }
      } else {
        process.emitWarning(msg, 'DeprecationWarning', deprecated);
      }
    }
    return Reflect.apply(fn, this, args);
  }
  try {
    Object.defineProperty(deprecated, 'length', {
      ...Object.getOwnPropertyDescriptor(fn, 'length'),
    });
  } catch {}
  return deprecated;
}

export { normalizeEncoding, spliceOne, setOwnProperty, kEmptyObject, promisify, once, sleep, assignFunctionName, isError, emitExperimentalWarning, lazyDOMException, pendingDeprecate };
export default { normalizeEncoding, spliceOne, setOwnProperty, kEmptyObject, promisify, once, sleep, assignFunctionName, isError, emitExperimentalWarning, lazyDOMException, pendingDeprecate };
"#;
