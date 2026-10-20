//! WebCrypto 键面（CryptoKey/算法帮助；§0.9 拆分：crypto 纯搬移，concat 字节恒等）。
pub const CRYPTO_KEYS_JS: &str = r#"
globalThis.CryptoKey = class CryptoKey {
  constructor() {
    const e = new TypeError("Illegal constructor");
    e.code = "ERR_ILLEGAL_CONSTRUCTOR";
    throw e;
  }
  // 深拷贝快照（hidden-slots 套件：篡改返回对象不影响内部；hash/publicExponent
  // 须逐层拷，浅铺会共享嵌套引用）。
  // internal-slots 套件：同一键返回稳定视图（篡改 sticky，真机同）。
  get algorithm() {
    const st = __wjs2_keyState.get(this);
    if (!st) return undefined;
    if (!st.view) {
      const a = st.alg ?? {};
      const out = {};
      for (const k of Object.keys(a)) {
        let v = a[k];
        if (k === "name" && typeof v === "string") v = __wjs2_canonAlgName(v);
        if (v !== null && typeof v === "object") {
          v = ArrayBuffer.isView(v) ? v.slice() : { ...v };
        }
        // 深拷贝 + defineProperty 写入（hidden-slots 套件污染 Object.prototype
        // 同名只读属性时，普通赋值即抛；defineProperty 绕过原型只读栏）。
        Object.defineProperty(out, k, { value: v, writable: true, enumerable: true, configurable: true });
      }
      st.view = out;
    }
    return st.view;
  }
  get extractable() { return !!__wjs2_keyState.get(this)?.extractable; }
  get type() { return __wjs2_keyState.get(this)?.kind ?? "secret"; }
  get usages() {
    const st = __wjs2_keyState.get(this);
    if (!st) return [];
    if (!st.usagesView) st.usagesView = [...(st.usages ?? [])];
    return st.usagesView;
  }
  // util.inspect 直读内部槽（hidden-slots 套件：伪造 getter 不得污染输出）。
  [Symbol.for("nodejs.util.inspect.custom")]() {
    const st = __wjs2_keyState.get(this) ?? {};
    const fmt = (v) => {
      if (typeof v === "string") return `'${v}'`;
      if (typeof v === "number" || typeof v === "boolean") return String(v);
      if (v instanceof Uint8Array) return `Uint8Array(${v.length}) [ ${[...v].join(", ")} ]`;
      if (Array.isArray(v)) return `[ ${v.map(fmt).join(", ")} ]`;
      if (v !== null && typeof v === "object") {
        return `{ ${Object.keys(v).map((k) => `${k}: ${fmt(v[k])}`).join(", ")} }`;
      }
      return String(v);
    };
    const alg = st.alg ?? {};
    const algShown = (alg !== null && typeof alg === "object" && typeof alg.name === "string")
      ? { ...alg, name: __wjs2_canonAlgName(alg.name) }
      : alg;
    return `CryptoKey {\n  type: ${fmt(st.kind ?? "secret")},\n  extractable: ${!!st.extractable},\n  algorithm: ${fmt(algShown)},\n  usages: ${fmt(st.usages ?? [])}\n}`;
  }
};
function __wjs2_normCurve(c) {
  const s = String(c ?? "").trim().toUpperCase().replace("_", "-");
  const map = { "P-256": "P-256", "P256": "P-256", "P-384": "P-384", "P384": "P-384", "P-521": "P-521", "P521": "P-521" };
  if (!map[s]) throw new Error(`NotSupportedError: unsupported curve '${c}' (P-256/384/521)`);
  return map[s];
}
function __wjs2_rsaPubExp(v) {
  if (v === undefined) return 65537;
  if (v instanceof Uint8Array) {
    let n = 0;
    for (const b of v) n = n * 256 + b;
    return n;
  }
  return Number(v);
}
function __wjs2BrandErr() {
  const e = new TypeError("Illegal constructor");
  e.code = "ERR_INVALID_THIS";
  return e;
}
// 算法名归一（internal-slots 套件真机实测）：Ed 系保 mixed（Ed25519/X25519/
// Ed448/X448），余者沿用大写存值。
function __wjs2_canonAlgName(n) {
  const u = String(n ?? "").toUpperCase();
  return { ED25519: "Ed25519", X25519: "X25519", ED448: "Ed448", X448: "X448" }[u] ?? n;
}
// 非对称 usages 公私分流（node 口径：public 仅 verify/encrypt/wrapKey 系，
// private 仅 sign/decrypt/unwrapKey/derive 系；private 为空即抛）。
function __wjs2_splitUsages(usages) {
  const PUB = ["verify", "encrypt", "wrapKey", "encapsulateBits", "encapsulateKey", "encapsulate"];
  const PRIV = ["sign", "decrypt", "unwrapKey", "deriveKey", "deriveBits", "decapsulate", "decapsulateBits", "decapsulateKey"];
  const pubU = [];
  const privU = [];
  for (const u of usages ?? []) {
    if (PUB.includes(u)) pubU.push(u);
    else if (PRIV.includes(u)) privU.push(u);
    else { pubU.push(u); privU.push(u); }
  }
  if (privU.length === 0) throw new DOMException("Usages cannot be empty when creating a key.", "SyntaxError");
  return { pubU, privU };
}
function __wjs2_x_bits(algorithm, st, length) {
  const pubKey = algorithm?.public;
  const pst = __wjs2_keyState.get(pubKey);
  if (!pst || pst.alg.name !== "X25519" || pst.kind === "private") {
    throw new TypeError("deriveBits: algorithm.public must be an X25519 public key");
  }
  const secret = __wjs2_x_derive(st.material, pst.material);
  if (length === undefined || length === null) return secret.buffer;
  const bits = Number(length);
  if (!Number.isInteger(bits) || bits < 0 || bits > secret.length * 8 || bits % 8 !== 0) {
    throw __wjs2_domErr("OperationError", "bad X25519 deriveBits length");
  }
  return secret.slice(0, bits / 8).buffer;
}
function __wjs2_ecdh_bits(algorithm, st, length) {
  const pubKey = algorithm?.public;
  if (pubKey === undefined) {
    const e = new TypeError("deriveBits: algorithm.public is required");
    e.code = "ERR_MISSING_OPTION";
    throw e;
  }
  const pst = __wjs2_keyState.get(pubKey);
  if (!pst) {
    const e = new TypeError("deriveBits: algorithm.public must be a CryptoKey");
    e.code = "ERR_INVALID_ARG_TYPE";
    throw e;
  }
  if (pst.alg.name !== "ECDH" || pst.kind !== "public") {
    if (pst.kind !== "public") throw __wjs2_domErr("InvalidAccessError", "deriveBits: algorithm.public must be a public key");
    throw __wjs2_domErr("InvalidAccessError", "key algorithm mismatch");
  }
  if (pst.alg.namedCurve !== st.alg.namedCurve) throw new Error("Named curve mismatch");
  const secret = __wjs2_ecdh_derive(st.alg.namedCurve, st.material, pst.material);
  if (length === undefined || length === null) return secret.buffer;
  const bits = Number(length);
  if (!Number.isInteger(bits) || bits < 0 || bits > secret.length * 8) {
    throw __wjs2_domErr("OperationError", "derived bit length is too small");
  }
  if (bits % 8 !== 0) {
    // 非 8 倍数即截断 + 尾字节掩膜（真机逐项）。
    const n = Math.floor(bits / 8), rem = bits % 8;
    const out = secret.slice(0, n + 1);
    out[n] &= (0xFF << (8 - rem)) & 0xFF;
    return out.buffer;
  }
  return secret.slice(0, bits / 8).buffer;
}
"#;
