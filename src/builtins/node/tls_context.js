
// ── SecureContext（node lib/internal/tls/common.js + secure-context.js 逐字移植，2026-09-26）──
// 原生 SecureContext（OpenSSL SSL_CTX）换成 `__NativeSecureContext`：参数校验与 OpenSSL 可观察
// 报错（未知方法 / no cipher match / 密钥不配对）在此复现，结果记成 PEM 串交 rustls 底座
// （`context.key/cert/ca` 旧读取面保留）。偏差：dhparam/crl/sigalgs/ecdhCurve/ticket/会话参数
// 只校验并记录，不下沉 rustls；pfx 不支持即 ERR_METHOD_NOT_IMPLEMENTED。
import __tlsErrors from 'node:internal/errors';
import { isIP as __netIsIP } from "node:net";
import {
  validateArray as __tlsVArray,
  validateBuffer as __tlsVBuffer,
  validateInt32 as __tlsVInt32,
  validateInteger as __tlsVInteger,
  validateObject as __tlsVObject,
  validateString as __tlsVString,
} from 'node:internal/validators';

const __tlsC = __tlsErrors.codes;
const TLS1_VERSION = 0x301;
const TLS1_1_VERSION = 0x302;
const TLS1_2_VERSION = 0x303;
const TLS1_3_VERSION = 0x304;
const SSL_OP_CIPHER_SERVER_PREFERENCE = 0x400000;

// OpenSSL 3 缺省构建可选的 TLS1.2 套件名（大小写敏感）与别名；1.3 套件名单。
const __OSSL_CIPHERS = new Set([
  "ECDHE-ECDSA-AES256-GCM-SHA384", "ECDHE-RSA-AES256-GCM-SHA384", "DHE-RSA-AES256-GCM-SHA384",
  "ECDHE-ECDSA-CHACHA20-POLY1305", "ECDHE-RSA-CHACHA20-POLY1305", "DHE-RSA-CHACHA20-POLY1305",
  "ECDHE-ECDSA-AES128-GCM-SHA256", "ECDHE-RSA-AES128-GCM-SHA256", "DHE-RSA-AES128-GCM-SHA256",
  "ECDHE-ECDSA-AES256-SHA384", "ECDHE-RSA-AES256-SHA384", "DHE-RSA-AES256-SHA256",
  "ECDHE-ECDSA-AES128-SHA256", "ECDHE-RSA-AES128-SHA256", "DHE-RSA-AES128-SHA256",
  "ECDHE-ECDSA-AES256-SHA", "ECDHE-RSA-AES256-SHA", "DHE-RSA-AES256-SHA",
  "ECDHE-ECDSA-AES128-SHA", "ECDHE-RSA-AES128-SHA", "DHE-RSA-AES128-SHA",
  "AES256-GCM-SHA384", "AES128-GCM-SHA256", "AES256-SHA256", "AES128-SHA256", "AES256-SHA", "AES128-SHA",
  "ECDHE-PSK-CHACHA20-POLY1305", "PSK-AES256-GCM-SHA384", "PSK-AES128-GCM-SHA256", "PSK-CHACHA20-POLY1305",
  "ECDHE-ECDSA-AES128-CCM", "ECDHE-ECDSA-AES256-CCM", "AES128-CCM", "AES256-CCM",
]);
const __OSSL_ALIASES = new Set([
  "DEFAULT", "ALL", "COMPLEMENTOFDEFAULT", "COMPLEMENTOFALL", "HIGH", "MEDIUM", "SECURE128", "SECURE192",
  "SUITEB128", "SUITEB128ONLY", "SUITEB192", "TLSv1.2", "TLSv1", "SSLv3",
  "kRSA", "RSA", "kDHE", "kEDH", "DH", "DHE", "EDH", "kECDHE", "kEECDH", "ECDH", "ECDHE", "EECDH",
  "aRSA", "aECDSA", "ECDSA", "aDSS", "DSS", "aNULL", "eNULL", "NULL", "PSK", "kPSK", "kECDHEPSK", "kDHEPSK",
  "AES", "AES128", "AES256", "AESGCM", "AESCCM", "AESCCM8", "CHACHA20", "CAMELLIA", "ARIA",
  "SHA", "SHA1", "SHA256", "SHA384", "AEAD",
]);
const __TLS13_SUITES = new Set([
  "TLS_AES_256_GCM_SHA384", "TLS_CHACHA20_POLY1305_SHA256", "TLS_AES_128_GCM_SHA256",
  "TLS_AES_128_CCM_SHA256", "TLS_AES_128_CCM_8_SHA256",
]);
function __noCipherMatch() {
  const e = new Error("error:0A0000B9:SSL routines::no cipher match");
  e.library = "SSL routines";
  e.reason = "no cipher match";
  e.code = "ERR_SSL_NO_CIPHER_MATCH";
  return e;
}
function __cipherTokenOk(tok) {
  return tok.split("+").every((p) => __OSSL_CIPHERS.has(p) || __OSSL_ALIASES.has(p));
}
const __SECURE_PROTOCOLS = new Set([
  "TLS_method", "TLS_client_method", "TLS_server_method",
  "TLSv1_method", "TLSv1_client_method", "TLSv1_server_method",
  "TLSv1_1_method", "TLSv1_1_client_method", "TLSv1_1_server_method",
  "TLSv1_2_method", "TLSv1_2_client_method", "TLSv1_2_server_method",
  "SSLv23_method", "SSLv23_client_method", "SSLv23_server_method",
]);
function __pemText(v) {
  return typeof v === "string" ? v : Buffer.from(v.buffer, v.byteOffset, v.byteLength).toString("utf8");
}

