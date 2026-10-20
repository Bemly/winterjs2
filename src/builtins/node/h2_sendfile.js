// ── respondWithFile/respondWithFD 投递核（node lib/internal/http2/core.js
// doSendFileFD/doSendFD/processRespondWithFD/startFilePipe 逐字移植；
// R19 C3 WIP 半截重构已弃后重做）────────────────────────────────────────────
// 入口 __h2SendFile(stream, src, headers, options, isFd)：
//   file（src=path）：ownsFd=true，open→fstat→类检→statCheck→应答→泵体；
//   fd（src=number|FileHandle）：ownsFd=false，无 open/类检（node 口径）。
// 非阻塞面：FIFO open/读永不卡 JS 线程——读端 O_NONBLOCK + setImmediate 轮询
// （node 靠线程池消化阻塞语义；本仓回调面同步底座，见 fs_async.js __fsOpenFifo）。
import { fileURLToPath } from "node:url";
// 路径归一（statSync/openSync 经 __fsPath 自带；nb-open 需预转串）。
function __h2PathStr(p) {
  if (typeof p === "string") return p;
  if (ArrayBuffer.isView(p)) return Buffer.from(p).toString("utf8");
  try { return fileURLToPath(p); } catch { return String(p); }
}
function __h2SendFile(stream, src, headers, options, isFd) {
  if (stream.destroyed) throw __code("ERR_HTTP2_INVALID_STREAM");
  if (stream.__headersSent) throw __code("ERR_HTTP2_HEADERS_SENT");
  if (options === undefined) options = {};
  if (options === null || typeof options !== "object") {
    throw __code("ERR_INVALID_ARG_TYPE", "options", "object", options);
  }
  if (options.offset !== undefined && typeof options.offset !== "number") {
    throw __code("ERR_INVALID_ARG_VALUE", "options.offset", options.offset);
  }
  if (options.length !== undefined && typeof options.length !== "number") {
    throw __code("ERR_INVALID_ARG_VALUE", "options.length", options.length);
  }
  if (options.statCheck !== undefined && typeof options.statCheck !== "function") {
    throw __code("ERR_INVALID_ARG_VALUE", "options.statCheck", options.statCheck);
  }
  let fd = null;
  if (isFd) {
    // node 口径：FileHandle 实例取 .fd，余者 ARG_TYPE 双形数组
    //（'of type number or an instance of FileHandle'，中央表按数组渲染）。
    if (src !== null && typeof src === "object" && typeof src.fd === "number") fd = src.fd;
    else if (typeof src === "number") fd = src;
    else throw __code("ERR_INVALID_ARG_TYPE", "fd", ["number", "FileHandle"], src);
  }
  if (headers === null || headers === undefined) headers = {};
  // 204/205/304 与 HEAD 禁 payload（node：prepare 后 statusCode 门 + headRequest）。
  const __st = headers[":status"] !== undefined ? (+headers[":status"] | 0) : 200;
  if (__st === 204 || __st === 205 || __st === 304 || stream.__headRequest === true) {
    throw __code("ERR_HTTP2_PAYLOAD_FORBIDDEN", __st);
  }
  const onError = options.onError;
  // node afterOpen 口径：open 错 → onError 否则 destroy（file 种）。
  const failOpen = (err) => {
    if (typeof onError === "function") { onError(err); return; }
    stream.destroy(err);
  };
  if (!isFd) {
    // 路径种：stat 先行嗅探（FIFO 即 nb 路径；常规/缺失走同步 openSync，
    // 失败快、不阻塞——阻塞只发生在特殊文件上）。
    let fifo = false;
    try { const __pst = fs.statSync(src); fifo = !!(__pst && typeof __pst.isFIFO === "function" && __pst.isFIFO()); }
    catch { fifo = false; }
    if (fifo) {
      __h2OpenFifoPoll(stream, src, headers, options, onError);
      return;
    }
    try {
      fd = fs.openSync(src, "r");
    } catch (err) { failOpen(err); return; }
    let stat;
    try { stat = fs.fstatSync(fd); }
    catch (err) { try { fs.closeSync(fd); } catch {} failOpen(err); return; }
    __h2SendFileStat(stream, fd, stat, headers, options, onError, true);
    return;
  }
  // fd 种：fstat 错即 destroy（node doSendFD 口径：无 onError 路由）。
  let fstat;
  try { fstat = fs.fstatSync(fd); }
  catch (err) { stream.destroy(err); return; }
  __h2SendFdStat(stream, fd, fstat, headers, options);
}
// FIFO 读端：O_NONBLOCK 轮询 rendezvous（ENXIO/EAGAIN=对端未到），余错走 failOpen。
function __h2OpenFifoPoll(stream, path, headers, options, onError) {
  const failOpen = (err) => {
    if (typeof onError === "function") { onError(err); return; }
    stream.destroy(err);
  };
  const attempt = () => {
    if (stream.destroyed || stream.__closed) {
      if (typeof onError === "function") return;
      stream.destroy(__code("ERR_HTTP2_INVALID_STREAM"));
      return;
    }
    let fd;
    try {
      fd = __h2OpenNb(__h2PathStr(path));
    } catch (err) {
      if (err && (err.code === "ENXIO" || err.code === "EAGAIN")) { setImmediate(attempt); return; }
      failOpen(err);
      return;
    }
    let stat;
    try { stat = fs.fstatSync(fd); }
    catch (err) { try { fs.closeSync(fd); } catch {} failOpen(err); return; }
    __h2SendFileStat(stream, fd, stat, headers, options, onError, true);
  };
  attempt();
}
// O_NONBLOCK open（读端；常规位 read，custom_flags 由 Rust 侧直通）。
// 错误富化口径与 fs __fsErr 同构（native 串 `CODE: msg, open 'path'` →
function __h2OpenNb(path) {
  let fd;
  try {
    fd = Number(__wjs2_fs_open(path, '{"read":true,"nonblock":true}'));
  } catch (e) {
    throw __h2EnrichFsErr(e, "open", path);
  }
  return fd;
}
function __h2EnrichFsErr(e, syscall, path) {
  const m = String((e && e.message) || e);
  if (e && typeof e.code === "string" && e.code !== "" && e.code !== "UNKNOWN") {
    if (e.path === undefined) e.path = path;
    return e;
  }
  const ioShape = m.match(/^([A-Z_]+): (.*), ([a-z_]+) '(.*)'$/);
  if (ioShape) {
    const err = new Error(m);
    err.code = ioShape[1];
    err.syscall = ioShape[3];
    err.path = ioShape[4];
    return err;
  }
  const code = (m.match(/^([A-Z_]+): /) || [])[1] || "UNKNOWN";
  const rest = m.replace(/^[A-Z_]+: /, "");
  const err = new Error(`${code}: ${rest}, ${syscall} '${path}'`);
  err.code = code;
  err.syscall = syscall;
  err.path = path;
  return err;
}
// file 种：类检（node doSendFileFD 逐字）→statCheck→应答→泵体。
function __h2SendFileStat(stream, fd, stat, headers, options, onError, ownsFd) {
  const failStat = (err) => {
    try { if (ownsFd) fs.closeSync(fd); } catch {}
    if (typeof onError === "function") { onError(err); return; }
    stream.destroy(err);
  };
  const isFile = typeof stat.isFile === "function" && stat.isFile();
  const isDir = typeof stat.isDirectory === "function" && stat.isDirectory();
  if (isDir) { failStat(__code("ERR_HTTP2_SEND_FILE")); return; }
  let offset = options.offset !== undefined ? options.offset : 0;
  let length = options.length !== undefined ? options.length : -1;
  if (!isFile) {
    // 非常规：显式 offset/length 即 NOSEEK（node：`!== undefined` 任一即中，
    // 0 亦中）；否则 offset=-1/length=-1 读到 EOF，不设 content-length。
    if (options.offset !== undefined || options.offset > 0 ||
        options.length !== undefined || options.length >= 0) {
      failStat(__code("ERR_HTTP2_SEND_FILE_NOSEEK"));
      return;
    }
    offset = -1;
    length = -1;
  }
  if (stream.destroyed) {
    try { if (ownsFd) fs.closeSync(fd); } catch {}
    stream.destroy(__code("ERR_HTTP2_INVALID_STREAM"));
    return;
  }
  // statCheck（file 种 2 参；false 即撤销，无错；抛即透传用户异常）。
  const h = { ...headers };
  if (typeof options.statCheck === "function") {
    if (options.statCheck.call(stream, stat, h) === false) {
      try { if (ownsFd) fs.closeSync(fd); } catch {}
      return;
    }
  }
  // statCheck 内自发应答即静默撤销（node HEADERS_SENT 复检口径）。
  if (stream.__headersSent) {
    try { if (ownsFd) fs.closeSync(fd); } catch {}
    return;
  }
  if (isFile) {
    // 常规文件自动 content-length（node 覆写口径；range 套件点名）。
    length = length < 0 ? stat.size - (+offset) : Math.min(stat.size - (+offset), length);
    h["content-length"] = length;
  }
  // 头上残错（非法头名等）即 destroy（node processRespondWithFD 口径：无 onError）。
  try { stream.respond(h, {}); }
  catch (err) {
    try { if (ownsFd) fs.closeSync(fd); } catch {}
    stream.destroy(err);
    return;
  }
  __h2PumpFile(stream, fd, offset, length, ownsFd, onError);
}
// fd 种（node doSendFD 逐字）：statOptions→statCheck（3 参）→直接泵体
//（无类检、无 content-length 自动头、无 open 错路由）。
function __h2SendFdStat(stream, fd, stat, headers, options) {
  if (stream.destroyed) {
    stream.destroy(__code("ERR_HTTP2_INVALID_STREAM"));
    return;
  }
  const statOptions = {
    offset: options.offset !== undefined ? options.offset : 0,
    length: options.length !== undefined ? options.length : -1,
  };
  const h = { ...headers };
  if (typeof options.statCheck === "function") {
    if (options.statCheck.call(stream, stat, h, statOptions) === false) return;
    if (stream.__headersSent) return;
  }
  try { stream.respond(h, {}); }
  catch (err) { stream.destroy(err); return; }
  __h2PumpFile(stream, fd, statOptions.offset | 0, statOptions.length | 0, false, options.onError);
}
// 泵体：常规=定长位置读；offset -1=游标读到 EOF（EAGAIN 即 setImmediate 续）。
function __h2PumpFile(stream, fd, offset, length, ownsFd, onError) {
  const streaming = offset === -1;
  let pos = streaming ? 0 : offset;
  const end = streaming ? -1 : offset + Math.max(0, length);
  if (!streaming && pos >= end) {
    try { if (ownsFd) fs.closeSync(fd); } catch {}
    if (!stream.writableEnded) stream.__finishWritable();
    return;
  }
  const CH = 0x8000;
  const step = () => {
    if (stream.destroyed || stream.__closed) {
      try { if (ownsFd) fs.closeSync(fd); } catch {}
      return;
    }
    const want = streaming ? CH : Math.min(CH, end - pos);
    const buf = Buffer.alloc(want);
    let r;
    try {
      r = fs.readSync(fd, buf, 0, want, streaming ? null : pos);
    } catch (err) {
      // EAGAIN=本轮无数据（FIFO 写者未到），让出再试；余错即 destroy。
      if (err && err.code === "EAGAIN") { setImmediate(step); return; }
      try { if (ownsFd) fs.closeSync(fd); } catch {}
      stream.destroy(err);
      return;
    }
    if (r <= 0) {
      // EOF：关 fd（ownsFd）→ 收尾（node FilePipe 到 EOF 即 END_STREAM）。
      try { if (ownsFd) fs.closeSync(fd); } catch {}
      if (!stream.writableEnded) stream.__finishWritable();
      return;
    }
    __wjs2_h2_data(stream.__conn, stream.id, __b64enc(buf.subarray(0, r)));
    if (!streaming) {
      pos += r;
      if (pos >= end) {
        try { if (ownsFd) fs.closeSync(fd); } catch {}
        if (!stream.writableEnded) stream.__finishWritable();
        return;
      }
    }
    setImmediate(step);
  };
  step();
}
