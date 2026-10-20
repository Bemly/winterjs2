import { EventEmitter } from "node:events";
import { Socket } from "node:net";
import { Duplex as __Duplex } from "node:stream";
import {
  validateFunction as __tlsVFunction,
  validateNumber as __tlsVNumber,
} from "node:internal/validators";
import { codes as __tlsCodes } from "node:internal/errors";
const Buffer = globalThis.Buffer;

// node：require('_tls_wrap') 触发 DEP0192（遗留模块别名；同源 node:tls 不告警）。
try {
  if (typeof import.meta?.url === "string" && import.meta.url.endsWith("_tls_wrap")) {
    process.emitWarning("The _tls_wrap module is deprecated. Use `node:tls` instead.", "DeprecationWarning", "DEP0192");
  }
} catch { /* import.meta 不可用即跳过 */ }

// ── node tls.js 原文（convertProtocols/validateALPNBuffer/convertALPNProtocols）──
// ALPN 列表 → wire 形（每名 u8 长度前缀）；Buffer 条目经 write 隐式 toString——
// node 同款怪癖，逐字保留。
function __convertProtocols(protocols) {
  const lens = new Array(protocols.length);
  const buff = Buffer.allocUnsafe(protocols.reduce((p, c, i) => {
    const len = Buffer.byteLength(c);
    if (len === 0) {
      throw new __tlsCodes.ERR_INVALID_ARG_VALUE(`protocols[${i}]`, c, "must be a non-empty string");
    }
    if (len > 255) {
      throw new __tlsCodes.ERR_OUT_OF_RANGE(
        "The byte length of the protocol at index " + `${i} exceeds the maximum length.`,
        "<= 255", len, true);
    }
    lens[i] = len;
    return p + 1 + len;
  }, 0));
  let offset = 0;
  for (let i = 0, c = protocols.length; i < c; i++) {
    buff[offset++] = lens[i];
    buff.write(protocols[i], offset);
    offset += lens[i];
  }
  return buff;
}
function __validateALPNBuffer(buffer) {
  // node 原文：wire 形校验——<len><proto> 序列，len 1..255，无尾字节、无零长条目；
  // 空串合法（等同无 ALPN）。
  let offset = 0;
  while (offset < buffer.length) {
    const len = buffer[offset];
    if (len === 0) {
      throw new __tlsCodes.ERR_INVALID_ARG_VALUE("ALPNProtocols", buffer, "must not contain zero-length protocol");
    }
    if (offset + 1 + len > buffer.length) {
      throw new __tlsCodes.ERR_INVALID_ARG_VALUE("ALPNProtocols", buffer, "contains truncated protocol");
    }
    offset += 1 + len;
  }
}
function convertALPNProtocols(protocols, out) {
  if (Array.isArray(protocols)) {
    out.ALPNProtocols = __convertProtocols(protocols);
  } else if (ArrayBuffer.isView(protocols)) {
    const buf = Buffer.from(protocols.buffer, protocols.byteOffset, protocols.byteLength);
    __validateALPNBuffer(buf);
    out.ALPNProtocols = buf;
  }
}

// ── node:tls（P2，2026-09-25）：TLSSocket 建在 net.Socket 之上（读写/流面/计时/ref 全继承，
// Rust 侧 TLS 流与 TCP 共用 net 泵），本文件只加 TLS 语义：握手结果（tlsInfo 事件）、
// 授权位、secureConnect、证书/套件/协议查询与 node 顶层 API（CA/SecureContext/身份校验）。

function __tlsErr(code, msg) {
  const e = new Error(msg);
  e.code = code;
  return e;
}
// node invalidArgTypeHelper 口径（setServername 文案逐字：Received 后缀）。
function __invalidArgReceived(v) {
  if (v === null || v === undefined) return " Received " + v;
  if (typeof v === "function") return " Received function " + (v.name || "");
  if (typeof v === "object") {
    const n = v.constructor?.name;
    if (typeof n === "string" && n !== "") return " Received an instance of " + n;
    return " Received " + JSON.stringify(v);
  }
  return " Received type " + typeof v + " (" + String(v) + ")";
}
// node：身份校验/报错 host 取 SNI 字符串，无 SNI（false/null/undefined）即回落连接目标。
function __sniHost(t, dflt) {
  if (typeof t.servername === "string" && t.servername !== "") return t.servername;
  return t.__targetHost ?? dflt;
}
// rustls 名失配哨兵（链错原文捕获、名错记哨兵走 checkServerIdentity；见 tls_wrap.rs）。
const __NAME_MISMATCH = "__wjs2_name_mismatch__";
function __pemOf(v) {
  if (v === undefined || v === null) return undefined;
  if (Array.isArray(v)) return v.map(__pemOf).filter((x) => x !== undefined).join("\n");
  if (typeof v === "object" && v !== null && !ArrayBuffer.isView(v) && v.pem !== undefined) return __pemOf(v.pem);
  return String(Buffer.isBuffer(v) || ArrayBuffer.isView(v) ? Buffer.from(v).toString("utf8") : v);
}

// rustls 套件名（Debug 形）→ node getCipher()：TLS1.3 name=standardName=IANA 名；
// TLS1.2 name 为 OpenSSL 名（常见套件表），standardName 为 IANA 名。
const __OSSL12 = {
  TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256: "ECDHE-RSA-AES128-GCM-SHA256",
  TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384: "ECDHE-RSA-AES256-GCM-SHA384",
  TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256: "ECDHE-RSA-CHACHA20-POLY1305",
  TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256: "ECDHE-ECDSA-AES128-GCM-SHA256",
  TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384: "ECDHE-ECDSA-AES256-GCM-SHA384",
  TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256: "ECDHE-ECDSA-CHACHA20-POLY1305",
};
function __cipherInfo(debugName, version) {
  if (!debugName) return undefined;
  const standardName = String(debugName).replace(/^TLS13_/, "TLS_");
  const name = version === "TLSv1.2" ? (__OSSL12[standardName] ?? standardName) : standardName;
  return { name, standardName, version: version ?? "unknown" };
}
function __legacyCert(b64) {
  if (!b64) return null;
  const raw = Buffer.from(atob(b64), "latin1");
  try {
    const { X509Certificate } = globalThis.require("node:crypto");
    const o = new X509Certificate(raw).toLegacyObject();
    o.raw = raw;
    return o;
  } catch {
    return { raw };
  }
}
// node getPeerCertificate(true)：issuerCertificate 链含信任锚——末级签发者落本端 ca 集即补齐，根自链。
// （DN 比较经 X509Certificate 字符串形；toLegacyObject 的 subject/issuer 在本实现为对象，不可直比。）
function __linkTrustAnchor(certs, mo) {
  const last = certs[certs.length - 1];
  if (!last || !last.raw) return;
  try {
    const { X509Certificate } = globalThis.require("node:crypto");
    const lastX = new X509Certificate(last.raw);
    if (typeof lastX.issuer !== "string" || typeof lastX.subject !== "string") return;
    if (lastX.issuer === lastX.subject) return; // 自签即末级
    for (const c of certs) {
      if (c === last || !c.raw) continue;
      try {
        if (new X509Certificate(c.raw).subject === lastX.issuer) return; // 链内已有
      } catch { /* 坏块跳过 */ }
    }
    const sc = mo.secureContext?.context ?? null;
    const caPem = sc !== null
      ? __pemOf(sc.ca)
      : (__pemOf(mo.ca) ?? (__defaultCA !== null ? __defaultCA.join("\n") : undefined));
    if (caPem === undefined) return;
    for (const p of String(caPem).split(/(?=-----BEGIN CERTIFICATE-----)/)) {
      if (!p.trim()) continue;
      let x;
      try { x = new X509Certificate(p); } catch { continue; }
      if (x.subject === lastX.issuer) {
        const leg = x.toLegacyObject();
        leg.raw = Buffer.from(x.raw);
        certs.push(leg);
        return;
      }
    }
  } catch { /* 解析失败即维持自链 */ }
}