const __kNative = Symbol("kNativeSecureContext");
class __NativeSecureContext {
  constructor() {
    this[__kNative] = true;
    this.__minProto = TLS1_2_VERSION;
    this.__maxProto = TLS1_3_VERSION;
    this.__cas = [];
    this.__certs = [];
    this.__keys = [];
    this.__ticketKeys = null;
    this.__rootCerts = false;
  }
  // 原生外部句柄访问器：接收者不是真上下文（原型链继承形）即 TypeError（external-accessor 套件）。
  get _external() {
    if (this === null || typeof this !== "object" || !Object.prototype.hasOwnProperty.call(this, __kNative)) {
      throw new TypeError("Illegal invocation");
    }
    return {};
  }
  init(secureProtocol, minV, maxV) {
    if (secureProtocol) {
      const p = String(secureProtocol);
      if (/^SSLv[23]_/.test(p) && !p.startsWith("SSLv23_")) {
        throw new __tlsC.ERR_TLS_INVALID_PROTOCOL_METHOD(`${p.slice(0, 5)} methods disabled`);
      }
      if (!__SECURE_PROTOCOLS.has(p)) throw new __tlsC.ERR_TLS_INVALID_PROTOCOL_METHOD(`Unknown method: ${p}`);
      const m = /^TLSv1(?:_(\d))?_/.exec(p);
      if (m) {
        const v = m[1] === undefined ? TLS1_VERSION : (m[1] === "1" ? TLS1_1_VERSION : TLS1_2_VERSION);
        minV = v;
        maxV = v;
      }
    }
    this.__minProto = minV;
    this.__maxProto = maxV;
  }
  setOptions(o) { this.__options = o; }
  getMinProto() { return this.__minProto; }
  getMaxProto() { return this.__maxProto; }
  setMinProto(v) { this.__minProto = v; }
  setMaxProto(v) { this.__maxProto = v; }
  setCipherSuites(suites) {
    for (const s of suites.split(":")) {
      const n = s.startsWith("!") ? s.slice(1) : s;
      if (!__TLS13_SUITES.has(n)) throw __noCipherMatch();
    }
    this.__cipherSuites = suites;
  }
  setCiphers(list) {
    if (list !== "") {
      // OpenSSL 串语法：`:`/`,`/空格分隔，`@STRENGTH`/`@SECLEVEL=n` 可紧贴上一项（`RSA@SECLEVEL=0`）。
      const positive = list.split(/[:, ]+/).map((t) => t.split("@")[0])
        .filter((t) => t && !/^[!\-+]/.test(t));
      if (positive.length > 0 && !positive.some(__cipherTokenOk)) throw __noCipherMatch();
    }
    this.__ciphers = list;
  }
  addCACert(cert) { this.__cas.push(__pemText(cert)); }
  addRootCerts() { this.__rootCerts = true; }
  setAllowPartialTrustChain() { this.__partialChain = true; }
  // OpenSSL PEM_read 口径：非 PEM 即 "no start line"；PEM 块解不开即 "ASN1 lib"。
  setCert(cert) {
    const pem = __pemText(cert);
    const osslErr = (msg, code, reason) => {
      const e = new Error(msg);
      e.library = "PEM routines";
      e.reason = reason;
      e.code = code;
      return e;
    };
    if (!pem.includes("-----BEGIN ")) {
      throw osslErr("error:0480006C:PEM routines::no start line", "ERR_OSSL_PEM_NO_START_LINE", "no start line");
    }
    try {
      new (globalThis.require("node:crypto").X509Certificate)(pem);
    } catch {
      throw osslErr("error:0488000D:PEM routines::ASN1 lib", "ERR_OSSL_PEM_ASN1_LIB", "ASN1 lib");
    }
    this.__certs.push(pem);
  }
  setKey(key, passphrase) {
    const pem = __pemText(key);
    const crypto = globalThis.require("node:crypto");
    let priv;
    try {
      priv = crypto.createPrivateKey(
        passphrase !== undefined && passphrase !== null ? { key: pem, passphrase } : pem);
    } catch (e) {
      const bad = passphrase !== undefined && passphrase !== null;
      const err = new Error(bad ? "error:1C800064:Provider routines::bad decrypt"
        : "error:1E08010C:DECODER routines::unsupported");
      err.library = bad ? "Provider routines" : "DECODER routines";
      err.reason = bad ? "bad decrypt" : "unsupported";
      err.code = bad ? "ERR_OSSL_BAD_DECRYPT" : "ERR_OSSL_UNSUPPORTED";
      err.cause = e;
      throw err;
    }
    // OpenSSL SSL_CTX_use_PrivateKey：与已设证书不配对即 "key values mismatch"。
    if (this.__certs.length > 0) {
      let match = true;
      try {
        const c = new crypto.X509Certificate(this.__certs[this.__certs.length - 1]);
        match = c.checkPrivateKey(priv);
      } catch { /* 证书不可解析：交底座报错 */ }
      if (!match) {
        const err = new Error("error:05800074:x509 certificate routines::key values mismatch");
        err.library = "x509 certificate routines";
        err.reason = "key values mismatch";
        err.code = "ERR_OSSL_X509_KEY_VALUES_MISMATCH";
        throw err;
      }
    }
    this.__keys.push(pem);
    this.__passphrases = [...(this.__passphrases ?? []), passphrase];
  }
  setCertificateCompression(packed) { this.__certCompression = packed; }
  setSigalgs(s) { this.__sigalgs = s; }
  setECDHCurve(c) { this.__ecdhCurve = c; }
  setDHParam(p) { this.__dhparam = p; return undefined; }
  addCRL(c) { (this.__crls ??= []).push(__pemText(c)); }
  setSessionIdContext(c) { this.__sessionIdContext = c; }
  loadPKCS12() { throw new __tlsC.ERR_METHOD_NOT_IMPLEMENTED("loadPKCS12()"); }
  setTicketKeys(k) { this.__ticketKeys = Buffer.from(k); }
  getTicketKeys() {
    if (this.__ticketKeys === null) this.__ticketKeys = globalThis.require("node:crypto").randomBytes(48);
    return Buffer.from(this.__ticketKeys);
  }
  setSessionTimeout(t) { this.__sessionTimeout = t; }
  // 旧读取面（Server/TLSSocket 取 PEM 串下沉 rustls）。
  get key() { return this.__keys.length ? this.__keys.join("\n") : undefined; }
  get cert() { return this.__certs.length ? this.__certs.join("\n") : undefined; }
  get ca() { return this.__cas.length ? this.__cas.join("\n") : undefined; }
}

