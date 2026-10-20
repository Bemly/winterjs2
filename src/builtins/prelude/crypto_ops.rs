//! WebCrypto 操作面（exportKey/加解密/派生/克隆；§0.9 拆分：crypto 纯搬移，concat 字节恒等）。
pub const CRYPTO_OPS_JS: &str = r#"
    exportKey(format, key) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      if (format === "raw-secret") format = "raw";
      if (format === "raw-public") format = "raw";
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("exportKey: not a CryptoKey");
      if (!st.extractable) throw __wjs2_domErr("InvalidAccessError", "key is not extractable");
      const aname = st.alg.name;
      if (aname === "RSASSA-PKCS1-V1_5" || aname === "RSA-OAEP" || aname === "RSA-PSS") {
        const hash = __wjs2_normHash(st.alg.hash ?? "SHA-256");
        const isPriv = st.kind === "private";
        const privDer = isPriv ? st.material : null;
        const pubDer = isPriv ? __wjs2_rsa_public(st.material) : st.material;
        if (format === "pkcs8") {
          if (!isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a private key");
          return st.material.slice().buffer;
        }
        if (format === "spki") {
          if (isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a public key");
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
              : { "SHA-256": "RSA-OAEP-256", "SHA-384": "RSA-OAEP-384", "SHA-512": "RSA-OAEP-512" }[hash] ?? "RSA-OAEP";
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
          if (!isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a private key");
          return st.material.slice().buffer;
        }
        if (format === "spki") {
          if (isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a public key");
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
          if (isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a public key");
          const parts = JSON.parse(__wjs2_ec_jwk_pub(curve, pubDer));
          const x = __wjs2_b64urlDecode(parts.x), y = __wjs2_b64urlDecode(parts.y);
          const out = new Uint8Array(1 + x.length + y.length);
          out[0] = 0x04; out.set(x, 1); out.set(y, 1 + x.length);
          return out.buffer;
        }
        throw new Error(`NotSupportedError: exportKey ${format} for EC needs pkcs8/spki/jwk/raw`);
      }
      if (aname === "ED25519" || aname === "X25519" || aname === "ED448") {
        const isPriv = st.kind === "private";
        const pubBytes = isPriv
          ? (aname === "ED25519" ? __wjs2_ed_public(st.material) : aname === "ED448" ? __wjs2_ed448_public(st.material) : __wjs2_x_public(st.material))
          : st.material;
        if (format === "pkcs8") {
          if (!isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a private key");
          return __wjs2_okp_pkcs8_from_seed(aname, st.material).buffer;
        }
        if (format === "spki") {
          if (isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a public key");
          return __wjs2_okp_spki_from_pub(aname, pubBytes).buffer;
        }
        if (format === "jwk") {
          const jwk = { kty: "OKP", crv: aname === "ED25519" ? "Ed25519" : aname === "ED448" ? "Ed448" : "X25519", x: __wjs2_b64urlEncode(pubBytes) };
          if (isPriv) jwk.d = __wjs2_b64urlEncode(st.material);
          // JWA 只给 Ed25519 定义了 "EdDSA"；X25519 无 alg（与 Node 一致，省略）。
          if (aname === "ED25519" || aname === "ED448") jwk.alg = "EdDSA";
          jwk.ext = true;
          jwk.key_ops = [...st.usages];
          return jwk;
        }
        if (format === "raw") {
          if (isPriv) throw __wjs2_domErr("InvalidAccessError", "Key must be a public key");
          return pubBytes.slice().buffer;
        }
        throw new Error(`NotSupportedError: exportKey ${format} for OKP needs pkcs8/spki/jwk/raw`);
      }
      if (format === "raw") return st.material.slice().buffer;
      if (format === "jwk") {
        // 各对称算法 JWK alg（真机逐项；无定义即省略）。
        let algTag;
        const an = st.alg.name;
        if (an === "AES-GCM") algTag = `A${st.alg.length}GCM`;
        else if (an === "AES-CBC") algTag = `A${st.alg.length}CBC`;
        else if (an === "AES-CTR") algTag = `A${st.alg.length}CTR`;
        else if (an === "AES-KW") algTag = `A${st.alg.length}KW`;
        else if (an === "CHACHA20-POLY1305") algTag = "C20P";
        else if (an === "HMAC") {
          const hn = __wjs2_normHash(st.alg.hash).split("-")[1];
          if (["1", "256", "384", "512"].includes(hn)) algTag = `HS${hn}`;
        }
        const jwk = { kty: "oct", k: __wjs2_b64urlEncode(st.material), ext: true, key_ops: [...st.usages] };
        if (algTag !== undefined) jwk.alg = algTag;
        return jwk;
      }
      throw new Error(`NotSupportedError: exportKey ${format} needs Phase 3 c-4`);
      })();
    },
    encrypt(algorithm, key, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("encrypt: not a CryptoKey");
      // wrong-alg 套件：算法先于用量（Key algorithm mismatch，真机逐项）。
      if ((typeof algorithm === "string" ? algorithm.toUpperCase() : String(algorithm?.name ?? "").toUpperCase()) !== String(st.alg.name ?? "").toUpperCase()) throw new Error("Key algorithm mismatch");
      __wjs2_needUsage(st, "encrypt");
      if (st.alg.name === "AES-GCM") {
        const p = __wjs2_aesParams(algorithm);
        // 任意 iv/截断 tag 走手工 J0 路径（encrypt-decrypt-aes 套件真机逐项）。
        const out = (p.iv.length === 12 && p.tagLength === 128)
          ? __wjs2_aesgcm_encrypt(st.material, p.iv, p.aad, __wjs2_dataBytes(data))
          : __wjs2_gcm_subtle(1, st.material, p.iv, p.aad ?? new Uint8Array(0), __wjs2_dataBytes(data), p.tagLength / 8);
        return out.buffer;
      }
      if (st.alg.name === "AES-CBC") {
        const iv = __wjs2_dataBytes(algorithm?.iv);
        if (iv.length !== 16) throw __wjs2_domErr("OperationError", "algorithm.iv must contain exactly 16 bytes");
        const out = __wjs2_aescbc_encrypt(st.material, iv, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "AES-CTR") {
        const counter = __wjs2_dataBytes(algorithm?.counter);
        if (counter.length !== 16) throw __wjs2_domErr("OperationError", "AES-CTR counter must be 16 bytes");
        const lenBits = Number(algorithm?.length ?? 128);
        if (!Number.isInteger(lenBits) || lenBits < 1 || lenBits > 128) {
          throw __wjs2_domErr("OperationError", "AES-CTR length must be between 1 and 128");
        }
        const out = __wjs2_aesctr_crypt(st.material, counter, lenBits, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "CHACHA20-POLY1305") {
        const iv = __wjs2_dataBytes(algorithm?.iv);
        if (iv.length !== 12) throw __wjs2_domErr("OperationError", "ChaCha20-Poly1305 iv must be 12 bytes");
        const tagLen = algorithm?.tagLength === undefined ? 128 : Number(algorithm.tagLength);
        if (tagLen !== 128) throw __wjs2_domErr("OperationError", `${algorithm?.tagLength} is not a valid ChaCha20-Poly1305 tag length`);
        const aad = algorithm?.additionalData === undefined ? new Uint8Array(0) : __wjs2_dataBytes(algorithm.additionalData);
        const out = __wjs2_chacha_encrypt(st.material, iv, aad, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "RSA-OAEP") {
        if (st.kind !== "public") throw new TypeError("encrypt: not an RSA public key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        const label = algorithm?.label === undefined ? undefined : __wjs2_dataBytes(algorithm.label);
        try {
          const out = __wjs2_rsa_encrypt(hash, st.material, __wjs2_dataBytes(data), label);
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      throw new TypeError("encrypt: unsupported key");
      })();
    },
    decrypt(algorithm, key, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("decrypt: not a CryptoKey");
      if ((typeof algorithm === "string" ? algorithm.toUpperCase() : String(algorithm?.name ?? "").toUpperCase()) !== String(st.alg.name ?? "").toUpperCase()) throw new Error("Key algorithm mismatch");
      __wjs2_needUsage(st, "decrypt");
      if (st.alg.name === "AES-GCM") {
        const p = __wjs2_aesParams(algorithm);
        // 认证失败即 OperationError（真机口径；native 纯 Error 形在此转，
        // 参数型错误原样透传）。
        try {
          const out = (p.iv.length === 12 && p.tagLength === 128)
            ? __wjs2_aesgcm_decrypt(st.material, p.iv, p.aad, __wjs2_dataBytes(data))
            : __wjs2_gcm_subtle(0, st.material, p.iv, p.aad ?? new Uint8Array(0), __wjs2_dataBytes(data), p.tagLength / 8);
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          // native 参数型错误（TypeError: 前缀）原样透传；余者皆认证失败。
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "AES-CBC") {
        const iv = __wjs2_dataBytes(algorithm?.iv);
        if (iv.length !== 16) throw __wjs2_domErr("OperationError", "algorithm.iv must contain exactly 16 bytes");
        try {
          const out = __wjs2_aescbc_decrypt(st.material, iv, __wjs2_dataBytes(data));
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "AES-CTR") {
        const counter = __wjs2_dataBytes(algorithm?.counter);
        if (counter.length !== 16) throw __wjs2_domErr("OperationError", "AES-CTR counter must be 16 bytes");
        const lenBits = Number(algorithm?.length ?? 128);
        if (!Number.isInteger(lenBits) || lenBits < 1 || lenBits > 128) {
          throw __wjs2_domErr("OperationError", "AES-CTR length must be between 1 and 128");
        }
        const out = __wjs2_aesctr_crypt(st.material, counter, lenBits, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "CHACHA20-POLY1305") {
        const iv = __wjs2_dataBytes(algorithm?.iv);
        if (iv.length !== 12) throw __wjs2_domErr("OperationError", "ChaCha20-Poly1305 iv must be 12 bytes");
        const tagLen = algorithm?.tagLength === undefined ? 128 : Number(algorithm.tagLength);
        if (tagLen !== 128) throw __wjs2_domErr("OperationError", `${algorithm?.tagLength} is not a valid ChaCha20-Poly1305 tag length`);
        const aad = algorithm?.additionalData === undefined ? new Uint8Array(0) : __wjs2_dataBytes(algorithm.additionalData);
        try {
          const out = __wjs2_chacha_decrypt(st.material, iv, aad, __wjs2_dataBytes(data));
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "RSA-OAEP") {
        if (st.kind !== "private") throw new TypeError("decrypt: not an RSA private key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        const label = algorithm?.label === undefined ? undefined : __wjs2_dataBytes(algorithm.label);
        try {
          const out = __wjs2_rsa_decrypt(hash, st.material, __wjs2_dataBytes(data), label);
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      throw new TypeError("decrypt: unsupported key");
      })();
    },
    sign(algorithm, key, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("sign: not a CryptoKey");
      if ((typeof algorithm === "string" ? algorithm.toUpperCase() : String(algorithm?.name ?? "").toUpperCase()) !== String(st.alg.name ?? "").toUpperCase()) throw new Error("Key algorithm mismatch");
      __wjs2_needUsage(st, "sign");
      if (st.alg.name === "HMAC") {
        const out = __wjs2_hmac_sign(__wjs2_normHash(st.alg.hash), st.material, __wjs2_dataBytes(data));
        return out.buffer;
      }
      if (st.alg.name === "RSASSA-PKCS1-V1_5") {
        if (st.kind !== "private") throw new TypeError("sign: not an RSA private key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        try {
          const out = __wjs2_rsa_sign(hash, st.material, __wjs2_dataBytes(data));
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "RSA-PSS") {
        if (st.kind !== "private") throw new TypeError("sign: not an RSA private key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        // 缺省 saltLength = digest 长度（WebCrypto 口径）。
        const defSalt = { "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 }[hash];
        const salt = algorithm?.saltLength === undefined ? defSalt : Number(algorithm.saltLength);
        if (!Number.isInteger(salt) || salt < 0) throw __wjs2_domErr("OperationError", "bad RSA-PSS saltLength");
        try {
          const out = __wjs2_pss_sign(hash, salt, st.material, __wjs2_dataBytes(data));
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "ED25519" || st.alg.name === "ED448") {
        if (st.kind !== "private") throw new TypeError("sign: not a private key");
        if (st.alg.name === "ED448" && algorithm?.context !== undefined && __wjs2_dataBytes(algorithm.context).length > 255) {
          throw __wjs2_domErr("OperationError", "ContextParams.context must be at most 255 bytes");
        }
        try {
          const ctx = algorithm?.context === undefined ? undefined : __wjs2_dataBytes(algorithm.context);
          const out = st.alg.name === "ED25519"
            ? __wjs2_ed_sign(st.material, __wjs2_dataBytes(data))
            : __wjs2_ed448_sign(st.material, __wjs2_dataBytes(data), ctx);
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "ECDSA") {
        if (st.kind !== "private") throw new TypeError("sign: not an EC private key");
        const hash = __wjs2_normHash(algorithm?.hash ?? "SHA-256");
        try {
          const out = __wjs2_ecdsa_sign(st.alg.namedCurve, hash, st.material, __wjs2_dataBytes(data));
          return out.buffer;
        } catch (e) {
          if (e instanceof DOMException) throw e;
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      throw new TypeError("sign: unsupported key");
      })();
    },
    verify(algorithm, key, signature, data) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(key);
      if (!st) throw new TypeError("verify: not a CryptoKey");
      // hmac 套件：算法先于用量（错算法钥即 Key algorithm mismatch，真机逐项）。
      const aname = typeof algorithm === "string" ? algorithm.toUpperCase() : String(algorithm?.name ?? "").toUpperCase();
      if (aname !== String(st.alg.name ?? "").toUpperCase()) throw new Error("Key algorithm mismatch");
      __wjs2_needUsage(st, "verify");
      if (st.alg.name === "HMAC") {
        return __wjs2_hmac_verify(__wjs2_normHash(st.alg.hash), st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
      }
      if (st.alg.name === "RSASSA-PKCS1-V1_5") {
        if (st.kind === "private") throw new TypeError("verify: not an RSA public key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        try {
          return __wjs2_rsa_verify(hash, st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
        } catch (e) {
          if (e instanceof DOMException) {
            if (e.name === "DataError") return false;
            throw e;
          }
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          if (String(e?.message ?? "").startsWith("DataError:")) return false;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "RSA-PSS") {
        if (st.kind === "private") throw new TypeError("verify: not an RSA public key");
        const hash = __wjs2_normHash(algorithm?.hash ?? st.alg.hash ?? "SHA-256");
        const defSalt = { "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 }[hash];
        const salt = algorithm?.saltLength === undefined ? defSalt : Number(algorithm.saltLength);
        if (!Number.isInteger(salt) || salt < 0) throw __wjs2_domErr("OperationError", "bad RSA-PSS saltLength");
        try {
          return __wjs2_pss_verify(hash, salt, st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
        } catch (e) {
          if (e instanceof DOMException) {
            if (e.name === "DataError") return false;
            throw e;
          }
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          if (String(e?.message ?? "").startsWith("DataError:")) return false;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "ED25519" || st.alg.name === "ED448") {
        if (st.kind === "private") throw new TypeError("verify: not a public key");
        if (st.alg.name === "ED448" && algorithm?.context !== undefined && __wjs2_dataBytes(algorithm.context).length > 255) {
          throw __wjs2_domErr("OperationError", "ContextParams.context must be at most 255 bytes");
        }
        try {
          const ctx = algorithm?.context === undefined ? undefined : __wjs2_dataBytes(algorithm.context);
          return st.alg.name === "ED25519"
            ? __wjs2_ed_verify(st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data))
            : __wjs2_ed448_verify(st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data), ctx);
        } catch (e) {
          if (e instanceof DOMException) {
            if (e.name === "DataError") return false;
            throw e;
          }
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          if (String(e?.message ?? "").startsWith("DataError:")) return false;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      if (st.alg.name === "ECDSA") {
        if (st.kind === "private") throw new TypeError("verify: not an EC public key");
        const hash = __wjs2_normHash(algorithm?.hash ?? "SHA-256");
        try {
          return __wjs2_ecdsa_verify(st.alg.namedCurve, hash, st.material, __wjs2_dataBytes(signature), __wjs2_dataBytes(data));
        } catch (e) {
          if (e instanceof DOMException) {
            if (e.name === "DataError") return false;
            throw e;
          }
          if (String(e?.message ?? "").startsWith("TypeError:")) throw e;
          if (String(e?.message ?? "").startsWith("DataError:")) return false;
          throw __wjs2_domErr("OperationError", "The operation failed for an operation-specific reason");
        }
      }
      throw new TypeError("verify: unsupported key");
      })();
    },
    deriveBits(algorithm, baseKey, length) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(baseKey);
      if (!st) throw new TypeError("deriveBits: not a CryptoKey");
      // lengths：null/缺省/非 8 倍数/负数即 OperationError；0 即空（真机逐项）。
      const needBytes = (len) => __wjs2_needBitsLength(len);
      if (st.alg.name === "X25519" || st.alg.name === "ECDH") {
        if (st.kind !== "private") throw __wjs2_domErr("InvalidAccessError", "deriveBits: base key is not a private key");
        __wjs2_needDeriveUsage(st, "deriveBits");
        if (st.alg.name === "X25519") return __wjs2_x_bits(algorithm, st, length);
        return __wjs2_ecdh_bits(algorithm, st, length);
      }
      if (st.alg.name === "HKDF") {
        __wjs2_needDeriveUsage(st, "deriveBits");
        const hash = __wjs2_normHash(algorithm?.hash);
        if (algorithm?.salt === undefined || algorithm?.info === undefined) {
          throw new TypeError("HKDF deriveBits needs hash, salt and info");
        }
        const n = needBytes(length);
        const hashLen = { "SHA-1": 20, "SHA-256": 32, "SHA-384": 48, "SHA-512": 64, "SHA3-256": 32, "SHA3-384": 48, "SHA3-512": 64 }[hash];
        if (n > 255 * hashLen) throw __wjs2_domErr("OperationError", "length exceeds the maximum derived bit length");
        const saltB = __wjs2_keyBytes(algorithm.salt), infoB = __wjs2_keyBytes(algorithm.info);
        if (infoB.length > 1024) throw __wjs2_domErr("OperationError", "algorithm.info must be at most 1024 bytes");
        const out = __wjs2_hkdf_derive(hash, st.material, saltB, infoB, n);
        return new Uint8Array(out).buffer;
      }
      if (st.alg.name === "PBKDF2") {
        __wjs2_needDeriveUsage(st, "deriveBits");
        const hash = __wjs2_normHash(algorithm?.hash);
        if (algorithm?.salt === undefined || algorithm?.iterations === undefined) {
          throw new TypeError("PBKDF2 deriveBits needs hash, salt and iterations");
        }
        const it = Number(algorithm.iterations);
        if (!Number.isInteger(it) || it < 1) throw __wjs2_domErr("OperationError", "PBKDF2 needs at least 1 iteration");
        const n = needBytes(length);
        const out = __wjs2_pbkdf2_derive(hash, st.material, __wjs2_keyBytes(algorithm.salt), it, n);
        return new Uint8Array(out).buffer;
      }
      throw new TypeError("deriveBits: unsupported key");
      })();
    },
    deriveKey(algorithm, baseKey, derivedKeyAlg, extractable, usages) {
      if (this !== globalThis.crypto.subtle) return Promise.reject(__wjs2BrandErr());
      return (async () => {
      const st = __wjs2_keyState.get(baseKey);
      if (!st) throw new TypeError("deriveKey: not a CryptoKey");
      __wjs2_needDeriveUsage(st, "deriveKey");
      let bitsOf;
      if (st.alg.name === "X25519" || st.alg.name === "ECDH") {
        if (st.kind !== "private") throw __wjs2_domErr("InvalidAccessError", "deriveKey: base key is not a private key");
        bitsOf = (length) => st.alg.name === "X25519"
          ? __wjs2_x_bits(algorithm, st, length)
          : __wjs2_ecdh_bits(algorithm, st, length);
      } else if (st.alg.name === "HKDF") {
        const hash = __wjs2_normHash(algorithm?.hash);
        if (algorithm?.salt === undefined || algorithm?.info === undefined) {
          throw new TypeError("HKDF deriveKey needs hash, salt and info");
        }
        const salt = __wjs2_keyBytes(algorithm.salt), info = __wjs2_keyBytes(algorithm.info);
        if (info.length > 1024) throw __wjs2_domErr("OperationError", "algorithm.info must be at most 1024 bytes");
        const hashLen = { "SHA-1": 20, "SHA-256": 32, "SHA-384": 48, "SHA-512": 64, "SHA3-256": 32, "SHA3-384": 48, "SHA3-512": 64 }[hash];
        bitsOf = (lengthBits) => {
          const n = __wjs2_needBitsLength(lengthBits);
          if (n > 255 * hashLen) throw __wjs2_domErr("OperationError", "length exceeds the maximum derived bit length");
          return __wjs2_hkdf_derive(hash, st.material, salt, info, n);
        };
      } else if (st.alg.name === "PBKDF2") {
        const hash = __wjs2_normHash(algorithm?.hash);
        if (algorithm?.salt === undefined || algorithm?.iterations === undefined) {
          throw new TypeError("PBKDF2 deriveKey needs hash, salt and iterations");
        }
        const it = Number(algorithm.iterations);
        if (!Number.isInteger(it) || it < 1) throw __wjs2_domErr("OperationError", "PBKDF2 needs at least 1 iteration");
        const salt = __wjs2_keyBytes(algorithm.salt);
        bitsOf = (lengthBits) => {
          const n = __wjs2_needBitsLength(lengthBits);
          return __wjs2_pbkdf2_derive(hash, st.material, salt, it, n);
        };
      } else {
        throw new TypeError("deriveKey: unsupported key");
      }
      const dname = String(derivedKeyAlg?.name ?? "").toUpperCase();
      let bytes;
      if (dname === "AES-GCM") {
        const length = Number(derivedKeyAlg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw new Error("NotSupportedError: derived AES-GCM length must be 128/192/256");
        bytes = new Uint8Array(bitsOf(length));
        return __wjs2_makeKey({ name: "AES-GCM", length }, bytes, [...(usages ?? [])].map(String), !!extractable);
      }
      if (dname === "AES-CBC") {
        const length = Number(derivedKeyAlg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw __wjs2_domErr("NotSupportedError", "derived AES-CBC length must be 128/192/256");
        bytes = new Uint8Array(bitsOf(length));
        return __wjs2_makeKey({ name: "AES-CBC", length }, bytes, [...(usages ?? [])].map(String), !!extractable);
      }
      if (dname === "AES-CTR") {
        const length = Number(derivedKeyAlg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw __wjs2_domErr("NotSupportedError", "derived AES-CTR length must be 128/192/256");
        bytes = new Uint8Array(bitsOf(length));
        return __wjs2_makeKey({ name: "AES-CTR", length }, bytes, [...(usages ?? [])].map(String), !!extractable);
      }
      if (dname === "AES-KW") {
        const length = Number(derivedKeyAlg?.length ?? 256);
        if (![128, 192, 256].includes(length)) throw __wjs2_domErr("NotSupportedError", "derived AES-KW length must be 128/192/256");
        bytes = new Uint8Array(bitsOf(length));
        return __wjs2_makeKey({ name: "AES-KW", length }, bytes, [...(usages ?? [])].map(String), !!extractable);
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
// constructors 套件：Crypto/SubtleCrypto 类面（原型挂载既有实现；
// notCrypto/notSubtle 经原型方法品牌检查，真机口径）。
globalThis.SubtleCrypto = class SubtleCrypto {
  constructor() {
    const e = new TypeError("Illegal constructor");
    e.code = "ERR_ILLEGAL_CONSTRUCTOR";
    throw e;
  }
  // constructors 套件：静态品牌检查（余下语义另案，统一报不支持）。
  static supports() {
    if (arguments.length < 2) {
      const e = new TypeError('The "operation" argument must be specified');
      e.code = "ERR_MISSING_ARGS";
      throw e;
    }
    if (this !== globalThis.SubtleCrypto) {
      const e = new TypeError('Value of "this" must be of type SubtleCrypto constructor');
      e.code = "ERR_INVALID_THIS";
      throw e;
    }
    return false;
  }
};
for (const __k of Object.keys(globalThis.crypto.subtle)) {
  globalThis.SubtleCrypto.prototype[__k] = globalThis.crypto.subtle[__k];
}
Object.setPrototypeOf(globalThis.crypto.subtle, globalThis.SubtleCrypto.prototype);
globalThis.Crypto = class Crypto {
  constructor() {
    const e = new TypeError("Illegal constructor");
    e.code = "ERR_ILLEGAL_CONSTRUCTOR";
    throw e;
  }
  get subtle() {
    if (this !== globalThis.crypto) throw __wjs2BrandErr();
    return globalThis.crypto.__subtleSingleton;
  }
};
Object.defineProperty(globalThis.crypto, "__subtleSingleton", {
  value: globalThis.crypto.subtle, writable: true, enumerable: false, configurable: true,
});
delete globalThis.crypto.subtle;
for (const __k of ["getRandomValues", "randomUUID"]) {
  globalThis.Crypto.prototype[__k] = globalThis.crypto[__k];
}
Object.setPrototypeOf(globalThis.crypto, globalThis.Crypto.prototype);
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
