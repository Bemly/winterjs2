function __rsaCrypt(key, data, isPublic, isEncrypt) {
  let k = __keyArg(key, "key");
  // 10f crypto二轮：public 方向遇私钥即派生公钥（`publicEncrypt(privPem)` 真机口径）。
  if (isPublic && k.type === "private") k = __derivePublic(k);
  // rsa-pss 禁加解密（keygen-rsa-pss 套件：'operation not supported for this keytype'）。
  if (k.__keyType === "rsa-pss") __osslKeytypeError();
  const opts = (typeof key === "object" && key !== null && !(key instanceof Uint8Array) && !__isKeyObject(key)) ? key : {};
  // 10f crypto二轮：默认 padding 按方向（加解密 OAEP=4；签式 v1.5=1，真机口径）。
  const padding = opts.padding ?? (isEncrypt === isPublic ? 4 : 1);
  // 10f crypto二轮：key 对象带 encoding 时字符串 data 同解码（真机实证口径）。
  const dataB = (typeof data === "string" && typeof opts.encoding === "string")
    ? Buffer.from(data, opts.encoding)
    : __cryptBytes(data, "data");
  if (padding === 4) {
    // 10f crypto二轮：oaepHash/oaepLabel 校验（真机逐字，dsa 套件点名）。
    const __recvAny = (v) => {
      if (v === null || v === undefined) return String(v);
      const t = typeof v;
      if (t === "object" || t === "function") return `an instance of ${v.constructor?.name ?? "Object"}`;
      let s;
      try { s = (t === "string") ? `'${v}'` : String(v); } catch { s = t; }
      return `type ${t} (${s})`;
    };
    if (opts.oaepHash !== undefined && typeof opts.oaepHash !== "string") {
      const err = new TypeError(
        `The "key.oaepHash" property must be of type string. Received ${__recvAny(opts.oaepHash)}`);
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    const hashFlat = opts.oaepHash ?? "sha1";
    const hash = __normHashName(hashFlat);
    if (hash === undefined) {
      const err = new Error("Invalid digest used");
      err.code = "ERR_OSSL_EVP_INVALID_DIGEST";
      throw err;
    }
    if (hash === "MD5") {
      const err = new Error(`Unsupported OAEP hash ${opts.oaepHash}`);
      err.code = "ERR_NOT_SUPPORTED";
      throw err;
    }
    let label = null;
    if (opts.oaepLabel !== undefined) {
      const ol = opts.oaepLabel;
      if (!(typeof ol === "string" || ol instanceof Uint8Array ||
            ol instanceof ArrayBuffer || ArrayBuffer.isView(ol))) {
        const err = new TypeError(
          `The "key.oaepLabel" property must be of type string or an instance of ArrayBuffer, Buffer, TypedArray, or DataView. Received ${__recvAny(ol)}`);
        err.code = "ERR_INVALID_ARG_TYPE";
        throw err;
      }
      label = __cryptBytes(ol, "label");
    }
    // 10f crypto二轮：mgf1Hash（真机口径；缺省跟随 oaepHash；未知即 INVALID_DIGEST）。
    let mgfHash = hash;
    if (opts.mgf1Hash !== undefined) {
      if (typeof opts.mgf1Hash !== "string") {
        const err = new TypeError(
          `The "key.mgf1Hash" property must be of type string. Received ${__recvAny(opts.mgf1Hash)}`);
        err.code = "ERR_INVALID_ARG_TYPE";
        throw err;
      }
      const mg = __normHashName(opts.mgf1Hash);
      if (mg === undefined || __OAEP_HLEN[mg] === undefined) {
        const err = new Error("Invalid digest used");
        err.code = "ERR_OSSL_EVP_INVALID_DIGEST";
        throw err;
      }
      mgfHash = mg;
    }
    // 混合哈希（mgf ≠ oaep）走 JS 编解码 + 裸 RSA（真机互操作另行交叉验证，见注释）。
    const mixedHash = mgfHash !== hash;
    // SHA-1 走手写 OAEP（digest 0.10 版本面，见 Rust 侧记）；SHA-2 走既有 natives
    if (isPublic && isEncrypt) {
      // 10f crypto二轮：混合哈希走 JS 编解码 + 裸 RSA（mgf1Hash 套件）。
      if (mixedHash) {
        const det = __rsaDetailsFromMaterial(k.type, k.__material);
        if (det !== null && __OAEP_HLEN[hash] !== undefined) {
          const em = __oaepMixedEncode(hash, mgfHash, det.modulusLength / 8, dataB, label);
          return Buffer.from(__cryptCall(() => __wjs2_rsa_raw(k.__material, em, 0)));
        }
      }
      if (hash === "SHA-1") {
        return Buffer.from(__cryptCall(() => __wjs2_node_rsa_oaep(k.__material, dataB, label, 1)));
      }
      return Buffer.from(__cryptCall(() => __wjs2_rsa_encrypt(hash, k.__material, dataB, label)));
    }
    if (!isPublic && !isEncrypt) {
      // 10f crypto二轮：混合哈希解码（同上）。
      if (mixedHash) {
        const det = __rsaDetailsFromMaterial(k.type, k.__material);
        if (det !== null && __OAEP_HLEN[hash] !== undefined) {
          const em = Buffer.from(__cryptCall(() => __wjs2_rsa_raw(k.__material, dataB, 1)));
          return __oaepMixedDecode(hash, mgfHash, det.modulusLength / 8, em, label);
        }
      }
      if (hash === "SHA-1") {
        return Buffer.from(__cryptCall(() => __wjs2_node_rsa_oaep(k.__material, dataB, label, 0)));
      }
      return Buffer.from(__cryptCall(() => __wjs2_rsa_decrypt(hash, k.__material, dataB, label)));
    }
    // 10f crypto二轮：私钥加密（OAEP-SHA1 反向 native；SHA-2 系另案）。
    if (!isPublic && isEncrypt) {
      if (hash === "SHA-1") {
        return Buffer.from(__cryptCall(() => __wjs2_node_rsa_oaep_flip(k.__material, dataB, label, 1)));
      }
      const err = new Error("RSA privateEncrypt not supported");
      err.code = "ERR_NOT_SUPPORTED";
      throw err;
    }
    // 10f crypto二轮：公钥解密（同上）。
    if (isPublic && !isEncrypt) {
      if (hash === "SHA-1") {
        return Buffer.from(__cryptCall(() => __wjs2_node_rsa_oaep_flip(k.__material, dataB, label, 0)));
      }
      const err = new Error("RSA publicDecrypt not supported");
      err.code = "ERR_NOT_SUPPORTED";
      throw err;
    }
  }
  if (padding === 1) {
    if (isPublic && isEncrypt) {
      return Buffer.from(__cryptCall(() => __wjs2_rsa_encrypt_v15(k.__material, dataB)));
    }
    if (!isPublic && !isEncrypt) {
      return Buffer.from(__cryptCall(() => __wjs2_rsa_decrypt_v15(k.__material, dataB)));
    }
    // 10f crypto二轮：v1.5 反向 native。
    if (!isPublic && isEncrypt) {
      return Buffer.from(__cryptCall(() => __wjs2_rsa_v15_flip(k.__material, dataB, 1)));
    }
    if (isPublic && !isEncrypt) {
      return Buffer.from(__cryptCall(() => __wjs2_rsa_v15_flip(k.__material, dataB, 0)));
    }
  }
  // 10f crypto二轮：NO_PADDING 裸运算（3；私钥 d 次幂/公钥 e 次幂）。
  if (padding === 3) {
    return Buffer.from(__cryptCall(() => __wjs2_rsa_raw(k.__material, dataB, isPublic ? 0 : 1)));
  }
  const err = new Error(`Unsupported RSA padding ${padding} for this operation`);
  err.code = "ERR_NOT_SUPPORTED";
  throw err;
}
export function publicEncrypt(key, data) { return __rsaCrypt(key, data, true, true); }
export function privateDecrypt(key, data) { return __rsaCrypt(key, data, false, false); }
export function privateEncrypt(key, data) { return __rsaCrypt(key, data, false, true); }
export function publicDecrypt(key, data) { return __rsaCrypt(key, data, true, false); }

class ECDHImpl {
  constructor(curve) {
    this.__curve = __normCurve(curve);
    if (this.__curve !== "P-256" && this.__curve !== "P-384" && this.__curve !== "P-521" && this.__curve !== "secp256k1") {
      const err = new Error(`ECDH curve ${curve} not supported (P-256/384/521/secp256k1)`);
      err.code = "ERR_NOT_SUPPORTED";
      throw err;
    }
    this.__priv = null;
    this.__pub = null;
  }
  generateKeys(encoding, format) {
    this.__priv = __cryptCall(() => __wjs2_ec_generate(this.__curve));
    this.__pub = __cryptCall(() => __wjs2_ec_public(this.__curve, this.__priv));
    // node 口径（ECDH.generateKeys）：encoding/format 透传 getPublicKey。
    return this.getPublicKey(encoding, format);
  }
  getPublicKey(encoding, format) {
    if (this.__pub === null) {
      // 真机口径（dh-curves 160 行钉住）：纯 Error，消息逐字。
      throw new Error("Failed to get ECDH public key");
    }
    if (format === "der" || format === "pem") return this.__pubToDer(format);
    // node 口径（getFormat）：仅容 undefined/'uncompressed'/'compressed'/
    // 'hybrid'；余者即 ERR_CRYPTO_ECDH_INVALID_FORMAT（dh-curves 116 行钉住）。
    if (format !== undefined && format !== "uncompressed" && format !== "compressed" && format !== "hybrid") {
      const err = new TypeError(`Invalid ECDH format: ${format}`);
      err.code = "ERR_CRYPTO_ECDH_INVALID_FORMAT";
      throw err;
    }
    const parts = JSON.parse(__cryptCall(() => __wjs2_ec_jwk_pub(this.__curve, this.__pub)));
    const x = __b64urlDec(parts.x), y = __b64urlDec(parts.y);
    let raw;
    if (format === "compressed") {
      raw = new Uint8Array(1 + x.length);
      raw[0] = (y[y.length - 1] & 1) ? 3 : 2;
      raw.set(x, 1);
    } else if (format === "hybrid") {
      raw = new Uint8Array(1 + x.length + y.length);
      raw[0] = (y[y.length - 1] & 1) ? 7 : 6;
      raw.set(x, 1); raw.set(y, 1 + x.length);
    } else {
      raw = new Uint8Array(1 + x.length + y.length);
      raw[0] = 4; raw.set(x, 1); raw.set(y, 1 + x.length);
    }
    // node 口径（encode）：缺省/falsy/'buffer'（严格大小写）即回 Buffer。
    if (!encoding || encoding === "buffer") return Buffer.from(raw);
    return Buffer.from(raw).toString(encoding);
  }
  __pubToDer(format) {
    if (format === "der") return Buffer.from(this.__pub);
    return __pemEncode("PUBLIC KEY", this.__pub);
  }
  getPrivateKey(encoding) {
    if (this.__priv === null) {
      // 真机口径（dh-curves 165 行钉住）：纯 Error，消息逐字。
      throw new Error("Failed to get ECDH private key");
    }
    const size = __curveSize(this.__curve);
    const parts = JSON.parse(__cryptCall(() => __wjs2_ec_jwk(this.__curve, this.__priv, this.__pub)));
    const d = __b64urlDec(parts.d);
    const out = new Uint8Array(size);
    out.set(d, size - d.length);
    if (!encoding || encoding === "buffer") return Buffer.from(out);
    return Buffer.from(out).toString(encoding);
  }
  setPrivateKey(priv, encoding) {
    const scalar = __cryptBytes(priv, "private key", encoding);
    // node 口径：标量非法即 ERR_CRYPTO_INVALID_KEYTYPE（RangeError），状态不变
    //（dh-curves 238 行钉住）。
    let imported;
    try {
      imported = __cryptCall(() => __wjs2_ec_import_priv(this.__curve, scalar));
    } catch {
      const err = new RangeError("Private key is not valid for specified curve.");
      err.code = "ERR_CRYPTO_INVALID_KEYTYPE";
      throw err;
    }
    this.__priv = imported;
    this.__pub = __cryptCall(() => __wjs2_ec_public(this.__curve, this.__priv));
    return this;
  }
  setPublicKey(pub, encoding) {
    // node 口径：ECDH.setPublicKey 废弃（DEP0031），deprecate 即首调警告一次
    //（dh-curves 12 行 + DEP0031 套件钉住）；C++ 侧 SEC1 点校验，坏点/跨曲线即
    // 'Failed to convert Buffer to EC_POINT'（dh-curves 151 行钉住）；存 DER。
    if (!__ecdhSetPubWarned) {
      __ecdhSetPubWarned = true;
      try {
        process.emitWarning("ecdh.setPublicKey() is deprecated.", { type: "DeprecationWarning", code: "DEP0031" });
      } catch {}
    }
    const raw = __cryptBytes(pub, "public key", encoding);
    this.__pub = __ecdhImportPoint(this.__curve, raw);
    return this;
  }
  computeSecret(peer, inputEncoding, outputEncoding) {
    if (this.__priv === null) {
      const err = new Error("ECDH keys not generated");
      err.code = "ERR_CRYPTO_INVALID_STATE";
      throw err;
    }
    // node 口径：存键对不一致（setPublicKey 换过公钥）即 ERR_CRYPTO_INVALID_KEYPAIR
    //（dh-curves 224 行钉住；FIPS 段同码）。
    const derived = __cryptCall(() => __wjs2_ec_public(this.__curve, this.__priv));
    if (!Buffer.from(derived).equals(Buffer.from(this.__pub))) {
      const err = new RangeError("Invalid key pair");
      err.code = "ERR_CRYPTO_INVALID_KEYPAIR";
      throw err;
    }
    const peerB = (typeof peer === "string") ? __cryptBytes(peer, "peer", inputEncoding) : __cryptBytes(peer, "peer");
    let peerDer;
    try {
      peerDer = __ecdhImportPointOrDer(this.__curve, peerB);
    } catch {
      throw __ecdhInvalidPublicKey();
    }
    let secret;
    try {
      secret = __cryptCall(() => __wjs2_ecdh_derive(this.__curve, this.__priv, peerDer));
    } catch {
      throw __ecdhInvalidPublicKey();
    }
    // node 口径（encode）：缺省/falsy/'buffer'（严格大小写）即回 Buffer。
    if (!outputEncoding || outputEncoding === "buffer") return Buffer.from(secret);
    return Buffer.from(secret).toString(outputEncoding);
  }
}
// ECDH 对端点导入（裸 SEC1 全形经 import 转 SPKI；余下 DER 直透；crypto_ec.js
// 五轮压缩点口径复用，见该处 503 行注释）。
function __ecdhImportPointOrDer(curve, raw) {
  const size = __curveSize(curve);
  if ((raw[0] === 2 || raw[0] === 3) && raw.length === 1 + size) {
    return Buffer.from(__cryptCall(() => __wjs2_ec_import_compressed(curve, raw)));
  }
  if (raw.length === 1 + 2 * size && (raw[0] === 4 || raw[0] === 6 || raw[0] === 7)) {
    const x = raw.slice(1, 1 + size), y = raw.slice(1 + size);
    return Buffer.from(__cryptCall(() => __wjs2_ec_import_pub(curve, x, y)));
  }
  return raw;
}
// ECDH.setPublicKey 存储形（校验+DER 化；坏点/跨曲线即指定文案，151 行钉住）。
function __ecdhImportPoint(curve, raw) {
  const size = __curveSize(curve);
  try {
    if ((raw[0] === 2 || raw[0] === 3) && raw.length === 1 + size) {
      return Buffer.from(__cryptCall(() => __wjs2_ec_import_compressed(curve, raw)));
    }
    if (raw.length === 1 + 2 * size && (raw[0] === 4 || raw[0] === 6 || raw[0] === 7)) {
      const x = raw.slice(1, 1 + size), y = raw.slice(1 + size);
      return Buffer.from(__cryptCall(() => __wjs2_ec_import_pub(curve, x, y)));
    }
    // DER 形（SPKI 直存；非法即错）。
    __cryptCall(() => __wjs2_ec_jwk_pub(curve, raw));
    return Buffer.from(raw);
  } catch {
    throw new Error("Failed to convert Buffer to EC_POINT");
  }
}
function __ecdhInvalidPublicKey() {
  const err = new Error("Public key is not valid for specified curve");
  err.code = "ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY";
  return err;
}
// DEP0031 首调警告旗（node deprecate 口径：进程内一次）。
let __ecdhSetPubWarned = false;
export function createECDH(curve, format) { return new ECDHImpl(curve); }

// 10f crypto首轮：真机 `crypto.ECDH(...)` 可无 new 调用（无废弃警告）。
function ECDH(...args) { return new ECDHImpl(...args); }
Object.setPrototypeOf(ECDH, ECDHImpl);
ECDH.prototype = ECDHImpl.prototype;
ECDH.prototype.constructor = ECDH;
// ECDH.convertKey（ecdh-convert-key 套件钉死）：任意 SEC1 点形互转；校验口径
// 与 getPublicKey 同（INVALID_FORMAT/INVALID_CURVE/ARG_TYPE）；坏点即
// 'Failed to convert Buffer to EC_POINT'；转换失败不污染后续操作（无共享错栈）。
function __ecdhConvertKey(key, curve, inputEncoding, outputEncoding, format) {
  const raw = __cryptBytes(key, "key", inputEncoding);
  __needStr(curve, "curve");
  const c = __ecCurve(curve);
  if (format !== undefined && format !== "uncompressed" && format !== "compressed" && format !== "hybrid") {
    const err = new TypeError(`Invalid ECDH format: ${format}`);
    err.code = "ERR_CRYPTO_ECDH_INVALID_FORMAT";
    throw err;
  }
  const pubDer = __ecdhImportPoint(c, raw);
  const parts = JSON.parse(__cryptCall(() => __wjs2_ec_jwk_pub(c, pubDer)));
  const x = __b64urlDec(parts.x), y = __b64urlDec(parts.y);
  let out;
  if (format === "compressed") {
    out = new Uint8Array(1 + x.length);
    out[0] = (y[y.length - 1] & 1) ? 3 : 2;
    out.set(x, 1);
  } else if (format === "hybrid") {
    out = new Uint8Array(1 + x.length + y.length);
    out[0] = (y[y.length - 1] & 1) ? 7 : 6;
    out.set(x, 1); out.set(y, 1 + x.length);
  } else {
    out = new Uint8Array(1 + x.length + y.length);
    out[0] = 4; out.set(x, 1); out.set(y, 1 + x.length);
  }
  if (!outputEncoding || outputEncoding === "buffer") return Buffer.from(out);
  return Buffer.from(out).toString(outputEncoding);
}
ECDH.convertKey = __ecdhConvertKey;