function __tlsToV(which, v, def) {
  v ??= def;
  if (v === 'TLSv1') return TLS1_VERSION;
  if (v === 'TLSv1.1') return TLS1_1_VERSION;
  if (v === 'TLSv1.2') return TLS1_2_VERSION;
  if (v === 'TLSv1.3') return TLS1_3_VERSION;
  throw new __tlsC.ERR_TLS_INVALID_PROTOCOL_VERSION(v, which);
}

function SecureContext(secureProtocol, secureOptions, minVersion, maxVersion) {
  if (!(this instanceof SecureContext)) {
    return new SecureContext(secureProtocol, secureOptions, minVersion,
                             maxVersion);
  }

  if (secureProtocol) {
    if (minVersion != null)
      throw new __tlsC.ERR_TLS_PROTOCOL_VERSION_CONFLICT(minVersion, secureProtocol);
    if (maxVersion != null)
      throw new __tlsC.ERR_TLS_PROTOCOL_VERSION_CONFLICT(maxVersion, secureProtocol);
  }

  const minV = __tlsToV('minimum', minVersion, __api.DEFAULT_MIN_VERSION);
  const maxV = __tlsToV('maximum', maxVersion, __api.DEFAULT_MAX_VERSION);

  this.context = new __NativeSecureContext();
  this.context.init(secureProtocol, minV, maxV);

  if (secureOptions) {
    __tlsVInteger(secureOptions, 'secureOptions');
    this.context.setOptions(secureOptions);
  }
}

