
// ── 服务端流对象（node ServerHttp2Stream：真 Duplex）────────────────────────
class Http2ServerStream extends Duplex {
  constructor(session, connId, id, options) {
    super(options ?? {});
    this.__session = session;
    this.__server = session.__server;
    this.__conn = connId;
    this.id = id;
    this.readable = true;
    this.writable = true;
    this.__destroyed = false;
    this.__closed = false;
    this.__aborted = false;
    this.__headersSent = false;
    this.__waitForTrailers = false;
    this.__trailersSent = false;
    this.__wantTrailersFired = false;
    this.__sentHeaders = null;
    this.__sentPseudoHeaders = null;
    this.__sentInfoHeaders = [];
    this.__trailers = null;
    this.sendDate = true;
    this.endAfterHeaders = false;
    this.__reqEnded = false;
    this.__rstSent = false;
    // 实例数据属性遮蔽 Duplex 原型只读 getter（socket.set 套件直写直读）
    Object.defineProperty(this, "readable", { value: true, writable: true, configurable: true });
    Object.defineProperty(this, "writable", { value: true, writable: true, configurable: true });
    // node：end 后 readable=false、finish 后 writable=false、close 后双 false
    //（compat-socket 套件点名；内部监听先挂，用户监听随后读到翻位值）。
    this.on("end", () => { this.readable = false; });
    this.on("finish", () => { this.writable = false; });
    this.on("close", () => { this.readable = false; this.writable = false; });
    session.__registerStream(this);
  }
  get session() { return this.__session; }
  get aborted() { return this.__aborted; }
  get closed() { return this.__closed; }
  get destroyed() { return this.__destroyed; }
  get headersSent() { return this.__headersSent; }
  get _header() { return this.__headersSent; }
  get headersSentRaw() { return this.__headersSent; }
  get sentHeaders() { return this.__sentHeaders; }
  get sentPseudoHeaders() { return this.__sentPseudoHeaders; }
  get sentInfoHeaders() { return this.__sentInfoHeaders; }
  get sentTrailers() { return this.__trailers; }
  get pushAllowed() { return false; }
  get bufferSize() { return this.writableLength; }
  get state() {
    const localClose = this.__closed || this.__trailersSent ? 1 : 0;
    const remoteClose = this.__reqEnded ? 1 : 0;
    return {
      state: this.__destroyed ? 7 : (localClose && remoteClose ? 7 : 2),
      weight: 16,
      sumDependencyWeight: 0,
      localClose,
      remoteClose,
      localWindowSize: 65535,
    };
  }
  // 伪头合成由 req 承担；stream 可读侧 = 请求体
  _read() {}
  __feedBody(b64chunk) {
    const u8 = __b64dec(b64chunk ?? "");
    if (u8.length > 0) this.push(Buffer.from(u8));
    // 流式体（P1）：compat req 从 stream 拉取——新块到达即唤它再拉（整收时代块在
    // req._read 前已齐，无需此步）。
    this.__pokeReq();
  }
  __pokeReq() {
    const r = this.__req;
    if (r && !r.__reqDone && !r.destroyed) queueMicrotask(() => r._read());
  }
  __endReq(trailersJson) {
    if (this.__reqEnded) return;
    this.__reqEnded = true;
    const t = JSON.parse(trailersJson ?? "[]");
    // 流式体：compat req 的 trailers/rawTrailers 在体尾才知道（构造时为空）。
    if (t.length > 0 && this.__req) {
      this.__req.trailers = __pairsToObj(t);
      this.__req.rawTrailers = t.flat();
    }
    // node 序：trailers 帧到即发，与读取无关。流动态下体块的 data 事件尚待派发，
    // 为保 data… → trailers 序挂在 end 前（prepend）；非流动（无人读）即刻发——
    // 否则等不到 end，`on('trailers', () => stream.end())` 形永挂（trailers 套件）。
    if (t.length > 0) {
      const fire = () => this.emit("trailers", __pairsToObj(t));
      if (this.readableFlowing === true) this.prependOnceListener("end", fire);
      else queueMicrotask(fire);
    }
    this.push(null);
    this.__pokeReq();
    this.__maybeAutoClose();
  }
  __onAborted() {
    if (this.__aborted) return;
    this.__aborted = true;
    this.emit("aborted");
    this.destroy();
  }
  // node onStreamClose：close 事件由 destroy 机制单发（§事件单发）
  __abort() {
    if (this.__closed || this.__destroyed || this.__aborted) return;
    this.__aborted = true;
    // 先自发 'aborted'（server-socket-destroy 套件点名），再级联下游
    //（顺序：stream aborted → req aborted → close；旧路径漏发自事件）。
    this.emit("aborted");
    if (this.__req && !this.__cleanClose) this.__req.__onAborted();
    if (this.__res && !this.__res.destroyed) this.__res.__destroySilent();
    this.destroy();
  }
  __abortDownstream() {
    if (this.__aborted) return;
    this.__aborted = true;
    // 整洁收尾（maybeAutoClose 置 __cleanClose）不 abort 下游——双向已尽，
    // 中断级联会吞掉待消费的 req 'end'（blk6）；
    // 真中断（对端 RST/destroy）无此旗，照旧级联（compat-aborted）。
    if (this.__req && !this.__cleanClose) this.__req.__onAborted();
    if (this.__res && !this.__res.destroyed) this.__res.__destroySilent();
  }
  __implicitRespond() {
    if (this.__headersSent || this.__destroyed) return;
    this.respond({});
  }
  respond(headers = {}, options = {}) {
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_STREAM");
    if (this.__headersSent) throw __code("ERR_HTTP2_HEADERS_SENT");
    if (headers === null || typeof headers !== "object") throw __code("ERR_HTTP2_HEADERS_OBJECT");
    if (options === null || typeof options !== "object") {
      throw __code("ERR_INVALID_ARG_TYPE", "options", "object", options);
    }
    // :status 校验（数字合法才入线；非数字按 node 线上默认 200 处理）
    let status = 200;
    if (headers[":status"] !== undefined) {
      const s = +headers[":status"];
      if (typeof headers[":status"] === "number" &&
          (!Number.isInteger(s) || s < 100 || s > 599)) {
        throw __code("ERR_HTTP2_STATUS_INVALID", headers[":status"]);
      }
      if (Number.isInteger(s) && s >= 100 && s <= 599) status = s;
    }
    __validateH2Headers({ ...headers, ":status": status }, [":status"]);
    this.__waitForTrailers = !!options.waitForTrailers;
    // node：HEAD 请求与 204/205/304 应答自动 END_STREAM（head-request/endafter
    // 套件口径；END_STREAM 随头出线，后续 write 报 ERR_STREAM_WRITE_AFTER_END）
    const endStreamAuto = this.__headRequest === true ||
      status === 204 || status === 205 || status === 304;
    if (endStreamAuto) this.endAfterHeaders = true;
    const entries = [];
    const sent = { __proto__: null };
    const sentPseudo = { __proto__: null };
    sentPseudo[":status"] = String(status);
    if (this.sendDate && headers["date"] === undefined && headers["Date"] === undefined) {
      entries.push(["date", new Date().toUTCString()]);
      sent["date"] = new Date().toUTCString();
    }
    for (const [k, v] of Object.entries(headers)) {
      if (k.startsWith(":")) continue;
      __validateHeaderName(k);
      const w = __headerToWire(v);
      if (Array.isArray(w)) {
        for (const one of w) { __validateHeaderValue(k, one); entries.push([k.toLowerCase(), one]); }
        sent[k.toLowerCase()] = w;
      } else {
        __validateHeaderValue(k, w);
        entries.push([k.toLowerCase(), w]);
        sent[k.toLowerCase()] = w;
      }
    }
    this.__headersSent = true;
    this.__sentHeaders = sent;
    this.__sentPseudoHeaders = sentPseudo;
    __wjs2_h2_respond(this.__conn, this.id, status, JSON.stringify(entries),
      (options.endStream || endStreamAuto) ? 1 : 0);
    if (options.endStream || endStreamAuto) {
      this.endAfterHeaders = true;
      this.__finishWritable();
    }
    return undefined;
  }
  // node：fd 可为 number 或 FileHandle（对象须带数值 fd）；错误消息
  // "The \"fd\" argument must be of type number or an instance of FileHandle."
  respondWithFD(fd, headers, options) {
    if (typeof fd !== "number" &&
        (fd === null || typeof fd !== "object" || typeof fd.fd !== "number")) {
      throw __code("ERR_INVALID_ARG_TYPE", "fd", "number or an instance of FileHandle", fd);
    }
    return this.respondWithFile(typeof fd === "object" ? fd.fd : fd, headers, options);
  }
  respondWithFile(filename, headers = {}, options = {}) {
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_STREAM");
    if (this.__headersSent) throw __code("ERR_HTTP2_HEADERS_SENT");
    if (typeof options !== "object" || options === null) {
      throw __code("ERR_INVALID_ARG_TYPE", "options", "object", options);
    }
    if (options.statCheck !== undefined && typeof options.statCheck !== "function") {
      throw __code("ERR_INVALID_ARG_VALUE", "options.statCheck", options.statCheck);
    }
    if (options.onError !== undefined && typeof options.onError !== "function") {
      throw __code("ERR_INVALID_ARG_VALUE", "options.onError", options.onError);
    }
    if (options.offset !== undefined && typeof options.offset !== "number") {
      throw __code("ERR_INVALID_ARG_VALUE", "options.offset", options.offset);
    }
    if (options.length !== undefined && typeof options.length !== "number") {
      throw __code("ERR_INVALID_ARG_VALUE", "options.length", options.length);
    }
    // 204/205/304 禁 payload
    if (headers[":status"] !== undefined) {
      const s = +headers[":status"];
      if (s === 204 || s === 205 || s === 304) {
        throw __code("ERR_HTTP2_PAYLOAD_FORBIDDEN", s);
      }
    }
    const isFd = typeof filename === "number";
    let fd = null;
    let stat;
    try {
      fd = isFd ? filename : fs.openSync(filename, "r");
      stat = fs.fstatSync(fd);
    } catch (err) {
      if (!isFd && fd !== null) { try { fs.close(fd, () => {}); } catch {} }
      if (typeof options.onError === "function") {
        options.onError(err);
        return;
      }
      // node：stat 失败且无 onError → 流 RST(INTERNAL_ERROR) + 'error'
      //（fd-invalid 套件：stream/req 双侧 ERR_HTTP2_STREAM_ERROR）
      this.__rstInternalError();
      return;
    }
    try {
      const h = { ...headers };
      // node statCheck 第三参为 {offset, length} 原样（fd-range 套件断言）
      const statCheckOpts = { offset: options.offset, length: options.length };
      if (typeof options.statCheck === "function") {
        if (options.statCheck.call(this, stat, h, statCheckOpts) === false) {
          if (!isFd) { try { fs.close(fd, () => {}); } catch {} }
          return;
        }
      }
      if (h[":status"] !== undefined && [204, 205, 304].includes(+h[":status"])) {
        throw __code("ERR_HTTP2_PAYLOAD_FORBIDDEN", +h[":status"]);
      }
      let offset = options.offset ?? 0;
      let length = options.length ?? (stat.size - offset);
      if (length < 0) length = stat.size - offset;
      if (length < 0) length = 0;
      const shouldSendBody = h[":status"] === undefined || !![200, 201, 202, 203, 206].includes(+h[":status"]) ||
        (+h[":status"] >= 300 && ![204, 205, 304].includes(+h[":status"]));
      this.respond(h, {});
      if (shouldSendBody && length > 0) {
        const CH = 0x8000;
        let pos = offset;
        while (pos < offset + length) {
          const n = Math.min(CH, offset + length - pos);
          const buf = Buffer.alloc(n);
          const r = fs.readSync(fd, buf, 0, n, pos);
          if (r <= 0) break;
          if (r < n) {
            __wjs2_h2_data(this.__conn, this.id, __b64enc(buf.subarray(0, r)));
            break;
          }
          __wjs2_h2_data(this.__conn, this.id, __b64enc(buf));
          pos += r;
        }
      }
      if (!isFd) { try { fs.close(fd, () => {}); } catch {} }
      if (!this.writableEnded) this.__finishWritable();
    } catch (err) {
      if (!isFd) { try { fs.close(fd, () => {}); } catch {} }
      if (typeof options.onError === "function") {
        options.onError(err);
        return;
      }
      // node：校验/读失败无 onError → 错误落流 'error'（fd-leak 套件），
      // 并 RST 对端（_destroy 统一发送，否则对端流永等响应）
      this.destroy(err);
    }
  }
  pushStream(headers, options, cb) {
    // 注：node 门序首位是"对端未开推送即同步抛 PUSH_DISABLED"，但 hyper 服务端不暴露对端
    // SETTINGS（enablePush 不可知）——同步抛会误伤对端开推送时的参数校验套件，维持回调形（记档）。
    if (typeof options === "function") { cb = options; options = {}; }
    if (typeof cb !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "Function", cb);
    }
    if (this.__session.type !== 0 || this.__isPush) {
      process.nextTick(() => cb(__code("ERR_HTTP2_NESTED_PUSH")));
      return;
    }
    // 底座无 PUSH_PROMISE（hyper；偏差记档）——回调路径收到 PUSH_DISABLED
    process.nextTick(() => cb(__code("ERR_HTTP2_PUSH_DISABLED")));
    return;
  }
  additionalHeaders(headers) {
    // node core.js 门序：毁/关 → INVALID_STREAM；已应答 → HEADERS_AFTER_RESPOND；
    // 再验对象；:status 定制（101 → STATUS_101，<100/>=200 → INVALID_INFO_STATUS）；
    // 伪头校验；落 sentInfoHeaders。1xx 下发走 native interim（bound 未通，
    // 另案；此处只记不发）。
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_STREAM");
    if (this.__headersSent) throw __code("ERR_HTTP2_HEADERS_AFTER_RESPOND");
    assertIsObject(headers, "headers");
    const status = headers[":status"];
    if (status !== undefined && status !== null) {
      const s = status | 0;
      if (s === 101) throw __code("ERR_HTTP2_STATUS_101");
      if (s < 100 || s >= 200) throw __code("ERR_HTTP2_INVALID_INFO_STATUS", status);
    }
    // 伪头仅 :status（node assertValidPseudoHeaderResponse 口径）。
    for (const k of Object.keys(headers)) {
      if (k.startsWith(":") && k !== ":status") throw __code("ERR_HTTP2_INVALID_PSEUDOHEADER", k);
    }
    this.__sentInfoHeaders.push(headers);
    return this;
  }
  priority(options) {
    __h2PriorityDeprecate();
    if (options === null || typeof options !== "object") {
      throw __code("ERR_INVALID_ARG_TYPE", "options", "object", options);
    }
    if (options.weight !== undefined) {
      const w = options.weight;
      if (typeof w !== "number" || !Number.isInteger(w) || w < 1 || w > 256) {
        throw __code("ERR_OUT_OF_RANGE", "options.weight", w);
      }
    }
    if (options.parent !== undefined && options.parent !== 0) {
      const p = options.parent;
      if (typeof p !== "number" || !Number.isInteger(p) || p < 1 || p > 2147483647) {
        throw __code("ERR_OUT_OF_RANGE", "options.parent", p);
      }
    }
    return this;
  }
  sendTrailers(trailers) {
    if (trailers === null || typeof trailers !== "object") {
      throw __code("ERR_INVALID_ARG_TYPE", "trailers", "object", trailers);
    }
    // node core.js sendTrailers 门序：已毁/已关 → INVALID_STREAM；已发 → ALREADY_SENT；
    // 未到 wantTrailers（含未开 waitForTrailers）→ NOT_READY（真机逐项实测）。
    if (this.destroyed || this.__closed) throw __code("ERR_HTTP2_INVALID_STREAM");
    if (this.__trailersSent) throw __code("ERR_HTTP2_TRAILERS_ALREADY_SENT");
    if (!this.__waitForTrailers || !this.__wantTrailersFired) {
      throw __code("ERR_HTTP2_TRAILERS_NOT_READY");
    }
    __validateH2Headers(trailers, []);
    const t = [];
    for (const [k, v] of Object.entries(trailers)) {
      const w = __headerToWire(v);
      if (Array.isArray(w)) for (const one of w) t.push([k, one]);
      else t.push([k, w]);
    }
    this.__trailersSent = true;
    this.__trailers = { ...trailers };
    __wjs2_h2_end(this.__conn, this.id, JSON.stringify(t));
    const cb = this.__finalCb;
    if (typeof cb === "function") queueMicrotask(cb);
    return this;
  }
  close(code = 0, cb) {
    if (typeof code === "function") { cb = code; code = 0; }
    __validateStreamClose(code, cb);
    if (typeof cb === "function") this.once("close", cb);
    if (this.__closed || this.__destroyed) return this;
    // node：非零非 CANCEL 的 close 码记账，_destroy 合成流错误（client 同口径）。
    if ((code ?? 0) !== 0) this.__closeCode = code;
    if (!this.__trailersSent) {
      __wjs2_h2_reset(this.__conn, this.id, code);
      this.__trailersSent = true;
    }
    this.destroy();
    return this;
  }
  destroy(err, code, cb) {
    if (typeof err === "number") { cb = code; code = err; err = undefined; }
    if (typeof code === "function") { cb = code; code = 0; }
    if (typeof cb === "function") this.once("close", cb);
    if (this.__destroyed) return this;
    if (err) this.__destroyErr = err;
    else if (!this.__destroyErr && (code ?? 0) !== 0) this.__destroyErr = undefined;
    super.destroy(err);
    return this;
  }
  __finishWritable() {
    // respond({endStream:true}) / respondWithFile 收尾路径；
    // __trailersSent 由 _final 在 __wjs2_h2_end 发出后置位（预置会跳过
    // END_STREAM → 客户端 'end' 永不到，与 res.end 预置同款挂死）
    super.end();
  }
  _write(chunk, encoding, cb) {
    if (this.__destroyed) {
      cb(__code("ERR_HTTP2_STREAM_CLOSED"));
      return;
    }
    this.__implicitRespond();
    const u8 = chunk instanceof Uint8Array ? chunk : __toU8(String(chunk), "write");
    __wjs2_h2_data(this.__conn, this.id, __b64enc(u8));
    queueMicrotask(cb);
  }
  _final(cb) {
    if (this.__destroyed) { cb(); return; }
    this.__implicitRespond();
    if (this.__waitForTrailers && !this.__trailersSent && !this.endAfterHeaders) {
      // wantTrailers 窗口：用户 sendTrailers 后回填收尾
      this.__finalCb = cb;
      queueMicrotask(() => {
        if (!this.__destroyed && !this.__trailersSent) {
          this.__wantTrailersFired = true;
          this.emit("wantTrailers");
        }
      });
      return;
    }
    if (!this.__trailersSent) {
      const t = this.__pendingTrailers ?? [];
      this.__trailersSent = true;
      __wjs2_h2_end(this.__conn, this.id, JSON.stringify(t));
    }
    // node 序：'finish' 先于 'close'（close 断言常读 writableFinished；
    // 先 destroy 会在 finished 置位前发 close，close-idle 套件现形）。
    cb();
    this.__maybeAutoClose();
  }
  // 双向尽（应答 End 已发 + 请求体已尽）→ 本地收尾（node 流全关后 socket 解绑）。
  // close 单发：只置旗清表，派发交 Duplex autoDestroy（手动再派即 double，
  // compat-socket 套件点名 1 次）。
  __maybeAutoClose() {
    if (this.__destroyed || this.__closed) return;
    if (!(this.__reqEnded && this.__trailersSent)) return;
    this.__closed = true;
    this.__session?.__unregisterStream(this);
    // __destroyed 由 destroy()/_destroy() 置位：此处预置会使 destroy() 提前
    // return 而吞掉 'close'（server-close-idle 套件：stream close 永不到）。
    // destroy 递延一拍：dispatch 期排入的 req.resume 微任务先消费 EOF，
    // 同 tick 直毁会抢在消费前断流，req 'end' 永不到（blk6）。
    // __cleanClose：此 destroy 系整洁收尾，_destroy 中断级联跳过下游 abort。
    this.__cleanClose = true;
    queueMicrotask(() => { if (!this.__destroyed) this.destroy(); });
  }
  _destroy(err, cb) {
    // 应答未完成即销毁 → RST 对端（node：无错 RST 干净收尾，客户端仅 'close'；
    // 有错 RST INTERNAL_ERROR，客户端 'error' ERR_HTTP2_STREAM_ERROR——真机实测）
    if (!this.__trailersSent && !this.__rstSent) {
      this.__rstSent = true;
      __wjs2_h2_reset(this.__conn, this.id, err ?? this.__destroyErr ? 2 : 0);
    }
    this.__closed = true;
    this.__destroyed = true;
    this.__session?.__unregisterStream(this);
    this.__abortDownstream();
    // node _destroy 合成口径：显式 err 优先；否则非零非 CANCEL 的 close/abort
    // 码合成流错误（stream-destroy-event-order；干净路无错，回归照旧）。
    const rstErr = err ?? this.__destroyErr ??
      __h2RstError(this.__closeCode) ?? __h2RstError(this.__abortCode);
    cb(rstErr);
  }
  setTimeout(msecs, callback) {
    if (typeof msecs !== "number") throw __code("ERR_INVALID_ARG_TYPE", "msecs", "number", msecs);
    if (callback !== undefined && typeof callback !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", callback);
    }
    if (typeof callback === "function") this.once("timeout", callback);
    const ms = Number(msecs) || 0;
    if (this.__timeoutTimer !== undefined) clearTimeout(this.__timeoutTimer);
    if (ms > 0 && !this.__closed) {
      this.__timeoutTimer = setTimeout(() => {
        this.__timeoutTimer = undefined;
        this.emit("timeout");
      }, ms);
    }
    return this;
  }
  __attach(req, res) { this.__req = req; this.__res = res; }
  // stat 失败等 native 层错误：RST(INTERNAL_ERROR) + 流 'error'（node 同码同文；
  // RST 由 _destroy 统一发送）
  __rstInternalError() {
    this.destroy(__code("ERR_HTTP2_STREAM_ERROR", "Stream closed with error code NGHTTP2_INTERNAL_ERROR"));
  }
}

