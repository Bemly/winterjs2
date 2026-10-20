export function generateKeyPair(type, options, ...rest) {
  // 真机 26.8.2 实测：二参（无 options）即抛 options-must-be-object（`Received undefined`），
  // 不默认 {}（旧 10e“二参合法”注释按 4.65 翻转，以实测为准）。
  if (typeof options === "function") {
    rest = [options, ...rest];
    options = undefined;
  }
  let cb = rest.find((a) => typeof a === "function");
  let pubEnc, privEnc;
  if (typeof options === "object" && options !== null) {
    pubEnc = options.publicKeyEncoding;
    privEnc = options.privateKeyEncoding;
  }
  if (!cb) {
    if (typeof rest[0] === "function") cb = rest[0];
  }
  if (typeof cb !== "function") {
    const err = new TypeError("generateKeyPair requires a callback for async form (use Sync variant otherwise)");
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  // 真机口径：type/options 同步抛（callback 不背锅），见 keygen 67/86 行。
  // options 归一全进 __genPairSync（具名族缺省即 ARG_TYPE；ed/x 系默认 {}）。
  __checkKeyPairHead(type, options);
  __checkKeyPairTypeKnown(type);
  if (typeof options === "object" && options !== null) {
    __checkKeyPairEncs(type, options, options.publicKeyEncoding, options.privateKeyEncoding);
  }
  // RSA 参数同步预检（async 口径同样同步抛，见 keygen 303 行）。
  if ((type === "rsa" || type === "rsa-pss") && typeof options === "object" && options !== null) {
    __checkRsaKeyOptions(options);
    if (type === "rsa-pss") __checkRsaPssKeyOptions(options);
  }
  // DSA 参数同步预检（同上，见 keygen.js 399 行）。
  if (type === "dsa" && typeof options === "object" && options !== null) {
    __checkDsaKeyOptions(options);
  }
  // DH 参数同步预检（全量校验；同上，见 keygen 549/557 行；async 亦同步抛）。
  if (type === "dh" && typeof options === "object" && options !== null) {
    __checkDhArgs(options);
  }
  queueMicrotask(() => {
    try {
      // 单次生成再分发编码：公钥私钥必须同对（双调 __genPairSync 即对不上，
      // x-JWK 错配/RSA 解密失败/DSA 双倍慢超时同源，crypto3 keygen-async 簇）。
      const out = __applyEncoding(__genPairSync(type, options), pubEnc, privEnc);
      cb(null, out.publicKey, out.privateKey);
    } catch (e) {
      cb(e);
    }
  });
}
// promisify 定制（keygen-promisify 套件）：默认 promisify 只取首个成功参，
// 真机以 { publicKey, privateKey } 双键决议。
// custom 正名 'generateKeyPair'（test-util-promisify-custom-names 不点名此件，
// 但 util promisify 已不再回填改名——匿名箭头 name '' 会使 promisify 件名空，
// custom 共享污染同前；仿 assignFunctionName 只改名不绑名，内层裸调仍指外层）。
generateKeyPair[Symbol.for("nodejs.util.promisify.custom")] = (() => {
  const __genPairCustom = (type, options, ...rest) =>
    new Promise((resolve, reject) => {
      generateKeyPair(type, options, ...rest, (err, publicKey, privateKey) => {
        if (err) reject(err);
        else resolve({ publicKey, privateKey });
      });
    });
  try {
    Object.defineProperty(__genPairCustom, 'name', { value: 'generateKeyPair', configurable: true });
  } catch {}
  return __genPairCustom;
})();
// DER 编解码（ECDSA der 签名用）
function __derLen(n) {
  if (n < 128) return new Uint8Array([n]);
  const bytes = [];
  while (n > 0) { bytes.unshift(n & 255); n = Math.floor(n / 256); }
  return new Uint8Array([128 | bytes.length, ...bytes]);
}
function __derInt(raw) {
  let v = raw;
  while (v.length > 1 && v[0] === 0) v = v.slice(1);
  const neg = (v[0] & 128) !== 0;
  const body = neg ? new Uint8Array([0, ...v]) : v;
  return new Uint8Array([2, ...__derLen(body.length), ...body]);
}
function __rawToDerSig(raw) {
  const half = raw.length / 2;
  const seq = new Uint8Array([...__derInt(raw.slice(0, half)), ...__derInt(raw.slice(half))]);
  return new Uint8Array([48, ...__derLen(seq.length), ...seq]);
}
function __derToRawSig(der, size) {
  const top = __derRead(der, 0);
  if (top.tag !== 48) throw new Error("bad signature");
  const kids = __derChildren(top.body);
  if (kids.length !== 2 || kids[0].tag !== 2 || kids[1].tag !== 2) throw new Error("bad signature");
  const norm = (t) => {
    let v = t.body;
    while (v.length > 1 && v[0] === 0) v = v.slice(1);
    if (v.length > size) throw new Error("bad signature");
    const out = new Uint8Array(size);
    out.set(v, size - v.length);
    return out;
  };
  const out = new Uint8Array(size * 2);
  out.set(norm(kids[0]), 0);
  out.set(norm(kids[1]), size);
  return out;
}
function __signCore(alg, data, keyObj, dsaEncoding, saltLength, padding) {
  __checkSignVerifyOptions(padding, saltLength, dsaEncoding);
  const dataB = __cryptBytes(data, "data");
  const kt = keyObj.__keyType;
  if (kt === "ed448") {
    // 10e Ed448 纯签名（真机口径：非 null 即 ERR_OSSL_INVALID_DIGEST，ml-dsa 同款）。
    if (alg !== null && alg !== undefined) {
      const err = new Error("Invalid digest");
      err.code = "ERR_OSSL_INVALID_DIGEST";
      throw err;
    }
    if (keyObj.__kind !== "private") {
      const err = new TypeError("sign requires a private key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    return __cryptCall(() => __wjs2_ed448_sign(keyObj.__material, dataB));
  }
  if (kt === "ed25519") {
    // EdDSA 无摘要算法（真 Node：非 null 即 ERR_OSSL_INVALID_DIGEST；
    // 此处用 CRYPTO 扁平码，Sign 类同口径，记档）
    if (alg !== null && alg !== undefined) {
      const err = new Error("Invalid digest");
      err.code = "ERR_CRYPTO_INVALID_DIGEST";
      throw err;
    }
    if (keyObj.__kind !== "private") {
      const err = new TypeError("sign requires a private key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    return __cryptCall(() => __wjs2_ed_sign(keyObj.__material, dataB));
  }
  if (typeof kt === "string" && kt.startsWith("ml-dsa-")) {
    // 9i-6：纯签名（真机口径：非 null 即 ERR_OSSL_INVALID_DIGEST）。
    if (alg !== null && alg !== undefined) {
      const err = new Error("Invalid digest");
      err.code = "ERR_OSSL_INVALID_DIGEST";
      throw err;
    }
    if (keyObj.__kind !== "private") {
      const err = new TypeError("sign requires a private key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    return __cryptCall(() => __wjs2_mldsa_sign(keyObj.__material, dataB));
  }
  const hash = __normHashName(alg);
  if (hash === undefined) {
    // 10f crypto二轮：x25519 无签名原语，报错优先于摘要校验（套件点名）。
    if (kt === "x25519" || kt === "x448") __osslKeytypeError();
    const err = new Error("Invalid digest");
    err.code = "ERR_CRYPTO_INVALID_DIGEST";
    throw err;
  }
  if (kt === "rsa" || kt === "rsa-pss") {
    if (keyObj.__kind !== "private") {
      const err = new TypeError("sign requires a private key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    __checkPssPadding(kt, padding);
    __checkRsaOpPadding(kt, padding);
    // SHA-1/MD5 走手写 v1.5（digest 0.10 版本面；PSS 模式除外）；SHA-2/PSS 走既有 natives
    // 10f crypto六轮：PSS 的 SHA-1 走 sha1_010 底座（MD5 仍不支持，无 0.10 可引）。
    const pss = kt === "rsa-pss" || padding === 6;
    if ((hash === "SHA-1" || hash === "MD5") && !pss) {
      return __cryptCall(() => __wjs2_node_rsa_v15_sign(keyObj.__material, dataB, hash));
    }
    if (hash === "MD5") {
      const err = new Error("RSA-PSS with MD5 not supported");
      err.code = "ERR_NOT_SUPPORTED";
      throw err;
    }
    if (pss) {
      // 10f crypto六轮：rsa-pss 键约束执行（真机 26 逐项）——params 缺席的键
      // 无约束全放行；saltLength 为下限（小即 PSS_SALTLEN_TOO_SMALL），
      // hashAlgorithm 限摘要（错即 DIGEST_NOT_ALLOWED）。
      // 序：salt 先、digest 后（sha1+小 salt 落 salt 错，套件 1007 行钉住）。
      const det = keyObj.__detail ?? {};
      const defSalt = { "SHA-1": 20, "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 }[hash] ?? 32;
      // 缺省 salt 取键约束值（真机实测：sha512_20 键缺省签出 salt 20，
      // 非摘要长 64；无约束键走摘要长）；显式值经 special 消解 + max 门。
      const detMod = typeof det.modulusLength === "number" ? det.modulusLength : null;
      const matDet = detMod === null ? __rsaDetailsFromMaterial("private", keyObj.__material) : null;
      const maxSalt = (detMod !== null ? detMod : (matDet !== null ? matDet.modulusLength : 0)) / 8 - defSalt - 2;
      const salt = saltLength === undefined
        ? (typeof det.saltLength === "number" ? det.saltLength : defSalt)
        : __resolvePssSalt(saltLength, defSalt, maxSalt);
      if (typeof det.saltLength === "number" && salt < det.saltLength) {
        const err = new Error("error:1C8000AC:Provider routines::pss saltlen too small");
        err.code = "ERR_OSSL_PSS_SALTLEN_TOO_SMALL";
        throw err;
      }
      if (typeof det.hashAlgorithm === "string" && __normHashName(det.hashAlgorithm) !== hash) {
        const err = new Error("error:1C8000AE:Provider routines::digest not allowed");
        err.code = "ERR_OSSL_DIGEST_NOT_ALLOWED";
        throw err;
      }
      // 10f crypto六轮：MGF1 自动切换（RFC4055 §3.1/3.3；真机逐项）——键约束
      // 的 mgf1 与消息摘要不同时手组 EMSA-PSS（零新 native）。
      const mgfDet = typeof det.mgf1HashAlgorithm === "string" ? __normHashName(det.mgf1HashAlgorithm) : null;
      if (mgfDet !== null && mgfDet !== hash) {
        const d = __rsaDetailsFromMaterial("private", keyObj.__material);
        if (d === null) {
          const err = new Error("OperationError: bad RSA-PSS key");
          err.code = "ERR_INVALID_ARG_VALUE";
          throw err;
        }
        const mHash = __dgstBytes(hash, dataB);
        const saltB = __randFill(new Uint8Array(salt));
        const em = __emsaPssEncode(hash, mgfDet, d.modulusLength - 1, mHash, saltB);
        if (!em) {
          const err = new Error("OperationError: EMSA-PSS encoding error");
          err.code = "ERR_INVALID_ARG_VALUE";
          throw err;
        }
        return Buffer.from(__cryptCall(() => __wjs2_rsa_raw(keyObj.__material, em, 1)));
      }
      return __cryptCall(() => __wjs2_pss_sign(hash, salt, keyObj.__material, dataB));
    }
    return __cryptCall(() => __wjs2_rsa_sign(hash, keyObj.__material, dataB));
  }
  if (kt === "ec") {
    if (keyObj.__kind !== "private") {
      const err = new TypeError("sign requires a private key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    const curve = keyObj.__detail.namedCurve;
    const raw = __cryptCall(() => __wjs2_ecdsa_sign(curve, hash, keyObj.__material, dataB));
    if ((dsaEncoding ?? "der") === "der") return __rawToDerSig(Buffer.from(raw));
    return Buffer.from(raw);
  }
  if (kt === "dsa") {
    if (keyObj.__kind !== "private") {
      const err = new TypeError("sign requires a private key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    const envStr = Buffer.from(keyObj.__material).toString("utf8");
    const der = __cryptCall(() => __wjs2_dsa_sign(hash, envStr, dataB));
    if ((dsaEncoding ?? "der") === "der") return Buffer.from(der);
    // ieee-p1363：r‖s 定长（q 长）；DER 解后拼。
    const qLen = Buffer.from(JSON.parse(envStr).q, "base64").length;
    return Buffer.from(__derToRawSig(Buffer.from(der), qLen));
  }
  if (kt === "x25519" || kt === "x448") __osslKeytypeError();
  const err = new Error(`sign not supported for ${kt}`);
  err.code = "ERR_NOT_SUPPORTED";
  throw err;
}
function __osslKeytypeError() {
  // 10f crypto二轮：X25519 等无签名原语的键（真机口径，套件正则点名）。
  const err = new Error("operation not supported for this keytype");
  err.code = "ERR_OSSL_EVP_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE";
  throw err;
}
// OSSL 形 JS 错误（`error:HEX:library::reason`；library/reason 经 `=`
// 赋值，原型链 setter 透传；opensslErrorStack 永不触碰）。
function __invalidSignOption(name, value) {
  const err = new TypeError(`The property 'options.${name}' is invalid. Received ${__inspectRecv(value)}`);
  err.code = "ERR_INVALID_ARG_VALUE";
  return err;
}
// 签验选项通用门（真机口径，sign-verify 套件 padding/saltLength/dsaEncoding
// 行钉住）：与键型无关，先于操作门；padding 非整数、saltLength 非 int32、
// dsaEncoding 非 der/ieee-p1363 即 invalid。
function __checkSignVerifyOptions(padding, saltLength, dsaEncoding) {
  if (padding !== undefined && (typeof padding !== "number" || !Number.isInteger(padding))) {
    throw __invalidSignOption("padding", padding);
  }
  if (saltLength !== undefined && (typeof saltLength !== "number" || !Number.isInteger(saltLength) || saltLength < -2147483648 || saltLength > 2147483647)) {
    throw __invalidSignOption("saltLength", saltLength);
  }
  if (dsaEncoding !== undefined && dsaEncoding !== "der" && dsaEncoding !== "ieee-p1363") {
    throw __invalidSignOption("dsaEncoding", dsaEncoding);
  }
}
function __osslOpErr(code, message) {
  const err = new Error(message);
  err.code = code;
  const m = message.match(/^error:[0-9A-Fa-f]+:([^:]*):(.*)$/);
  if (m) {
    err.library = m[1];
    err.reason = m[2].replace(/^:/, "");
  }
  return err;
}
// rsa 签验 padding 门（真机口径）：rsa 键 padding 仅容 undefined/1（v1.5）/6
//（PSS，走 PSS 核）；余者（OAEP 等）即 ILLEGAL_OR_UNSUPPORTED_PADDING_MODE
//（sign-verify 套件 OAEP 行钉住）。
function __checkRsaOpPadding(kt, padding) {
  if (padding === 3) {
    throw __osslOpErr("ERR_OSSL_INVALID_PADDING_MODE",
      "error:1C8000A8:Provider routines::invalid padding mode");
  }
  if (kt === "rsa" && padding !== undefined && padding !== 1 && padding !== 6 && padding !== -1 && padding !== 5) {
    throw __osslOpErr("ERR_OSSL_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE",
      "error:1C8000A5:Provider routines::illegal or unsupported padding mode");
  }
}
// rsa-pss 显式非 PSS padding 即非法（keygen-rsa-pss 套件；真机
// ERR_OSSL_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE；RSA 键可 PSS padding 不限）。
// PSS saltLength resolution (node semantics, pinned by pss probes):
// -1 means digest length; -2/-3/-4 mean max usable; other negatives mean
// INVALID_SALT_LENGTH; explicit over max means DATA_TOO_LARGE.
function __resolvePssSalt(saltLength, hLen, maxSalt) {
  let eff;
  if (saltLength === -1) eff = hLen;
  else if (saltLength === -2 || saltLength === -3 || saltLength === -4) eff = maxSalt;
  else if (saltLength < 0) {
    throw __osslOpErr("ERR_OSSL_INVALID_SALT_LENGTH",
      "error:1C800070:Provider routines::invalid salt length");
  } else eff = saltLength + 0;
  if (eff > maxSalt) {
    throw __osslOpErr("ERR_OSSL_RSA_DATA_TOO_LARGE_FOR_KEY_SIZE",
      "error:0200006E:rsa routines::data too large for key size");
  }
  return eff;
}
function __checkPssPadding(kt, padding) {  if (kt === "rsa-pss" && padding !== undefined && padding !== 6) {
    throw __osslOpErr("ERR_OSSL_ILLEGAL_OR_UNSUPPORTED_PADDING_MODE",
      "error:1C8000A5:Provider routines::illegal or unsupported padding mode");
  }
}
function __verifyCore(alg, data, keyObj, sig, dsaEncoding, saltLength, padding) {
  __checkSignVerifyOptions(padding, saltLength, dsaEncoding);
  const dataB = __cryptBytes(data, "data");
  const sigB = __cryptBytes(sig, "signature");
  const kt = keyObj.__keyType;
  // 10f crypto二轮：OKP 验签收私钥（派生公钥，真机口径；旧"verify requires a public key"记档修）。
  if ((kt === "ed448" || kt === "ed25519" || (typeof kt === "string" && kt.startsWith("ml-dsa-"))) &&
      keyObj.__kind === "private") {
    keyObj = __derivePublic(keyObj);
  }
  if (kt === "ed448") {
    // 10e Ed448 纯验签（真机口径同 sign：非 null 即 ERR_OSSL_INVALID_DIGEST）。
    if (alg !== null && alg !== undefined) {
      const err = new Error("Invalid digest");
      err.code = "ERR_OSSL_INVALID_DIGEST";
      throw err;
    }
    if (keyObj.__kind === "private") {
      const err = new TypeError("verify requires a public key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    // 10f crypto二轮：签名形态错（非 114B）回 false，不抛（空签名套件点名）。
    if (sigB.length !== 114) return false;
    return __cryptCall(() => __wjs2_ed448_verify(keyObj.__material, sigB, dataB));
  }
  if (kt === "ed25519") {
    if (alg !== null && alg !== undefined) {
      const err = new Error("Invalid digest");
      err.code = "ERR_CRYPTO_INVALID_DIGEST";
      throw err;
    }
    if (keyObj.__kind === "private") {
      const err = new TypeError("verify requires a public key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    // 10f crypto二轮：签名形态错（非 64B）回 false，不抛（空签名套件点名）。
    if (sigB.length !== 64) return false;
    return __cryptCall(() => __wjs2_ed_verify(keyObj.__material, sigB, dataB));
  }
  if (typeof kt === "string" && kt.startsWith("ml-dsa-")) {
    // 9i-6：纯签名（真机口径同 sign：非 null 即 ERR_OSSL_INVALID_DIGEST）。
    if (alg !== null && alg !== undefined) {
      const err = new Error("Invalid digest");
      err.code = "ERR_OSSL_INVALID_DIGEST";
      throw err;
    }
    if (keyObj.__kind === "private") {
      const err = new TypeError("verify requires a public key");
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    return __cryptCall(() => __wjs2_mldsa_verify(keyObj.__material, sigB, dataB));
  }
  const hash = __normHashName(alg);
  if (hash === undefined) {
    // 10f crypto二轮：x25519 无签名原语，报错优先于摘要校验（套件点名）。
    if (kt === "x25519" || kt === "x448") __osslKeytypeError();
    const err = new Error("Invalid digest");
    err.code = "ERR_CRYPTO_INVALID_DIGEST";
    throw err;
  }
  // 公钥派生（私钥亦可验，Node 同款）
  const pubDer = keyObj.__kind === "private" ? __derivePublic(keyObj).__material : keyObj.__material;
  if (kt === "rsa" || kt === "rsa-pss") {
    __checkPssPadding(kt, padding);
    __checkRsaOpPadding(kt, padding);
    // 10f crypto二轮：签名长短 != 钥长即 false，不抛（空签名套件点名）。
    const det = __rsaDetailsFromMaterial("public", pubDer);
    if (det !== null && sigB.length !== det.modulusLength / 8) return false;
    const pss = kt === "rsa-pss" || padding === 6;
    if ((hash === "SHA-1" || hash === "MD5") && !pss) {
      return __cryptCall(() => __wjs2_node_rsa_v15_verify(pubDer, sigB, dataB, hash));
    }
    if (hash === "MD5") {
      // 10f crypto六轮：SHA-1 走底座，MD5 维持不支持（同 sign 侧）。
      const err = new Error("RSA-PSS with MD5 not supported");
      err.code = "ERR_NOT_SUPPORTED";
      throw err;
    }
    if (pss) {
      const defSalt = { "SHA-1": 20, "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 }[hash] ?? 32;
      // 缺省 salt 取键约束值（同 sign 侧；真机实测 sign/verify 双缺省互通）。
      const vdet0 = keyObj.__detail ?? {};
      // 验签侧：超 max 即 false 不抛（pss 探针钉死），非法值仍抛；
      // AUTO（-2 全长/-4 摘要封顶）逐 salt 试探（与 OpenSSL auto 恢复同真值）。
      const vMaxSalt = (det !== null ? det.modulusLength : 0) / 8 - defSalt - 2;
      const vAutoCap = saltLength === -4 ? Math.min(vMaxSalt, defSalt) : vMaxSalt;
      const vAuto = saltLength === -2 || saltLength === -4;
      let salt;
      if (saltLength === undefined) {
        salt = (typeof vdet0.saltLength === "number" ? vdet0.saltLength : defSalt);
      } else if (!vAuto) {
        try {
          salt = __resolvePssSalt(saltLength, defSalt, vMaxSalt);
        } catch (e) {
          if (e !== null && typeof e === "object" && e.code === "ERR_OSSL_RSA_DATA_TOO_LARGE_FOR_KEY_SIZE") return false;
          throw e;
        }
      }
      // 10f crypto六轮：验签侧 MGF1 自动切换（同 sign 侧；验不过回 false 不抛）。
      const vdet = keyObj.__detail ?? {};
      const vmgf = typeof vdet.mgf1HashAlgorithm === "string" ? __normHashName(vdet.mgf1HashAlgorithm) : null;
      if (vmgf !== null && vmgf !== hash) {
        if (det === null) return false;
        let em;
        try {
          em = Buffer.from(__cryptCall(() => __wjs2_rsa_raw(pubDer, Buffer.from(sigB), 0)));
        } catch {
          return false;
        }
        const mHash = __dgstBytes(hash, dataB);
        if (vAuto) {
          for (let s = 0; s <= vAutoCap; s++) {
            try {
              if (__emsaPssVerify(hash, vmgf, det.modulusLength - 1, mHash, em, s)) return true;
            } catch {}
          }
          return false;
        }
        return __emsaPssVerify(hash, vmgf, det.modulusLength - 1, mHash, em,
          saltLength === undefined ? undefined : salt);
      }
      if (vAuto) {
        // native salt 上限 512（超限为结构错；真机大键长 salt 另案记档）。
        const vCap = Math.min(vAutoCap, 512);
        for (let s = 0; s <= vCap; s++) {
          if (__cryptCall(() => __wjs2_pss_verify(hash, s, pubDer, sigB, dataB))) return true;
        }
        return false;
      }
      return __cryptCall(() => __wjs2_pss_verify(hash, salt, pubDer, sigB, dataB));
    }
    return __cryptCall(() => __wjs2_rsa_verify(hash, pubDer, sigB, dataB));
  }
  if (kt === "ec") {
    const curve = keyObj.__detail.namedCurve;
    const size = __curveSize(curve);
    // 10f crypto二轮：签名形态错回 false，不抛（空签名套件点名）。
    let raw;
    try {
      raw = (dsaEncoding ?? "der") === "der" ? __derToRawSig(sigB, size) : sigB;
    } catch {
      return false;
    }
    if (raw.length !== 2 * size) return false;
    return __cryptCall(() => __wjs2_ecdsa_verify(curve, hash, pubDer, raw, dataB));
  }
  if (kt === "dsa") {
    const envStr = Buffer.from(keyObj.__material).toString("utf8");
    const der = (dsaEncoding ?? "der") === "der" ? sigB : __rawToDerSig(sigB);
    return __cryptCall(() => __wjs2_dsa_verify(hash, envStr, der, dataB));
  }
  if (kt === "x25519" || kt === "x448") __osslKeytypeError();
  const err = new Error(`verify not supported for ${kt}`);
  err.code = "ERR_NOT_SUPPORTED";
  throw err;
}
// node 口径（C++ 键解析/错误构造在错误对象上 `=` 赋值 library——原型链用户
// setter 透传；sign 两件 'bye, bye' 套件钉住）：仅当链上存在 library setter
// 才赋值（抛 setter 错）；余者无痕（成功路径/无 setter 全不碰，valid-key 探针
// 钉住；opensslErrorStack 永不 `=` 触碰，case2 钉住）。
function __fireKeyLibrarySetter(e) {
  if (e === null || (typeof e !== "object" && typeof e !== "function")) return;
  try {
    let o = e;
    let fire = false;
    while (o !== null) {
      const d = Object.getOwnPropertyDescriptor(o, "library");
      if (d !== undefined) {
        fire = typeof d.set === "function";
        break;
      }
      o = Object.getPrototypeOf(o);
    }
    if (fire) e.library = e.library;
  } catch (se) {
    throw se;
  }
}
function __keyArg(key, what) {
  if (__isKeyObject(key)) return key;
  if (typeof key === "string" || key instanceof Uint8Array || key instanceof ArrayBuffer || ArrayBuffer.isView(key)) {
    try {
      return createPrivateKey(key);
    } catch (e) {
      __fireKeyLibrarySetter(e);
      try {
        return createPublicKey(key);
      } catch (e2) {
        __fireKeyLibrarySetter(e2);
        throw e2;
      }
    }
  }
  if (typeof key === "object" && key !== null) {
    const inner = key.key ?? key;
    if (__isKeyObject(inner)) return inner;
    // 真机口径（preparePrivateKey：无 .key 即 ARG_TYPE；sign-verify 套件钉住）；
    // 裸 JWK/空对象不直认（与真机一致，裸 JWK 须经 {key, format} 包装）。
    if (key.key === undefined || key.key === null) {
      const err = new TypeError(`The "${what}" argument must be a KeyObject or key material`);
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    try {
      return createPrivateKey(key);
    } catch (e) {
      __fireKeyLibrarySetter(e);
      try {
        return createPublicKey(key);
      } catch (e2) {
        __fireKeyLibrarySetter(e2);
        throw e2;
      }
    }
  }
  const err = new TypeError(`The "${what}" argument must be a KeyObject or key material`);
  err.code = "ERR_INVALID_ARG_TYPE";
  throw err;
}
export function sign(alg, data, key, callback) {
  // 真机口径：null/undefined 键即 SIGN_KEY_REQUIRED（sign-verify 395 行钉住）。
  if (key === null || key === undefined) {
    const err = new Error("No key provided to sign");
    err.code = "ERR_CRYPTO_SIGN_KEY_REQUIRED";
    throw err;
  }
  // 10f crypto二轮：callback 异步形（真机口径；同步核复用）。
  if (typeof callback === "function") {
    queueMicrotask(() => {
      try { callback(null, sign(alg, data, key)); }
      catch (e) { callback(e); }
    });
    return undefined;
  }
  const k = __keyArg(key, "key");
  const opts = (typeof key === "object" && key !== null && !(key instanceof Uint8Array) && !__isKeyObject(key)) ? key : {};
  const out = __signCore(alg, data, k, opts.dsaEncoding, opts.saltLength, opts.padding);
  return Buffer.from(out);
}
export function verify(alg, data, key, signature, callback) {
  // 10f crypto二轮：callback 异步形（真机口径；同步核复用）。
  if (typeof callback === "function") {
    queueMicrotask(() => {
      try { callback(null, verify(alg, data, key, signature)); }
      catch (e) { callback(e); }
    });
    return undefined;
  }
  // 真机口径：签名先验（key 解析前；sign-verify 套件 441 行钉住）。
  const sigB0 = __cryptBytes(signature, "signature");
  const k = __keyArg(key, "key");
  const opts = (typeof key === "object" && key !== null && !(key instanceof Uint8Array) && !__isKeyObject(key)) ? key : {};
  return __verifyCore(alg, data, k, sigB0, opts.dsaEncoding, opts.saltLength, opts.padding);
}
class SignImpl {
  constructor(alg, options) {
    // 真机口径：算法须字符串（sign-verify 套件 407 行钉住；ed25519 仍走后门）。
    if (typeof alg !== "string") {
      const err = new TypeError(`The "algorithm" argument must be of type string.${__argTypeHelper(alg)}`);
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    if (alg !== null && __normHashName(alg) === undefined && alg !== "ed25519") {
      const err = new Error("Invalid digest");
      err.code = "ERR_CRYPTO_INVALID_DIGEST";
      throw err;
    }
    if (alg === "ed25519") {
      const err = new Error("Invalid digest");
      err.code = "ERR_CRYPTO_INVALID_DIGEST";
      throw err;
    }
    this.__alg = alg;
    this.__parts = [];
  }
  update(data, enc) {
    this.__parts.push(__cryptBytes(data, "data", enc));
    return this;
  }
  // 真机口径：Sign/Verify 具流形（write/update 追加，end 写末块；sign-verify
  // 套件 s1stream.end 行钉住）。
  write(data, enc) { return this.update(data, enc); }
  end(data, enc) {
    if (data !== undefined) this.update(data, enc);
    return this;
  }
  // 流兼容垫片（sign-verify 套件 _write 行钉住；同步校验先于回调）。
  _write(data, enc, cb) {
    this.update(data, enc);
    if (typeof cb === "function") cb();
  }
  sign(key, ...rest) {
    if (key === null || key === undefined) {
      const err = new Error("No key provided to sign");
      err.code = "ERR_CRYPTO_SIGN_KEY_REQUIRED";
      throw err;
    }
    // 10f crypto二轮：rest 串 = 输出编码（真机口径；dsaEncoding/saltLength 只收
    // key-options 对象）；key-options 整体透传（passphrase 同）。
    // 显式 null 不被 ?? 洗掉（真机口径：null 即 invalid，sign-verify 套件钉住）。
    const pickSignOpt = (a, b) => (a !== undefined ? a : b);
    let outputEncoding, dsaEncoding, saltLength;
    for (const r of rest) {
      if (typeof r === "string") outputEncoding = r;
      else if (typeof r === "number") saltLength = r;
      else if (r && typeof r === "object") { dsaEncoding = pickSignOpt(r.dsaEncoding, dsaEncoding); saltLength = pickSignOpt(r.saltLength, saltLength); }
    }
    const flat = __joinParts(this.__parts);
    if (typeof key === "object" && key !== null && !(key instanceof Uint8Array) && !__isKeyObject(key) && !ArrayBuffer.isView(key)) {
      const k = __keyArg(key, "key");
      const out = __signCore(this.__alg, flat, k, pickSignOpt(key.dsaEncoding, dsaEncoding), pickSignOpt(key.saltLength, saltLength), key.padding);
      const buf = Buffer.from(out);
      // node 口径（encode）：缺省/'buffer'（严格大小写）即回 Buffer。
      if (outputEncoding === undefined || outputEncoding === "buffer") return buf;
      return buf.toString(outputEncoding);
    }
    const out = __signCore(this.__alg, flat, __keyArg(key, "key"), dsaEncoding, saltLength, undefined);
    const buf = Buffer.from(out);
    if (outputEncoding === undefined || outputEncoding === "buffer") return buf;
    return buf.toString(outputEncoding);
  }
}
class VerifyImpl {
  constructor(alg, options) {
    if (typeof alg !== "string") {
      const err = new TypeError(`The "algorithm" argument must be of type string.${__argTypeHelper(alg)}`);
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    if (alg !== null && __normHashName(alg) === undefined && alg !== "ed25519") {
      const err = new Error("Invalid digest");
      err.code = "ERR_CRYPTO_INVALID_DIGEST";
      throw err;
    }
    if (alg === "ed25519") {
      const err = new Error("Invalid digest");
      err.code = "ERR_CRYPTO_INVALID_DIGEST";
      throw err;
    }
    this.__alg = alg;
    this.__parts = [];
  }
  update(data, enc) {
    this.__parts.push(__cryptBytes(data, "data", enc));
    return this;
  }
  // 真机口径：同 Sign（流形 write/end）。
  write(data, enc) { return this.update(data, enc); }
  end(data, enc) {
    if (data !== undefined) this.update(data, enc);
    return this;
  }
  // 流兼容垫片（sign-verify 套件 _write 行钉住；同步校验先于回调）。
  _write(data, enc, cb) {
    this.update(data, enc);
    if (typeof cb === "function") cb();
  }
  verify(key, signature, ...rest) {
    // 10f crypto二轮：rest 串 = 签名编码（真机口径；余同 sign）。
    // 显式 null 不被 ?? 洗掉（同 sign）。
    const pickVerifyOpt = (a, b) => (a !== undefined ? a : b);
    let signatureEncoding, dsaEncoding, saltLength;
    for (const r of rest) {
      if (typeof r === "string") signatureEncoding = r;
      else if (typeof r === "number") saltLength = r;
      else if (r && typeof r === "object") { dsaEncoding = pickVerifyOpt(r.dsaEncoding, dsaEncoding); saltLength = pickVerifyOpt(r.saltLength, saltLength); }
    }
    const flat = __joinParts(this.__parts);
    // 真机口径：签名先验（key 解析前；sign-verify 套件 441 行钉住）。
    const sigB = signatureEncoding !== undefined ? Buffer.from(String(signature), signatureEncoding) : __cryptBytes(signature, "signature");
    if (typeof key === "object" && key !== null && !(key instanceof Uint8Array) && !__isKeyObject(key) && !ArrayBuffer.isView(key)) {
      const k = __keyArg(key, "key");
      return __verifyCore(this.__alg, flat, k, sigB, pickVerifyOpt(key.dsaEncoding, dsaEncoding), pickVerifyOpt(key.saltLength, saltLength), key.padding);
    }
    return __verifyCore(this.__alg, flat, __keyArg(key, "key"), sigB, dsaEncoding, saltLength, undefined);
  }
}
export function createSign(alg, options) { return new SignImpl(alg, options); }
export function createVerify(alg, options) { return new VerifyImpl(alg, options); }
// P2 crypto三件簇：真机 `crypto.Sign/Verify(...)` 可无 new 调用（legacy 函数形，
// 无废弃警告；Cipheriv/Decipheriv 同款，见 crypto_cipher.js）。
function Sign(...args) { return new SignImpl(...args); }
Object.setPrototypeOf(Sign, SignImpl);
Sign.prototype = SignImpl.prototype;
Sign.prototype.constructor = Sign;
function Verify(...args) { return new VerifyImpl(...args); }
Object.setPrototypeOf(Verify, VerifyImpl);
Verify.prototype = VerifyImpl.prototype;
Verify.prototype.constructor = Verify;
// 10f crypto二轮：混合 OAEP 编解码（oaepHash ≠ mgf1Hash；几何经真机预言机定案：
// 种子长取 oaep 哈希长、掩码走 mgf1Hash、界为 k-2*hLen-2；双向真机交叉见黑盒）。
const __OAEP_HLEN = { "SHA-1": 20, "SHA-224": 28, "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 };
function __oaepTooLarge() {
  const err = new Error("error:0200006E:rsa routines::data too large for key size");
  err.code = "ERR_OSSL_RSA_DATA_TOO_LARGE_FOR_KEY_SIZE";
  throw err;
}
function __oaepDecodingError() {
  const err = new Error("error:02000079:rsa routines::oaep decoding error");
  err.code = "ERR_OSSL_RSA_OAEP_DECODING_ERROR";
  throw err;
}
function __oaepMixedEncode(oaepName, mgfName, k, data, labelB) {
  const hLen = __OAEP_HLEN[oaepName];
  if (data.length > k - 2 * hLen - 2) __oaepTooLarge();
  const lHash = __dgstBytes(oaepName, labelB ?? Buffer.alloc(0));
  const psLen = k - data.length - 2 * hLen - 2;
  const db = Buffer.concat([lHash, Buffer.alloc(psLen), Buffer.from([1]), Buffer.from(data)]);
  const seed = __randFill(new Uint8Array(hLen));
  const dbMask = __mgf1Bytes(mgfName, seed, db.length);
  const maskedDB = Buffer.from(db.map((b, i) => b ^ dbMask[i]));
  const seedMask = __mgf1Bytes(mgfName, maskedDB, hLen);
  const maskedSeed = Buffer.from(seed.map((b, i) => b ^ seedMask[i]));
  return Buffer.concat([Buffer.from([0]), maskedSeed, maskedDB]);
}
function __oaepMixedDecode(oaepName, mgfName, k, em, labelB) {
  const hLen = __OAEP_HLEN[oaepName];
  if (em.length !== k || em[0] !== 0) __oaepDecodingError();
  const maskedSeed = em.slice(1, 1 + hLen), maskedDB = em.slice(1 + hLen);
  const seedMask = __mgf1Bytes(mgfName, maskedDB, hLen);
  const seed = Buffer.from(maskedSeed.map((b, i) => b ^ seedMask[i]));
  const dbMask = __mgf1Bytes(mgfName, seed, maskedDB.length);
  const db = Buffer.from(maskedDB.map((b, i) => b ^ dbMask[i]));
  const lHash = __dgstBytes(oaepName, labelB ?? Buffer.alloc(0));
  if (!db.slice(0, hLen).equals(lHash)) __oaepDecodingError();
  const rest = db.slice(hLen);
  const one = rest.indexOf(1);
  if (one < 0 || !rest.slice(0, one).equals(Buffer.alloc(one))) __oaepDecodingError();
  return rest.slice(one + 1);
}
function __dgstBytes(name, data) {
  return createHash(name).update(Buffer.from(data)).digest();
}
function __mgf1Bytes(name, seed, outLen) {
  const hLen = __OAEP_HLEN[name];
  let out = Buffer.alloc(0);
  let counter = 0;
  while (out.length < outLen) {
    const c = Buffer.alloc(4);
    c.writeUInt32BE(counter++);
    out = Buffer.concat([out, __dgstBytes(name, Buffer.concat([Buffer.from(seed), c]))]);
  }
  return out.slice(0, outLen);
}
// 10f crypto六轮：EMSA-PSS 手组（MGF1 哈希可与消息摘要不同；RFC4055 §3.1/3.3
// 自动切换。MGF1/摘要走既有 __mgf1Bytes/__dgstBytes，模幂走 __wjs2_rsa_raw；
// 零新 native。侧信道记档：非恒定时间实现，与混合 OAEP 手写同口径）。
// 编码成功回 Buffer EM；长度不足回 null（调用方按 OpenSSL 口径报错）。
function __emsaPssEncode(msgName, mgfName, emBits, mHash, salt) {
  const hLen = __OAEP_HLEN[msgName];
  const emLen = Math.ceil(emBits / 8);
  if (emLen < hLen + salt.length + 2) return null;
  const h = __dgstBytes(msgName, Buffer.concat([Buffer.alloc(8), Buffer.from(mHash), Buffer.from(salt)]));
  const db = Buffer.concat([Buffer.alloc(emLen - salt.length - hLen - 2), Buffer.from([1]), Buffer.from(salt)]);
  const dbMask = __mgf1Bytes(mgfName, h, db.length);
  for (let i = 0; i < db.length; i++) db[i] ^= dbMask[i];
  db[0] &= 0xff >> (8 * emLen - emBits);
  return Buffer.concat([db, Buffer.from(h), Buffer.from([0xbc])]);
}
function __emsaPssVerify(msgName, mgfName, emBits, mHash, em, wantSaltLen) {
  const hLen = __OAEP_HLEN[msgName];
  const emLen = Math.ceil(emBits / 8);
  if (em.length !== emLen || em[emLen - 1] !== 0xbc) return false;
  const topBits = 8 * emLen - emBits;
  if (topBits > 0 && (em[0] & (0xff << (8 - topBits))) !== 0) return false;
  const maskedDB = em.slice(0, emLen - hLen - 1), h = em.slice(emLen - hLen - 1, emLen - 1);
  const dbMask = __mgf1Bytes(mgfName, h, maskedDB.length);
  const db = Buffer.from(maskedDB.map((b, i) => b ^ dbMask[i]));
  db[0] &= 0xff >> topBits;
  let salt;
  if (wantSaltLen === undefined) {
    let i = 0;
    while (i < db.length && db[i] === 0) i++;
    if (i >= db.length || db[i] !== 1) return false;
    salt = db.slice(i + 1);
  } else {
    const s = Number(wantSaltLen);
    const psLen = emLen - hLen - s - 2;
    if (psLen < 0 || !db.slice(0, psLen).equals(Buffer.alloc(psLen)) || db[psLen] !== 1) return false;
    salt = db.slice(psLen + 1);
  }
  const h2 = __dgstBytes(msgName, Buffer.concat([Buffer.alloc(8), Buffer.from(mHash), Buffer.from(salt)]));
  return Buffer.from(h).equals(Buffer.from(h2));
}
