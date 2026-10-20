//! `node:https`：HTTPS Server/Client——帧层复用 `node:internal/http_framing`
//! （与 `node:http` 同语义，含 10b keep-alive/流式体/chunked），传输经
//! `node:tls`（Phase 9d-6）。
//! 偏差记档（帧层头注沿用）：
//! - 客户端 TLS 选项透传：`servername`/`ca`（PEM 串）/`rejectUnauthorized`
//!   （`tls.connect` 同口径）；缺省系统 roots 校验。
//! - 服务端 `createServer({ key, cert }, listener)`（PEM 串必填）。

/// 内嵌 ESM 源（`node:https`；tls 底座 + 共享帧层）。
pub const SOURCE: &str = r#"
import * as tls from "node:tls";
import { codes } from "node:internal/errors";
import {
  STATUS_CODES, METHODS, maxHeaderSize, IncomingMessage, ServerResponse,
  OutgoingMessage, Agent as BaseAgent, withHttpServer, withClientRequest,
  normalizeRequestArgs, requestFrom, getFrom,
  validateHeaderName, validateHeaderValue,
} from "node:internal/http_framing";

const FLAVOR = { protocol: "https:", defaultPort: 443, other: "node:http" };
const __HttpServerBase = withHttpServer(tls.Server);
// options 类型门（node https.js Server 构造口径，mirror http.rs __serverOptions：
// 'foo'/42/true/[] → ERR_INVALID_ARG_TYPE；undefined/null 缺省、函数=监听器）。
function __serverOptions(args) {
  const first = args[0];
  if (first === undefined || first === null) return undefined;
  if (typeof first === "function") return undefined;
  if (first !== null && typeof first === "object" && !Array.isArray(first)) return first;
  throw new codes.ERR_INVALID_ARG_TYPE("options", "object", first);
}
// Server（node https.js Server 原文口径）：ALPNProtocols 缺省 ['http/1.1']（经
// tls.Server 构造器 convertALPNProtocols 落 wire 形）——opts 显式给了
// ALPNProtocols/ALPNCallback 才不注缺省；其余选项原样透传。
function Server(...args) {
  let ALPNProtocols = ["http/1.1"];
  const checked = __serverOptions(args);
  let opts = checked;
  if (typeof args[0] === "function") opts = {};
  else if (opts === undefined) opts = {};
  else if (opts.ALPNProtocols || opts.ALPNCallback) ALPNProtocols = undefined;
  const wire = opts === undefined ? undefined : { noDelay: true, ALPNProtocols, ...opts };
  const s = new __HttpServerBase(wire);
  const first = args[0];
  if (typeof first === "function") s.on("request", first);
  else if (args.length > 1 && typeof args[1] === "function") s.on("request", args[1]);
  return s;
}
Object.setPrototypeOf(Server, __HttpServerBase);
Server.prototype = __HttpServerBase.prototype;
const ClientRequest = withClientRequest(
  (host, port, extra) => tls.connect({
    port, host,
    // node 口径：socketPath 在场即 UDS（unix-socket 套件；http 侧 openSocket
    // 同款，tls.connect 读 .path 面）。
    ...(extra.socketPath !== undefined ? { path: extra.socketPath } : {}),
    // node 口径（getAllowUnauthorized）：NODE_TLS_REJECT_UNAUTHORIZED=0 即
    // 缺省 false（显式值恒赢；警告由帧层统一发一次）。此处回落——键位只收
    // 显式值（override-global-agent 套件）。
    ...(extra.rejectUnauthorized !== undefined ? { rejectUnauthorized: extra.rejectUnauthorized }
      : ((typeof process === "object" && process !== null && process.env !== undefined &&
          process.env.NODE_TLS_REJECT_UNAUTHORIZED === "0") ? { rejectUnauthorized: false } : {})),
    servername: extra.servername,
    ca: extra.ca,
    // node 口径：显式 undefined 即缺席（connect 内有缺省；显式传 undefined 反而触校验门）。
    ...(extra.checkServerIdentity !== undefined ? { checkServerIdentity: extra.checkServerIdentity } : {}),
  }),
  FLAVOR,
);
// Agent 函数式构造器（node 口径：`https.Agent({...})` 无 new 亦合法）。
function Agent(options = {}) {
  if (!(this instanceof Agent)) return new Agent(options);
  BaseAgent.prototype.__init.call(this, options);
}
Object.setPrototypeOf(Agent.prototype, BaseAgent.prototype);
Object.setPrototypeOf(Agent, BaseAgent);
Agent.prototype.__openSocket = (host, port, extra) => tls.connect({
  port, host,
  // 同上：socketPath→path（unix-socket 套件经 Agent 面 createConnection）。
  ...(extra.socketPath !== undefined ? { path: extra.socketPath } : {}),
  // 同上：env 缺省回落（键位只收显式值）。
  ...(extra.rejectUnauthorized !== undefined ? { rejectUnauthorized: extra.rejectUnauthorized }
    : ((typeof process === "object" && process !== null && process.env !== undefined &&
        process.env.NODE_TLS_REJECT_UNAUTHORIZED === "0") ? { rejectUnauthorized: false } : {})),
  servername: extra.servername,
  ca: extra.ca,
  ...(extra.checkServerIdentity !== undefined ? { checkServerIdentity: extra.checkServerIdentity } : {}),
});
// node internal/http/agent.js normalizeServerName 口径：显式 servername（含 '')
// 恒赢；缺席即取请求 Host 头（去端口/方括号；IP 即 ''——SNI 禁 IP）。
function __httpsServerName(options, req) {
  if (!options.servername && options.servername !== "") {
    let sn = options.host;
    try {
      const hh = req.getHeader("host");
      if (hh !== undefined && hh !== null && hh !== "") {
        const s = String(Array.isArray(hh) ? hh[0] : hh);
        if (s[0] === "[") { const i = s.indexOf("]"); sn = i === -1 ? s : s.substring(1, i); }
        else sn = s.split(":")[0];
      }
    } catch { /* 取头失败即回落 host */ }
    options.servername = (typeof sn === "string" && (sn.includes(":") || /^[\d.]+$/.test(sn))) ? "" : sn;
  }
}
Agent.prototype.createSocket = function (req, options, cb) {
  __httpsServerName(options, req);
  return BaseAgent.prototype.createSocket.call(this, req, options, cb);
};
// node lib/https.js 口径：createConnection(port[, host][, options]) 变参归一；
// 单对象形直传、多参形拷贝（mustNotMutate 语义），位置 port/host 恒赢。
Agent.prototype.createConnection = function (...args) {
  let options;
  if (args[0] !== null && typeof args[0] === "object") options = args[0];
  else if (args[1] !== null && typeof args[1] === "object") options = { ...args[1] };
  else if (args[2] === null || typeof args[2] !== "object") options = {};
  else options = { ...args[2] };
  if (typeof args[0] === "number") options.port = args[0];
  if (typeof args[1] === "string") options.host = args[1];
  return BaseAgent.prototype.createConnection.call(this, options);
};
Agent.prototype.__defaultPort = 443;
// https 键位在 http 基础上加 TLS 会话面字段（node lib/https.js 口径；pfx/cert
// 等进键——不同证书不同池位，agent-getname 套件逐字段对拍）。
Agent.prototype.getName = function (options = {}) {
  let name = BaseAgent.prototype.getName.call(this, options);
  name += ":";
  if (options.ca) name += options.ca;
  name += ":";
  if (options.cert) name += options.cert;
  name += ":";
  if (options.clientCertEngine) name += options.clientCertEngine;
  name += ":";
  if (options.ciphers) name += options.ciphers;
  name += ":";
  if (options.key) name += options.key;
  name += ":";
  if (options.pfx) name += __pfxAgentKey(options.pfx, options.passphrase);
  name += ":";
  if (options.rejectUnauthorized !== undefined) name += options.rejectUnauthorized;
  name += ":";
  if (options.servername && options.servername !== options.host) name += options.servername;
  name += ":";
  if (options.minVersion) name += options.minVersion;
  name += ":";
  if (options.maxVersion) name += options.maxVersion;
  name += ":";
  if (options.secureProtocol) name += options.secureProtocol;
  name += ":";
  if (options.crl) name += options.crl;
  name += ":";
  if (options.honorCipherOrder !== undefined) name += options.honorCipherOrder;
  name += ":";
  if (options.ecdhCurve) name += options.ecdhCurve;
  name += ":";
  if (options.dhparam) name += options.dhparam;
  name += ":";
  if (options.secureOptions !== undefined) name += options.secureOptions;
  name += ":";
  if (options.sessionIdContext) name += options.sessionIdContext;
  name += ":";
  if (options.sigalgs) name += JSON.stringify(options.sigalgs);
  name += ":";
  if (options.privateKeyIdentifier) name += options.privateKeyIdentifier;
  name += ":";
  if (options.privateKeyEngine) name += options.privateKeyEngine;
  return name;
};
function __pfxAgentKey(pfx, passphrase) {
  if (!Array.isArray(pfx)) return pfx;
  let key = "";
  for (const v of pfx) {
    const raw = v?.buf || v;
    const pass = v?.passphrase || passphrase;
    key += `:${raw}:${pass}`;
  }
  return key;
}
const globalAgent = new Agent({ keepAlive: true, scheduling: "lifo" });
FLAVOR.defaultAgent = globalAgent;

export function request(a, b, c) {
  const [options, cb] = normalizeRequestArgs(a, b, c, FLAVOR);
  return requestFrom(ClientRequest, options, cb);
}
export function get(a, b, c) {
  const [options, cb] = normalizeRequestArgs(a, b, c, FLAVOR);
  return getFrom(ClientRequest, options, cb);
}
export function createServer(options, cb) {
  // node https.js：createServer = new Server(opts, cb)（ALPN 缺省面在 Server 内；
  // Server 构造器已挂首参 listener，此处禁重复挂——重复即每请求双派发，
  // 第二次 writeHead 必 ERR_HTTP_HEADERS_SENT，hwm 系 15 件根因）。
  return new Server(options, cb);
}

export {
  STATUS_CODES, METHODS, maxHeaderSize, Server, ServerResponse,
  IncomingMessage, ClientRequest, OutgoingMessage, Agent, globalAgent,
  validateHeaderName, validateHeaderValue,
};
const __api = {
  STATUS_CODES, METHODS, maxHeaderSize, request, get, Agent, globalAgent,
  Server, ServerResponse, IncomingMessage, ClientRequest, OutgoingMessage, createServer,
  validateHeaderName, validateHeaderValue,
};
export default __api;
"#;
