//! Buffer 全局出口（bufApi/decode/encode/transfer/pool）（prelude 分域；拼接顺序见 mod.rs）。
pub const BUFFER_API_JS: &str = r#"
globalThis.__wjs2_bufApi = {
  get INSPECT_MAX_BYTES() { return INSPECT_MAX_BYTES; },
  set INSPECT_MAX_BYTES(v) {
    __wjs2_bufValidateNumber(v, 'INSPECT_MAX_BYTES', 0);
    INSPECT_MAX_BYTES = v;
  },
  kMaxLength,
  kStringMaxLength,
  isUtf8(input) {
    if ((ArrayBuffer.isView(input) && !(input instanceof DataView)) || __wjs2_bufIsAnyAB(input)) {
      const u8 = __wjs2_bufAsU8(input) ?? new Uint8Array(0);
      try {
        new TextDecoder('utf-8', { fatal: true }).decode(u8);
        return true;
      } catch {
        return false;
      }
    }
    throw __wjs2_bufArgTypeErr('input', ['ArrayBuffer', 'Buffer', 'TypedArray'], input);
  },
  isAscii(input) {
    if ((ArrayBuffer.isView(input) && !(input instanceof DataView)) || __wjs2_bufIsAnyAB(input)) {
      const u8 = __wjs2_bufAsU8(input) ?? new Uint8Array(0);
      for (let i = 0; i < u8.length; i++) {
        if (u8[i] > 0x7f) return false;
      }
      return true;
    }
    throw __wjs2_bufArgTypeErr('input', ['ArrayBuffer', 'Buffer', 'TypedArray'], input);
  },
  btoa(input) {
    if (arguments.length === 0) throw __wjs2_bufMissingArgsErr('input');
    return globalThis.btoa(`${input}`);
  },
  atob(input) {
    if (arguments.length === 0) throw __wjs2_bufMissingArgsErr('input');
    return globalThis.atob(`${input}`);
  },
  transcode(source, fromEncoding, toEncoding) {
    if (!__wjs2_bufIsU8(source)) {
      throw __wjs2_bufArgTypeErr('source', ['Buffer', 'Uint8Array'], source);
    }
    if (source.length === 0) return new __wjs2_bufFastBuffer();
    fromEncoding = __wjs2_bufNormalizeEncoding(fromEncoding) || fromEncoding;
    toEncoding = __wjs2_bufNormalizeEncoding(toEncoding) || toEncoding;
    const fromOps = __wjs2_bufGetEncodingOps(fromEncoding);
    const toOps = __wjs2_bufGetEncodingOps(toEncoding);
    if (fromOps === undefined || toOps === undefined) {
      const e = new RangeError(`Unable to transcode Buffer [U_UNKNOWN_ENCODING]`);
      e.code = 'ERR_UNKNOWN_ENCODING';
      e.errno = -1;
      throw e;
    }
    const decoded = fromOps.slice(source, 0, source.length);
    return __wjs2_bufFromStringFast(decoded, toOps);
  },
};

// Uint8Array 构造失败文案桥（V8 "Invalid typed array length: N" 口径；
// SM 抛自有文案，套件按 V8 插值断言；newTarget 必须透传，否则 TypedArray
// 子类化（`class X extends Uint8Array`）全灭为基类原型——10f buffer 实测）。
(() => {
  const U8 = globalThis.Uint8Array;
  globalThis.Uint8Array = new Proxy(U8, {
    construct(target, args, newTarget) {
      try {
        return Reflect.construct(target, args, newTarget);
      } catch (e) {
        throw new RangeError(`Invalid typed array length: ${args[0]}`);
      }
    },
  });
})();

