//! `node:v8`（Node `lib/v8.js` 最小桥，MIT；plan 9j：解 vite `-r dev` 挡路）。
//!
//! 忠实面：`startupSnapshot` 守卫（vite 仅在 try 内调 `isBuildingSnapshot()`）。
//!
//! 偏差（记档，plan2 §4 的"v8 口径跳过"仍然有效）：堆统计
//! （`getHeapStatistics`/`getHeapSpaceStatistics`/`getHeapCodeStatistics`，
//! V8 口径数字在本引擎无意义）、`Serializer`/`Deserializer`（v8 序列化格式与
//! mozjs structuredClone 不互通）、coverage/`setFlagsFromString` 等一律不导出——
//! 用到即报"not a function" TypeError（与缺内建相比，至少 import 不炸）。

/// 内嵌 ESM 源（零 native，纯形）。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/v8.js (startupSnapshot-only bridge; see module docs).
const startupSnapshot = {
  isBuildingSnapshot() {
    return false;
  },
  addSerializeCallback() {
    return undefined;
  },
  addDeserializeCallback() {
    return undefined;
  },
  setDeserializeMainFunction() {
    return undefined;
  },
};

// V8 旗串（P2-process R7：exception-capture 套件点名；收下即返，
// --abort-on-uncaught-exception 等不兑现——capture 截获后面本无 abort，另案记档）。
function setFlagsFromString(flags) {
  return undefined;
}

// serialize/deserialize（自洽二进制往返；非 V8 线格式，跨引擎不互通——
// v8 口径跳过仍有效，deserialize-buffer 套件只测自家往返）。
// 类型面：undefined/null/bool/f64/string/bigint/Buffer/ArrayBuffer/
// 视图（含 DataView）/Array/纯对象；函数/symbol 拒 TypeError。
const __V8_KINDS = [
  Int8Array, Uint8Array, Uint8ClampedArray, Int16Array, Uint16Array,
  Int32Array, Uint32Array, Float32Array, Float64Array, BigInt64Array,
  BigUint64Array, DataView,
];
function __v8Encode(value) {
  const bytes = [];
  const u32 = (n) => {
    bytes.push(n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff);
  };
  const raw = (u8) => { u32(u8.length); for (const b of u8) bytes.push(b); };
  const str = (s) => raw(new TextEncoder().encode(s));
  const enc = (v) => {
    if (v === undefined) { bytes.push(0); return; }
    if (v === null) { bytes.push(1); return; }
    if (v === false) { bytes.push(2); return; }
    if (v === true) { bytes.push(3); return; }
    if (typeof v === 'number') {
      bytes.push(4);
      const b = new Uint8Array(8);
      new DataView(b.buffer).setFloat64(0, v, true);
      for (const x of b) bytes.push(x);
      return;
    }
    if (typeof v === 'string') { bytes.push(5); str(v); return; }
    if (typeof v === 'bigint') { bytes.push(6); str(v.toString()); return; }
    if (typeof Buffer !== 'undefined' && Buffer.isBuffer(v)) {
      bytes.push(7); raw(v); return;
    }
    if (v instanceof ArrayBuffer) {
      bytes.push(9); raw(new Uint8Array(v)); return;
    }
    if (ArrayBuffer.isView(v)) {
      const k = __V8_KINDS.indexOf(v.constructor);
      if (k < 0 || v.buffer instanceof SharedArrayBuffer) {
        throw new TypeError('could not serialize view');
      }
      bytes.push(8, k);
      raw(new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
      return;
    }
    if (Array.isArray(v)) {
      bytes.push(10); u32(v.length);
      for (const e of v) enc(e);
      return;
    }
    if (typeof v === 'object') {
      const keys = Object.keys(v);
      bytes.push(11); u32(keys.length);
      for (const k of keys) { str(k); enc(v[k]); }
      return;
    }
    throw new TypeError(`could not serialize value of type ${typeof v}`);
  };
  enc(value);
  return Uint8Array.from(bytes);
}
function serialize(value) {
  return Buffer.from(__v8Encode(value));
}
function deserialize(buf) {
  const u8 = buf instanceof ArrayBuffer ? new Uint8Array(buf)
    : ArrayBuffer.isView(buf) ? new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength)
    : (() => { throw new TypeError('deserialize needs a Buffer'); })();
  let off = 0;
  const u32 = () => {
    const v = u8[off] | (u8[off + 1] << 8) | (u8[off + 2] << 16) | (u8[off + 3] << 24);
    off += 4;
    return v >>> 0;
  };
  const raw = () => { const n = u32(); const s = u8.slice(off, off + n); off += n; return s; };
  const str = () => new TextDecoder().decode(raw());
  const dec = () => {
    const tag = u8[off++];
    switch (tag) {
      case 0: return undefined;
      case 1: return null;
      case 2: return false;
      case 3: return true;
      case 4: { const v = new DataView(u8.buffer, u8.byteOffset + off, 8).getFloat64(0, true); off += 8; return v; }
      case 5: return str();
      case 6: return BigInt(str());
      case 7: return Buffer.from(raw());
      case 9: return raw().buffer;
      case 8: {
        const k = u8[off++];
        const C = __V8_KINDS[k];
        const b = raw();
        if (C === DataView) return new DataView(b.buffer, b.byteOffset, b.byteLength);
        return new C(b.buffer, b.byteOffset, b.byteLength / C.BYTES_PER_ELEMENT);
      }
      case 10: { const n = u32(); const a = []; for (let i = 0; i < n; i++) a.push(dec()); return a; }
      case 11: {
        const n = u32(); const o = {};
        for (let i = 0; i < n; i++) { const k = str(); o[k] = dec(); }
        return o;
      }
      default: throw new TypeError('invalid serialized data');
    }
  };
  return dec();
}

export { startupSnapshot, setFlagsFromString, serialize, deserialize };
export default { startupSnapshot, setFlagsFromString, serialize, deserialize };
"#;