class TLSSocket extends Socket {
  constructor(socket, options) {
    // node internal/tls/wrap.js TLSSocket(socket, opts)：首参非 Socket 即 options；
    // socket 须为 Duplex 系否则 TypeError；allowHalfOpen 有 socket 参即取 socket 自身。
    const __hasSock = socket !== undefined && socket !== null;
    if (__hasSock && !(socket instanceof Socket) && !(socket instanceof __Duplex)) {
      throw new __tlsCodes.ERR_INVALID_ARG_TYPE("socket", "net.Socket", socket);
    }
    super({ allowHalfOpen: __hasSock ? !!socket.allowHalfOpen : !!(options && options.allowHalfOpen) });
    this.encrypted = true;
    this.authorized = false;
    this.authorizationError = null;
    this.alpnProtocol = false;
    this.servername = null;
    this._secureEstablished = false;
    this.__tls = null;
    this.__tlsOpts = options ?? {};
    this.__wrapped = socket ?? null;
    // P2-tls-b 包裹面：verifyError 不销毁、经 ssl.verifyError() 上报；onConnectSecure 仅包装层安装。
    this.__isServer = !!(this.__tlsOpts && this.__tlsOpts.isServer === true);
    this.__verifyErr = null;
    this.ssl = { verifyError: () => (this.__verifyErr !== null ? this.__verifyErr : null) };
    this.__tlsId = undefined;
    this.__eofDone = false;
    this.__tlsCloseDone = false;
    this.__wrappedClosed = false;
    this.__sniCalled = false;
    // isServer 包裹（STARTTLS/net.Server 面）：socket 在手即起引擎（node 同形）。
    if (this.__isServer && this.__wrapped) {
      this.__wrapWire(this.__wrapped, this.__tlsOpts, false);
      this.readable = true; this.writable = true;
      this._handle = this.__makeHandle();
      if (this.__wrapped instanceof Socket && this.__wrapped.__connected === true) this.__tlsBegin();
      else if (this.__wrapped instanceof Socket) this.__wrapped.once("connect", () => this.__tlsBegin());
      else queueMicrotask(() => this.__tlsBegin());
    }
  }
  connect(...args) {
    let port, host = "localhost", options = {}, cb;
    if (typeof args[0] === "object" && args[0] !== null) {
      const o = args[0];
      port = o.port; host = o.host ?? host; options = o; // node：host 缺省 localhost；servername 只走 SNI/身份，不回灌 host
      cb = typeof args[1] === "function" ? args[1] : undefined;
    } else {
      port = args[0];
      if (typeof args[1] === "string") { host = args[1]; options = args[2] ?? {}; cb = typeof args[3] === "function" ? args[3] : (typeof args[2] === "function" ? args[2] : undefined); }
      else if (typeof args[1] === "object" && args[1] !== null) { options = args[1]; cb = typeof args[2] === "function" ? args[2] : undefined; }
      else { cb = typeof args[1] === "function" ? args[1] : undefined; }
    }
    if (typeof options === "function") { cb = options; options = {}; }
    const mo = (this.__tlsOpts = { ...this.__tlsOpts, ...options });
    // node normalizeConnectArgs：positional port/host 并回选项面（ECONNRESET 字段从给定选项读，缺省不回灌）。
    if (typeof args[0] === "number") mo.port = args[0];
    if (typeof args[0] !== "object" && typeof args[1] === "string") mo.host = args[1]; // node：对象形后随串忽略
    this.__verify = mo.rejectUnauthorized !== false;
    // node isPipeName 口径：path 形两来源——首参串或选项 .path；其余 host 缺省 localhost。
    const isPath = (typeof args[0] === "string" && args[0] !== "")
      || (typeof args[0] === "object" && args[0] !== null && typeof args[0].path === "string" && args[0].path !== "");
    const pathVal = isPath ? (typeof args[0] === "string" ? args[0] : args[0].path) : undefined;
    // node exports.connect：prependListener('end', onConnectEnd)——握手前对端断开即 ECONNRESET；onConnectSecure 内摘除。
    if (this.__viaConnectApi) {
      this.once("secure", () => this.__onSecureClient());
      this.prependListener("end", this.__onConnectEnd);
    }
    if (mo.socket !== undefined && mo.socket !== null) {
      // tls.connect({socket})：cb 经 onConnectSecure→'secureConnect'（node exports.connect 口径）。
      if (cb) this.once("secureConnect", cb);
      return this.__wrapConnect(mo);
    }
    if (!this.__viaConnectApi || isPath) {
      if (isPath) {
        // node tls.connect(path[, options][, cb])：UDS 内建 net.Socket 直连后走包裹引擎；via 形 cb 挂 secureConnect。
        if (cb) this.once("secureConnect", cb);
      }
      {
      // 纯 TLSSocket 直拨（node net.Socket.connect 口径：cb 挂 'connect'；verifyError 不销毁、'secure' 上报）。
      const inner = new Socket({ allowHalfOpen: !!mo.allowHalfOpen });
      const dial = { ...mo };
      if (isPath) dial.path = pathVal;
      else {
        if (port !== undefined) dial.port = port;
        if (host !== undefined) dial.host = host;
      }
      inner.connect(dial, !isPath && !this.__viaConnectApi ? cb : undefined);
      this.__wrapWire(inner, mo, true);
      this.readable = true; this.writable = true;
      this._handle = this.__makeHandle();
      if (inner.__connected === true) this.__tlsBegin();
      else inner.once("connect", () => this.__tlsBegin());
      return this;
      }
    }
    // tls.connect() 直拨面（既有底座：Rust 侧握手后连体，读写泵共用 net 泵）。
    if (cb) this.once("secureConnect", cb);
    const sc = mo.secureContext?.context ?? {};
    const wire = {};
    const servername = mo.servername || (host && !__isIP(host) ? host : undefined);
    if (servername !== undefined) wire.servername = String(servername);
    // N2：SNI 实发旗（直拨面；无名/IP 即未发，finishInit 口径回 false 不回灌 host）。
    this.__sniSent = (servername !== undefined);
    // node：显式 secureContext 在场即整体采用（ca 等选项不再读）；否则读 ca 选项。
    const ca = mo.secureContext !== undefined
      ? __pemOf(sc.ca)
      : (__pemOf(mo.ca) ?? (__defaultCA !== null ? __defaultCA.join("\n") : undefined));
    if (ca !== undefined) wire.ca = ca;
    if (mo.rejectUnauthorized !== undefined) wire.rejectUnauthorized = !!mo.rejectUnauthorized;
    if (this.ALPNProtocols !== undefined) wire.alpnB64 = Buffer.from(this.ALPNProtocols).toString("base64");
    // 连接期面（net.__realConnect 同口径：先建柄、读写可达，写缓冲至握手完成）。
    this.__targetHost = String(host);
    this.__targetPort = Number(port);
    this.remoteAddress = undefined; this.remotePort = undefined; this.remoteFamily = undefined;
    this.readable = true; this.writable = true;
    this.__handleClosed = false;
    this._handle = this.__makeHandle();
    this.__id = Number(__wjs2_tls_connect(this.__targetHost, this.__targetPort, JSON.stringify(wire), this));
    return this;
  }
  get bufferSize() {
    // node：柄写队列；wrap 镜像 wrapped.writableLength，销毁后 undefined；直拨维持 net 口径。
    if (this.__wrapped) {
      if (this.destroyed || this._handle === null) return undefined;
      return this.__wrapped.writableLength ?? this.__pendBytes;
    }
    return this.__pendBytes;
  }
  _destroySSL() {
    // node TLSSocket._destroySSL：仅弃 TLS 柄（引擎摘表），不连带底层 socket——
    // 随后用户 destroy() 收尾；直拨面 TLS 与流同体，引擎由 destroy 统一收。
    if (this.__tlsId !== undefined) {
      try { __wjs2_tls_wrap_kill(this.__tlsId); } catch { /* 已摘即无事 */ }
      this.__tlsId = undefined;
    }
    this._secureEstablished = false;
  }
  __nameCheck() { // 名失配哨兵 → checkServerIdentity 定夺（node onConnectSecure 口径；过放行，错落位）。
    const mo = this.__wrapOpts ?? this.__tlsOpts ?? {};
    let r = null;
    try { r = (mo.checkServerIdentity ?? checkServerIdentity)(__sniHost(this) ?? "localhost", this.getPeerCertificate(true)) || null; } catch (e) { r = e; }
    this.__nameChecked = true;
    if (r !== null) { this.__verifyErr = r; this.authorized = false; this.authorizationError = r.code ?? r.message; }
    return r;
  }
  __applyTlsInfo(info) {
    this.__tls = info;
    this.alpnProtocol = info.alpn ?? false;
    // node _finishInit 口径：SNI 以实发为准——服务端收即 servername（'' 亦收），
    // 无收即 false；客户端仅实发（wire 置名）才收，回落 host 冒充即 N2 偏差。
    if (info.servername && (this.__tlsServer || this.__isServer || this.__sniSent)) this.servername = info.servername;
    else if (this.__tlsServer || this.__isServer) this.servername = false;
    else if (this.__sniSent === false) this.servername = false;
    // 捕获式校验错：哨兵走身份校验（通过即放行）；链错原文映射（reject 面由连接事件收口）。
    if (info.verifyErr && this.__verifyErr === null) {
      if (String(info.verifyErr) === __NAME_MISMATCH) { this.__nameCheck(); return; }
      this.__verifyErr = __handshakeErr(String(info.verifyErr), __sniHost(this));
      this.authorized = false;
      this.authorizationError = this.__verifyErr.code ?? this.__verifyErr.message;
    }
  }
  __secureVerdict() { // node onConnectSecure 裁决段：捕获错/身份校验落授权位；false=须销毁（调用方冲刷前销毁，不发应用字节）。
    const mo = this.__tlsOpts ?? {};
    let verifyError = this.__verifyErr;
    if (!verifyError && !this.__nameChecked) {
      const csi = typeof mo.checkServerIdentity === "function" ? mo.checkServerIdentity : checkServerIdentity;
      try { verifyError = csi(__sniHost(this), this.getPeerCertificate(true)) || null; } catch (e) { verifyError = e; }
    }
    this.__verdictDone = true;
    if (verifyError) {
      this.authorized = false;
      this.authorizationError = verifyError.code || verifyError.message;
      if (mo.rejectUnauthorized !== false) { this.__verifyErr = verifyError; return false; }
      return true;
    }
    this.authorized = true;
    this.authorizationError = null;
    return true;
  }
  __ev(kind, payload) {
    if (kind === "tlsInfo") {
      try { this.__applyTlsInfo(JSON.parse(payload)); } catch { /* 坏载荷即无信息 */ }
      // 直拨 server 面（tls_listen 派发的 conn TLSSocket）：握手完成点位——
      // node _finishInit 口径 + server 'secureConnection'。
      if (this.__tlsServer) {
        this._secureEstablished = true;
        this.authorized = false; // node：服务端未 requestCert 即 false（真机对拍）
        this.emit("secure");
        const srv = this.__tlsServer;
        queueMicrotask(() => srv.emit("secureConnection", this));
      }
      return;
    }
    if (kind === "error" && !this._secureEstablished) {
      // 握手失败：rustls 细码 → node/OpenSSL 码与文案（net 的 "connect CODE addr" 整形会吞掉细节）。
      let o = {};
      try { o = JSON.parse(payload); } catch { /* 非 JSON 即原样 */ }
      if (o.code === "ERR_TLS_HANDSHAKE") {
        this.destroy(__handshakeErr(String(o.msg ?? ""), __sniHost(this)));
        return;
      }
    }
    if (kind === "connect") {
      // 链错 + 须校验：node 握手硬失败口径——无 'secure' 直错（名错已在身份校验定夺，不进此支）。
      if (this.__verifyErr && !this.__nameChecked && (this.__tlsOpts ?? {}).rejectUnauthorized !== false) {
        this.destroy(this.__verifyErr);
        return;
      }
      // via 面裁决先于 base 冲刷（定夺前不发应用字节；失败销毁，预连写弃置不上网）。
      if (this.__viaConnectApi && !this.__secureVerdict()) {
        this.destroy(this.__verifyErr);
        return;
      }
      // 握手已过：授权位按校验捕获面落位（node onConnectSecure 口径——无错即
      // authorized=true；rejectUnauthorized:false 时校验错不销毁、只记位）。
      if (this.__verifyErr) {
        this.authorized = false;
        this.authorizationError = this.__verifyErr.code ?? this.__verifyErr.message;
      } else {
        this.authorized = true;
        this.authorizationError = null;
      }
      this._secureEstablished = true;
      super.__ev(kind, payload);
      this.emit("secure");
      // node：secureConnect 仅由 onConnectSecure 发（此处直发会双发）。
      return;
    }
    return super.__ev(kind, payload);
  }

