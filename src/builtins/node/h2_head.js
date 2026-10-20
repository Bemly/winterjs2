import { EventEmitter } from "node:events";
import { Readable, Writable, Duplex } from "node:stream";
import { codes } from "node:internal/errors";
import { kSocket } from "node:internal/http2/util";
import { kTimeout } from "node:internal/timers";
import { addAbortListener } from "node:internal/events/abort_listener";
import * as net from "node:net";
import fs from "node:fs";
const Buffer = globalThis.Buffer;

function __b64dec(s) {
  const bin = atob(s);
  const u8 = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
  return u8;
}
function __b64enc(u8) {
  let s = "";
  for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode(...u8.subarray(i, i + 0x8000));
  return btoa(s);
}
function __toU8(data, what) {
  if (typeof data === "string") return new TextEncoder().encode(data);
  if (data instanceof Uint8Array) return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  throw new TypeError(`${what}: data must be string or BufferSource`);
}
function __h2Err(code, msg, name = "Error") {
  // 类按 name 取（node 的 RangeError/TypeError 系 instanceof 与 name 同真）。
  const C = name === "RangeError" ? RangeError : name === "TypeError" ? TypeError : Error;
  const e = new C(msg);
  e.code = code;
  e.name = name;
  return e;
}
// http2 专属错误码（errors.rs 内核表外补；消息逐字对齐 node/lib/internal/errors.js）
const __codes = {
  ERR_HTTP2_STATUS_INVALID: (s) => __h2Err("ERR_HTTP2_STATUS_INVALID", `Invalid status code: ${s}`, "RangeError"),
  ERR_HTTP2_INFO_STATUS_NOT_ALLOWED: () => __h2Err("ERR_HTTP2_INFO_STATUS_NOT_ALLOWED", "Informational status codes cannot be used", "RangeError"),
  ERR_HTTP2_INVALID_INFO_STATUS: (s) => __h2Err("ERR_HTTP2_INVALID_INFO_STATUS", `Invalid informational status code: ${s}`, "RangeError"),
  ERR_HTTP2_STATUS_101: () => __h2Err("ERR_HTTP2_STATUS_101", "HTTP status code 101 (Switching Protocols) is forbidden in HTTP/2"),
  ERR_HTTP2_INVALID_PSEUDOHEADER: (s) => __h2Err("ERR_HTTP2_INVALID_PSEUDOHEADER", `"${s}" is an invalid pseudoheader or is used incorrectly`, "TypeError"),
  ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED: () => __h2Err("ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED", "Cannot set HTTP/2 pseudo-headers", "TypeError"),
  ERR_HTTP2_HEADER_SINGLE_VALUE: (s) => __h2Err("ERR_HTTP2_HEADER_SINGLE_VALUE", `Header field "${s}" must only have a single value`, "TypeError"),
  ERR_HTTP2_TRAILERS_CANNOT_BE_SENT: () => __h2Err("ERR_HTTP2_TRAILERS_CANNOT_BE_SENT", "Trailers cannot be sent at this stage."),
  ERR_HTTP2_TRAILERS_ALREADY_SENT: () => __h2Err("ERR_HTTP2_TRAILERS_ALREADY_SENT", "Trailers has already been sent."),
  ERR_HTTP2_TRAILERS_NOT_READY: () => __h2Err("ERR_HTTP2_TRAILERS_NOT_READY", "Trailing headers cannot be sent until after the wantTrailers event is emitted"),
  ERR_HTTP2_PUSH_DISABLED: () => __h2Err("ERR_HTTP2_PUSH_DISABLED", "Push streams are not enabled on this session."),
  ERR_HTTP2_NESTED_PUSH: () => __h2Err("ERR_HTTP2_NESTED_PUSH", "A push stream cannot be initiated from within a push stream."),
  ERR_HTTP2_GOAWAY_SESSION: () => __h2Err("ERR_HTTP2_GOAWAY_SESSION", "New streams cannot be created after receiving a GOAWAY"),
  ERR_HTTP2_SESSION_ERROR: (n) => __h2Err("ERR_HTTP2_SESSION_ERROR", `Session closed with error code ${n}`),
  ERR_HTTP2_MAX_PENDING_SETTINGS_ACK: () => __h2Err("ERR_HTTP2_MAX_PENDING_SETTINGS_ACK", "Maximum concurrent SETTINGS frames not acknowledged"),
  ERR_HTTP2_INVALID_SETTING_VALUE: (name, v) => __h2Err("ERR_HTTP2_INVALID_SETTING_VALUE", `Invalid value for setting "${name}": ${v}`, "RangeError"),
  ERR_HTTP2_PAYLOAD_FORBIDDEN: (s) => __h2Err("ERR_HTTP2_PAYLOAD_FORBIDDEN", `Responses with ${s} status must not have a payload`),
  ERR_HTTP2_NO_PAYLOAD: () => __h2Err("ERR_HTTP2_NO_PAYLOAD", "No payload supplied"),
  ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH: () => __h2Err("ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH", "Packed settings length must be a multiple of six", "RangeError"),
  ERR_HTTP2_INVALID_PROTOCOL: (v, a) => __h2Err("ERR_HTTP2_INVALID_PROTOCOL", `Protocol "${v}" does not contain "${a}"`),
  ERR_HTTP2_SEND_FILE: () => __h2Err("ERR_HTTP2_SEND_FILE", "Filename passed to sendFile must be absolute"),
  ERR_HTTP2_SEND_FILE_NOSEEK: () => __h2Err("ERR_HTTP2_SEND_FILE_NOSEEK", "Offset or length can only be specified for regular files"),
  ERR_HTTP2_PING_CANCEL: () => __h2Err("ERR_HTTP2_PING_CANCEL", "HTTP2 ping cancelled"),
  ABORT_ERR: () => {
    const e = __h2Err("ABORT_ERR", "This operation was aborted");
    e.name = "AbortError";
    return e;
  },
  ERR_HTTP2_PING_LENGTH: () => __h2Err("ERR_HTTP2_PING_LENGTH", "HTTP2 ping payload must be 8 bytes", "RangeError"),
  ERR_HTTP2_TOO_MANY_INVALID_FRAMES: (s) => __h2Err("ERR_HTTP2_TOO_MANY_INVALID_FRAMES", `Too many invalid HTTP/2 frames: ${s}`),
  ERR_HTTP2_FRAME_ERROR: (s) => __h2Err("ERR_HTTP2_FRAME_ERROR", `HTTP/2 frame error: ${s}`),
  ERR_HTTP2_STREAM_CLOSED: () => __h2Err("ERR_HTTP2_STREAM_CLOSED", "The stream has been destroyed"),
  // node：reason 实参只进 cause，message 恒此串（真机 26.8.2 实测）
  ERR_HTTP2_STREAM_CANCEL: () => __h2Err("ERR_HTTP2_STREAM_CANCEL", "The pending stream has been canceled"),
  ERR_HTTP2_INVALID_SESSION: () => __h2Err("ERR_HTTP2_INVALID_SESSION", "The session has been destroyed"),
  ERR_HTTP2_SOCKET_UNBOUND: () => __h2Err("ERR_HTTP2_SOCKET_UNBOUND", "The socket has been unbound from the session."),
  ERR_HTTP2_OUT_OF_BUFFERS: () => __h2Err("ERR_HTTP2_OUT_OF_BUFFERS", "Out of buffers"),
  ERR_HTTP2_HEADERS_OBJECT: () => __h2Err("ERR_HTTP2_HEADERS_OBJECT", "Headers must be an object"),
  ERR_HTTP2_UNSUPPORTED_PROTOCOL: (p) => __h2Err("ERR_HTTP2_UNSUPPORTED_PROTOCOL", `protocol "${p}" is unsupported.`),
  ERR_HTTP2_ALTSVC_INVALID_ORIGIN: () => __h2Err("ERR_HTTP2_ALTSVC_INVALID_ORIGIN", "HTTP/2 ALTSVC frames require a valid origin", "TypeError"),
  ERR_HTTP2_ALTSVC_LENGTH: () => __h2Err("ERR_HTTP2_ALTSVC_LENGTH", "HTTP/2 ALTSVC frames are limited to 16382 bytes", "TypeError"),
  ERR_HTTP2_INVALID_ORIGIN: () => __h2Err("ERR_HTTP2_INVALID_ORIGIN", "HTTP/2 ORIGIN frames require a valid origin", "TypeError"),
  ERR_HTTP2_ORIGIN_LENGTH: () => __h2Err("ERR_HTTP2_ORIGIN_LENGTH", "HTTP/2 ORIGIN frames are limited to 16382 bytes", "TypeError"),
  ERR_HTTP2_CONNECT_AUTHORITY: () => __h2Err("ERR_HTTP2_CONNECT_AUTHORITY", ":authority header is required for CONNECT requests"),
  ERR_HTTP2_CONNECT_PATH: () => __h2Err("ERR_HTTP2_CONNECT_PATH", "The :path header is forbidden for CONNECT requests"),
  ERR_HTTP2_CONNECT_SCHEME: () => __h2Err("ERR_HTTP2_CONNECT_SCHEME", "The :scheme header is forbidden for CONNECT requests"),
  ERR_HTTP2_HEADERS_AFTER_RESPOND: () => __h2Err("ERR_HTTP2_HEADERS_AFTER_RESPOND", "Cannot specify additional headers after response initiated"),
  ERR_HTTP2_INVALID_HEADER_VALUE: (v, n) => __h2Err("ERR_HTTP2_INVALID_HEADER_VALUE", `Invalid header value: "${v}" for header "${n}"`),
  ERR_HTTP2_NO_SOCKET_MANIPULATION: () => __h2Err("ERR_HTTP2_NO_SOCKET_MANIPULATION", "HTTP/2 sockets should not be directly manipulated (e.g. read and written)"),
  ERR_HTTP2_STREAM_SELF_DEPENDENCY: () => __h2Err("ERR_HTTP2_STREAM_SELF_DEPENDENCY", "A stream cannot depend on itself"),
  ERR_HTTP2_INVALID_STREAM: () => __h2Err("ERR_HTTP2_INVALID_STREAM", "The stream has been destroyed"),
  ERR_HTTP2_HEADERS_SENT: () => __h2Err("ERR_HTTP2_HEADERS_SENT", "Response has already been initiated."),
};
// node core.js：Http2Stream.priority 在 RFC 9113 废止优先级信令后弃用（DEP0194，进程级一次）。
let __h2PriorityWarned = false;
function __h2PriorityDeprecate() {
  if (__h2PriorityWarned) return;
  __h2PriorityWarned = true;
  process.emitWarning(
    "http2Stream.priority is longer supported after priority signalling was deprecated in RFC 9113",
    "DeprecationWarning", "DEP0194");
}
let __h2ReqPriorityWarned = false;
function __h2RequestPriorityDeprecate() {
  if (__h2ReqPriorityWarned) return;
  __h2ReqPriorityWarned = true;
  process.emitWarning("Priority signaling has been deprecated as of RFC 9113.", "DeprecationWarning", "DEP0194");
}
// node Http2Stream.close(code, callback) 门：code 须 uint32 整数、callback 须函数（有则）。
function __validateStreamClose(code, cb) {
  if (typeof code !== "number") throw __code("ERR_INVALID_ARG_TYPE", "code", "number", code);
  // node 口径分两档：非整数 `an integer`，越界整数 `>= 0 && <= 4294967295`
  //（invalidargtypes 套件 1.01/-1 双断言）。
  if (!Number.isInteger(code)) throw new codes.ERR_OUT_OF_RANGE("code", "an integer", code);
  if (code < 0 || code > 4294967295) {
    throw new codes.ERR_OUT_OF_RANGE("code", ">= 0 && <= 4294967295", code);
  }
  if (cb !== undefined && typeof cb !== "function") throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", cb);
}
// node getURLOrigin：`new URL(s).origin`——解析失败即抛 ERR_INVALID_URL（不吞），
// 非特殊协议（foo://bar）得 'null' 由调用方判 INVALID_ORIGIN。
function __urlOrigin(s) {
  return new URL(s).origin;
}
function __code(name, ...args) {
  const f = codes[name];
  // E() 工厂是 class（必须 new；返回对象的工厂 new 后同样取返回对象）
  if (typeof f === "function") { try { return new f(...args); } catch { /* fallthrough */ } }
  const g = __codes[name];
  if (typeof g === "function") return g(...args);
  return __h2Err(name, args.length ? String(args[0]) : name);
}
// node toHeaderObject 口径：重复值 cookie 以 "; " 并串、set-cookie 保数组、
// 其余以 ", " 并串（cookies 套件实测：abc → "1, 2, 3"、cookie → "a=b; c=d; e=f"）
function __pairsToObj(pairs) {
  const out = Object.create(null);
  for (const [k, v] of pairs) {
    const lk = String(k).toLowerCase();
    const val = String(v);
    if (out[lk] === undefined) out[lk] = val;
    else if (lk === "cookie") out[lk] = `${out[lk]}; ${val}`;
    else if (lk === "set-cookie") {
      out[lk] = Array.isArray(out[lk]) ? [...out[lk], val] : [out[lk], val];
    } else out[lk] = `${out[lk]}, ${val}`;
  }
  return out;
}
// node checkIsHttpToken 同款（header 名校验）
const __TOKEN_RE = /^[\^_`a-zA-Z\-0-9!#$%&'*+.|~]+$/;
function __validateHeaderName(name) {
  if (typeof name !== "string") {
    throw __code("ERR_INVALID_ARG_TYPE", "name", "string", name);
  }
  if (!__TOKEN_RE.test(name)) {
    throw __code("ERR_INVALID_HTTP_TOKEN", name);
  }
}
// node compat.js 口径：validateString 后 trim().toLowerCase()（headers 套件
// 点名 ` FOO-BAR\n\t` 与小写同键；get/has/remove/set/append/trailer 全走此口）。
function __normName(name) {
  if (typeof name !== "string") {
    throw __code("ERR_INVALID_ARG_TYPE", "name", "string", name);
  }
  return name.trim().toLowerCase();
}
function __validateHeaderValue(name, value) {
  if (value === undefined || value === null) {
    throw __code("ERR_HTTP2_INVALID_HEADER_VALUE", value, name);
  }
  if (Array.isArray(value)) {
    for (const v of value) __validateHeaderValue(name, v);
    return;
  }
  if (typeof value !== "string" && typeof value !== "number" && typeof value !== "boolean") {
    throw __code("ERR_INVALID_ARG_TYPE", `header "${name}"`, "string|number|boolean", value);
  }
}
// node validateLinkHeaderValue 逐字（write-early-hints 套件）：string 须合
// link 形、array 逐元校验后 ", " 并串、余者 ERR_INVALID_ARG_VALUE。
const __LINK_VALUE_RE = /^(?:<[^>\r\n]*>)(?:\s*;\s*[^;"\s]+(?:=(")?[^;"\s]*\1)?)*$/;
const __LINK_VALUE_REASON = 'must be an array or string of format "</styles.css>; rel=preload; as=style"';
function __validateLinkValue(value) {
  if (typeof value === "string") {
    if (!__LINK_VALUE_RE.test(value)) {
      throw __code("ERR_INVALID_ARG_VALUE", "hints", value, __LINK_VALUE_REASON);
    }
    return value;
  }
  if (Array.isArray(value)) {
    let out = "";
    for (let i = 0; i < value.length; i++) {
      if (typeof value[i] !== "string" || !__LINK_VALUE_RE.test(value[i])) {
        throw __code("ERR_INVALID_ARG_VALUE", "hints", value[i], __LINK_VALUE_REASON);
      }
      out += value[i];
      if (i !== value.length - 1) out += ", ";
    }
    return out;
  }
  throw __code("ERR_INVALID_ARG_VALUE", "hints", value, __LINK_VALUE_REASON);
}
function __headerToWire(value) {
  if (Array.isArray(value)) return value.map((v) => String(v));
  return String(value);
}

// ── internal/http2/util 同款校验（套件直接可对齐；10g 欠账 G10）──────────────
const __PSEUDO_RE = /^:[a-zA-Z0-9_]+$/;
function assertValidPseudoHeader(key) {
  if (!__PSEUDO_RE.test(key)) throw __code("ERR_HTTP2_INVALID_PSEUDOHEADER", key);
}
function assertIsObject(val, name, type) {
  if (val === undefined || val === null || (typeof val !== "object" && typeof val !== "function") || Array.isArray(val)) {
    throw __code("ERR_INVALID_ARG_TYPE", name, type ?? "Object", val);
  }
}
function assertWithinRange(name, value, min, max) {
  if (typeof value !== "number" || !Number.isInteger(value) || value < min || value > max) {
    throw __code("ERR_HTTP2_INVALID_SETTING_VALUE", name, value);
  }
}
// connection-specific 头（RFC 7540 §8.1.2.2，node isConnectionSpecificHeader 同表）
const __CONN_HEADERS = new Set(["connection", "upgrade", "http2-settings", "te", "transfer-encoding", "keep-alive", "proxy-connection"]);
// node validateH2Headers：伪头白名单（respond 只允许 :status；trailers 全禁）+ 单值
function __validateH2Headers(headers, allowedPseudo = []) {
  if (headers === null || typeof headers !== "object") throw __code("ERR_HTTP2_HEADERS_OBJECT");
  for (const key of Object.keys(headers)) {
    const lk = String(key).toLowerCase();
    if (lk.startsWith(":")) {
      if (!allowedPseudo.includes(lk)) throw __code("ERR_HTTP2_INVALID_PSEUDOHEADER", lk);
      const v = headers[key];
      if (Array.isArray(v)) {
        if (v.length !== 1) throw __code("ERR_HTTP2_HEADER_SINGLE_VALUE", key);
      } else if (v === undefined) {
        throw __code("ERR_HTTP2_INVALID_HEADER_VALUE", v, key);
      }
    } else {
      __validateHeaderName(String(key));
      __validateHeaderValue(String(key), headers[key]);
    }
  }
}
// node util.js kSingleValueHeaders：大小写变体重复或数组多值即 HEADER_SINGLE_VALUE。
const __SINGLE_VALUE_HEADERS = new Set([
  ":status", ":method", ":authority", ":scheme", ":path", ":protocol",
  "access-control-allow-credentials", "access-control-max-age", "access-control-request-method",
  "age", "authorization", "content-encoding", "content-language", "content-length",
  "content-location", "content-md5", "content-range", "content-type", "date", "dnt", "etag",
  "expires", "from", "host", "if-match", "if-modified-since", "if-none-match", "if-range",
  "if-unmodified-since", "last-modified", "location", "max-forwards", "proxy-authorization",
  "range", "referer", "retry-after", "tk", "upgrade-insecure-requests", "user-agent",
  "x-content-type-options",
]);
// node mapToHeaders 的校验面：非伪头名须是 HTTP token（ERR_INVALID_HTTP_TOKEN），单值头不得重复。
const __REQ_PSEUDO = new Set([":method", ":authority", ":scheme", ":path", ":protocol"]);
function __mapToHeadersCheck(headers) {
  const seen = new Set();
  for (const key of Object.keys(headers)) {
    const lk = String(key).toLowerCase();
    if (lk.startsWith(":") && !__REQ_PSEUDO.has(lk)) throw __code("ERR_HTTP2_INVALID_PSEUDOHEADER", lk);
  }
  for (const key of Object.keys(headers)) {
    const lk = String(key).toLowerCase();
    const v = headers[key];
    if (__SINGLE_VALUE_HEADERS.has(lk)) {
      if (seen.has(lk) || (Array.isArray(v) && v.length > 1)) throw __code("ERR_HTTP2_HEADER_SINGLE_VALUE", lk);
      seen.add(lk);
    }
    if (!lk.startsWith(":") && lk !== "x-wjs-pho") __validateHeaderName(String(key));
  }
}
// node request() 选项类型门（validateBoolean/validateNumber 同码 ERR_INVALID_ARG_TYPE）。
function __validateRequestOptions(options) {
  if (options === undefined || options === null) return;
  for (const [k, t] of [["endStream", "boolean"], ["exclusive", "boolean"], ["silent", "boolean"],
    ["waitForTrailers", "boolean"], ["parent", "number"], ["weight", "number"]]) {
    const v = options[k];
    if (v !== undefined && typeof v !== t) throw __code("ERR_INVALID_ARG_TYPE", `options.${k}`, t, v);
  }
}
// 设置项取值/校验（node updateSettingsInternal 同口径；customSettings 放行）
const __SETTING_RANGES = {
  headerTableSize: [0, 0xffffffff],
  enablePush: "boolean",
  initialWindowSize: [0, 0x7fffffff],
  maxFrameSize: [16384, 16777215],
  maxConcurrentStreams: [0, 0xffffffff],
  maxHeaderListSize: [0, 0xffffffff],
  maxHeaderSize: [0, 0xffffffff],
  enableConnectProtocol: "boolean",
};
function __settingErr(name, v, isBool) {
  // node：数值档 RangeError、布尔档 TypeError（getpackedsettings 套件逐字）
  const e = __h2Err("ERR_HTTP2_INVALID_SETTING_VALUE", `Invalid value for setting "${name}": ${v}`, isBool ? "TypeError" : "RangeError");
  return e;
}
function __validateSettings(settings, argName = "settings") {
  if (settings === null || typeof settings !== "object") {
    throw __code("ERR_INVALID_ARG_TYPE", argName, "object", settings);
  }
  const out = {};
  for (const key of Object.keys(settings)) {
    const v = settings[key];
    const spec = __SETTING_RANGES[key];
    if (spec === undefined) {
      if (key === "customSettings") {
        out[key] = { ...v };
        continue;
      }
      continue; // 未知设置项忽略（node 静默忽略未知名——non-critical）
    }
    if (spec === "boolean") {
      if (typeof v !== "boolean") throw __settingErr(key, v, true);
      out[key] = v;
    } else if (typeof v !== "number" || !Number.isInteger(v) || v < spec[0] || v > spec[1]) {
      throw __settingErr(key, v, false);
    } else {
      out[key] = v;
    }
  }
  return out;
}
function __applySettings(base, extra) {
  return { ...base, ...extra };
}
const __DEFAULT_SETTINGS = {
  headerTableSize: 4096,
  enablePush: true,
  initialWindowSize: 65535,
  maxFrameSize: 16384,
  maxConcurrentStreams: 4294967295,
  // node 真机：两键皆 u32max（真机 remoteSettings/localSettings 实测；
  // 旧 65535 系误植，too-large-headers 套件点名）。
  maxHeaderListSize: 4294967295,
  maxHeaderSize: 4294967295,
  enableConnectProtocol: false,
};

// ── socket 代理（node socketProxyPair 口径：net.Socket 假面 + 流委派）────────
const __SOCK_MANIP_KEYS = ["read", "write", "pause", "resume"];
function __socketManipErr() {
  return __code("ERR_HTTP2_NO_SOCKET_MANIPULATION");
}
function __mkSocketProxy(stream, server, peerObj) {
  const base = Object.create(net.Socket.prototype);
  Object.defineProperty(base, "connecting", { value: false, writable: true, configurable: true });
  const handler = {
    get(t, prop) {
      if (prop === "readable" || prop === "writable") return stream[prop];
      if (prop === "destroyed") return stream.__destroyed;
      if (__SOCK_MANIP_KEYS.includes(prop)) throw __socketManipErr();
      if (prop === "on" || prop === "once" || prop === "emit" ||
          prop === "end") {
        return stream[prop].bind(stream);
      }
      if (prop === "destroy") {
        // node：socket.destroy() 杀整个连接（stream.session.destroy 同收尾）
        return (...args) => {
          stream.destroy(...args);
          stream.session.destroy();
        };
      }
      if (prop === "setTimeout") return stream.session.setTimeout.bind(stream.session);
      if (prop === "address") return () => server.address();
      if (prop === "remoteAddress") return peerObj.addr;
      if (prop === "remotePort") return peerObj.port;
      if (prop === "localAddress") return server.__listening ? server.__listening.address : undefined;
      if (prop === "localPort") return server.__listening ? server.__listening.port : undefined;
      if (prop === "_server") return server;
      const v = Reflect.get(t, prop);
      if (v !== undefined || prop in t) return v;
      // node socketProxy 口径：未知属性一律走 session socket 背书
      //（socket-set 套件：request.socket._isProcessing 与 session.socket 互通）。
      const sess = stream.__session;
      if (sess) {
        const sock = sess.socket;
        const sv = sock[prop];
        return typeof sv === "function" ? sv.bind(sock) : sv;
      }
      return server.__sockBag[prop];
    },
    set(t, prop, value) {
      if (prop === "readable") { stream.readable = !!value; return true; }
      if (prop === "writable") { stream.writable = !!value; return true; }
      if (prop === "destroyed") { stream.__destroyed = !!value; return true; }
      if (__SOCK_MANIP_KEYS.includes(prop)) throw __socketManipErr();
      if (prop === "on" || prop === "once" || prop === "emit" ||
          prop === "end" || prop === "destroy") {
        stream[prop] = value;
        return true;
      }

      if (prop === "setTimeout") { stream.session.setTimeout = value; return true; }
      // 未知属性落 session socket（与 get 对称，node socketProxy 口径）。
      if (stream.__session) { stream.__session.socket[prop] = value; return true; }
      t[prop] = value;
      server.__sockBag[prop] = value;
      return true;
    },
  };
  return new Proxy(base, handler);
}

// ── 会话级 socket（session.socket：EventEmitter 假面 + peer 反射）────────────
function __mkSessionSocket(session, peerObj, localObj) {
  const sock = new EventEmitter();
  sock.remoteAddress = peerObj.addr;
  sock.remotePort = peerObj.port;
  sock.localAddress = localObj?.address;
  sock.localPort = localObj?.port;
  sock.remoteFamily = peerObj.addr?.includes(":") ? "IPv6" : "IPv4";
  sock.connecting = false;
  sock.destroyed = false;
  sock.readable = true;
  sock.writable = true;
  sock.destroy = (err) => {
    if (sock.destroyed) return;
    sock.destroyed = true;
    if (err) sock.emit("error", err);
    session.destroy();
    // node：会话销毁后 socket 发 'close'（server-socket-destroy 套件点名）。
    queueMicrotask(() => sock.emit("close"));
  };
  sock.end = () => { session.close(); return sock; };
  sock.write = () => true;
  setTimeout(() => { if (!session.destroyed) sock.emit("connect"); }, 0);
  return sock;
}

// ── 客户端事件面（SETTINGS 解码 + RST 码面；h2_client.js __ev 委托，守 1000 行门）──
// RST 码 → node 流错误（stream-destroy-event-order 真机逐项）：
// 0/8（NO_ERROR/CANCEL）干净无错；已知码 NGHTTP2_名；未知码用裸数字。
const __NGHTTP2_NAMES = {
  1: "NGHTTP2_PROTOCOL_ERROR", 2: "NGHTTP2_INTERNAL_ERROR",
  3: "NGHTTP2_FLOW_CONTROL_ERROR", 4: "NGHTTP2_SETTINGS_TIMEOUT",
  5: "NGHTTP2_STREAM_CLOSED", 6: "NGHTTP2_FRAME_SIZE_ERROR",
  7: "NGHTTP2_REFUSED_STREAM", 8: "NGHTTP2_CANCEL",
  9: "NGHTTP2_COMPRESSION_ERROR", 10: "NGHTTP2_CONNECT_ERROR",
  11: "NGHTTP2_ENHANCE_YOUR_CALM", 12: "NGHTTP2_INADEQUATE_SECURITY",
  13: "NGHTTP2_HTTP_1_1_REQUIRED",
};
function __h2RstError(code) {
  const n = Number(code);
  if (!Number.isInteger(n) || n === 0 || n === 8) return undefined;
  return __h2Err("ERR_HTTP2_STREAM_ERROR",
    `Stream closed with error code ${__NGHTTP2_NAMES[n] ?? n}`);
}
// 服务端 SETTINGS id→名（getUnpackedSettings 同表；未知 id 忽略）。
function __decodeRemotePairs(pairs) {
  const upd = {};
  for (const [sid, val] of pairs) {
    switch (Number(sid)) {
      case 0x1: upd.headerTableSize = val; break;
      case 0x2: upd.enablePush = val !== 0; break;
      case 0x3: upd.maxConcurrentStreams = val; break;
      case 0x4: upd.initialWindowSize = val; break;
      case 0x5: upd.maxFrameSize = val; break;
      case 0x6: upd.maxHeaderListSize = val; upd.maxHeaderSize = val; break;
      case 0x8: upd.enableConnectProtocol = val !== 0; break;
      default: break;
    }
  }
  return upd;
}
// ECP 1→0 毒化（cant-turn-off）：node 客户端把 ECP 1→0 当连接错误。
// 开流/待开流先报，再报会话错（探针序：REQ error → REQ close → CLIENT error）。
// 返 true = 已毒化（调用方 break，不再发 remoteSettings）。
function __maybeProtoPoison(session, upd) {
  if (upd.enableConnectProtocol !== false || session.__sawECP1 !== true) return false;
  const err = __h2Err("ERR_HTTP2_ERROR", "Protocol error");
  for (const [, st] of session.__streams) {
    st.emit("error", err);
    st.destroy();
  }
  session.__streams.clear();
  for (const pst of session.__pendingOpens.splice(0)) {
    pst.emit("error", err);
    pst.destroy();
  }
  session.emit("error", err);
  session.destroy();
  return true;
}
// 客户端流 aborted（h2_client.js __onAborted 委托；放此处守 1000 行门）。
// 干净收尾只发 aborted（destroy 探针）；对端 RST 置 aborted 并按码错误。
function __clientOnAborted(st, rstCode) {
  if (st.aborted) return;
  const n = Number(rstCode);
  const hasCode = Number.isInteger(n) && (rstCode !== undefined && rstCode !== null && rstCode !== "");
  const err = hasCode ? __h2RstError(n) : undefined;
  if (hasCode && n !== 0) {
    st.aborted = true;
    st.rstCode = n;
  }
  st.emit("aborted");
  st.push(null);
  if (err !== undefined) st.destroy(err);
  else st.destroy();
}
// GOAWAY 非零码的会话级销毁（propagate-session-destroy-code）：
// 全流 SESSION_ERROR + 会话 error + 销毁（closed 保持 false，connClose 后才 close）。
function __goawaySessionDestroy(session, code) {
  if (session.destroyed) return;
  const serr = __h2Err("ERR_HTTP2_SESSION_ERROR", `Session closed with error code ${code}`);
  for (const [, st] of session.__streams) {
    if (!st.destroyed) {
      st.rstCode = code;
      st.emit("error", serr);
      st.destroy();
      session.__goneStreams.add(st.id);
    }
  }
  session.__streams.clear();
  for (const pst of session.__pendingOpens.splice(0)) {
    pst.emit("error", serr);
    pst.destroy();
  }
  session.destroyed = true;
  session.emit("error", serr);
  // 连接拆除走正常 teardown（Close 臂 abort 在途任务收尾）。
  if (session.__id) {
    __wjs2_net_destroy(session.__id);
    session.__id = 0;
  }
}
// GOAWAY 非零码的流级翻 mapping（too-large-headers：h2 线上 GOAWAY(11)，
// node 线上 RST_STREAM(11)——未收应答的在途流翻成对应流错误）。
function __goawayStreamErrors(session, inner) {
  const code = Number(inner.code);
  const lastId = Number(inner.lastId ?? 0);
  if (code !== 11) return;
  const err = __h2Err("ERR_HTTP2_STREAM_ERROR",
    "Stream closed with error code NGHTTP2_ENHANCE_YOUR_CALM");
  for (const [sid, st] of session.__streams) {
    if (Number(sid) > lastId && !st.__responseReceived && !st.destroyed) {
      st.rstCode = 11;
      st.emit("error", err);
      st.destroy();
    }
  }
}

// ── 客户端开流（h2_client.js ClientHttp2Stream.__openNow 委托；放此处守 1000 行门）
function __clientOpenNow(st, extraBody, streaming = false) {
  if (st.__opened) return;
  st.__opened = true;
  st.__streaming = !!streaming;
  const parts = extraBody ? [...st.__pendingBody, extraBody] : st.__pendingBody;
  st.__pendingBody = [];
  let total = 0;
  for (const b of parts) total += b.length;
  const flat = new Uint8Array(total);
  let off = 0;
  for (const b of parts) { flat.set(b, off); off += b.length; }
  try {
    __wjs2_h2_open(st.__session.__id, st.id,
      JSON.stringify({
        method: st.sentHeaders[":method"], path: st.sentHeaders[":path"],
        scheme: st.sentHeaders[":scheme"], authority: st.sentHeaders[":authority"],
        // 扩展 CONNECT 的 `:protocol`（RFC 8441；hyper 经 extensions 上线）
        protocol: st.sentHeaders[":protocol"] ?? "",
        waitTrailers: st.__waitTrailers,
        // 流式上传（cancel-while-client-reading）：首块随头出线但流保持
        // 开放，后续块经 __wjs2_h2_data、收尾经 __wjs2_h2_end。
        streaming: !!st.__streaming,
        // node request() 缺省：无 body 意图的方法（GET/HEAD/DELETE）END_STREAM
        // 随头出线（endafterheaders 套件）；POST 等分开发。native 不再按体空猜。
        endStream: !!st.__endStream,
        headers: Object.entries(st.sentHeaders)
          .filter(([k]) => !k.startsWith(":"))
          .map(([k, v]) => [
            k,
            // node prepareRequestHeaders 口径：cookie 数组 "; " 并串为单条线
            (k === "cookie" && Array.isArray(v)) ? v.join("; ") : v,
          ]),
      }),
      __b64enc(flat));
  } catch {
    // 会话已亡（销毁竞态）：流随会话终止，不再上抛
    st.__detachFromSession();
  }
  if (st.__waitTrailers) {
    queueMicrotask(() => {
      if (st.destroyed) return;
      st.__wantTrailersFired = true;
      st.emit("wantTrailers");
    });
  }
}

// ── Http2Session（服务端每连接一个；'session' 事件载体）──────────────────────
let __h2SessionSeq = 1;
class Http2Session extends EventEmitter {
  constructor(server, connId, peerObj) {
    super();
    this.__id = __h2SessionSeq++;
    this.__server = server;
    this.__conn = connId;
    this.type = 0; // NGHTTP2_SESSION_SERVER
    this.encrypted = !!server.__secure;
    this.connecting = false;
    this.destroyed = false;
    this.closed = false;
    this.__peer = peerObj ?? { addr: undefined, port: 0 };
    this.__settings = { ...__DEFAULT_SETTINGS, ...(server.__opts?.settings ?? {}) };
    this.__remoteSettings = { ...__DEFAULT_SETTINGS };
    this.__pendingSettingsAck = false;
    this.__outstandingSettings = 0;
    this.__maxOutstandingSettings = server.__opts?.maxOutstandingSettings ?? Infinity;
    // node 缺省 maxOutstandingPings = 2（ping.js 超额即 CANCEL）
    this.__maxOutstandingPings = server.__opts?.maxOutstandingPings ?? 2;
    this.__streams = new Map();
    this.state = {
      effectiveLocalWindowSize: 65535,
      effectiveRemoteWindowSize: 65535,
      localWindowSize: 65535,
      remoteWindowSize: 65535,
      outboundQueueSize: 0,
      deflateDynamicTableSize: 4096,
      inflateDynamicTableSize: 4096,
    };
    // node core.js：会话构造即绑 kSocket（server-socket-destroy 套件直探
    // session[kSocket]，懒 getter 在直接访问时给 undefined）。
    this[kSocket] = __mkSessionSocket(this, this.__peer, server.__listening);
    this.__ev = this.__ev.bind(this);
  }
  get socket() {
    if (this[kSocket] === undefined) {
      this[kSocket] = __mkSessionSocket(this, this.__peer, this.__server.__listening);
    }
    return this[kSocket];
  }
  get localSettings() { return this.__settings; }
  get remoteSettings() { return this.__remoteSettings; }
  get pendingSettingsAck() { return this.__pendingSettingsAck; }
  get originSet() { return this.encrypted ? undefined : undefined; }
  get alpnProtocol() { return this.encrypted ? "h2" : "h2c"; }
  get unrefed() { return false; }
  setTimeout(msecs, callback) {
    if (typeof msecs !== "number") throw __code("ERR_INVALID_ARG_TYPE", "msecs", "number", msecs);
    if (callback !== undefined && typeof callback !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", callback);
    }
    if (typeof callback === "function") this.once("timeout", callback);
    const ms = Number(msecs) || 0;
    // 套件直探 `session[kTimeout]._idleTimeout`（compat-socket 口径）。
    if (this[kTimeout] === undefined || this[kTimeout] === null) this[kTimeout] = { _idleTimeout: 0 };
    this[kTimeout]._idleTimeout = ms;
    if (this.__timeoutTimer !== undefined) clearTimeout(this.__timeoutTimer);
    if (ms > 0 && !this.closed && !this.destroyed) {
      this.__timeoutTimer = setTimeout(() => {
        this.__timeoutTimer = undefined;
        this.emit("timeout");
      }, ms);
    }
    return this;
  }
  ref() { if (this.__conn) __wjs2_net_ref(this.__conn); return this; }
  unref() { if (this.__conn) __wjs2_net_unref(this.__conn); return this; }
  settings(settings = {}, cb) {
    const validated = __validateSettings(settings);
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    // ECP 1→0（cant-turn-off）：node 把"开后关"当连接级 PROTOCOL_ERROR——
    // 服务端发坏 SETTINGS 后会话死、客户端连接错。hyper 无 mid-connection
    // SETTINGS API：stream_id 0 的 reset 令 Rust  waiter 在握手 SETTINGS
    // 落线后直写坏帧（wire-faithful），连接不断（node 同形：坏帧后 socket
    // 照开，收尾由客户端 teardown 触发）。本地会话记毒化：后续上线
    // HEADERS 直接丢弃（对端残留 HEADERS 到达时不再建流）。
    if (validated.enableConnectProtocol === false && this.__settings.enableConnectProtocol === true) {
      this.__settings = __applySettings(this.__settings, validated);
      this.__pendingSettingsAck = true;
      this.__poisoned = true;
      if (this.__conn) __wjs2_h2_reset(this.__conn, 0, 1);
      setTimeout(() => {
        this.__pendingSettingsAck = false;
        if (!this.destroyed) {
          this.emit("localSettings", this.__settings);
          if (typeof cb === "function") cb();
        }
      }, 1);
      return this;
    }
    this.__pendingSettingsAck = true;
    this.__outstandingSettings++;
    if (Number.isFinite(this.__maxOutstandingSettings) &&
        this.__outstandingSettings >= this.__maxOutstandingSettings) {
      this.__outstandingSettings = 0;
      this.__pendingSettingsAck = false;
      process.nextTick(() => {
        this.emit("error", __code("ERR_HTTP2_MAX_PENDING_SETTINGS_ACK"));
        this.destroy();
      });
      return this;
    }
    setTimeout(() => {
      this.__outstandingSettings = Math.max(0, this.__outstandingSettings - 1);
      if (this.__outstandingSettings === 0) this.__pendingSettingsAck = false;
      this.__settings = __applySettings(this.__settings, validated);
      if (!this.destroyed) {
        this.emit("localSettings", this.__settings);
        if (typeof cb === "function") cb();
      }
    }, 1);
    return this;
  }
  updateSettings(settings) {
    const validated = __validateSettings(settings);
    this.__settings = __applySettings(this.__settings, validated);
    return this;
  }
  // node：windowSize 校验（number、0..2^31-1）后仅本地状态面（flow control
  // 由底座管理；setLocalWindowSize-errors/套件只断言校验与 state 反映）
  setLocalWindowSize(windowSize) {
    // node：destroyed 校验先于实参校验（client-destroy 套件）
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    if (typeof windowSize !== "number") {
      throw __code("ERR_INVALID_ARG_TYPE", "windowSize", "number", windowSize);
    }
    if (windowSize < 0 || windowSize > 2147483647) {
      throw __code("ERR_OUT_OF_RANGE", "windowSize", ">= 0 && <= 2147483647", windowSize);
    }
    this.state.effectiveLocalWindowSize = windowSize;
    this.state.localWindowSize = windowSize;
    return this;
  }
  // node 口径：ping([payload, ]callback)；payload 非 ArrayBufferView →
  // TypeError、长度≠8 → RangeError ERR_HTTP2_PING_LENGTH、超 maxOutstandingPings
  // → 返 false 且回调 ERR_HTTP2_PING_CANCEL（真机 ping.js/onping 实测）
  ping(payload, callback) {
    if (typeof payload === "function") { callback = payload; payload = undefined; }
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    let buf = null;
    if (payload !== undefined && payload !== null) {
      if (typeof payload !== "object" || !ArrayBuffer.isView(payload)) {
        throw __code("ERR_INVALID_ARG_TYPE", "payload", ["Buffer", "TypedArray", "DataView"], payload);
      }
      const u8 = payload instanceof Uint8Array ? payload : new Uint8Array(payload.buffer, payload.byteOffset, payload.byteLength);
      if (u8.length !== 8) throw __code("ERR_HTTP2_PING_LENGTH");
      buf = u8;
    }
    if (typeof callback !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", callback);
    }
    const cap = Number.isInteger(this.__maxOutstandingPings) ? this.__maxOutstandingPings : 2;
    if ((this.__outstandingPings ?? 0) >= cap) {
      const cancel = __code("ERR_HTTP2_PING_CANCEL");
      queueMicrotask(() => callback(cancel));
      return false;
    }
    this.__outstandingPings = (this.__outstandingPings ?? 0) + 1;
    const ret = Buffer.alloc(8);
    if (buf) Buffer.from(buf.buffer, buf.byteOffset, buf.byteLength).copy(ret);
    setTimeout(() => {
      this.__outstandingPings = Math.max(0, (this.__outstandingPings ?? 1) - 1);
      if (this.destroyed) { callback(__code("ERR_HTTP2_PING_CANCEL")); return; }
      callback(null, 1, ret);
    }, 1);
    return true;
  }
  goaway(code = 0, lastStreamID = 0, opaqueData) {
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    // node core.js 逐字：三参各自类型门（无对象形重载）。
    if (typeof code !== "number") throw __code("ERR_INVALID_ARG_TYPE", "code", "number", code);
    if (typeof lastStreamID !== "number") throw __code("ERR_INVALID_ARG_TYPE", "lastStreamID", "number", lastStreamID);
    if (opaqueData !== undefined && !ArrayBuffer.isView(opaqueData)) {
      throw __code("ERR_INVALID_ARG_TYPE", "opaqueData", ["Buffer", "TypedArray", "DataView"], opaqueData);
    }
    this.__goaway = { code, lastStreamID, opaqueData };
    // 底座无 GOAWAY 帧下发（偏差记档）：本会话立即收尾（node goaway 后不再收流）。
    setImmediate(() => this.close());
    return this;
  }
  setNextStreamID(id) {
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    if (typeof id !== "number") throw __code("ERR_INVALID_ARG_TYPE", "id", "number", id);
    if (!Number.isInteger(id) || id <= 0 || id > 4294967295) {
      throw new codes.ERR_OUT_OF_RANGE("id", "> 0 and <= 4294967295", id);
    }
    this.__nextStreamID = id;
    return this;
  }
  // ALTSVC/ORIGIN 帧无底座（偏差记档）：参数校验逐字移植 node core.js，校验过即 no-op。
  altsvc(alt, originOrStream) {
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    let origin;
    if (typeof originOrStream === "string") {
      origin = __urlOrigin(originOrStream);
      if (origin === "null") throw __code("ERR_HTTP2_ALTSVC_INVALID_ORIGIN");
    } else if (typeof originOrStream === "number") {
      if (originOrStream >>> 0 !== originOrStream || originOrStream === 0) {
        throw new codes.ERR_OUT_OF_RANGE("originOrStream", `> 0 && < ${2 ** 32}`, originOrStream);
      }
    } else if (originOrStream !== undefined) {
      if (originOrStream !== null && typeof originOrStream === "object") origin = originOrStream.origin;
      if (typeof origin !== "string") {
        throw __code("ERR_INVALID_ARG_TYPE", "originOrStream", ["string", "number", "URL", "object"], originOrStream);
      } else if (origin === "null" || origin.length === 0) {
        throw __code("ERR_HTTP2_ALTSVC_INVALID_ORIGIN");
      }
    }
    if (typeof alt !== "string") throw __code("ERR_INVALID_ARG_TYPE", "alt", "string", alt);
    if (!/^[\x09\x20-\x5b\x5d-\x7e\x80-\xff]*$/.test(alt)) throw new codes.ERR_INVALID_CHAR(undefined, "alt");
    if (alt.length + (origin !== undefined ? origin.length : 0) > 16382) throw __code("ERR_HTTP2_ALTSVC_LENGTH");
    return this;
  }
  origin(...origins) {
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    let len = 0;
    for (let origin of origins) {
      if (typeof origin === "string") origin = __urlOrigin(origin);
      else if (origin != null && typeof origin === "object") origin = origin.origin;
      if (typeof origin !== "string") throw __code("ERR_INVALID_ARG_TYPE", "origin", "string", origin);
      if (origin === "null") throw __code("ERR_HTTP2_INVALID_ORIGIN");
      len += origin.length;
    }
    if (len > 16382) throw __code("ERR_HTTP2_ORIGIN_LENGTH");
  }
  __registerStream(stream) {
    this.__streams.set(stream.id, stream);
  }
  __unregisterStream(stream) {
    this.__streams.delete(stream.id);
  }
  close(cb) {
    if (typeof cb === "function") this.once("close", cb);
    if (this.closed || this.destroyed) {
      if (typeof cb === "function") process.nextTick(cb);
      return this;
    }
    this.closed = true;
    if (this.__conn) {
      __wjs2_net_destroy(this.__conn);
    } else {
      setImmediate(() => this.__finish());
    }
    return this;
  }
  destroy(code = 0, cb) {
    if (typeof code === "function") { cb = code; code = 0; }
    if (typeof cb === "function") this.once("close", cb);
    if (this.destroyed) return this;
    this.destroyed = true;
    // node session.destroy(code) 口径（propagate-session-destroy-code）：
    // 非零非 CANCEL 码 → 开流逐个 RST(code) + 流错误（SESSION_ERROR）+
    // 会话 error（同错）；closed 保持 false 直到 connClose（测试点名）。
    // 干净码走旧路（优雅关，不断流语义不变）。
    const c = Number(code) || 0;
    if (c !== 0 && c !== 8) {
      const serr = __code("ERR_HTTP2_SESSION_ERROR", c);
      // GOAWAY(code) 先行（客户端凭此判会话级销毁，逐流 RST 随后）；
      // 魔数流号经 H2RespondReset 复用通道（新 native 需 mod.rs，另案）。
      if (this.__conn) __wjs2_h2_reset(this.__conn, 0xffffffff, c);
      for (const [, st] of this.__streams) {
        try { __wjs2_h2_reset(this.__conn, st.id, c); } catch {}
        st.rstCode = c;
        st.emit("error", serr);
        st.destroy();
      }
      this.__streams.clear();
      this.emit("error", serr);
    }
    // closed 不置位（connClose → __finish 置位并发 close；测试断言 destroy 后 closed=false）。
    // 不经 close() 委派（destroyed 置位后 close 直接返回，连接永不拆除——旧坑）。
    if (this.__conn) {
      __wjs2_net_destroy(this.__conn);
    } else {
      setImmediate(() => this.__finish());
    }
    return this;
  }
  __finish() {
    if (this.__finished) return;
    this.__finished = true;
    this.closed = true;
    this.destroyed = true;
    for (const [, st] of this.__streams) {
      if (!st.destroyed) st.destroy();
    }
    this.__streams.clear();
    queueMicrotask(() => this.emit("close"));
  }
  __ev(kind, payload) {
    switch (kind) {
      case "connClose": {
        this.__server.__sessions.delete(this.__conn);
        this.__finish();
        break;
      }
    }
  }
}
