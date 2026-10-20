// ── 服务端 ──────────────────────────────────────────────────────────────
class Http2Server extends EventEmitter {
  constructor(options, secure) {
    super();
    if (typeof options !== "object" || options === null) {
      throw __code("ERR_INVALID_ARG_TYPE", "options", "object", options);
    }
    this.__id = 0;
    this.__listening = null;
    this.__secure = !!secure;
    this.__tlsOpts = null;
    this.__streams = new Map();
    this.__sessions = new Map();
    this.__sockBag = {};
    this.__opts = options;
    this[kPendingOptions] = this.__opts;
    // node setupCompat 口径：compat 路由（'connect'/405、'request'）只在
    // 'request' 监听器出现过后才接线（connect-method[-extended] 套件：
    // 纯 'stream' 监听的 CONNECT 不走 'connect'、不 405）。
    this.__compatAttached = false;
    this.on("newListener", (ev) => {
      if (ev === "request") this.__compatAttached = true;
    });
    if (options.settings !== undefined) __validateSettings(options.settings, "options.settings");
    // node validateUint32（maxSessionInvalidFrames/maxSessionRejectedStreams）：非 uint32 即 OUT_OF_RANGE。
    for (const k of ["maxSessionInvalidFrames", "maxSessionRejectedStreams"]) {
      const v = options[k];
      if (v === undefined) continue;
      if (typeof v !== "number") throw __code("ERR_INVALID_ARG_TYPE", k, "number", v);
      if (!Number.isInteger(v) || v < 0 || v > 4294967295) {
        throw new (codes.ERR_OUT_OF_RANGE)(k, ">= 0 && <= 4294967295", v);
      }
    }
    if (options.maxOutstandingSettings !== undefined) {
      if (typeof options.maxOutstandingSettings !== "number" ||
          !Number.isInteger(options.maxOutstandingSettings) ||
          options.maxOutstandingSettings < 0) {
        throw __code("ERR_OUT_OF_RANGE", "options.maxOutstandingSettings", options.maxOutstandingSettings);
      }
    }
    if (secure) {
      for (const k of ["maxSessionInvalidStreams", "maxSessionRejectedStreams", "maxSessionInvalidFrames"]) {
        if (options[k] !== undefined &&
            (typeof options[k] !== "number" || !Number.isInteger(options[k]) || options[k] < 0)) {
          throw __code("ERR_OUT_OF_RANGE", `options.${k}`, options[k]);
        }
      }
      if (options.ALPNCallback !== undefined && options.ALPNProtocols !== undefined) {
        throw __code("ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS");
      }
      if (options.key !== undefined && options.cert !== undefined) {
        this.__tlsOpts = { tls: { key: String(Array.isArray(options.key) ? options.key[0] : options.key),
                                  cert: String(Array.isArray(options.cert) ? options.cert[0] : options.cert) } };
      } else {
        this.__tlsOpts = null;
      }
    }
    // request 监听器只由 createServer/createSecureServer 接线（§4.39）
    this.__ev = this.__ev.bind(this);
  }
  get socket() { return this.__sockBag; }
  setTimeout(msecs, callback) {
    if (callback !== undefined && typeof callback !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", callback);
    }
    // node Http2Server.setTimeout：记 `this.timeout`，返回 this。
    this.timeout = msecs;
    if (typeof callback === "function") this.once("timeout", callback);
    const ms = Number(msecs) || 0;
    if (this.__timeoutTimer !== undefined) clearTimeout(this.__timeoutTimer);
    if (ms > 0) {
      this.__timeoutTimer = setTimeout(() => {
        this.__timeoutTimer = undefined;
        for (const [, s] of this.__sessions) s.emit("timeout");
        this.emit("timeout");
      }, ms);
    }
    return this;
  }
  updateSettings(settings) {
    const validated = __validateSettings(settings);
    this.__opts.settings = __applySettings(this.__opts.settings ?? {}, validated);
    return this;
  }
  close(cb) {
    if (typeof cb === "function") this.once("close", cb);
    if (this.__closing) return this;
    this.__closing = true;
    if (this.__id) {
      __wjs2_net_destroy(this.__id);
      this.__id = 0;
    } else {
      // 未监听或已关：直接收尾（node server.close 无监听也回调）
      queueMicrotask(() => {
        if (this.__sessions.size === 0) this.__emitClose();
      });
    }
    // node：server.close() 主动关闲置连接（无在途流的会话立即收尾；
    // 有在途流的会话不受影响，自然排空后才收；busy 存活由 p7-closebusy 实测）。
    // 递延 250ms 再杀：在途 HEADERS/END 先落盘且客户端自然收尾优先
    //（blk1/3/4：finish 同 tick 即 close()，杀早了会抢在客户端泵机处理 END
    // 之前断连，客户端无 'error' 监听即 fatal；自然收尾通常毫秒级，250ms
    // 只杀真闲置。Rust 侧 graceful GOAWAY 是第二道网）。
    const __idle = [...this.__sessions.values()].filter(
      (s) => s.__streams.size === 0 && !s.closed && !s.destroyed);
    if (__idle.length > 0) {
      setTimeout(() => {
        for (const s of __idle) {
          if (s.__streams.size === 0 && !s.closed && !s.destroyed) s.close();
        }
      }, 250);
    }
    return this;
  }
  __emitClose() {
    if (this.__closeEmitted) return;
    this.__closeEmitted = true;
    this.emit("close");
  }
  __sessionFor(connId, peerObj) {
    let s = this.__sessions.get(connId);
    if (s === undefined) {
      s = new Http2Session(this, connId, peerObj);
      this.__sessions.set(connId, s);
      this.emit("session", s);
    }
    return s;
  }
  __abortEntry(entry) {
    if (!entry) return;
    entry.stream.__abort();
    this.__streams.delete(entry.stream.id);
  }
  __ev(kind, payload) {
    switch (kind) {
      case "listening": {
        const o = JSON.parse(payload);
        this.__listening = { address: o.addr, port: o.port, family: String(o.addr).includes(":") ? "IPv6" : "IPv4" };
        this.emit("listening");
        break;
      }
      case "request": {
        const o = JSON.parse(payload);
        const id = Number(o.streamId);
        // 毒化连接（cant-turn-off）：ECP 关后对端残留 HEADERS 到达即丢，
        // 不建流不发事件（node：会话已死，HEADERS 到即无声丢弃）。
        const preSess = this.__sessions.get(Number(o.connId));
        if (preSess && preSess.__poisoned) break;
        const peer = String(o.peer ?? "");
        const peerObj = {
          addr: peer.includes(":") ? peer.slice(0, peer.lastIndexOf(":")) : peer,
          port: peer.includes(":") ? Number(peer.slice(peer.lastIndexOf(":") + 1)) : 0,
        };
        const session = this.__sessionFor(Number(o.connId), peerObj);
        const stream = new Http2ServerStream(session, Number(o.connId), id);
        const streaming = o.streaming === true;
        const bodyEmpty = !streaming && (o.body ?? "") === "";
        const trailersEmpty = !streaming && (o.trailers ?? "[]") === "[]";
        const flags = bodyEmpty && trailersEmpty ? 5 : 4;
        stream.endAfterHeaders = bodyEmpty && trailersEmpty;
        const sock = __mkSocketProxy(stream, this, peerObj);
        const scheme = this.__secure ? "https" : "http";
        const req = new Http2ServerRequest(stream, o, scheme, sock);
        const res = new Http2ServerResponse(req, stream);
        this.__streams.set(id, { stream, req, res });
        stream.once("close", () => this.__streams.delete(id));
            const method = req.headers[":method"];
            if (method === "HEAD") {
              // HEAD 应答自动 END_STREAM（respond 内处理）
              stream.__headRequest = true;
            }
            // node setupCompat 口径：'connect'/405 只在 compat 接线后；
            // 纯 'stream' 监听的 CONNECT（含扩展）走通用 'stream' 路径。
            if (method === "CONNECT" && this.__compatAttached) {
              // node onServerStream 逐字：无 'connect' 监听即 405 + end（旧 501 系误植）。
              if (!this.emit("connect", req, res)) { res.statusCode = 405; res.end(); }
            } else if (req.headers.expect !== undefined) {
          // node onServerStream 逐字（expect-handling/checkContinue 套件）：
          // 100-continue + checkContinue 监听 → 只发 checkContinue（写 100 由
          // writeContinue/native interim 承接，hyper 底座 bound 另案）；
          // 其余 expect + checkExpectation 监听 → 只发 checkExpectation；
          // 其余 expect 无人监听 → 417 + end。
          // 无监听的 100-continue：writeContinue 记账后仍发 request（线形留待 native）。
          const ex = req.headers.expect;
          if (ex === "100-continue") {
            if (this.listenerCount("checkContinue") > 0) {
              this.emit("checkContinue", req, res);
            } else {
              res.writeContinue();
              this.emit("stream", stream, req.headers, flags, req.rawHeaders);
              if (this.listenerCount("request") > 0) this.emit("request", req, res);
              // node：会话侧 'stream' 在服务端 'stream'/'request' 之后
              //（用户会话监听注册晚于 compat 内部接线，p9-stream-order 实测）。
              session.emit("stream", stream, req.headers, flags, req.rawHeaders);
            }
          } else if (this.listenerCount("checkExpectation") > 0) {
            this.emit("checkExpectation", req, res);
          } else {
            res.statusCode = 417;
            res.end();
          }
          if (!streaming) {
            stream.__feedBody(o.body ?? "");
            stream.__endReq(o.trailers ?? "[]");
          }
          if (this.listenerCount("request") > 0 || this.listenerCount("checkContinue") > 0 ||
              this.listenerCount("checkExpectation") > 0) {
            queueMicrotask(() => { if (!req.destroyed && !req.__userPaused) req.resume(); });
          }
        } else {
          // node 顺序：'stream'（raw）先于 'request'（compat），会话侧 'stream' 最末
          //（p9-stream-order 实测：server session → server stream → request → session stream）。
          this.emit("stream", stream, req.headers, flags, req.rawHeaders);
          const hasCompat = this.listenerCount("request") > 0;
          if (hasCompat) this.emit("request", req, res);
          session.emit("stream", stream, req.headers, flags, req.rawHeaders);
          // 流式体：`body`/`reqEnd` 流事件逐块跟进（见下）；否则头即终。
          if (!streaming) {
            stream.__feedBody(o.body ?? "");
            stream.__endReq(o.trailers ?? "[]");
          }
          if (hasCompat) queueMicrotask(() => { if (!req.destroyed && !req.__userPaused) req.resume(); });
        }
        break;
      }
      case "body": {
        const o = JSON.parse(payload);
        const e = this.__streams.get(Number(o.streamId));
        if (e) e.stream.__feedBody(o.payload ?? "");
        break;
      }
      case "reqEnd": {
        const o = JSON.parse(payload);
        const e = this.__streams.get(Number(o.streamId));
        if (e) e.stream.__endReq(o.payload ?? "[]");
        break;
      }
      case "aborted": {
        const o = JSON.parse(payload);
        const entry = this.__streams.get(Number(o.streamId));
        // 对端 RST（wire 窥探，payload 为码）：记 abort 码，_destroy 按码合成
        // 流错误（stream-destroy-event-order）；空 payload 系会话收尾，无错。
        if (entry && o.payload !== undefined && o.payload !== "") {
          entry.stream.rstCode = Number(o.payload);
          entry.stream.__abortCode = Number(o.payload);
        }
        this.__abortEntry(entry);
        break;
      }
      case "connOpen": {
        // 连接建立即建会话并发 'session'（包装层 streamId 恒 0；payload = "connId ip:port"）。
        const raw = String(JSON.parse(payload).payload ?? "");
        const sp = raw.indexOf(" ");
        const connId = Number(raw.slice(0, sp));
        const peer = raw.slice(sp + 1);
        const peerObj = {
          addr: peer.includes(":") ? peer.slice(0, peer.lastIndexOf(":")) : peer,
          port: peer.includes(":") ? Number(peer.slice(peer.lastIndexOf(":") + 1)) : 0,
        };
        this.__sessionFor(connId, peerObj);
        break;
      }
      case "connClose": {
        // Rust 侧 payload = conn_id 裸串；包装层 streamId 恒 0，须读 .payload
        const connId = Number(JSON.parse(payload).payload);
        const s = this.__sessions.get(connId);
        if (s) s.__ev("connClose", payload);
        // 全会话已收且监听器已关 → server 'close'
        queueMicrotask(() => {
          if (this.__closing && !this.__id && this.__sessions.size === 0) this.__emitClose();
        });
        break;
      }
      case "error": {
        const o = JSON.parse(payload);
        this.emit("error", __h2Err(o.code, o.msg));
        break;
      }
      case "close": {
        // 监听器已关（node：不杀活连接）。'close' 等全部会话排空
        // （connClose 走 conn 自身 target，不受本次 purge 影响）。
        queueMicrotask(() => {
          if (this.__closing && !this.__id && this.__sessions.size === 0) this.__emitClose();
        });
        break;
      }
    }
  }
  address() { return this.__listening; }
  listen(...args) {
    let port, host = null, cb = null;
    if (typeof args[0] === "function") {
      // node：listen(cb) 即随机端口 + 回调（client-request-listeners-warning）。
      cb = args[0];
      port = 0;
    } else if (typeof args[0] === "object" && args[0] !== null) {
      port = args[0].port;
      host = args[0].host ?? null;
      cb = typeof args[1] === "function" ? args[1] : null;
    } else {
      port = args[0];
      for (let i = 1; i < args.length; i++) {
        if (typeof args[i] === "string" && host === null) host = args[i];
        else if (typeof args[i] === "function") cb = args[i];
      }
    }
    if (cb) this.once("listening", cb);
    this.__port = Number(port);
    this.__closing = false;
    // 同步 bind 口径：native 回 "id:picked"，当 tick 即置 __listening
    //（error-order 套件 listen(0) 后同步读 address().port）。
    // settings 直传 native（enableConnectProtocol/maxHeaderListSize 走 hyper
    // Builder；connect-method-extended/too-large-headers 套件）。
    const __listenOpts = { ...(this.__tlsOpts ?? {}) };
    if (this.__opts.settings !== undefined) {
      __listenOpts.settings = __validateSettings(this.__opts.settings, "options.settings");
    }
    const __ret = String(__wjs2_h2_listen(Number(port), host === null ? "0.0.0.0" : host,
      JSON.stringify(__listenOpts), this)).split(":");
    this.__id = Number(__ret[0]);
    const __picked = Number(__ret[1] ?? 0);
    if (__picked !== 0) {
      const __h = host === null ? "0.0.0.0" : host;
      this.__listening = {
        address: __h,
        port: __picked,
        family: __h.includes(":") ? "IPv6" : "IPv4",
      };
      this._connectionKey = `${__h.includes(":") ? 6 : 4}:${__h}:${this.__port}`;
    }
    return this;
  }
  [Symbol.asyncDispose]() {
    return new Promise((resolve) => {
      this.close(() => resolve(undefined));
    });
  }
  ref() { if (this.__id) __wjs2_net_ref(this.__id); return this; }
  unref() { if (this.__id) __wjs2_net_unref(this.__id); return this; }
}