// ── Http2ServerRequest（Readable；从底层 stream 拉取）────────────────────────
class Http2ServerRequest extends Readable {
  constructor(stream, info, scheme, socketProxy) {
    super();
    this.__stream = stream;
    this.__socket = socketProxy;
    this.complete = false;
    this.aborted = false;
    this.httpVersion = "2.0";
    this.httpVersionMajor = 2;
    this.httpVersionMinor = 0;
    const method = String(info.method);
    // host-only 请求：客户端以 x-wjs-no-authority 标记"用户未给 :authority"
    //（h2 底座强制要求 authority 只能线上传；serverrequest-host 套件要求
    // 服务端不可见合成的 :authority）。标记头本身同步剥离。
    const __hpairs = JSON.parse(info.headers);
    let __noAuth = false;
    for (const [k] of __hpairs) {
      if (k === "x-wjs-no-authority") { __noAuth = true; break; }
    }
    const authority = __noAuth ? "" : String(info.authority ?? "");
    let phoHint = null;
    let protocol = "";
    const reg = [];
    for (const [k, v] of __hpairs) {
      if (k === "x-wjs-pho") { phoHint = String(v).split(","); continue; }
      if (k === "x-wjs-no-authority") continue;
      // 扩展 CONNECT（RFC 8441）：`:protocol` 只活在线上伪头区，hyper 经
      // extensions 暴露，Rust 侧已拼入头表尾——此处摘出合成伪头。
      if (k === ":protocol") { protocol = String(v); continue; }
      reg.push([k, v]);
    }
    const pseudo = [];
    pseudo.push([":method", method]);
    // 扩展 CONNECT 带 :protocol + :path（常规 CONNECT 两键皆禁；
    // connect-method/connect-method-extended 双套件点名）。
    if (protocol) pseudo.push([":protocol", protocol]);
    if (method !== "CONNECT" || protocol) pseudo.push([":path", String(info.path)]);
    if (authority) pseudo.push([":authority", authority]);
    pseudo.push([":scheme", scheme]);
    let all;
    if (phoHint) {
      const ordered = [];
      for (const name of phoHint) {
        const i = pseudo.findIndex(([n]) => n === name);
        if (i !== -1) ordered.push(pseudo.splice(i, 1)[0]);
      }
      all = [...ordered, ...pseudo, ...reg];
    } else {
      all = [...pseudo, ...reg];
    }
    this.headers = __pairsToObj(all);
    this.rawHeaders = all.flat();
    const trailers = JSON.parse(info.trailers ?? "[]");
    this.trailers = __pairsToObj(trailers);
    this.rawTrailers = trailers.flat();
    this.__url = String(info.path);
    stream.__attach(this, null);
    // stream 末尾 → req 收尾
    stream.on("end", () => {
      if (this.__reqDone) return;
      this.__reqDone = true;
      this.complete = true;
      this.push(null);
    });
    stream.on("aborted", () => this.__onAborted());
  }
  get stream() { return this.__stream; }
  get socket() { return this.__socket; }
  get connection() { return this.__socket; }
  get session() { return this.__stream.__session; }
  get method() { return this.headers[":method"]; }
  set method(v) {
    if (typeof v !== "string") throw __code("ERR_INVALID_ARG_TYPE", "method", "string", v);
    if (!__TOKEN_RE.test(v)) throw __code("ERR_INVALID_ARG_VALUE", "method", v);
    this.headers[":method"] = v;
  }
  get scheme() { return this.headers[":scheme"]; }
  set scheme(v) {
    if (typeof v !== "string") throw __code("ERR_INVALID_ARG_TYPE", "scheme", "string", v);
    this.headers[":scheme"] = v;
  }
  get authority() { return this.headers[":authority"] ?? this.headers.host; }
  set authority(v) {
    if (typeof v !== "string") throw __code("ERR_INVALID_ARG_TYPE", "authority", "string", v);
    this.headers[":authority"] = v;
  }
  get url() { return this.__url; }
  set url(v) { this.__url = v; }
  pause() { this.__userPaused = true; return super.pause(); }
  resume() { this.__userPaused = false; return super.resume(); }
  _read() {
    if (this.__reqDone) return;
    const s = this.__stream;
    let chunk;
    while ((chunk = s.read()) !== null) {
      if (!this.push(chunk)) return;
    }
    if (s.readableEnded) {
      this.__reqDone = true;
      this.complete = true;
      this.push(null);
    }
  }
  setTimeout(msecs, callback) {
    if (typeof msecs !== "number") throw __code("ERR_INVALID_ARG_TYPE", "msecs", "number", msecs);
    if (callback !== undefined && typeof callback !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", callback);
    }
    if (typeof callback === "function") this.once("timeout", callback);
    const ms = Number(msecs) || 0;
    if (this.__timeoutTimer !== undefined) clearTimeout(this.__timeoutTimer);
    if (ms > 0 && !this.complete && !this.__stream.__closed) {
      this.__timeoutTimer = setTimeout(() => {
        this.__timeoutTimer = undefined;
        this.emit("timeout");
      }, ms);
    }
    return this;
  }
  __onAborted() {
    if (this.aborted) return;
    this.aborted = true;
    this.complete = true;
    this.emit("aborted");
    this.destroy();
  }
}

