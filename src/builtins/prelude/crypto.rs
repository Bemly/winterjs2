//! WebCrypto（CryptoKey/crypto 对象/派生）（prelude 分域；拼接顺序见 mod.rs）。
pub const CRYPTO_JS: &str = r#"
globalThis.CryptoKey = class CryptoKey {
  constructor() { throw new TypeError("Illegal constructor"); }
  // 深拷贝快照（hidden-slots 套件：篡改返回对象不影响内部；hash/publicExponent
  // 须逐层拷，浅铺会共享嵌套引用）。
  get algorithm() {
    const a = __wjs2_keyState.get(this)?.alg;
    if (a === undefined) return undefined;
    // 深拷贝 + defineProperty 写入（hidden-slots 套件污染 Object.prototype
    // 同名只读属性时，普通赋值即抛；defineProperty 绕过原型只读栏）。
    const out = {};
    for (const k of Object.keys(a)) {
      let v = a[k];
      if (v !== null && typeof v === "object") {
        v = ArrayBuffer.isView(v) ? v.slice() : { ...v };
      }
      Object.defineProperty(out, k, { value: v, writable: true, enumerable: true, configurable: true });
    }
    return out;
  }
  get extractable() { return !!__wjs2_keyState.get(this)?.extractable; }
  get type() { return __wjs2_keyState.get(this)?.kind ?? "secret"; }
  get usages() { return [...(__wjs2_keyState.get(this)?.usages ?? [])]; }
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
    return `CryptoKey {\n  type: ${fmt(st.kind ?? "secret")},\n  extractable: ${!!st.extractable},\n  algorithm: ${fmt(alg)},\n  usages: ${fmt(st.usages ?? [])}\n}`;
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
// subtle 操作失败一律 DOMException（node 口径：op 错即 OperationError 名 +
// 通用消息；TypeError 系保持原样，只转码为 OperationError 的；prelude 原生错
// 无 code，按 "OperationError: " 消息前缀同判）。
function __wjs2_subtleOp(fn) {
  try {
    return fn();
  } catch (e) {
    const msg = e !== null && (typeof e === "object" || typeof e === "function") ? String(e.message ?? e) : String(e);
    if ((e !== null && typeof e === "object" && e.code === "OperationError") || msg.startsWith("OperationError:")) {
      throw new DOMException("The operation failed for an operation-specific reason", "OperationError");
    }
    throw e;
  }
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
    throw new Error("OperationError: bad X25519 deriveBits length");
  }
  return secret.slice(0, bits / 8).buffer;
}
function __wjs2_ecdh_bits(algorithm, st, length) {
  const pubKey = algorithm?.public;
  const pst = __wjs2_keyState.get(pubKey);
  if (!pst || pst.alg.name !== "ECDH" || pst.kind === "private") {
    throw new TypeError("deriveBits: algorithm.public must be an ECDH public key");
  }
  if (pst.alg.namedCurve !== st.alg.namedCurve) throw new Error("InvalidAccessError: ECDH curves differ");
  const secret = __wjs2_ecdh_derive(st.alg.namedCurve, st.material, pst.material);
  if (length === undefined || length === null) return secret.buffer;
  const bits = Number(length);
  if (!Number.isInteger(bits) || bits < 0 || bits > secret.length * 8 || bits % 8 !== 0) {
    throw new Error("OperationError: bad ECDH deriveBits length");
  }
  return secret.slice(0, bits / 8).buffer;
}
globalThis.crypto = {
  getRandomValues(view) {
    if (this !== globalThis.crypto) throw __wjs2BrandErr();
    __wjs2_fill_random(view);
    return view;
  },
  randomUUID() { return __wjs2_random_uuid(); },
  subtle: {
    digest(algorithm, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const name = typeof algorithm === "string" ? algorithm : String(algorithm?.name ?? algorithm);
      let view = data;
      if (view instanceof ArrayBuffer) view = new Uint8Array(view);
      else if (ArrayBuffer.isView(view) && !(view instanceof Uint8Array)) {
        view = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
      }
      const out = __wjs2_subtle_digest(name, view);
      return out.buffer;
      })();
    },
    generateKey(alg, extractable, usages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const name = typeof alg === "string" ? alg.toUpperCase() : String(alg?.name ?? "").toUpperCase();
      usages = [...(usages ?? [])].map(String);
      if (name === "AES-GCM") {
        const length = Number(alg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw new Error("NotSupportedError: AES-GCM length must be 128/192/256");
        const bytes = new Uint8Array(length / 8);
        crypto.getRandomValues(bytes);
        return __wjs2_makeKey({ name: "AES-GCM", length }, bytes, usages, !!extractable);
      }
      if (name === "HMAC") {
        const hash = __wjs2_normHash(alg?.hash);
        let length = alg?.length === undefined ? null : Number(alg.length);
        const outLen = { "SHA-1": 160, "SHA-256": 256, "SHA-384": 384, "SHA-512": 512 }[hash];
        if (length === null) length = outLen;
        if (!Number.isInteger(length) || length <= 0 || length > 1024 * 1024) {
          throw new Error("NotSupportedError: bad HMAC length");
        }
        const bytes = new Uint8Array(Math.ceil(length / 8));
        crypto.getRandomValues(bytes);
        return __wjs2_makeKey({ name: "HMAC", hash, length }, bytes, usages, !!extractable);
      }
      if (name === "RSASSA-PKCS1-V1_5" || name === "RSA-OAEP" || name === "RSA-PSS") {
        const length = Number(alg?.modulusLength ?? 2048);
        // node 口径：512 起（hidden-slots 套件 1024；真机实测 512/1024/2048 皆可）。
        if (!Number.isInteger(length) || length < 512 || length > 16384) throw new Error("NotSupportedError: RSA modulusLength must be an integer between 512 and 16384");
        const e = __wjs2_rsaPubExp(alg?.publicExponent);
        if (!Number.isInteger(e) || e < 2 || e > 2 ** 33 - 1) throw new Error("DataError: bad RSA publicExponent");
        const hash = __wjs2_normHash(alg?.hash ?? "SHA-256");
        const privDer = __wjs2_rsa_generate(length, e);
        const pubDer = __wjs2_rsa_public(privDer);
        const expBytes = (() => { const out = []; let n = e; do { out.unshift(n & 255); n = Math.floor(n / 256); } while (n > 0); return new Uint8Array(out); })();
        const keyAlg = { name, modulusLength: length, publicExponent: expBytes, hash };
        const { pubU: rsaPubU, privU: rsaPrivU } = __wjs2_splitUsages(usages);
        const mkPub = __wjs2_makeKey(keyAlg, pubDer, rsaPubU, !!extractable, "public");
        const mkPriv = __wjs2_makeKey(keyAlg, privDer, rsaPrivU, !!extractable, "private");
        return { publicKey: mkPub, privateKey: mkPriv };
      }
      if (name === "ECDSA" || name === "ECDH") {
        const curve = __wjs2_normCurve(alg?.namedCurve);
        const privDer = __wjs2_ec_generate(curve);
        const pubDer = __wjs2_ec_public(curve, privDer);
        const { pubU: ecPubU, privU: ecPrivU } = __wjs2_splitUsages(usages);
        const mkPub = __wjs2_makeKey({ name, namedCurve: curve }, pubDer, ecPubU, !!extractable, "public");
        const mkPriv = __wjs2_makeKey({ name, namedCurve: curve }, privDer, ecPrivU, !!extractable, "private");
        return { publicKey: mkPub, privateKey: mkPriv };
      }
      if (name === "ED25519") {
        const seed = __wjs2_ed_generate();
        const pub = __wjs2_ed_public(seed);
        const { pubU: edPubU, privU: edPrivU } = __wjs2_splitUsages(usages);
        const mkPub = __wjs2_makeKey({ name, namedCurve: "Ed25519" }, pub, edPubU, !!extractable, "public");
        const mkPriv = __wjs2_makeKey({ name, namedCurve: "Ed25519" }, seed, edPrivU, !!extractable, "private");
        return { publicKey: mkPub, privateKey: mkPriv };
      }
      if (name === "X25519") {
        const priv = __wjs2_x_generate();
        const pub = __wjs2_x_public(priv);
        const { pubU: xPubU, privU: xPrivU } = __wjs2_splitUsages(usages);
        const mkPub = __wjs2_makeKey({ name, namedCurve: "X25519" }, pub, xPubU, !!extractable, "public");
        const mkPriv = __wjs2_makeKey({ name, namedCurve: "X25519" }, priv, xPrivU, !!extractable, "private");
        return { publicKey: mkPub, privateKey: mkPriv };
      }
      if (name === "ED448") {
        throw new Error(`NotSupportedError: generateKey ${name} needs follow-up`);
      }
      throw new Error(`NotSupportedError: generateKey ${name} needs Phase 3 c-4`);
      })();
    },
    importKey(format, keyData, alg, extractable, usages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const name = typeof alg === "string" ? alg.toUpperCase() : String(alg?.name ?? "").toUpperCase();
      usages = [...(usages ?? [])].map(String);
      const needHash = name === "HMAC" ? __wjs2_normHash(alg?.hash) : undefined;
      if (name === "RSASSA-PKCS1-V1_5" || name === "RSA-OAEP" || name === "RSA-PSS") {
        const hash = __wjs2_normHash(alg?.hash ?? "SHA-256");
        if (format === "jwk") {
          if (!keyData || keyData.kty !== "RSA" || typeof keyData.n !== "string" || typeof keyData.e !== "string") {
            throw new Error("DataError: bad RSA JWK (n/e)");
          }
          const n = __wjs2_b64urlDecode(keyData.n), e = __wjs2_b64urlDecode(keyData.e);
          const expBytes = e.slice();
          if (typeof keyData.d === "string") {
            const d = __wjs2_b64urlDecode(keyData.d);
            const privDer = __wjs2_rsa_import_priv(n, e, d);
            const keyAlg = { name, modulusLength: n.length * 8, publicExponent: expBytes, hash };
            return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
          }
          const pubDer = __wjs2_rsa_import_pub(n, e);
          const keyAlg = { name, modulusLength: n.length * 8, publicExponent: expBytes, hash };
          return __wjs2_makeKey(keyAlg, pubDer, usages, !!extractable, "public");
        }
        if (format === "pkcs8") {
          const privDer = __wjs2_keyBytes(keyData);
          // PKCS#8 自验证（解析失败即 DataError；公钥顺带导出供 algorithm）。
          const pubDer = __wjs2_rsa_public(privDer);
          const parts = JSON.parse(__wjs2_rsa_jwk(privDer, pubDer));
          const n = __wjs2_b64urlDecode(parts.n), e = __wjs2_b64urlDecode(parts.e);
          const keyAlg = { name, modulusLength: n.length * 8, publicExponent: e.slice(), hash };
          return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
        }
        if (format === "spki") {
          const pubDer = __wjs2_keyBytes(keyData);
          const parts = JSON.parse(__wjs2_rsa_jwk_pub(pubDer));
          const n = __wjs2_b64urlDecode(parts.n), e = __wjs2_b64urlDecode(parts.e);
          const keyAlg = { name, modulusLength: n.length * 8, publicExponent: e.slice(), hash };
          return __wjs2_makeKey(keyAlg, pubDer, usages, !!extractable, "public");
        }
        throw new Error(`NotSupportedError: importKey ${format} for RSA needs pkcs8/spki/jwk`);
      }
      if (name === "ECDSA" || name === "ECDH") {
        const curve = __wjs2_normCurve(alg?.namedCurve);
        const keyAlg = { name, namedCurve: curve };
        if (format === "jwk") {
          if (!keyData || keyData.kty !== "EC" || keyData.crv !== curve
            || typeof keyData.x !== "string" || typeof keyData.y !== "string") {
            throw new Error("DataError: bad EC JWK (x/y/crv)");
          }
          const x = __wjs2_b64urlDecode(keyData.x), y = __wjs2_b64urlDecode(keyData.y);
          if (typeof keyData.d === "string") {
            const privDer = __wjs2_ec_import_priv(curve, __wjs2_b64urlDecode(keyData.d));
            // 公钥一致性：JWK 的 x/y 须与 d 对应（防混入）。
            const expect = __wjs2_ec_public(curve, privDer);
            const got = __wjs2_ec_import_pub(curve, x, y);
            const same = expect.length === got.length && expect.every((b, i) => b === got[i]);
            if (!same) throw new Error("DataError: EC JWK x/y does not match d");
            return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
          }
          return __wjs2_makeKey(keyAlg, __wjs2_ec_import_pub(curve, x, y), usages, !!extractable, "public");
        }
        if (format === "pkcs8") {
          const privDer = __wjs2_keyBytes(keyData);
          // PKCS#8 自验证（曲线错/损坏即 DataError）。
          __wjs2_ec_public(curve, privDer);
          return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
        }
        if (format === "spki") {
          const pubDer = __wjs2_keyBytes(keyData);
          // SPKI 自验证（曲线错即 DataError）。
          __wjs2_ec_jwk_pub(curve, pubDer);
          return __wjs2_makeKey(keyAlg, pubDer, usages, !!extractable, "public");
        }
        if (format === "raw") {
          const v = __wjs2_keyBytes(keyData);
          if (v.length < 1 || v[0] !== 0x04) throw new Error("DataError: EC raw public must be uncompressed (0x04‖x‖y)");
          const size = (v.length - 1) / 2;
          if (![32, 48, 66].includes(size) || 1 + 2 * size !== v.length) throw new Error("DataError: bad EC raw length");
          const x = v.slice(1, 1 + size), y = v.slice(1 + size);
          const c2 = size === 32 ? "P-256" : size === 48 ? "P-384" : "P-521";
          if (c2 !== curve) throw new Error("DataError: EC raw length does not match namedCurve");
          return __wjs2_makeKey(keyAlg, __wjs2_ec_import_pub(curve, x, y), usages, !!extractable, "public");
        }
        throw new Error(`NotSupportedError: importKey ${format} for EC needs jwk/pkcs8/spki/raw`);
      }
      if (name === "ED25519" || name === "X25519") {
        // JWK crv 用混合大小写（RFC 8037；内部 name 全大写，不外泄）。
        const crv = name === "ED25519" ? "Ed25519" : "X25519";
        const keyAlg = { name, namedCurve: crv };
        if (format === "jwk") {
          if (!keyData || keyData.kty !== "OKP" || keyData.crv !== crv
            || typeof keyData.x !== "string") {
            throw new Error("DataError: bad OKP JWK (kty/crv/x)");
          }
          const x = __wjs2_b64urlDecode(keyData.x);
          if (x.length !== 32) throw new Error("DataError: bad OKP JWK (x length)");
          if (typeof keyData.d === "string") {
            const seed = __wjs2_b64urlDecode(keyData.d);
            if (seed.length !== 32) throw new Error("DataError: bad OKP JWK (d length)");
            // 私钥一致性：JWK 的 x 须与 d 对应（防混入）。
            const expect = name === "ED25519" ? __wjs2_ed_public(seed) : __wjs2_x_public(seed);
            const same = expect.length === 32 && expect.every((b, i) => b === x[i]);
            if (!same) throw new Error("DataError: OKP JWK x does not match d");
            return __wjs2_makeKey(keyAlg, seed, usages, !!extractable, "private");
          }
          return __wjs2_makeKey(keyAlg, x, usages, !!extractable, "public");
        }
        if (format === "raw") {
          const v = __wjs2_keyBytes(keyData);
          if (v.length !== 32) throw new Error("DataError: OKP raw key must be 32 bytes");
          return __wjs2_makeKey(keyAlg, v, usages, !!extractable, "public");
        }
        if (format === "pkcs8") {
          const seed = __wjs2_okp_seed_from_pkcs8(name, __wjs2_keyBytes(keyData));
          return __wjs2_makeKey(keyAlg, seed, usages, !!extractable, "private");
        }
        if (format === "spki") {
          const publ = __wjs2_okp_pub_from_spki(name, __wjs2_keyBytes(keyData));
          return __wjs2_makeKey(keyAlg, publ, usages, !!extractable, "public");
        }
        throw new Error(`NotSupportedError: importKey ${format} for OKP needs jwk/raw/pkcs8/spki`);
      }
      let bytes;
      if (format === "raw") {
        bytes = __wjs2_keyBytes(keyData);
      } else if (format === "jwk") {
        if (!keyData || keyData.kty !== "oct" || typeof keyData.k !== "string") {
          throw new Error("NotSupportedError: only oct JWK keys for now");
        }
        bytes = __wjs2_b64urlDecode(keyData.k);
      } else throw new Error(`NotSupportedError: importKey ${format} needs Phase 3 c-4`);
      if (name === "AES-GCM") {
        if (![16, 24, 32].includes(bytes.length)) throw new TypeError("AES-GCM raw key must be 16/24/32 bytes");
        return __wjs2_makeKey({ name, length: bytes.length * 8 }, bytes, usages, !!extractable);
      }
      if (name === "HMAC") {
        return __wjs2_makeKey({ name, hash: needHash, length: bytes.length * 8 }, bytes, usages, !!extractable);
      }
      throw new Error(`NotSupportedError: importKey ${name} needs Phase 3 c-4`);
      })();
    },
    exportKey(format, key) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("exportKey: not a CryptoKey");
      if (!st.extractable) throw new Error("InvalidAccessError: key is not extractable");
      const aname = st.alg.name;
      if (aname === "RSASSA-PKCS1-V1_5" || aname === "RSA-OAEP" || aname === "RSA-PSS") {
        const hash = __wjs2_normHash(st.alg.hash ?? "SHA-256");
        const isPriv = st.kind === "private";
        const privDer = isPriv ? st.material : null;
        const pubDer = isPriv ? __wjs2_rsa_public(st.material) : st.material;
        if (format === "pkcs8") {
          if (!isPriv) throw new Error("InvalidAccessError: not a private key");
          return st.material.slice().buffer;
        }
        if (format === "spki") {
          return pubDer.slice().buffer;
        }
        if (format === "jwk") {
          const parts = isPriv
            ? JSON.parse(__wjs2_rsa_jwk(privDer, pubDer))
            : JSON.parse(__wjs2_rsa_jwk_pub(pubDer));
          const jwk = { kty: "RSA", n: parts.n, e: parts.e };
          if (isPriv) { jwk.d = parts.d; jwk.p = parts.p; jwk.q = parts.q; jwk.dp = parts.dp; jwk.dq = parts.dq; jwk.qi = parts.qi; }
          jwk.alg = aname === "RSASSA-PKCS1-V1_5"
            ? { "SHA-256": "RS256", "SHA-384": "RS384", "SHA-512": "RS512" }[hash] ?? "RS256"
            : aname === "RSA-PSS"
              ? { "SHA-256": "PS256", "SHA-384": "PS384", "SHA-512": "PS512" }[hash] ?? "PS256"
              : { "SHA-256": "RSA-OAEP", "SHA-384": "RSA-OAEP-384", "SHA-512": "RSA-OAEP-512" }[hash] ?? "RSA-OAEP";
          jwk.ext = true;
          jwk.key_ops = [...st.usages];
          return jwk;
        }
        throw new Error(`NotSupportedError: exportKey ${format} for RSA needs pkcs8/spki/jwk`);
      }
      if (aname === "ECDSA" || aname === "ECDH") {
        const curve = st.alg.namedCurve;
        const isPriv = st.kind === "private";
        const pubDer = isPriv ? __wjs2_ec_public(curve, st.material) : st.material;
        if (format === "pkcs8") {
          if (!isPriv) throw new Error("InvalidAccessError: not a private key");
          return st.material.slice().buffer;
        }
        if (format === "spki") {
          return pubDer.slice().buffer;
        }
        if (format === "jwk") {
          const parts = isPriv
            ? JSON.parse(__wjs2_ec_jwk(curve, st.material, pubDer))
            : JSON.parse(__wjs2_ec_jwk_pub(curve, pubDer));
          const jwk = { kty: "EC", crv: curve, x: parts.x, y: parts.y };
          if (isPriv) jwk.d = parts.d;
          jwk.ext = true;
          jwk.key_ops = [...st.usages];
          return jwk;
        }
        if (format === "raw") {
          if (isPriv) throw new Error("InvalidAccessError: raw export needs a public key");
          const parts = JSON.parse(__wjs2_ec_jwk_pub(curve, pubDer));
          const x = __wjs2_b64urlDecode(parts.x), y = __wjs2_b64urlDecode(parts.y);
          const out = new Uint8Array(1 + x.length + y.length);
          out[0] = 0x04; out.set(x, 1); out.set(y, 1 + x.length);
          return out.buffer;
        }
        throw new Error(`NotSupportedError: exportKey ${format} for EC needs pkcs8/spki/jwk/raw`);
      }
      if (aname === "ED25519" || aname === "X25519") {
        const isPriv = st.kind === "private";
        const pubBytes = isPriv
          ? (aname === "ED25519" ? __wjs2_ed_public(st.material) : __wjs2_x_public(st.material))
          : st.material;
        if (format === "pkcs8") {
          if (!isPriv) throw new Error("InvalidAccessError: not a private key");
          return __wjs2_okp_pkcs8_from_seed(aname, st.material).buffer;
        }
        if (format === "spki") {
          return __wjs2_okp_spki_from_pub(aname, pubBytes).buffer;
        }
        if (format === "jwk") {
          const jwk = { kty: "OKP", crv: aname === "ED25519" ? "Ed25519" : "X25519", x: __wjs2_b64urlEncode(pubBytes) };
          if (isPriv) jwk.d = __wjs2_b64urlEncode(st.material);
          // JWA 只给 Ed25519 定义了 "EdDSA"；X25519 无 alg（与 Node 一致，省略）。
          if (aname === "ED25519") jwk.alg = "EdDSA";
          jwk.ext = true;
          jwk.key_ops = [...st.usages];
          return jwk;
        }
        if (format === "raw") {
          if (isPriv) throw new Error("InvalidAccessError: raw export needs a public key");
          return pubBytes.slice().buffer;
        }
        throw new Error(`NotSupportedError: exportKey ${format} for OKP needs pkcs8/spki/jwk/raw`);
      }
      if (format === "raw") return st.material.slice().buffer;
      if (format === "jwk") {
        return { kty: "oct", k: __wjs2_b64urlEncode(st.material), alg: st.alg.name === "AES-GCM" ? `A${st.alg.length}GCM` : `HS${__wjs2_normHash(st.alg.hash).split("-")[1]}`, ext: true, key_ops: [...st.usages] };
      }
      throw new Error(`NotSupportedError: exportKey ${format} needs Phase 3 c-4`);
      })();
    },
    encrypt(algorithm, key, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("encrypt: not a CryptoKey");
      __wjs2_needUsage(st, "encrypt");
      if (st.alg.name === "AES-GCM") {
        const out = __wjs2_subtleOp(() => {
          const p = __wjs2_aesParams(algorithm);
          return __wjs2_aesgcm_encrypt(st.material, p.iv, p.aad, __wjs2_dataBytes(data));
        });
        return out.buffer;
      }
      if (st.alg.name === "RSA-OAEP") {
        if (st.kind !== "public") throw new TypeError("encrypt: not an RSA public key");
        const out = __wjs2_subtleOp(() => {
          const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
          const label = algorithm?.label === undefined ? undefined : __wjs2_dataBytes(algorithm.label);
          return __wjs2_rsa_encrypt(hash, st.material, __wjs2_dataBytes(data), label);
        });
        return out.buffer;
      }
      throw new TypeError("encrypt: unsupported key");
      })();
    },
    decrypt(algorithm, key, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("decrypt: not a CryptoKey");
      __wjs2_needUsage(st, "decrypt");
      if (st.alg.name === "AES-GCM") {
        const out = __wjs2_subtleOp(() => {
          const p = __wjs2_aesParams(algorithm);
          return __wjs2_aesgcm_decrypt(st.material, p.iv, p.aad, __wjs2_dataBytes(data));
        });
        return out.buffer;
      }
      if (st.alg.name === "RSA-OAEP") {
        if (st.kind !== "private") throw new TypeError("decrypt: not an RSA private key");
        const out = __wjs2_subtleOp(() => {
          const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
          const label = algorithm?.label === undefined ? undefined : __wjs2_dataBytes(algorithm.label);
          return __wjs2_rsa_decrypt(hash, st.material, __wjs2_dataBytes(data), label);
        });
        return out.buffer;
      }
      throw new TypeError("decrypt: unsupported key");
      })();
    },
    sign(algorithm, key, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("sign: not a CryptoKey");
      __wjs2_needUsage(st, "sign");
      if (st.alg.name === "HMAC") {
        const out = __wjs2_hmac_sign(__wjs2_normHash(st.alg.hash), st.material, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "RSASSA-PKCS1-V1_5") {
        if (st.kind !== "private") throw new TypeError("sign: not an RSA private key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        const out = __wjs2_rsa_sign(hash, st.material, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "RSA-PSS") {
        if (st.kind !== "private") throw new TypeError("sign: not an RSA private key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        // 缺省 saltLength = digest 长度（WebCrypto 口径）。
        const defSalt = { "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 }[hash];
        const salt = algorithm?.saltLength === undefined ? defSalt : Number(algorithm.saltLength);
        if (!Number.isInteger(salt) || salt < 0) throw new Error("OperationError: bad RSA-PSS saltLength");
        const out = __wjs2_pss_sign(hash, salt, st.material, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "ED25519") {
        if (st.kind !== "private") throw new TypeError("sign: not an Ed25519 private key");
        const out = __wjs2_ed_sign(st.material, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "ECDSA") {
        if (st.kind !== "private") throw new TypeError("sign: not an EC private key");
        const hash = __wjs2_normHash(algorithm?.hash ?? "SHA-256");
        const out = __wjs2_ecdsa_sign(st.alg.namedCurve, hash, st.material, __wjs2_dataBytes(data));
        return out.buffer;
      }
      throw new TypeError("sign: unsupported key");
      })();
    },
    verify(algorithm, key, signature, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("verify: not a CryptoKey");
      __wjs2_needUsage(st, "verify");
      if (st.alg.name === "HMAC") {
        return __wjs2_hmac_verify(__wjs2_normHash(st.alg.hash), st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
      }
      if (st.alg.name === "RSASSA-PKCS1-V1_5") {
        if (st.kind === "private") throw new TypeError("verify: not an RSA public key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        return __wjs2_rsa_verify(hash, st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
      }
      if (st.alg.name === "RSA-PSS") {
        if (st.kind === "private") throw new TypeError("verify: not an RSA public key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        const defSalt = { "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 }[hash];
        const salt = algorithm?.saltLength === undefined ? defSalt : Number(algorithm.saltLength);
        if (!Number.isInteger(salt) || salt < 0) throw new Error("OperationError: bad RSA-PSS saltLength");
        return __wjs2_pss_verify(hash, salt, st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
      }
      if (st.alg.name === "ED25519") {
        if (st.kind === "private") throw new TypeError("verify: not an Ed25519 public key");
        return __wjs2_ed_verify(st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
      }
      if (st.alg.name === "ECDSA") {
        if (st.kind === "private") throw new TypeError("verify: not an EC public key");
        const hash = __wjs2_normHash(algorithm?.hash ?? "SHA-256");
        return __wjs2_ecdsa_verify(st.alg.namedCurve, hash, st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
      }
      throw new TypeError("verify: unsupported key");
      })();
    },
    deriveBits(algorithm, baseKey, length) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(baseKey);
      if (!st || (st.alg.name !== "ECDH" && st.alg.name !== "X25519")) {
        throw new TypeError("deriveBits: not an ECDH/X25519 key");
      }
      if (st.kind !== "private") throw new TypeError("deriveBits: needs a private key");
      __wjs2_needUsage(st, "deriveBits");
      if (st.alg.name === "X25519") return __wjs2_x_bits(algorithm, st, length);
      return __wjs2_ecdh_bits(algorithm, st, length);
      })();
    },
    deriveKey(algorithm, baseKey, derivedKeyAlg, extractable, usages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(baseKey);
      if (!st || (st.alg.name !== "ECDH" && st.alg.name !== "X25519")) {
        throw new TypeError("deriveKey: not an ECDH/X25519 key");
      }
      if (st.kind !== "private") throw new TypeError("deriveKey: needs a private key");
      __wjs2_needUsage(st, "deriveKey");
      const bitsOf = (length) => st.alg.name === "X25519"
        ? __wjs2_x_bits(algorithm, st, length)
        : __wjs2_ecdh_bits(algorithm, st, length);
      const dname = String(derivedKeyAlg?.name ?? "").toUpperCase();
      let bytes;
      if (dname === "AES-GCM") {
        const length = Number(derivedKeyAlg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw new Error("NotSupportedError: derived AES-GCM length must be 128/192/256");
        bytes = new Uint8Array(bitsOf(length));
        return __wjs2_makeKey({ name: "AES-GCM", length }, bytes, [...(usages ?? [])].map(String), !!extractable);
      }
      if (dname === "HMAC") {
        const hash = __wjs2_normHash(derivedKeyAlg?.hash);
        let length = derivedKeyAlg?.length === undefined ? null : Number(derivedKeyAlg.length);
        bytes = new Uint8Array(bitsOf(length));
        if (length === null) length = bytes.length * 8;
        return __wjs2_makeKey({ name: "HMAC", hash, length }, bytes, [...(usages ?? [])].map(String), !!extractable);
      }
      throw new Error(`NotSupportedError: deriveKey to ${dname || "?"} needs follow-up`);
      })();
    },
    // 未实现面（methods-not-async 套件点名须在场 + 非 async + 品牌检查；
    // 真实现随算法轮补齐，此处先 NotSupportedError 占位）。
    wrapKey(format, key, wrappingKey, wrapAlgorithm) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return Promise.reject(new Error("NotSupportedError: wrapKey needs follow-up"));
    },
    unwrapKey(format, wrappedKey, unwrappingKey, unwrapAlgorithm, unwrappedKeyAlgorithm, extractable, keyUsages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return Promise.reject(new Error("NotSupportedError: unwrapKey needs follow-up"));
    },
    encapsulateBits(encapsulation, publicKey) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return Promise.reject(new Error("NotSupportedError: encapsulateBits needs follow-up"));
    },
    encapsulateKey(encapsulation, publicKey) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return Promise.reject(new Error("NotSupportedError: encapsulateKey needs follow-up"));
    },
    decapsulateBits(decapsulation, privateKey, length) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return Promise.reject(new Error("NotSupportedError: decapsulateBits needs follow-up"));
    },
    decapsulateKey(decapsulation, privateKey, unwrappedKeyAlgorithm, extractable, keyUsages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return Promise.reject(new Error("NotSupportedError: decapsulateKey needs follow-up"));
    },
    getPublicKey(key, usages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return Promise.reject(new Error("NotSupportedError: getPublicKey needs follow-up"));
    },
  },
};
// structuredClone 的 CryptoKey 面（hidden-slots 套件）：native 只走 JSON 纯数据，
// 类实例自身无可枚举属性——此处先把键换成快照描述子再进原生，出后再复原为键。
// 快照 deep-copy（publicExponent 切片），篡改隔离；transfer 含键暂不支持（抛）。
(function __wjs2_wrapStructuredClone() {
  const __orig = globalThis.structuredClone;
  if (typeof __orig !== "function") return;
  const __b64enc = (u8) => {
    let s = "";
    for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000));
    return btoa(s);
  };
  const __b64dec = (s) => {
    const bin = atob(String(s));
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  };
  const __packKey = (k) => {
    const st = __wjs2_keyState.get(k);
    if (!st) return k;
    // 手工 walk（JSON replacer 看不到 TypedArray：toJSON 先转 plain object）。
    const __packAlg = (v) => {
      if (v instanceof ArrayBuffer) return { __wjs2Bytes: __b64enc(new Uint8Array(v)) };
      if (ArrayBuffer.isView(v)) return { __wjs2Bytes: __b64enc(new Uint8Array(v.buffer, v.byteOffset, v.byteLength)) };
      if (Array.isArray(v)) return v.map(__packAlg);
      if (v !== null && typeof v === "object") {
        const o = {};
        for (const key of Object.keys(v)) o[key] = __packAlg(v[key]);
        return o;
      }
      return v;
    };
    const alg = __packAlg(st.alg ?? null);
    return {
      __wjs2CryptoKey: true,
      alg,
      materialB64: __b64enc(st.material),
      usages: [...(st.usages ?? [])],
      extractable: !!st.extractable,
      kind: st.kind ?? "secret",
    };
  };
  const __pack = (v, seen) => {
    if (v !== null && typeof v === "object") {
      if (seen.has(v)) return seen.get(v);
      if (v instanceof CryptoKey) {
        const p = __packKey(v);
        seen.set(v, p);
        return p;
      }
      if (v instanceof ArrayBuffer) {
        const p = { __wjs2Bytes: __b64enc(new Uint8Array(v)) };
        seen.set(v, p);
        return p;
      }
      if (ArrayBuffer.isView(v)) {
        const p = { __wjs2Bytes: __b64enc(new Uint8Array(v.buffer, v.byteOffset, v.byteLength)), __wjs2View: v.constructor.name };
        seen.set(v, p);
        return p;
      }
      if (Array.isArray(v)) {
        const out = [];
        seen.set(v, out);
        for (const e of v) out.push(__pack(e, seen));
        return out;
      }
      const out = {};
      seen.set(v, out);
      for (const k of Object.keys(v)) out[k] = __pack(v[k], seen);
      return out;
    }
    return v;
  };
  const __reviveBytes = (v) => {
    if (v !== null && typeof v === "object" && typeof v.__wjs2Bytes === "string" && Object.keys(v).length <= 2) {
      const u8 = __b64dec(v.__wjs2Bytes);
      return v.__wjs2View === "DataView" ? new DataView(u8.buffer) : u8;
    }
    return v;
  };
  const __unpack = (v) => {
    if (v !== null && typeof v === "object") {
      if (v.__wjs2CryptoKey === true) {
        const bytes = __b64dec(v.materialB64);
        return __wjs2_makeKey(__unpack(v.alg), bytes, v.usages, v.extractable, v.kind);
      }
      const r = __reviveBytes(v);
      if (r !== v) return r;
      if (Array.isArray(v)) return v.map(__unpack);
      const out = {};
      for (const k of Object.keys(v)) out[k] = __unpack(v[k]);
      return out;
    }
    return v;
  };
  globalThis.structuredClone = function (value, options) {
    // 快道：不含 CryptoKey 即原样直通（transfer/detach 语义零扰动）。
    const __seen = new Set();
    const __hasKey = (v) => {
      if (v !== null && typeof v === "object") {
        if (__seen.has(v)) return false;
        __seen.add(v);
        if (v instanceof CryptoKey) return true;
        if (Array.isArray(v)) return v.some(__hasKey);
        return Object.keys(v).some((k) => __hasKey(v[k]));
      }
      return false;
    };
    if (!__hasKey(value)) return __orig.call(this, value, options);
    return __unpack(__orig.call(this, __pack(value, new Map()), options));
  };
})();
"#;