// String.prototype.repeat 的 RangeError 文案桥（V8 口径："Invalid string length"/
// "Invalid count value: N"；SM 文案不同，套件正则按 V8 断言）
(() => {
  const rep = String.prototype.repeat;
  Object.defineProperty(String.prototype, 'repeat', {
    value: function (count) {
      if (typeof count === 'number' && count < 0) {
        throw new RangeError(`Invalid count value: ${count}`);
      }
      try {
        return rep.call(this, count);
      } catch (e) {
        throw e instanceof RangeError ? new RangeError('Invalid string length') : e;
      }
    },
    writable: true,
    configurable: true,
    enumerable: false,
  });
})();
globalThis.__wjs2_bufDecode = __wjs2_bufDecode;
globalThis.__wjs2_bufEncode = __wjs2_bufEncode;
globalThis.Buffer = Buffer;
})();
const __wjs2_keyState = new WeakMap();
function NotSupportedError_(what) { return new Error(`NotSupportedError: unsupported ${what}`); }
// WebCrypto 错误面（真机逐项）：DOMException 定名（OperationError/DataError/
// InvalidAccessError/NotSupportedError），套件断 name。
function __wjs2_domErr(name, message) { return new DOMException(String(message), String(name)); }
function __wjs2_normHash(h) {
  const s = typeof h === "string" ? h : String(h?.name ?? "");
  const up = s.trim().toUpperCase();
  // 真机逐项：无横线形（SHA256 等）一律拒，仅横线形大小写无关。
  const map = { "SHA-1": "SHA-1", "SHA-256": "SHA-256", "SHA-384": "SHA-384", "SHA-512": "SHA-512", "SHA3-256": "SHA3-256", "SHA3-384": "SHA3-384", "SHA3-512": "SHA3-512" };
  if (!map[up]) throw new DOMException("Unrecognized algorithm name", "NotSupportedError");
  return map[up];
}
function __wjs2_makeKey(alg, material, usages, extractable, kind) {
  const k = Object.create(CryptoKey.prototype);
  // hash 归一为 {name} 对象（node WebCrypto 口径；hidden-slots 套件点名
  // algorithm.hash.name；字符串输入在此统一转形，消费侧只认对象形）。
  const a = alg === null || typeof alg !== "object" ? {} : { ...alg };
  if (typeof a.hash === "string") a.hash = { name: a.hash };
  const kk = kind ?? "secret";
  // 用量校验（真机逐项）：非法即 SyntaxError Unsupported key usage。
  __wjs2_checkUsages(String(a.name ?? "").toUpperCase(), kk, usages ?? []);
  __wjs2_keyState.set(k, { alg: a, material, usages, extractable, kind: kk });
  return k;
}
// 各算法/钥型可用量表（export-import/derive/wrap 各套件真机逐项）。
function __wjs2_allowedUsages(name, kind) {
  if (kind === "secret") {
    if (name === "HMAC") return ["sign", "verify"];
    if (name === "HKDF" || name === "PBKDF2") return ["deriveKey", "deriveBits"];
    if (name.startsWith("AES-") || name === "CHACHA20-POLY1305") return ["encrypt", "decrypt", "wrapKey", "unwrapKey"];
    return null;
  }
  if (kind === "public") {
    if (name === "ECDSA" || name === "ED25519" || name === "ED448") return ["verify"];
    if (name === "ECDH" || name === "X25519") return [];
    if (name === "RSASSA-PKCS1-V1_5" || name === "RSA-PSS") return ["verify"];
    if (name === "RSA-OAEP") return ["encrypt", "wrapKey"];
    return null;
  }
  if (kind === "private") {
    if (name === "ECDSA" || name === "ED25519" || name === "ED448") return ["sign", "verify"];
    if (name === "ECDH" || name === "X25519") return ["deriveKey", "deriveBits"];
    if (name === "RSASSA-PKCS1-V1_5" || name === "RSA-PSS") return ["sign", "verify"];
    if (name === "RSA-OAEP") return ["decrypt", "unwrapKey"];
    return null;
  }
  return null;
}
function __wjs2_checkUsages(name, kind, usages) {
  const allowed = __wjs2_allowedUsages(name, kind);
  if (allowed === null) return;
  for (const u of usages ?? []) {
    if (!allowed.includes(u)) throw new DOMException(`Unsupported key usage for ${name} key`, "SyntaxError");
  }
}
// JWK kty 校验（真机逐项）：缺失/非串即 Invalid keyData；值错即 kty 文案。
function __wjs2_checkJwkKty(keyData, expected) {
  if (typeof keyData?.kty !== "string") throw __wjs2_domErr("DataError", "Invalid keyData");
  if (keyData.kty !== expected) throw __wjs2_domErr("DataError", 'Invalid JWK "kty" Parameter');
}
// JWK use/alg 校验（export-import 各套件真机逐项）：alg 缺省即过，白名单外
// 即 DataError；use 按签名/加密两族（缺省即过）；ext=false + extractable 即失配。
function __wjs2_checkJwkMeta(keyData, algs, sigFamily, extractable) {
  if (keyData?.alg !== undefined) {
    if (algs !== null && !algs.includes(keyData.alg)) {
      throw __wjs2_domErr("DataError", 'JWK "alg" does not match the requested algorithm');
    }
  }
  if (keyData?.use !== undefined) {
    const ok = sigFamily ? keyData.use === "sig" : keyData.use === "enc";
    if (!ok) throw __wjs2_domErr("DataError", 'Invalid JWK "use" Parameter');
  }
  if (keyData?.ext === false && extractable === true) {
    throw __wjs2_domErr("DataError", 'JWK "ext" Parameter and extractable mismatch');
  }
}
const __wjs2_keyUsageValues = ["encrypt", "decrypt", "sign", "verify", "deriveKey", "deriveBits", "wrapKey", "unwrapKey"];
function __wjs2_checkKeyOps(keyData, usages, name, kind) {
  const ops = keyData?.key_ops;
  if (ops === undefined) return;
  if (!Array.isArray(ops)) throw __wjs2_domErr("DataError", "Invalid keyData");
  for (const u of usages ?? []) {
    if (!ops.includes(u)) throw __wjs2_domErr("DataError", "Key operations and usage mismatch");
  }
  const allowed = __wjs2_allowedUsages(name, kind);
  if (allowed !== null) {
    for (const o of ops) {
      if (__wjs2_keyUsageValues.includes(o) && !allowed.includes(o)) {
        throw new DOMException(`Unsupported key usage for ${name} key`, "SyntaxError");
      }
    }
  }
  if (new Set(ops).size !== ops.length) throw __wjs2_domErr("DataError", "Duplicate key operation");
}
// KeyObject.from 桥（hidden-slots 套件：绕过可伪造的原型 getter，直读内部槽）。
globalThis.__wjs2_cryptoKeyInfo = (k) => {
  const s = __wjs2_keyState.get(k);
  if (!s) return null;
  return { kind: s.kind, material: s.material, usages: s.usages, extractable: s.extractable, alg: s.alg };
};
function __wjs2_keyBytes(v) {
  if (v instanceof ArrayBuffer) return new Uint8Array(v);
  if (ArrayBuffer.isView(v)) return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
  throw new TypeError("key data must be a BufferSource");
}
function __wjs2_dataBytes(v) {
  if (typeof v === "string") return new TextEncoder().encode(v);
  return __wjs2_keyBytes(v);
}
function __wjs2_needUsage(st, op) {
  if (!st.usages.includes(op)) throw new DOMException(`Unable to use this key to ${op}`, "InvalidAccessError");
}
// derive 系用量门（真机逐字，独立文案）。
function __wjs2_needDeriveUsage(st, op) {
  if (!st.usages.includes(op)) throw new DOMException(`baseKey does not have ${op} usage`, "InvalidAccessError");
}
// deriveBits/deriveKey 共用长度门（真机逐项）：null/缺省 OperationError；
// 超出 u32 Range 即 TypeError；非 8 倍数 OperationError。
function __wjs2_needBitsLength(len) {
  if (len === null || len === undefined) throw new DOMException("length cannot be null", "OperationError");
  const n = Number(len);
  if (!Number.isFinite(n) || n < 0 || n > 4294967295) throw new TypeError("length out of range");
  if (!Number.isInteger(n) || n % 8 !== 0) throw new DOMException("length must be a multiple of 8", "OperationError");
  return n / 8;
}
function __wjs2_aesParams(algorithm) {
  const iv = __wjs2_dataBytes(algorithm?.iv ?? new Uint8Array(0));
  const aad = algorithm?.additionalData === undefined ? undefined : __wjs2_dataBytes(algorithm.additionalData);
  const tagLength = algorithm?.tagLength === undefined ? 128 : Number(algorithm.tagLength);
  if (![32, 64, 96, 104, 112, 120, 128].includes(tagLength)) {
    throw __wjs2_domErr("OperationError", `${algorithm?.tagLength} is not a valid AES-GCM tag length`);
  }
  return { iv, aad, tagLength };
}
function __wjs2_b64urlEncode(u8) {
  let s = "";
  for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000));
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function __wjs2_b64urlDecode(str) {
  str = String(str).replace(/-/g, "+").replace(/_/g, "/");
  while (str.length % 4) str += "=";
  const bin = atob(str);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
"#;