// node compat.js 的进程级一次性告警（UnsupportedWarning）。
let __h2StatusMessageWarned = false;
function __h2StatusMessageWarn() {
  if (__h2StatusMessageWarned) return;
  __h2StatusMessageWarned = true;
  process.emitWarning("Status message is not supported by HTTP/2 (RFC7540 8.1.2.4)", "UnsupportedWarning");
}
let __h2ConnectionWarned = false;
function __h2ConnectionHeaderWarn() {
  if (__h2ConnectionWarned) return;
  __h2ConnectionWarned = true;
  process.emitWarning("The provided connection header is not valid, the value will be dropped from the header and will never be in use.", "UnsupportedWarning");
}

// ── Http2ServerResponse（Writable；写路径委派给底层 stream）──────────────────
class Http2ServerResponse extends Writable {
  constructor(req, stream) {
    super({ autoDestroy: true });
    this.req = req;
    this.__stream = stream;
    this.__conn = stream.__conn;
    this.__id = stream.id;
    this.__statusCode = 200;
    this.__headers = Object.create(null);
    this.__trailers = Object.create(null);
    this.headersSent = false;
    this.sendDate = true;
    this.__finishEmitted = false;
    this.__ended = false;
    stream.on("drain", () => this.emit("drain"));
    stream.on("finish", () => {
      if (!this.__finishEmitted) {
        this.__finishEmitted = true;
        this.emit("finish");
      }
    });
    stream.on("close", () => {
      if (!this.__ended) this.__ended = true;
      queueMicrotask(() => this.emit("close"));
    });
    // close 单发：Writable autoDestroy 与 stream 转发各派一次，
    // 真机只认一次——emit 层幂等（compat-socket 套件点名 1 次）。
    const __resEmit = this.emit.bind(this);
    this.__closeEmitted = false;
    this.emit = (ev, ...a) => {
      if (ev === "close") {
        if (this.__closeEmitted) return false;
        this.__closeEmitted = true;
      }
      return __resEmit(ev, ...a);
    };
    stream.__attach(req, this);
  }
  get stream() { return this.__stream; }
  get socket() { return this.__stream.__destroyed ? undefined : this.req.socket; }
  get connection() { return this.socket; }
  get session() { return this.__stream.__session; }
  // node 口径：finished/writableEnded 即时位（end() 同步置位）；长度/水位读底层流
  get finished() { return this.__ended === true; }
  get writableEnded() { return this.__ended === true; }
  get writableFinished() { return this.__finishEmitted === true; }
  get writableLength() { return this.__stream.writableLength; }
  get writableHighWaterMark() { return this.__stream.writableHighWaterMark; }
  get writableCorked() { return this.__stream.writableCorked; }
  get _header() { return this.headersSent; }
  setHeader(name, value) {
    // node compat.js：名先 trim + 小写；伪头禁用（PSEUDOHEADER_NOT_ALLOWED）；再验 token/值。
    // 已发头即 HEADERS_SENT（set/append/remove 三处同门）。
    if (this.headersSent) throw __code("ERR_HTTP2_HEADERS_SENT");
    const k = __normName(name);
    if (k.startsWith(":")) throw __code("ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED");
    __validateHeaderName(k);
    __validateHeaderValue(k, value);
    // node compat.js：h2 禁 connection 头——告警一次（进程级）并丢弃。
    if (k === "connection") { __h2ConnectionHeaderWarn(); return this; }
    this.__headers[k] = value;
    return this;
  }
  getHeader(name) {
    const v = this.__headers[__normName(name)];
    return v === undefined ? undefined : __headerToWire(v);
  }
  getHeaders() {
    const out = { __proto__: null };
    for (const [k, v] of Object.entries(this.__headers)) out[k] = __headerToWire(v);
    return out;
  }
  getHeaderNames() { return Object.keys(this.__headers); }
  hasHeader(name) { return this.__headers[__normName(name)] !== undefined; }
  removeHeader(name) {
    if (this.headersSent) throw __code("ERR_HTTP2_HEADERS_SENT");
    const k = __normName(name);
    // node compat.js：删 date 即关 sendDate（send-date 套件点名）。
    if (k === "date") {
      this.sendDate = false;
      return this;
    }
    delete this.__headers[k];
    return this;
  }
  appendHeader(name, value) {
    if (this.headersSent) throw __code("ERR_HTTP2_HEADERS_SENT");
    const k = __normName(name);
    // node compat assertValidHeader 口径：伪头一律 PSEUDOHEADER_NOT_ALLOWED。
    if (k.startsWith(":")) throw __code("ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED");
    __validateHeaderName(k);
    __validateHeaderValue(k, value);
    const cur = this.__headers[k];
    if (cur === undefined) this.__headers[k] = value;
    else if (Array.isArray(cur)) cur.push(value);
    else this.__headers[k] = [cur, value];
    return this;
  }
  get statusMessage() { __h2StatusMessageWarn(); return ""; }
  set statusMessage(v) { __h2StatusMessageWarn(); }
  set statusCode(status) {
    // node compat.js 逐字：`code |= 0`；1xx → INFO_STATUS_NOT_ALLOWED；<100/>599 → STATUS_INVALID。
    const code = status | 0;
    if (code >= 100 && code < 200) throw __code("ERR_HTTP2_INFO_STATUS_NOT_ALLOWED");
    if (code < 100 || code > 599) throw __code("ERR_HTTP2_STATUS_INVALID", code);
    this.__statusCode = code;
  }
  get statusCode() { return this.__statusCode ?? 200; }
  setTrailer(name, value) {
    const k = __normName(name);
    __validateHeaderName(k);
    __validateHeaderValue(k, value);
    this.__trailers[k] = value;
    return this;
  }
  addTrailers(obj) {
    if (obj === null || typeof obj !== "object") {
      throw __code("ERR_INVALID_ARG_TYPE", "headers", "object", obj);
    }
    for (const [k, v] of Object.entries(obj)) this.setTrailer(k, v);
    return this;
  }
  writeHead(status, ...rest) {
    // node compat.js 门序：流已关/已毁 → 静默回 this；已发头 → HEADERS_SENT；
    // 状态码经 statusCode setter 校验；末尾 kBeginSend 立即发头（修前只缓冲到 write/end）。
    if (this.__stream.__destroyed || this.__stream.destroyed || this.__stream.__closed) return this;
    if (this.headersSent) throw __code("ERR_HTTP2_HEADERS_SENT");
    this.statusCode = status;
    for (const r of rest) {
      if (typeof r === "string") {
        // reason phrase：h2 不支持——与 statusMessage 同一进程级一次性告警（node compat.js）。
        __h2StatusMessageWarn();
      } else if (Array.isArray(r)) {
        // node compat.js 双形：首元数组即 tuple 形 [[k,v]…]（append 语义，
        // 重复键并串），否则平铺 [k,v,k,v]（奇数长即 ERR_INVALID_ARG_VALUE）。
        if (r.length > 0 && Array.isArray(r[0])) {
          for (const e of r) this.appendHeader(e[0], e[1]);
        } else {
          if (r.length % 2 !== 0) throw __code("ERR_INVALID_ARG_VALUE", "headers", r);
          for (let i = 0; i + 1 < r.length; i += 2) this.appendHeader(r[i], r[i + 1]);
        }
      } else if (r !== null && typeof r === "object") {
        for (const [k, v] of Object.entries(r)) this.setHeader(k, v);
      }
    }
    this.__sendHead();
    return this;
  }
  __sendHead() {
    if (this.headersSent || this.__stream.__destroyed) return;
    const entries = [];
    for (const [k, v] of Object.entries(this.__headers)) {
      if (k.startsWith(":")) continue;
      const w = __headerToWire(v);
      if (Array.isArray(w)) for (const one of w) entries.push([k, one]);
      else entries.push([k, w]);
    }
    if (this.sendDate && !this.hasHeader("date")) {
      entries.push(["date", new Date().toUTCString()]);
    }
    this.headersSent = true;
    this.__stream.__headersSent = true;
    // HEAD 请求：END_STREAM 随头出线（node：writeHead 即发 flags=5 的 HEADERS，
    // end(chunk) 仅账面收尾，体不上线；Rust 侧 end_stream 语义承接）。
    __wjs2_h2_respond(this.__conn, this.__id, this.statusCode, JSON.stringify(entries),
      this.__stream.__headRequest === true ? 1 : 0);
  }
  write(chunk, encoding, cb) {
    // node：response.destroy() 后 write 静默 false、回调不触发（write block3 真机 p1）；
    // 流死于底下（close/RST/abort）但 response 存活时 write → 回调
    // ERR_HTTP2_INVALID_STREAM（block1 真机 p18；__destroySilent 不算显式 destroy）。
    if (this.__userDestroyed) return false;
    if (this.__stream.__destroyed) {
      const err = __code("ERR_HTTP2_INVALID_STREAM");
      if (typeof encoding === "function") encoding(err);
      else if (typeof cb === "function") cb(err);
      return false;
    }
    if (this.__ended) {
      const err = __code("ERR_STREAM_WRITE_AFTER_END");
      if (typeof encoding === "function") encoding(err);
      else if (typeof cb === "function") cb(err);
      return false;
    }
    this.__sendHead();
    return this.__stream.write(chunk, encoding, cb);
  }
  cork() { this.__stream.cork?.(); }
  uncork() { this.__stream.uncork?.(); }
  end(chunk, encoding, cb) {
    if (typeof chunk === "function") { cb = chunk; chunk = null; encoding = null; }
    else if (typeof encoding === "function") { cb = encoding; encoding = null; }
    if (this.__ended) {
      // node h2 口径：end 可重复调用不抛错；cb 至少一次（finish 前 → finish 时，后 → nextTick）
      if (typeof cb === "function") {
        if (this.__finishEmitted) process.nextTick(cb);
        else this.once("finish", cb);
      }
      return this;
    }
    this.__ended = true;
    if (typeof cb === "function") this.once("finish", cb);
    this.__sendHead();
    if (chunk !== null && chunk !== undefined) this.__stream.write(chunk, encoding);
    const t = [];
    for (const [k, v] of Object.entries(this.__trailers)) {
      const w = __headerToWire(v);
      if (Array.isArray(w)) for (const one of w) t.push([k, one]);
      else t.push([k, w]);
    }
    this.__stream.__pendingTrailers = t;
    // __trailersSent 由 stream._final 在 __wjs2_h2_end 发出后置位；
    // 此处预置会让 _final 跳过 END_STREAM → 客户端 'end' 永不到（挂死根因）。
    this.__stream.end();
    return this;
  }
  // node 口径：destroy 恒发 finish（clean/err 均先 finish 后 close），
  // 错误不落 res（错误走 stream 'error'，res.on('error') 不触发）。
  destroy(err) {
    if (this.destroyed) return this;
    // 显式 destroy 标记（write 静默口径；底层连带 __destroySilent 不置位）。
    this.__userDestroyed = true;
    if (!this.__finishEmitted) {
      this.__finishEmitted = true;
      this.emit("finish");
    }
    this.__stream.destroy(err ?? null);
    return super.destroy();
  }
  _destroy(err, cb) {
    if (!this.__finishEmitted) {
      this.__finishEmitted = true;
      this.emit("finish");
    }
    cb(err);
  }
  __destroySilent() {
    if (this.destroyed) return;
    super.destroy();
  }
  createPushResponse(headers, cb) {
    // 形如 node（头 + 回调双参）：回调非函数即同步抛；流已关走 INVALID_STREAM；
    // push 本体 hyper 无 server push——DISABLED（bound，另案）。
    if (typeof cb !== "function") throw __code("ERR_INVALID_ARG_TYPE", "callback", "Function", cb);
    if (headers === null || typeof headers !== "object") {
      throw __code("ERR_INVALID_ARG_TYPE", "headers", "object", headers);
    }
    if (this.__stream.__destroyed || this.__stream.destroyed || this.__stream.__closed) {
      queueMicrotask(() => cb(__code("ERR_HTTP2_INVALID_STREAM")));
      return undefined;
    }
    queueMicrotask(() => cb(__code("ERR_HTTP2_PUSH_DISABLED")));
    return undefined;
  }
  writeContinue() {
    // node：即 writeInformation(100)；已发头/已终即 false（布尔，非链式）。
    return this.writeInformation(100);
  }
  writeInformation(type, info) {
    // node compat.js 逐字：信息性应答仅 100–199 且禁 101；
    // 已发头/已终即 false；落 stream.additionalHeaders（1xx 下发走 native
    // interim-headers，hyper 底座 bound——此处只记 sentInfoHeaders，不发线）。
    if (typeof type !== "number" || type < 100 || type > 199) {
      throw __code("ERR_HTTP2_STATUS_INVALID", type);
    }
    if (type === 101) {
      throw __code("ERR_HTTP2_STATUS_INVALID", type);
    }
    if (this.__stream.__headersSent || this.__ended) return false;
    if (info !== undefined && info !== null) {
      if (typeof info !== "object" || Array.isArray(info)) {
        throw __code("ERR_INVALID_ARG_TYPE", "headers", "object", info);
      }
    }
    const hdrs = { __proto__: null, ":status": type };
    if (info !== undefined && info !== null) {
      for (const [k, v] of Object.entries(info)) {
        __validateHeaderName(k);
        __validateHeaderValue(k, v);
        hdrs[k] = v;
      }
    }
    this.__stream.additionalHeaders(hdrs);
    return true;
  }
  writeEarlyHints(hints, cb) {
    // node compat.js 逐字：hints 须对象（validateObject 口径 ARG_TYPE）；
    // link 经 validateLinkHeaderValue（string/array 并串，余者 ARG_VALUE）；
    // 空 link 即 false；余键 assertValidHeader + token 门。
    if (hints === null || typeof hints !== "object" || Array.isArray(hints)) {
      throw __code("ERR_INVALID_ARG_TYPE", "hints", "object", hints);
    }
    const link = __validateLinkValue(hints.link);
    const rest = { __proto__: null };
    for (const k of Object.keys(hints)) {
      if (k === "link") continue;
      const name = k.trim().toLowerCase();
      __validateHeaderName(name);
      __validateHeaderValue(name, hints[k]);
      rest[name] = hints[k];
    }
    if (link.length === 0) {
      if (typeof cb === "function") queueMicrotask(cb);
      return false;
    }
    rest.link = link;
    const ok = this.writeInformation(103, rest);
    if (typeof cb === "function") queueMicrotask(cb);
    return ok;
  }
  flushHeaders() {
    if (!this.headersSent) this.__sendHead();
  }
  setTimeout(msecs, callback) {
    if (typeof msecs !== "number") throw __code("ERR_INVALID_ARG_TYPE", "msecs", "number", msecs);
    if (callback !== undefined && typeof callback !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", callback);
    }
    if (typeof callback === "function") this.once("timeout", callback);
    const ms = Number(msecs) || 0;
    if (this.__timeoutTimer !== undefined) clearTimeout(this.__timeoutTimer);
    if (ms > 0 && !this.__stream.__closed) {
      this.__timeoutTimer = setTimeout(() => {
        this.__timeoutTimer = undefined;
        this.emit("timeout");
      }, ms);
    }
    return this;
  }
}