  // ── P2-tls-b：TLSSocket 包裹面（node internal/tls/wrap 对应件）──────────
  // rustls 由 JS 字节驱动：wrapped 密文经 feed 推进，回程带密文/明文/握手旗/校验捕获。
  __wrapWire(wrapped, mo, forwardConnect) {
    this.__wrapped = wrapped;
    this.__wrapOpts = mo;
    // 接管生命周期：wrapped FIN 不自动回 FIN（close_notify 由 TLS 层管理）。
    if (typeof wrapped.allowHalfOpen !== "undefined") wrapped.allowHalfOpen = true;
    wrapped.on("error", (e) => {
      this.__hadError = true;
      if (!this.destroyed) this.destroy(e);
    });
    wrapped.on("close", () => this.__wrapClose());
    wrapped.on("data", (b) => this.__tlsIn(b));
    wrapped.on("end", () => this.__tlsEof());
    if (forwardConnect) {
      wrapped.on("connect", () => {
        this.emit("connect");
        this.emit("ready");
      });
    }
  }
  __wrapConnect(mo) {
    this.__wrapWire(mo.socket, mo, true);
    this.readable = true; this.writable = true;
    this._handle = this.__makeHandle();
    // node _start 口径：connecting 挂 'connect'，已连立即起手；非 net.Socket 流无 connect 生命周期、microtask 起手。
    const w = mo.socket;
    if (w instanceof Socket) {
      if (w.__connected === true) this.__tlsBegin();
      else w.once("connect", () => this.__tlsBegin());
    } else {
      queueMicrotask(() => this.__tlsBegin());
    }
    return this;
  }
  __wireConfig(mo, wrapped) {
    if (this.ALPNProtocols === undefined && mo.ALPNProtocols !== undefined) {
      convertALPNProtocols(mo.ALPNProtocols, this);
    }
    const wire = {};
    if (this.__isServer) {
      // node TLSSocket(server 面)：secureContext 显式在场即整体采用，否则 key/cert 直给现构。
      let sc = mo.secureContext?.context;
      if (sc === undefined && mo.key !== undefined && mo.cert !== undefined) {
        sc = createSecureContext(mo).context;
      }
      if (sc === undefined) sc = this._sharedCreds?.context ?? {};
      const key = __pemOf(sc.key);
      // node：服务端出示链 = cert + ca（extra chain；中间证书经 ca 送达客户端）。
      const cert0 = __pemOf(sc.cert), extra0 = __pemOf(sc.ca);
      const cert = cert0 !== undefined && extra0 !== undefined ? cert0 + "\n" + extra0 : cert0;
      if (key !== undefined) wire.key = key;
      if (cert !== undefined) wire.cert = cert;
      // N1 附带：服务端自持首证 DER-b64（getCertificate/multi-keys 套件；出示链取首块；
      // 仅 PEM 形，DER 原字节不动——白名单外不碰）。
      if (cert0 !== undefined && String(cert0).includes("-----BEGIN CERTIFICATE-----")) {
        const body = String(cert0).split("-----END CERTIFICATE-----")[0]
          .replace(/-----(BEGIN|END) CERTIFICATE-----/g, "").replace(/\s+/g, "");
        if (body !== "") this.__ownCert = body;
      }
      if (typeof mo.SNICallback === "function") this.__SNICB = mo.SNICallback;
    } else {
      // node：servername 缺省 host（host 为 IP 禁设）。
      let servername = mo.servername ?? this.servername;
      if ((servername === undefined || servername === null) && wrapped !== undefined) {
        const h = mo.host ?? wrapped.__targetHost;
        if (typeof h === "string" && h !== "" && !__isIP(h)) servername = h;
      }
      if (servername !== undefined && servername !== null && servername !== "") {
        wire.servername = String(servername);
        this.servername = String(servername);
      }
      // N2：SNI 实发旗（包裹面；空串/IP 回落即未发）。
      this.__sniSent = (servername !== undefined && servername !== null && servername !== "");
      const ca = mo.secureContext !== undefined
        ? __pemOf(mo.secureContext?.context?.ca)
        : (__pemOf(mo.ca) ?? (__defaultCA !== null ? __defaultCA.join("\n") : undefined));
      if (ca !== undefined) wire.ca = ca;
      // node 构造路径默认：exports.connect 归一后恒显式；裸 TLSSocket 直连缺省假
      // （wrap.js:954 `!!options.rejectUnauthorized`），显式值恒优先。
      wire.rejectUnauthorized = mo.rejectUnauthorized !== undefined
        ? !!mo.rejectUnauthorized
        : !!this.__viaConnectApi;
    }
    if (this.ALPNProtocols !== undefined) {
      wire.alpnB64 = Buffer.from(this.ALPNProtocols).toString("base64");
    } else if (this.__tlsServer?.ALPNProtocols !== undefined) {
      // accept 出的 socket 继承 server ALPN 表（wire 形；归属认 __tlsServer）。
      wire.alpnB64 = Buffer.from(this.__tlsServer.ALPNProtocols).toString("base64");
    }
    return wire;
  }
  __tlsBegin() {
    if (this.__tlsId !== undefined || this.destroyed) return;
    const mo = this.__wrapOpts ?? {};
    const wire = this.__wireConfig(mo, this.__wrapped);
    this.__tlsId = Number(__wjs2_tls_wrap_open(this.__isServer ? 1 : 0, JSON.stringify(wire)));
    this.__pump(JSON.parse(__wjs2_tls_wrap_feed(this.__tlsId, new Uint8Array(0))));
  }
  __tlsIn(b) {
    if (this.__tlsId === undefined || this.destroyed) return;
    const u8 = typeof b === "string" ? Buffer.from(b, "utf8") : b;
    let r;
    try { r = JSON.parse(__wjs2_tls_wrap_feed(this.__tlsId, u8)); }
    catch { this.destroy(); return; } // 引擎已摘即无事
    this.__pump(r);
  }
  __pump(r) {
    if (!r) return;
    // 中止错先冲 alert 字节（close-error 套件：对端据 alert 判握手失败，不派 secure）。
    if (r.out) {
      try { this.__wrapped.write(Buffer.from(r.out, "base64")); } catch { /* 坏载荷即弃 */ }
    }
    if (r.err !== undefined && r.err !== null) {
      // rustls 协议错 → node/OpenSSL 码映射（复用直拨面映射表）；中止携带的
      // 校验原文同步授权位（旧捕获路径经 secure 面落位，此处等价）。
      if (r.verifyErr !== undefined && r.verifyErr !== null && String(r.verifyErr) !== __NAME_MISMATCH) {
        this.__verifyErr = __handshakeErr(String(r.verifyErr), __sniHost(this));
        this.authorized = false;
        this.authorizationError = this.__verifyErr.code ?? this.__verifyErr.message;
      }
      this.destroy(__handshakeErr(String(r.err), __sniHost(this)));
      return;
    }
    if (r.plain) this.__ingestData(Buffer.from(r.plain, "base64"));
    // 包裹面校验捕获（名失配哨兵走身份校验定夺；链错原文映射；处置收口见 secure 面）。
    if (r.verifyErr !== undefined && r.verifyErr !== null && this.__verifyErr === null) {
      if (String(r.verifyErr) === __NAME_MISMATCH) { this.__nameCheck(); }
      else {
        this.__verifyErr = __handshakeErr(String(r.verifyErr), __sniHost(this));
        this.authorized = false;
        this.authorizationError = this.__verifyErr.code ?? this.__verifyErr.message;
      }
    }
    if (r.info) {
      try { this.__applyTlsInfo(r.info); } catch { /* 坏载荷即无信息 */ }
    }
    if (r.sni) {
      this.servername = r.sni; // 服务端：SNI 即 servername（node _finishInit 同）
      if (this.__SNICB && !this.__sniCalled) {
        this.__sniCalled = true;
        try { this.__SNICB(r.sni, () => {}); } catch { /* 回调抛错不阻握手（上下文换装另案） */ }
      }
    }
    if (r.hs && !this._secureEstablished) this.__finishInit();
    else if (r.eof && !this.__eofDone) this.__tlsEof();
  }
  __finishInit() {
    // node TLSSocket._finishInit + 连接期收尾（remote 继承 wrapped、缓冲写冲刷、end 补发）。
    this._secureEstablished = true;
    this.__connected = true;
    const w = this.__wrapped;
    if (w) {
      if (this.remoteAddress === undefined) this.remoteAddress = w.remoteAddress;
      if (this.remotePort === undefined) this.remotePort = w.remotePort;
      if (this.remoteFamily === undefined) this.remoteFamily = w.remoteFamily;
      if ((this.localAddress === null || this.localAddress === undefined) && w.localAddress != null) {
        this.localAddress = w.localAddress;
        this.localPort = w.localPort;
        this.localFamily = w.localFamily;
      }
    }
    const pend = this.__pendW; this.__pendW = []; this.__pendBytes = 0;
    for (const [u8, cb2] of pend) {
      this.__nativeWrite(u8);
      this.__sockQAdd(u8.length);
      if (cb2) queueMicrotask(cb2);
    }
    if (this.__endAfterFlush) { this.__endAfterFlush = false; this.end(); }
    // node：ALPN 协商后 ALPNCallback（server 面）。
    const ac = this.__wrapOpts?.ALPNCallback;
    if (typeof ac === "function" && typeof this.alpnProtocol === "string") {
      try { ac(this.alpnProtocol); } catch { /* 抛错按 tlsClientError 面另案 */ }
    }
    this.emit("secure");
  }
  __tlsEof() {
    if (this.__eofDone) return;
    this.__eofDone = true;
    if (this.__tlsId !== undefined) {
      let r = null;
      try { r = JSON.parse(__wjs2_tls_wrap_eof(this.__tlsId)); } catch { /* 引擎已走即无事 */ }
      if (r && r.plain) this.__ingestData(Buffer.from(r.plain, "base64"));
    }
    // 残余交付 + 'end' + 非 halfOpen 自动 end（close_notify 出站）复用 net 终结面。
    try { Socket.prototype.__ev.call(this, "end", ""); } catch { /* 监听抛错不阻收尾 */ }
  }
  __wrapClose() {
    this.__wrappedClosed = true;
    if (this.__tlsCloseDone) return;
    this.__tlsCloseDone = true;
    if (this.__tlsId !== undefined) {
      try { __wjs2_tls_wrap_kill(this.__tlsId); } catch { /* 已摘即无事 */ }
      this.__tlsId = undefined;
    }
    this.destroyed = true; this.writable = false; this.readable = false;
    this._handle = null;
    this.__dataBuf = [];
    this.emit("close", this.__hadError === true);
  }
  __onConnectEnd() {
    // node onConnectEnd：握手完成前对端断开 → ECONNRESET（destroy 递送）。
    if (this._secureEstablished) return;
    const mo = this.__tlsOpts ?? {};
    const e = new Error("Client network socket disconnected before secure TLS connection was established");
    e.code = "ECONNRESET";
    e.path = mo.path;
    e.host = mo.host;
    e.port = mo.port;
    e.localAddress = mo.localAddress;
    this.destroy(e);
  }
  __onSecureClient() {
    this.removeListener("end", this.__onConnectEnd); // node onConnectSecure：摘 onConnectEnd（1817 行）。
    // 裁决已在 connect 事件预跑（冲刷前）；wrap-via 面 secure 直达，此处置行。
    if (!this.__verdictDone && !this.__secureVerdict()) { this.destroy(this.__verifyErr); return; }
    this.emit("secureConnect");
  }
  __nativeWrite(u8) {
    if (this.__tlsId === undefined) { __wjs2_net_write(this.__id, u8); return; } // 直拨面直通
    let r;
    try { r = JSON.parse(__wjs2_tls_wrap_write(this.__tlsId, u8)); }
    catch { this.destroy(); return; }
    if (r.err !== undefined && r.err !== null) {
      this.destroy(__handshakeErr(String(r.err), __sniHost(this)));
      return;
    }
    if (r.out) this.__wrapped.write(Buffer.from(r.out, "base64"));
  }
  __nativeEnd() {
    if (this.__tlsId === undefined) {
      if (this.__id && this.__connected) __wjs2_net_end(this.__id);
      else this.__endAfterFlush = true;
      return;
    }
    let r = null;
    try { r = JSON.parse(__wjs2_tls_wrap_shutdown(this.__tlsId)); } catch { /* 引擎已走即无事 */ }
    if (r && r.out) this.__wrapped.write(Buffer.from(r.out, "base64"));
    if (this.__wrapped && !this.__wrapped.destroyed) this.__wrapped.end();
  }
  __nativeKill() {
    if (this.__tlsId === undefined) {
      // 直拨面直通；包裹面（握手前 destroy）落穿销毁 wrapped。
      if (this.__id) { __wjs2_net_destroy(this.__id); return; }
    } else {
      try { __wjs2_tls_wrap_kill(this.__tlsId); } catch { /* 已摘即无事 */ }
      this.__tlsId = undefined;
    }
    const w = this.__wrapped;
    if (w && !w.destroyed) w.destroy();
    else if (!w || this.__wrappedClosed) this.__wrapClose(); // wrapped 已闭：close 事件不会再来自它
  }
  getProtocol() { return this.__tls?.protocol ?? null; }
  getCipher() { return __cipherInfo(this.__tls?.cipher, this.__tls?.protocol); }
  getPeerCertificate(detailed) {
    const chain = this.__tls?.peerChain ?? [];
    if (chain.length === 0) return {};
    const certs = chain.map(__legacyCert);
    if (detailed) {
      __linkTrustAnchor(certs, this.__wrapOpts ?? this.__tlsOpts ?? {});
      for (let i = 0; i < certs.length; i++) certs[i].issuerCertificate = certs[i + 1] ?? certs[i];
    }
    return certs[0];
  }
  getPeerX509Certificate() {
    const b = this.__tls?.peerChain?.[0];
    if (!b) return undefined;
    const { X509Certificate } = globalThis.require("node:crypto");
    return new X509Certificate(Buffer.from(atob(b), "latin1"));
  }
  getCertificate() { return this.__ownCert ? __legacyCert(this.__ownCert) : {}; }
  getX509Certificate() { return undefined; }
  getSession() { return undefined; }
  isSessionReused() { return false; }
  getTLSTicket() { return undefined; }
  getFinished() { return undefined; }
  getPeerFinished() { return undefined; }
  getEphemeralKeyInfo() { return this.__tls?.protocol ? {} : null; }
  getSharedSigalgs() { return []; }
  setServername(name) {
    // node wrap.js 口径：先校验 string 形（含 Received 后缀），再禁服务端签发。
    if (typeof name !== "string") throw __tlsErr("ERR_INVALID_ARG_TYPE", `The "name" argument must be of type string.${__invalidArgReceived(name)}`);
    if (this.__isServer || this.__tlsServer) throw __tlsErr("ERR_TLS_SNI_FROM_SERVER", "Cannot issue SNI from a TLS server-side socket");
    this.servername = name;
  }
  setMaxSendFragment(size) { return size >= 512 && size <= 16384; }
  renegotiate(options, cb) {
    const f = typeof options === "function" ? options : cb;
    // TLS1.3/rustls 无重协商（node 对 1.3 同样报错）。
    if (typeof f === "function") process.nextTick(f, __tlsErr("ERR_TLS_RENEGOTIATION_DISABLED", "TLS session renegotiation disabled for this socket"));
    return false;
  }
  disableRenegotiation() {}
  enableTrace() {}
  exportKeyingMaterial() { throw __tlsErr("ERR_TLS_INVALID_STATE", "TLS socket connection must be securely established"); }
}