export function createSecureContext(options) {
  options ||= {};
  const {
    honorCipherOrder,
    minVersion,
    maxVersion,
    secureProtocol,
  } = options;

  let { secureOptions } = options;

  if (honorCipherOrder)
    secureOptions |= SSL_OP_CIPHER_SERVER_PREFERENCE;

  const c = new SecureContext(secureProtocol, secureOptions,
                              minVersion, maxVersion);

  configSecureContext(c.context, options);

  return c;
}

function getDefaultEcdhCurve() {
  return __api.DEFAULT_ECDH_CURVE || 'auto';
}

function getDefaultCiphers() {
  return __api.DEFAULT_CIPHERS;
}

function addCACerts(context, certs, name) {
  certs.forEach((cert) => {
    validateKeyOrCertOption(name, cert);
    context.addCACert(cert);
  });
}

function setCerts(context, certs, name) {
  certs.forEach((cert) => {
    validateKeyOrCertOption(name, cert);
    context.setCert(cert);
  });
}

function validateKeyOrCertOption(name, value) {
  if (typeof value !== 'string' && !ArrayBuffer.isView(value)) {
    throw new __tlsC.ERR_INVALID_ARG_TYPE(
      name,
      [
        'string',
        'Buffer',
        'TypedArray',
        'DataView',
      ],
      value,
    );
  }
}

function setKey(context, key, passphrase, name) {
  validateKeyOrCertOption(`${name}.key`, key);
  if (passphrase !== undefined && passphrase !== null)
    __tlsVString(passphrase, `${name}.passphrase`);
  context.setKey(key, passphrase);
}

function processCiphers(ciphers, name) {
  ciphers = (ciphers || getDefaultCiphers()).split(':');

  const cipherList = ciphers.filter((cipher) => {
    if (cipher.length === 0) return false;
    if (cipher.startsWith('TLS_')) return false;
    if (cipher.startsWith('!TLS_')) return false;
    return true;
  }).join(':');

  const cipherSuites = ciphers.filter((cipher) => {
    if (cipher.length === 0) return false;
    if (cipher.startsWith('TLS_')) return true;
    if (cipher.startsWith('!TLS_')) return true;
    return false;
  }).join(':');

  // Specifying empty cipher suites for both TLS1.2 and TLS1.3 is invalid, its
  // not possible to handshake with no suites.
  if (cipherSuites === '' && cipherList === '')
    throw new __tlsC.ERR_INVALID_ARG_VALUE(name, ciphers);

  return { cipherList, cipherSuites };
}

