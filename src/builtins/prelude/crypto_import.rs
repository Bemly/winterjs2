//! WebCrypto 导入面（digest/generateKey/importKey；§0.9 拆分：crypto 纯搬移，concat 字节恒等）。
pub const CRYPTO_IMPORT_JS: &str = r#"
globalThis.crypto = {
  getRandomValues(view) {
    if (this !== globalThis.crypto) throw __wjs2BrandErr();
    // random 套件逐字（真机实测）：非视图 TypeError；Float32/64/DataView 即
    // TypeMismatchError(17) DOMException；空整数视图照常。
    if (!ArrayBuffer.isView(view)) {
      throw new TypeError("getRandomValues requires a typed array view");
    }
    if (view instanceof Float32Array || view instanceof Float64Array || view instanceof DataView) {
      throw new DOMException("The data argument must be an integer-type TypedArray", "TypeMismatchError");
    }
    // random 套件：超 64KiB 即 QuotaExceededError（真机逐项）。
    if (view.byteLength > 65536) {
      throw new QuotaExceededError(`The requested length exceeds ${view.byteLength} bytes`);
    }
    // random 套件：BigInt 视图经同缓冲 Uint8 复用填充（native 缺 BigInt 标记）。
    if (view instanceof BigInt64Array || view instanceof BigUint64Array) {
      __wjs2_fill_random(new Uint8Array(view.buffer, view.byteOffset, view.byteLength));
      return view;
    }
    __wjs2_fill_random(view);
    return view;
  },
  randomUUID() {
    if (this !== globalThis.crypto) throw __wjs2BrandErr();
    return __wjs2_random_uuid();
  },
  subtle: {
    digest(algorithm, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      // cSHAKE 面（digest 套件：无定制即 SHAKE；outputLength 为比特）。
      const algNameRaw = typeof algorithm === "string" ? algorithm : algorithm?.name;
      const algUp = String(algNameRaw ?? "").toUpperCase();
      if (algUp === "CSHAKE128" || algUp === "CSHAKE256") {
        const params = (typeof algorithm === "object" && algorithm !== null) ? algorithm : {};
        const outLen = Number(params.outputLength);
        if (!Number.isInteger(outLen) || outLen < 0 || outLen % 8 !== 0) {
          throw __wjs2_domErr("NotSupportedError", "Invalid CShakeParams outputLength");
        }
        const fnLen = params.functionName === undefined ? 0 : __wjs2_keyBytes(params.functionName).length;
        if (fnLen > 0) throw __wjs2_domErr("NotSupportedError", "Unsupported CShakeParams functionName");
        const cuLen = params.customization === undefined ? 0 : __wjs2_keyBytes(params.customization).length;
        if (cuLen > 0) throw __wjs2_domErr("NotSupportedError", "Unsupported CShakeParams customization");
        let view = data;
        if (view instanceof ArrayBuffer) view = new Uint8Array(view);
        else if (ArrayBuffer.isView(view) && !(view instanceof Uint8Array)) {
          view = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
        } else if (!(view instanceof Uint8Array)) {
          const e = new TypeError("digest data requires a Uint8Array");
          e.code = "ERR_INVALID_ARG_TYPE";
          throw e;
        }
        const out = __wjs2_shake_digest(algUp === "CSHAKE128" ? "128" : "256", outLen / 8, view);
        return new Uint8Array(out).buffer;
      }
      const name = __wjs2_normHash(algNameRaw);
      let view = data;
      if (view instanceof ArrayBuffer) view = new Uint8Array(view);
      else if (ArrayBuffer.isView(view) && !(view instanceof Uint8Array)) {
        view = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
      } else if (!(view instanceof Uint8Array)) {
        const e = new TypeError("digest data requires a Uint8Array");
        e.code = "ERR_INVALID_ARG_TYPE";
        throw e;
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
      if (usages.length === 0) {
        throw __wjs2_domErr("SyntaxError", "Usages cannot be empty when creating a key.");
      }
      if (name === "AES-GCM") {
        const length = Number(alg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw new Error("NotSupportedError: AES-GCM length must be 128/192/256");
        const bytes = new Uint8Array(length / 8);
        crypto.getRandomValues(bytes);
        return __wjs2_makeKey({ name: "AES-GCM", length }, bytes, usages, !!extractable);
      }
      if (name === "AES-CBC") {
        const length = Number(alg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw __wjs2_domErr("NotSupportedError", "AES-CBC length must be 128/192/256");
        const bytes = new Uint8Array(length / 8);
        crypto.getRandomValues(bytes);
        return __wjs2_makeKey({ name: "AES-CBC", length }, bytes, usages, !!extractable);
      }
      if (name === "AES-CTR") {
        const length = Number(alg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw __wjs2_domErr("NotSupportedError", "AES-CTR length must be 128/192/256");
        const bytes = new Uint8Array(length / 8);
        crypto.getRandomValues(bytes);
        return __wjs2_makeKey({ name: "AES-CTR", length }, bytes, usages, !!extractable);
      }
      if (name === "AES-KW") {
        const length = Number(alg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw __wjs2_domErr("NotSupportedError", "AES-KW length must be 128/192/256");
        const bytes = new Uint8Array(length / 8);
        crypto.getRandomValues(bytes);
        return __wjs2_makeKey({ name: "AES-KW", length }, bytes, usages, !!extractable);
      }
      if (name === "HMAC") {
        if (alg?.hash === undefined) {
          const e = new TypeError("HMAC generateKey needs hash");
          e.code = "ERR_MISSING_OPTION";
          throw e;
        }
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
        const mkPub = __wjs2_makeKey({ name }, pub, edPubU, !!extractable, "public");
        const mkPriv = __wjs2_makeKey({ name }, seed, edPrivU, !!extractable, "private");
        return { publicKey: mkPub, privateKey: mkPriv };
      }
      if (name === "X25519") {
        const priv = __wjs2_x_generate();
        const pub = __wjs2_x_public(priv);
        const { pubU: xPubU, privU: xPrivU } = __wjs2_splitUsages(usages);
        const mkPub = __wjs2_makeKey({ name }, pub, xPubU, !!extractable, "public");
        const mkPriv = __wjs2_makeKey({ name }, priv, xPrivU, !!extractable, "private");
        return { publicKey: mkPub, privateKey: mkPriv };
      }
      if (name === "ED448") {
        throw new Error(`NotSupportedError: generateKey ${name} needs follow-up`);
      }
      throw __wjs2_domErr("NotSupportedError", "Unrecognized algorithm name");
      })();
    },
    importKey(format, keyData, alg, extractable, usages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      // raw-secret 即 raw（secret 钥；OCB 等用，aead 套件口径）。
      if (format === "raw-secret") format = "raw";
      const name = typeof alg === "string" ? alg.toUpperCase() : String(alg?.name ?? "").toUpperCase();
      usages = [...(usages ?? [])].map(String);
      const needHash = name === "HMAC" ? __wjs2_normHash(alg?.hash) : undefined;
      if (name === "RSASSA-PKCS1-V1_5" || name === "RSA-OAEP" || name === "RSA-PSS") {
        const hash = __wjs2_normHash(alg?.hash ?? "SHA-256");
        const isPrivFmt = format === "pkcs8" || (format === "jwk" && keyData !== null && typeof keyData === "object" && typeof keyData.d === "string");
        if (isPrivFmt && usages.length === 0) {
          throw __wjs2_domErr("SyntaxError", "Usages cannot be empty when importing a private key.");
        }
        const badData = () => __wjs2_domErr("DataError", "Invalid keyData");
        if (format === "jwk") {
          if (!keyData || keyData.kty !== "RSA" || typeof keyData.n !== "string" || typeof keyData.e !== "string") {
            throw badData();
          }
          __wjs2_checkJwkMeta(keyData,
            name === "RSASSA-PKCS1-V1_5" ? { "SHA-256": ["RS256"], "SHA-384": ["RS384"], "SHA-512": ["RS512"] }[hash]
            : name === "RSA-PSS" ? { "SHA-256": ["PS256"], "SHA-384": ["PS384"], "SHA-512": ["PS512"] }[hash]
            : { "SHA-256": ["RSA-OAEP-256"], "SHA-384": ["RSA-OAEP-384"], "SHA-512": ["RSA-OAEP-512"] }[hash],
            name !== "RSA-OAEP");
          __wjs2_checkKeyOps(keyData, usages, name, typeof keyData.d === "string" ? "private" : "public");
          let n, e;
          try {
            n = __wjs2_b64urlDecode(keyData.n); e = __wjs2_b64urlDecode(keyData.e);
          } catch { throw badData(); }
          const expBytes = e.slice();
          if (typeof keyData.d === "string") {
            let d, privDer;
            try {
              d = __wjs2_b64urlDecode(keyData.d);
              privDer = __wjs2_rsa_import_priv(n, e, d);
            } catch { throw badData(); }
            const keyAlg = { name, modulusLength: n.length * 8, publicExponent: expBytes, hash };
            return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
          }
          let pubDer;
          try {
            pubDer = __wjs2_rsa_import_pub(n, e);
          } catch { throw badData(); }
          const keyAlg = { name, modulusLength: n.length * 8, publicExponent: expBytes, hash };
          return __wjs2_makeKey(keyAlg, pubDer, usages, !!extractable, "public");
        }
        if (format === "pkcs8") {
          let privDer, pubDer, parts, n, e;
          try {
            privDer = __wjs2_keyBytes(keyData);
            // PKCS#8 自验证（解析失败即 DataError；公钥顺带导出供 algorithm）。
            pubDer = __wjs2_rsa_public(privDer);
            parts = JSON.parse(__wjs2_rsa_jwk(privDer, pubDer));
            n = __wjs2_b64urlDecode(parts.n); e = __wjs2_b64urlDecode(parts.e);
          } catch { throw badData(); }
          const keyAlg = { name, modulusLength: n.length * 8, publicExponent: e.slice(), hash };
          return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
        }
        if (format === "spki") {
          let pubDer, parts, n, e;
          try {
            pubDer = __wjs2_keyBytes(keyData);
            parts = JSON.parse(__wjs2_rsa_jwk_pub(pubDer));
            n = __wjs2_b64urlDecode(parts.n); e = __wjs2_b64urlDecode(parts.e);
          } catch { throw badData(); }
          const keyAlg = { name, modulusLength: n.length * 8, publicExponent: e.slice(), hash };
          return __wjs2_makeKey(keyAlg, pubDer, usages, !!extractable, "public");
        }
        throw new Error(`NotSupportedError: importKey ${format} for RSA needs pkcs8/spki/jwk`);
      }
      if (name === "ECDSA" || name === "ECDH") {
        const curve = __wjs2_normCurve(alg?.namedCurve);
        const keyAlg = { name, namedCurve: curve };
        // 私钥空用量即 SyntaxError（真机逐字；公钥面不限）。
        const isPrivFmt = format === "pkcs8" || (format === "jwk" && keyData !== null && typeof keyData === "object" && typeof keyData.d === "string");
        if (isPrivFmt && usages.length === 0) {
          throw __wjs2_domErr("SyntaxError", "Usages cannot be empty when importing a private key.");
        }
        // 素材坏即 DataError（真机一律 Invalid keyData；crv 失配文案保留）。
        const badData = () => __wjs2_domErr("DataError", "Invalid keyData");
        if (format === "jwk") {
          if (!keyData || keyData.kty !== "EC" || typeof keyData.x !== "string" || typeof keyData.y !== "string") {
            throw badData();
          }
          if (!keyData.crv) throw badData();
          if (keyData.crv !== curve) throw new Error('JWK "crv" does not match the requested algorithm');
          __wjs2_checkJwkMeta(keyData, name === "ECDSA" ? { "P-256": ["ES256"], "P-384": ["ES384"], "P-521": ["ES512"] }[curve] : ["ECDH-ES"], name === "ECDSA");
          __wjs2_checkKeyOps(keyData, usages, name, typeof keyData.d === "string" ? "private" : "public");
          let x, y;
          try {
            x = __wjs2_b64urlDecode(keyData.x); y = __wjs2_b64urlDecode(keyData.y);
          } catch { throw badData(); }
          if (typeof keyData.d === "string") {
            let privDer;
            try {
              privDer = __wjs2_ec_import_priv(curve, __wjs2_b64urlDecode(keyData.d));
            } catch { throw badData(); }
            // 公钥一致性：JWK 的 x/y 须与 d 对应（防混入）。
            let expect, got;
            try {
              expect = __wjs2_ec_public(curve, privDer);
              got = __wjs2_ec_import_pub(curve, x, y);
            } catch { throw badData(); }
            const same = expect.length === got.length && expect.every((b, i) => b === got[i]);
            if (!same) throw badData();
            return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
          }
          let pubBytes;
          try {
            pubBytes = __wjs2_ec_import_pub(curve, x, y);
          } catch { throw badData(); }
          return __wjs2_makeKey(keyAlg, pubBytes, usages, !!extractable, "public");
        }
        if (format === "pkcs8") {
          let privDer;
          try {
            privDer = __wjs2_keyBytes(keyData);
            // PKCS#8 自验证（曲线错/损坏即 DataError）。
            __wjs2_ec_public(curve, privDer);
          } catch { throw badData(); }
          return __wjs2_makeKey(keyAlg, privDer, usages, !!extractable, "private");
        }
        if (format === "spki") {
          let pubDer;
          try {
            pubDer = __wjs2_keyBytes(keyData);
            // SPKI 自验证 + 压缩点归一（45859 口径：导出恒非压缩）。
            pubDer = __wjs2_ec_spki_normalize(curve, pubDer);
          } catch { throw badData(); }
          return __wjs2_makeKey(keyAlg, pubDer, usages, !!extractable, "public");
        }
        if (format === "raw") {
          let v;
          try {
            v = __wjs2_keyBytes(keyData);
          } catch { throw badData(); }
          // 压缩点即解压（raw 45859 同款；0x02/0x03 + x）。
          if (v.length > 1 && (v[0] === 0x02 || v[0] === 0x03)) {
            const size = v.length - 1;
            if (![32, 48, 66].includes(size)) throw badData();
            const c2 = size === 32 ? "P-256" : size === 48 ? "P-384" : "P-521";
            if (c2 !== curve) throw badData();
            let pubDer;
            try {
              pubDer = __wjs2_ec_import_compressed(curve, v);
            } catch { throw badData(); }
            return __wjs2_makeKey(keyAlg, pubDer, usages, !!extractable, "public");
          }
          if (v.length < 1 || v[0] !== 0x04) throw badData();
          const size = (v.length - 1) / 2;
          if (![32, 48, 66].includes(size) || 1 + 2 * size !== v.length) throw badData();
          const x = v.slice(1, 1 + size), y = v.slice(1 + size);
          const c2 = size === 32 ? "P-256" : size === 48 ? "P-384" : "P-521";
          if (c2 !== curve) throw badData();
          let pubBytes;
          try {
            pubBytes = __wjs2_ec_import_pub(curve, x, y);
          } catch { throw badData(); }
          return __wjs2_makeKey(keyAlg, pubBytes, usages, !!extractable, "public");
        }
        throw new Error(`NotSupportedError: importKey ${format} for EC needs jwk/pkcs8/spki/raw`);
      }
      if (name === "ED25519" || name === "X25519") {
        // JWK crv 用混合大小写（RFC 8037；内部 name 全大写，不外泄）。
        // algorithm 面无 namedCurve（OKP 真机口径，与 generateKey 一致）。
        const crv = name === "ED25519" ? "Ed25519" : "X25519";
        const keyAlg = { name };
        const isPrivFmt = format === "pkcs8" || (format === "jwk" && keyData !== null && typeof keyData === "object" && typeof keyData.d === "string");
        if (isPrivFmt && usages.length === 0) {
          throw __wjs2_domErr("SyntaxError", "Usages cannot be empty when importing a private key.");
        }
        const badData = () => __wjs2_domErr("DataError", "Invalid keyData");
        if (format === "jwk") {
          if (!keyData || keyData.kty !== "OKP" || typeof keyData.x !== "string") {
            throw badData();
          }
          if (!keyData.crv) throw badData();
          if (keyData.crv !== crv) throw new Error('JWK "crv" Parameter and algorithm name mismatch');
          __wjs2_checkJwkMeta(keyData, name === "ED25519" ? ["EdDSA"] : null, name === "ED25519");
          __wjs2_checkKeyOps(keyData, usages, name, typeof keyData.d === "string" ? "private" : "public");
          let x;
          try {
            x = __wjs2_b64urlDecode(keyData.x);
          } catch { throw badData(); }
          if (x.length !== 32) throw badData();
          if (typeof keyData.d === "string") {
            let seed;
            try {
              seed = __wjs2_b64urlDecode(keyData.d);
            } catch { throw badData(); }
            if (seed.length !== 32) throw badData();
            // 私钥一致性：JWK 的 x 须与 d 对应（防混入）。
            const expect = name === "ED25519" ? __wjs2_ed_public(seed) : __wjs2_x_public(seed);
            const same = expect.length === 32 && expect.every((b, i) => b === x[i]);
            if (!same) throw badData();
            return __wjs2_makeKey(keyAlg, seed, usages, !!extractable, "private");
          }
          return __wjs2_makeKey(keyAlg, x, usages, !!extractable, "public");
        }
        if (format === "raw") {
          let v;
          try {
            v = __wjs2_keyBytes(keyData);
          } catch { throw badData(); }
          if (v.length !== 32) throw badData();
          return __wjs2_makeKey(keyAlg, v, usages, !!extractable, "public");
        }
        if (format === "pkcs8") {
          let seed;
          try {
            seed = __wjs2_okp_seed_from_pkcs8(name, __wjs2_keyBytes(keyData));
          } catch { throw badData(); }
          return __wjs2_makeKey(keyAlg, seed, usages, !!extractable, "private");
        }
        if (format === "spki") {
          let publ;
          try {
            publ = __wjs2_okp_pub_from_spki(name, __wjs2_keyBytes(keyData));
          } catch { throw badData(); }
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
        if (![16, 24, 32].includes(bytes.length)) throw __wjs2_domErr("DataError", "Invalid key length");
        return __wjs2_makeKey({ name, length: bytes.length * 8 }, bytes, usages, !!extractable);
      }
      if (name === "AES-CBC") {
        if (![16, 24, 32].includes(bytes.length)) throw __wjs2_domErr("DataError", "Invalid key length");
        return __wjs2_makeKey({ name, length: bytes.length * 8 }, bytes, usages, !!extractable);
      }
      if (name === "AES-CTR") {
        if (![16, 24, 32].includes(bytes.length)) throw __wjs2_domErr("DataError", "Invalid key length");
        return __wjs2_makeKey({ name, length: bytes.length * 8 }, bytes, usages, !!extractable);
      }
      if (name === "AES-KW") {
        if (![16, 24, 32].includes(bytes.length)) throw __wjs2_domErr("DataError", "Invalid key length");
        return __wjs2_makeKey({ name, length: bytes.length * 8 }, bytes, usages, !!extractable);
      }
      if (name === "HMAC") {
        return __wjs2_makeKey({ name, hash: needHash, length: bytes.length * 8 }, bytes, usages, !!extractable);
      }
      if (name === "HKDF") {
        if (format !== "raw") throw __wjs2_domErr("NotSupportedError", `importKey ${format} for HKDF needs raw`);
        return __wjs2_makeKey({ name }, bytes, usages, !!extractable);
      }
      if (name === "PBKDF2") {
        if (format !== "raw") throw __wjs2_domErr("NotSupportedError", `importKey ${format} for PBKDF2 needs raw`);
        return __wjs2_makeKey({ name }, bytes, usages, !!extractable);
      }
      throw __wjs2_domErr("NotSupportedError", "Unrecognized algorithm name");
      })();
    },
"#;