function __handshakeErr(msg, host) {
  const map = [
    // rustls InvalidContentType（对端非 TLS 字节）→ OpenSSL wrong version number（node 套件断言形）。
    [/InvalidContentType/, "ERR_SSL_HANDSHAKE_FAILURE", "SSL routines::wrong version number"],
    // rustls 对端 fatal NoApplicationProtocol（ALPN 无交集）→ node 专用码（alpn 套件 TestFatalAlert 点名）。
    [/NoApplicationProtocol/, "ERR_SSL_TLSV1_ALERT_NO_APPLICATION_PROTOCOL", "TLSv1 alert no application protocol"],
    // 名错：Debug 形 NotValidForName + webpki Display 形（NotValidForNameContext 定制 Display；捕获层存 Display）。
    [/NotValidForName|not valid for (any names|name)/, "ERR_TLS_CERT_ALTNAME_INVALID", `Hostname/IP does not match certificate's altnames: Host: ${host}. is not in the cert's altnames`],
    [/__wjs2_depth_zero_self_signed__/, "DEPTH_ZERO_SELF_SIGNED_CERT", "self-signed certificate; if the root CA is installed locally, try running Node.js with --use-system-ca"], // 自签哨兵（Rust SELF_SIGNED_SENTINEL；真机见 strict-probe）。
    [/UnknownIssuer/, "UNABLE_TO_VERIFY_LEAF_SIGNATURE", "unable to verify the first certificate"],
    [/Expired/, "CERT_HAS_EXPIRED", "certificate has expired"],
    [/NotValidYet/, "CERT_NOT_YET_VALID", "certificate is not yet valid"],
    [/BadSignature/, "CERT_SIGNATURE_FAILURE", "certificate signature failure"],
  ];
  for (const [re, code, text] of map) {
    if (re.test(msg)) {
      const e = __tlsErr(code, text);
      e.detail = msg;
      return e;
    }
  }
  const e = __tlsErr("ERR_SSL_HANDSHAKE_FAILURE", msg.replace(/^ERR_TLS_HANDSHAKE: /, ""));
  e.detail = msg;
  return e;
}
function __isIP(h) { return /^[\d.]+$/.test(h) || String(h).includes(":"); }