function configSecureContext(context, options = {}, name = 'options') {
  __tlsVObject(options, name);

  const {
    allowPartialTrustChain,
    ca,
    cert,
    certificateCompression,
    ciphers = getDefaultCiphers(),
    clientCertEngine,
    crl,
    dhparam,
    ecdhCurve = getDefaultEcdhCurve(),
    key,
    passphrase,
    pfx,
    privateKeyIdentifier,
    privateKeyEngine,
    sessionIdContext,
    sessionTimeout,
    sigalgs,
    ticketKeys,
  } = options;

  // Set the cipher list and cipher suite before anything else because
  // @SECLEVEL=<n> changes the security level and that affects subsequent
  // operations.
  if (ciphers !== undefined && ciphers !== null)
    __tlsVString(ciphers, `${name}.ciphers`);

  const {
    cipherList,
    cipherSuites,
  } = processCiphers(ciphers, `${name}.ciphers`);

  if (cipherSuites !== '')
    context.setCipherSuites(cipherSuites);
  context.setCiphers(cipherList);

  if (cipherList === '' &&
      context.getMinProto() < TLS1_3_VERSION &&
      context.getMaxProto() > TLS1_2_VERSION) {
    context.setMinProto(TLS1_3_VERSION);
  }

  // Add CA before the cert to be able to load cert's issuer in C++ code.
  // NOTE(@jasnell): ca, cert, and key are permitted to be falsy, so do not
  // change the checks to !== undefined checks.
  if (ca) {
    addCACerts(context, Array.isArray(ca) ? ca : [ca], `${name}.ca`);
  } else {
    context.addRootCerts();
  }

  if (allowPartialTrustChain) {
    context.setAllowPartialTrustChain();
  }

  if (cert) {
    setCerts(context, Array.isArray(cert) ? cert : [cert], `${name}.cert`);
  }

  // Set the key after the cert.
  if (key) {
    if (Array.isArray(key)) {
      for (let i = 0; i < key.length; ++i) {
        const val = key[i];
        const pem = (
          val?.pem !== undefined ? val.pem : val);
        const pass = (
          val?.passphrase !== undefined ? val.passphrase : passphrase);
        setKey(context, pem, pass, name);
      }
    } else {
      setKey(context, key, passphrase, name);
    }
  }

  if (certificateCompression != null) {
    __tlsVArray(certificateCompression, `${name}.certificateCompression`);

    if (certificateCompression.length > 0) {
      if (certificateCompression.length > 3) {
        throw new __tlsC.ERR_INVALID_ARG_VALUE(
          `${name}.certificateCompression`, certificateCompression,
          'can specify at most 3 algorithms');
      }
      let packed = certificateCompression.length;
      for (let i = 0; i < certificateCompression.length; i++) {
        const algoName = certificateCompression[i];
        let id;
        if (algoName === 'zlib') id = 1;
        else if (algoName === 'brotli') id = 2;
        else if (algoName === 'zstd') id = 3;
        else {
          throw new __tlsC.ERR_INVALID_ARG_VALUE(
            `${name}.certificateCompression[${i}]`, algoName,
            "must be 'zlib', 'brotli', or 'zstd'");
        }
        packed |= id << (8 * (i + 1));
      }
      context.setCertificateCompression(packed);
    }
  }

  if (sigalgs !== undefined && sigalgs !== null) {
    __tlsVString(sigalgs, `${name}.sigalgs`);

    if (sigalgs === '')
      throw new __tlsC.ERR_INVALID_ARG_VALUE(`${name}.sigalgs`, sigalgs);

    context.setSigalgs(sigalgs);
  }

  if (privateKeyIdentifier !== undefined && privateKeyIdentifier !== null) {
    if (privateKeyEngine === undefined || privateKeyEngine === null) {
      // Engine is required when privateKeyIdentifier is present
      throw new __tlsC.ERR_INVALID_ARG_VALUE(`${name}.privateKeyEngine`,
                                             privateKeyEngine);
    }
    if (key) {
      // Both data key and engine key can't be set at the same time
      throw new __tlsC.ERR_INVALID_ARG_VALUE(`${name}.privateKeyIdentifier`,
                                             privateKeyIdentifier);
    }

    if (typeof privateKeyIdentifier === 'string' &&
        typeof privateKeyEngine === 'string') {
      throw new __tlsC.ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED();
    } else if (typeof privateKeyIdentifier !== 'string') {
      throw new __tlsC.ERR_INVALID_ARG_TYPE(`${name}.privateKeyIdentifier`,
                                            ['string', 'null', 'undefined'],
                                            privateKeyIdentifier);
    } else {
      throw new __tlsC.ERR_INVALID_ARG_TYPE(`${name}.privateKeyEngine`,
                                            ['string', 'null', 'undefined'],
                                            privateKeyEngine);
    }
  }

  __tlsVString(ecdhCurve, `${name}.ecdhCurve`);
  context.setECDHCurve(ecdhCurve);

  if (dhparam !== undefined && dhparam !== null) {
    validateKeyOrCertOption(`${name}.dhparam`, dhparam);
    const warning = context.setDHParam(dhparam === 'auto' || dhparam);
    if (warning)
      process.emitWarning(warning, 'SecurityWarning');
  }

  if (crl !== undefined && crl !== null) {
    if (Array.isArray(crl)) {
      for (const val of crl) {
        validateKeyOrCertOption(`${name}.crl`, val);
        context.addCRL(val);
      }
    } else {
      validateKeyOrCertOption(`${name}.crl`, crl);
      context.addCRL(crl);
    }
  }

  if (sessionIdContext !== undefined && sessionIdContext !== null) {
    __tlsVString(sessionIdContext, `${name}.sessionIdContext`);
    context.setSessionIdContext(sessionIdContext);
  }

  if (pfx !== undefined && pfx !== null) {
    context.loadPKCS12(pfx, passphrase);
  }

  if (typeof clientCertEngine === 'string') {
    throw new __tlsC.ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED();
  } else if (clientCertEngine !== undefined && clientCertEngine !== null) {
    throw new __tlsC.ERR_INVALID_ARG_TYPE(`${name}.clientCertEngine`,
                                          ['string', 'null', 'undefined'],
                                          clientCertEngine);
  }

  if (ticketKeys !== undefined && ticketKeys !== null) {
    __tlsVBuffer(ticketKeys, `${name}.ticketKeys`);
    if (ticketKeys.byteLength !== 48) {
      throw new __tlsC.ERR_INVALID_ARG_VALUE(
        `${name}.ticketKeys`,
        ticketKeys.byteLength,
        'must be exactly 48 bytes');
    }
    context.setTicketKeys(ticketKeys);
  }

  if (sessionTimeout !== undefined && sessionTimeout !== null) {
    __tlsVInt32(sessionTimeout, `${name}.sessionTimeout`, 0);
    context.setSessionTimeout(sessionTimeout);
  }
}