class Server extends EventEmitter {
  constructor(options, cb) {
    super();
    this.__id = 0;
    this.__listening = null;
    this.__tlsOpts = {};
    this.__contexts = [];
    // node internal/tls/wrap.js Server()：参数形 → ALPN 互斥 → setSecureContext（校验全在
    // createSecureContext）→ 握手超时/SNI/PSK 回调校验。
    if (typeof options === "function") {
      cb = options;
      options = {};
    } else if (options == null || typeof options === "object") {
      options ??= {};
    } else {
      throw new __tlsC.ERR_INVALID_ARG_TYPE("options", "Object", options);
    }
    this._contexts = [];
    this.requestCert = options.requestCert === true;
    this.rejectUnauthorized = options.rejectUnauthorized !== false;
    this.ALPNCallback = options.ALPNCallback;
    if (this.ALPNCallback && options.ALPNProtocols) {
      throw new __tlsC.ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS();
    }
    if (options.sessionTimeout) this.sessionTimeout = options.sessionTimeout;
    if (options.ticketKeys) this.ticketKeys = options.ticketKeys;
    this.setSecureContext(options);
    // node Server 构造：ALPN 列表 → wire 形（TLS 引擎协商 + ALPNCallback 面）。
    if (options.ALPNProtocols !== undefined) convertALPNProtocols(options.ALPNProtocols, this);
    // ALPN 表随监听下发 native（alpn-server-client 套件；字符串归一）。
    // 注：setSecureContext 内不做（当时 this.ALPNProtocols 尚未就绪）。
    if (this.ALPNProtocols !== undefined && Array.isArray(options.ALPNProtocols)) {
      this.__tlsOpts.ALPNProtocols =
        options.ALPNProtocols.map((p) => Buffer.isBuffer(p) ? p.toString("latin1") : String(p));
    }
    this.__handshakeTimeout = options.handshakeTimeout || (120 * 1000);
    this.__SNICallback = options.SNICallback;
    this.__pskCallback = options.pskCallback;
    this.__pskIdentityHint = options.pskIdentityHint;
    __tlsVNumber(this.__handshakeTimeout, "options.handshakeTimeout");
    if (this.__SNICallback) __tlsVFunction(this.__SNICallback, "options.SNICallback");
    if (this.__pskCallback) __tlsVFunction(this.__pskCallback, "options.pskCallback");
    if (this.__pskIdentityHint) __tlsVString(this.__pskIdentityHint, "options.pskIdentityHint");
    if (typeof cb === "function") this.on("secureConnection", cb);
    // node tls.Server：内部 'connection' 监听承担手动升级形（net.Server 收链后
    // `tlsServer.emit('connection', rawSocket)` 直入口径）——裸 net socket 即包
    // TLSSocket(isServer) 发 secureConnection；Rust 派发面发的已是 TLSSocket
    // （encrypted=true），经此监听原样放行不重包。
    this.on("connection", (socket) => {
      if (!(socket instanceof Socket) || socket.encrypted) return;
      const s = new TLSSocket(socket, {
        isServer: true,
        server: this,
        ...(this.__tlsOpts ?? {}),
        ...(this.__SNICallback ? { SNICallback: this.__SNICallback } : {}),
      });
      s.server = this;
      this.__conns = (this.__conns ?? 0) + 1;
      s.once("close", () => { this.__conns = Math.max(0, (this.__conns ?? 1) - 1); });
      queueMicrotask(() => this.emit("secureConnection", s));
    });
    // 派发钩子预绑定（dispatch 以 global 为 this，见 §4.34/§4.36）
    this.__ev = this.__ev.bind(this);
  }
  listen(...args) {
    let port = 0, host = null, cb = null;
    let udsPath = null;
    if (typeof args[0] === "object" && args[0] !== null) {
      port = args[0].port ?? 0;
      host = args[0].host ?? null;
      if (typeof args[0].path === "string") udsPath = args[0].path;
      cb = typeof args[1] === "function" ? args[1] : null;
    } else {
      for (const a of args) {
        if (typeof a === "number" || (typeof a === "string" && /^\d+$/.test(a) && port === 0)) port = Number(a);
        else if (typeof a === "string" && host === null) {
          // node isPipeName 口径：含 "/" 的串才是 UDS path，否则按 host。
          if (/^\d+$/.test(a) && port === 0) port = Number(a);
          else if (a.includes("/")) { host = a; udsPath = a; }
          else host = a;
        }
        else if (typeof a === "function") cb = a;
      }
    }
    if (cb) this.once("listening", cb);
    if (udsPath !== null) {
      // node tls.Server.listen(path)：UDS（复用 net 的 UDS:" 前缀约定）。
      this.__udsPath = udsPath;
      this.__port = 0;
      this.__id = Number(__wjs2_tls_listen(0, "UDS:" + udsPath, JSON.stringify(this.__tlsOpts), this));
      return this;
    }
    this.__port = Number(port);
    this.__id = Number(__wjs2_tls_listen(Number(port), host === null ? "0.0.0.0" : host, JSON.stringify(this.__tlsOpts), this));
    return this;
  }
  __ev(kind, payload) {
    switch (kind) {
      case "listening": {
        const o = JSON.parse(payload);
        if (o.uds) {
          // node：UDS server address() 回路径串。
          this.__listening = { address: o.path, port: 0, family: "IPv4" };
          this.emit("listening");
          break;
        }
        this.__listening = { address: o.addr, port: o.port, family: String(o.addr).includes(":") ? "IPv6" : "IPv4" };
        this.emit("listening");
        break;
      }
      case "connection": {
        const o = JSON.parse(payload);
        const s = new TLSSocket();
        if (o.uds) Socket.prototype.__attachUds.call(s, o);
        else Socket.prototype.__attachConn.call(s, o);
        s.server = this;
        s.__tlsServer = this;
        s.authorized = false;
        // N1 附带：派发面服务端自持首证（同 __wrapWire 服务端面；multi-keys 套件）。
        try {
          const pem = this.__tlsOpts?.secureContext?.context?.cert ?? this.__tlsOpts?.cert;
          if (typeof pem === "string" && pem.includes("-----BEGIN CERTIFICATE-----")) {
            const body = pem.split("-----END CERTIFICATE-----")[0]
              .replace(/-----(BEGIN|END) CERTIFICATE-----/g, "").replace(/\s+/g, "");
            if (body !== "") s.__ownCert = body;
          }
        } catch { /* 取不到即无自持 */ }
        this.__conns = (this.__conns ?? 0) + 1;
        s.once("close", () => { this.__conns = Math.max(0, (this.__conns ?? 1) - 1); });
        // node：connection 在 TCP accept 即发（握手未定）；secureConnection 由
        // conn 侧 tlsInfo 事件点发（握手完成）——纯 net.connect 到 tls server
        // 的面（streamwrap 系）依赖此序。
        this.emit("connection", s);
        break;
      }
      case "tlsClientError": {
        const o = JSON.parse(payload);
        const e = __tlsErr(o.code, o.msg);
        e.library = "SSL routines";
        this.emit("tlsClientError", e, undefined);
        break;
      }
      case "error": {
        const o = JSON.parse(payload);
        const e = __tlsErr(o.code, o.msg);
        e.port = this.__listening ? this.__listening.port : this.__port;
        this.emit("error", e);
        break;
      }
      case "close": this.emit("close"); break;
    }
  }
  address() {
    if (this.__udsPath !== undefined) return this.__udsPath;
    return this.__listening;
  }
  close(cb) {
    if (typeof cb === "function") this.once("close", cb);
    if (this.__id) __wjs2_net_destroy(this.__id);
    return this;
  }
  addContext(hostname, context) {
    if (typeof hostname !== "string") throw __tlsErr("ERR_INVALID_ARG_TYPE", `The "hostname" argument must be of type string.`);
    // SNI 多证书：底座单证书解析器暂不分流（记档），先登记。
    this.__contexts.push([hostname, context]);
  }
  // node Server.prototype.setSecureContext：选项逐项落实例 → createSecureContext 共享凭据；
  // rustls 底座取其 key/cert PEM（secureContext 选项形同样经此）。
  setSecureContext(options) {
    __tlsVObject(options, "options");
    for (const k of ["pfx", "key", "passphrase", "cert", "clientCertEngine", "ca", "minVersion",
      "maxVersion", "secureProtocol", "crl", "ciphers", "dhparam"]) {
      this[k] = options[k] ? options[k] : undefined;
    }
    this.sigalgs = options.sigalgs;
    this.ecdhCurve = options.ecdhCurve;
    this.honorCipherOrder = options.honorCipherOrder !== undefined ? !!options.honorCipherOrder : true;
    this.secureOptions = options.secureOptions || undefined;
    this.sessionIdContext = options.sessionIdContext ? options.sessionIdContext : undefined;
    if (options.sessionTimeout) this.sessionTimeout = options.sessionTimeout;
    if (options.ticketKeys) this.ticketKeys = options.ticketKeys;
    this.privateKeyIdentifier = options.privateKeyIdentifier;
    this.privateKeyEngine = options.privateKeyEngine;
    this.certificateCompression = options.certificateCompression;
    this._sharedCreds = createSecureContext({
      pfx: this.pfx, key: this.key, passphrase: this.passphrase, cert: this.cert,
      clientCertEngine: this.clientCertEngine, ca: this.ca, ciphers: this.ciphers,
      sigalgs: this.sigalgs, ecdhCurve: this.ecdhCurve, dhparam: this.dhparam,
      minVersion: this.minVersion, maxVersion: this.maxVersion, secureProtocol: this.secureProtocol,
      secureOptions: this.secureOptions, honorCipherOrder: this.honorCipherOrder, crl: this.crl,
      sessionIdContext: this.sessionIdContext, ticketKeys: this.ticketKeys,
      sessionTimeout: this.sessionTimeout, privateKeyIdentifier: this.privateKeyIdentifier,
      privateKeyEngine: this.privateKeyEngine, certificateCompression: this.certificateCompression,
    });
    const sc = options.secureContext?.context ?? this._sharedCreds.context;
    const key = __pemOf(sc.key);
    // node：服务端出示链 = cert + ca（extra chain；中间证书经 ca 送达客户端）。
    const cert0 = __pemOf(sc.cert), extra = __pemOf(sc.ca);
    const cert = cert0 !== undefined && extra !== undefined ? cert0 + "\n" + extra : cert0;
    // node：key/cert 可缺（握手时无证书可出示 → tlsClientError）。
    this.__tlsOpts = key !== undefined && cert0 !== undefined ? { key, cert } : {};
  }
  getTicketKeys() { return this._sharedCreds.context.getTicketKeys(); }
  setTicketKeys(keys) {
    __tlsVBuffer(keys);
    if (keys.byteLength !== 48) {
      throw __tlsErr("ERR_ASSERTION", "Session ticket keys must be a 48-byte buffer");
    }
    this._sharedCreds.context.setTicketKeys(keys);
  }
  ref() { return this; }
  unref() { return this; }
}