// ── 身份校验算法族（node lib/tls.js checkServerIdentity 原文移植；与 tls.js 同模块拼接，导出唯一）──
function __unfqdn(h) { return String(h).replace(/\.$/, ""); }
function __asciiLower(h) { return h.replace(/[A-Z]/g, (c) => String.fromCharCode(32 + c.charCodeAt(0))); }
function __splitHost(h) { return __asciiLower(__unfqdn(h)).split("."); }
// IDNA：ASCII 原样（避 URL 归一化分歧）；非 ASCII 走 WHATWG 域名（ICU，与 domainToASCII 同形，无 punycode 告警）。
function __toASCII(h) {
  if (!/[^\x00-\x7F]/.test(h)) return h;
  try { return new URL("http://" + h + "/").hostname; } catch { return h; }
}
// RFC6125 左首标签通配（node check() 原文：空拒、等长、空标签拒、非 ASCII 拒、右向比对、xn-- 逐字）。
function __checkDns(hp, pat) {
  if (!pat) return false;
  const pp = __splitHost(pat);
  if (hp.length !== pp.length || pp.includes("") || pp.some((s) => /[^\u0021-\u007F]/.test(s))) return false;
  for (let i = hp.length - 1; i > 0; i--) if (hp[i] !== pp[i]) return false;
  const sub = hp[0], sg = pp[0].split("*", 3);
  if (sg.length === 1 || pp[0].includes("xn--")) return sub === pp[0];
  if (sg.length > 2 || pp.length <= 2) return false;
  return sg[0].length + sg[1].length <= sub.length && sub.startsWith(sg[0]) && sub.endsWith(sg[1]);
}
// 引号感知切分（node splitEscapedAltNames 原文；NUL 转义字面即此形）。
const __altJsonRe = /^"(?:[^"\\\u0000-\u001f]|\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4}))*"/;
function __splitAlt(s) {
  const out = [];
  let cur = "", o = 0;
  while (o !== s.length) {
    const sp = s.indexOf(",", o), q = s.indexOf('"', o);
    if (q !== -1 && (sp === -1 || q < sp)) {
      cur += s.substring(o, q);
      const m = __altJsonRe.exec(s.substring(q));
      if (!m) { const e = new SyntaxError("Invalid subject alternative name string"); e.code = "ERR_TLS_CERT_ALTNAME_FORMAT"; throw e; }
      cur += JSON.parse(m[0]);
      o = q + m[0].length;
    } else if (sp !== -1) {
      cur += s.substring(o, sp);
      out.push(cur);
      cur = "";
      o = sp + 2;
    } else {
      cur += s.substring(o);
      o = s.length;
    }
  }
  out.push(cur);
  return out;
}
// inet_pton/ntop 手工版（cares_wrap canonicalizeIP 同形：v4 严格无前导零；v6 小写 RFC5952 压缩，zone 丢弃，非法即 ""）。
function __canon4(s) {
  const p = s.split(".");
  if (p.length !== 4) return "";
  const o = [];
  for (const x of p) {
    if (!/^\d{1,3}$/.test(x) || (x.length > 1 && x[0] === "0") || +x > 255) return "";
    o.push(String(+x));
  }
  return o.join(".");
}
function __canon6(s) {
  let v = s;
  const pct = v.indexOf("%");
  if (pct !== -1) v = v.slice(0, pct);
  let tail = [];
  if (v.includes(".")) {
    const i = v.lastIndexOf(":"), q = __canon4(v.slice(i + 1));
    if (i === -1 || q === "") return "";
    const b = q.split(".").map(Number);
    tail = [(b[0] << 8) | b[1], (b[2] << 8) | b[3]];
    v = v.slice(0, i);
    if (v === ":") v = "::";
  }
  const dbl = v.split("::");
  if (dbl.length > 2) return "";
  const pg = (p) => {
    if (p === "") return [];
    const o = [];
    for (const g of p.split(":")) {
      if (!/^[0-9a-fA-F]{1,4}$/.test(g)) return null;
      o.push(parseInt(g, 16));
    }
    return o;
  };
  let g;
  if (dbl.length === 2) {
    const L = pg(dbl[0]), R = pg(dbl[1]);
    if (!L || !R) return "";
    const z = 8 - tail.length - L.length - R.length;
    if (z < 1) return "";
    g = [...L, ...new Array(z).fill(0), ...R, ...tail];
  } else {
    const G = pg(v);
    if (!G) return "";
    g = [...G, ...tail];
    if (g.length !== 8) return "";
  }
  if (g[5] === 0xffff && g[0] === 0 && g[1] === 0 && g[2] === 0 && g[3] === 0 && g[4] === 0)
    return "::ffff:" + [g[6] >> 8 & 255, g[6] & 255, g[7] >> 8 & 255, g[7] & 255].join(".");
  const h = g.map((x) => x.toString(16));
  let bi = -1, bl = 0, ci = -1, cl = 0;
  for (let i = 0; i <= 8; i++) {
    if (i < 8 && g[i] === 0) { if (ci < 0) { ci = i; cl = 1; } else cl++; }
    else { if (cl > bl) { bl = cl; bi = ci; } ci = -1; cl = 0; }
  }
  if (bl < 2) return h.join(":");
  return h.slice(0, bi).join(":") + "::" + h.slice(bi + bl).join(":");
}
function __canonIP(s) { return s.includes(":") ? __canon6(s) : __canon4(s); }
export function checkServerIdentity(hostname, cert) {
  const subject = cert?.subject;
  const alt = cert?.subjectaltname;
  const dns = [], ips = [];
  hostname = "" + hostname;
  const asciiBare = __unfqdn(__toASCII(hostname));
  hostname = __unfqdn(hostname);
  if (alt) {
    for (const name of alt.includes('"') ? __splitAlt(alt) : alt.split(", ")) {
      if (name.startsWith("DNS:")) dns.push(name.slice(4));
      else if (name.startsWith("IP Address:")) ips.push(__canonIP(name.slice(11)));
    }
  }
  let valid = false, reason = "Unknown reason";
  if (__netIsIP(hostname)) {
    valid = ips.includes(__canonIP(hostname));
    if (!valid) reason = `IP: ${hostname} is not in the cert's list: ${ips.join(", ")}`;
  } else if (dns.length > 0 || subject?.CN) {
    const hp = __splitHost(asciiBare);
    if (dns.length > 0) {
      valid = dns.some((d) => __checkDns(hp, d));
      if (!valid) reason = `Host: ${hostname}. is not in the cert's altnames: ${alt}`;
    } else {
      const cn = subject.CN;
      valid = Array.isArray(cn) ? cn.some((c) => __checkDns(hp, c)) : (!!cn && __checkDns(hp, cn));
      if (!valid) reason = `Host: ${hostname}. is not cert's CN: ${cn}`;
    }
  } else {
    reason = "Cert does not contain a DNS name";
  }
  if (valid) return undefined;
  const e = new Error(`Hostname/IP does not match certificate's altnames: ${reason}`);
  e.code = "ERR_TLS_CERT_ALTNAME_INVALID";
  e.reason = reason;
  e.host = hostname;
  e.cert = cert;
  return e;
}