// ── CA 面（SecureContext 见 tls_context.js）────────────────────────────────
let __defaultCA = null;
function __certsOf(kind) {
  return JSON.parse(__wjs2_tls_ca_certs(kind));
}
export function getCACertificates(type = "default") {
  if (typeof type !== "string") {
    const e = new TypeError(`The "type" argument must be of type string.`);
    e.code = "ERR_INVALID_ARG_TYPE";
    throw e;
  }
  if (type === "default") return __defaultCA !== null ? [...__defaultCA] : [...rootCertificates];
  if (type === "bundled") return rootCertificates;
  if (type === "system" || type === "extra") return __certsOf(type);
  const e = new TypeError(`The argument 'type' must be one of: 'default', 'bundled', 'system', 'extra'. Received '${type}'`);
  e.code = "ERR_INVALID_ARG_VALUE";
  throw e;
}
// node tls.js setDefaultCACertificates：数组/元素类型校验 → 原生 resetRootCertStore（逐个解析，
// 一个都解析不出即 ERR_CRYPTO_OPERATION_FAILED，默认集不变）。
export function setDefaultCACertificates(certs) {
  if (!Array.isArray(certs)) {
    throw new __tlsC.ERR_INVALID_ARG_TYPE("certs", "Array", certs);
  }
  for (let i = 0; i < certs.length; i++) {
    if (typeof certs[i] !== "string" && !ArrayBuffer.isView(certs[i])) {
      throw new __tlsC.ERR_INVALID_ARG_TYPE(`certs[${i}]`, ["string", "ArrayBufferView"], certs[i]);
    }
  }
  const { X509Certificate } = globalThis.require("node:crypto");
  const valid = [];
  for (const c of certs) {
    const pem = typeof c === "string" ? c : Buffer.from(c.buffer, c.byteOffset, c.byteLength).toString("utf8");
    try {
      new X509Certificate(pem);
      if (!valid.includes(pem)) valid.push(pem);
    } catch {
      // 形似 PEM 证书块但 ASN.1 解不开 → OpenSSL PEM 层报错（整批作废，默认集不变）；
      // 根本不是 PEM 的串才静默跳过。
      if (pem.includes("-----BEGIN CERTIFICATE-----")) {
        const e = new Error("error:0488000D:PEM routines::ASN1 lib");
        e.code = "ERR_OSSL_PEM_ASN1_LIB";
        e.library = "PEM routines";
        e.reason = "ASN1 lib";
        throw e;
      }
    }
  }
  if (certs.length > 0 && valid.length === 0) {
    const e = new Error("No valid certificates found in the provided array");
    e.code = "ERR_CRYPTO_OPERATION_FAILED";
    throw e;
  }
  __defaultCA = valid;
}
// node 口径：bundled/root 证书 PEM 无尾换行，且 getCACertificates('bundled') 即同一冻结数组。
export const rootCertificates = Object.freeze(__certsOf("bundled").map((c) => c.replace(/\n+$/, "")));

