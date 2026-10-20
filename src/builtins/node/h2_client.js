const kPendingOptions = Symbol("options");

// ── 客户端 ──────────────────────────────────────────────────────────────
class ClientHttp2Stream extends Duplex {
  constructor(session, id, headers, options) {
    super();
    this.__session = session;
    this.id = id;
    this.sentHeaders = headers;
    // node：干净关的流 rstCode 为 0（connect-method-extended 套件点名）
    this.rstCode = 0;
    this.__ended = false;
    this.aborted = false;
    this.__opened = false;
    this.__waitTrailers = !!(options && options.waitForTrailers);
    this.__pendingBody = [];
    this.endAfterHeaders = false;
    this.__responseReceived = false;
  }
  get session() { return this.__session; }
  get bufferSize() { return this.writableLength; }
  get pushAllowed() { return false; }
  get sentPseudoHeaders() {
    const out = { __proto__: null };
    for (const [k, v] of Object.entries(this.sentHeaders)) {
      if (k.startsWith(":")) out[k] = Array.isArray(v) ? String(v[0]) : String(v);
    }
    return out;
  }
  get sentInfoHeaders() { return []; }
  get sentTrailers() { return null; }
  get state() {
    return {
      state: this.destroyed ? 7 : 2,
      weight: 16,
      sumDependencyWeight: 0,
      localClose: this.writableEnded ? 1 : 0,
      remoteClose: this.readableEnded ? 1 : 0,
      localWindowSize: 65535,
    };
  }
  _read() {}
  __openNow(extraBody, streaming = false) {
    __clientOpenNow(this, extraBody, streaming);
  }
  // CONNECT 隧道抑发（2026-10-21，http2 lane，4.322 对侧·4.326 修正）：
  // hyper 不驱动 CONNECT 请求体（无 body 管道；随头的体直接抛弃）且 200 前见
  // EOS 即不建 upgrade 对（致命）。node（nghttp2）无此接管，隧道双向通。
  // 修法：隧道形 CONNECT（无 :protocol 的纯隧道；RFC 8441 扩展 CONNECT 除外）
  // 应答前只发 HEADERS（open 空体），写侧全缓冲、END 递延；应答到达后经升级桥
  // flush DATA + 补 END（2xx 建隧道半关、非 2xx 语义闭合）。已应答/trailers 形原路径。
  // 极限：immediate-FIN 形（目标接通即 FIN）仍输竞速（DATA 只能 200 后发，天生晚
  // 一 RTT），connect-method 套件书面偏离；耐心对端全双工通（黑盒见证）。
  __connectDeferEnd() {
    if (this.__responseReceived || this.__waitTrailers) return false;
    const h = this.sentHeaders ?? {};
    return String(h[":method"] ?? "").toUpperCase() === "CONNECT" && h[":protocol"] === undefined;
  }
  _write(chunk, encoding, cb) {
    // 已开流直发块；悬置期缓冲（connect 冲刷）；单发流写即错。
    if (this.__ended || (!this.__streaming && this.__opened)) {
      cb(__code("ERR_STREAM_WRITE_AFTER_END"));
      return;
    }
    const u8 = chunk instanceof Uint8Array ? chunk : __toU8(String(chunk), "write");
    if (this.__connectDeferEnd()) {
      // 隧道抑 EOS 期：写侧只缓冲（hyper 不驱动 CONNECT 请求体，直发即弃；
      // 应答后经升级桥补发，见 __onResponse）。
      this.__pendingBody.push(u8);
      queueMicrotask(cb);
      return;
    }
    if (this.__opened) {
      try {
        __wjs2_h2_data(this.__session.__id, this.id, __b64enc(u8));
      } catch {
        this.__detachFromSession();
      }
      queueMicrotask(cb);
      return;
    }
    this.__pendingBody.push(u8);
    queueMicrotask(cb);
  }
  _final(cb) {
    this.__ended = true;
    if (!this.__opened) {
      if (this.__connectDeferEnd()) {
        // 隧道抑 EOS：HEADERS-only 开流（hyper 不驱动 CONNECT 请求体，
        // 体随头即被抛弃；攒体由 JS 持有、应答后经升级桥补发）。
        // 空体仍非流式（Rust 空隧道本就不 End），有体用 streaming 形开流。
        const __held = this.__pendingBody;
        this.__pendingBody = [];
        this.__openNow(null, __held.length > 0);
        this.__pendingBody = __held;
        this.__deferEnd = true;
      } else {
        this.__openNow(null);
      }
    } else if (this.__streaming) {
      // 隧道抑 EOS：抑期攒体（_write 已缓冲）不出线、END 递延，
      // 应答后一并补（__onResponse）。
      if (this.__connectDeferEnd()) {
        this.__deferEnd = true;
      } else {
        // 流式收尾：余块刷出 + END（waitTrailers 由 sendTrailers 收尾，此处跳过）。
        if (this.__pendingBody.length > 0) {
          let total = 0;
          for (const b of this.__pendingBody) total += b.length;
          const flat = new Uint8Array(total);
          let off = 0;
          for (const b of this.__pendingBody) { flat.set(b, off); off += b.length; }
          this.__pendingBody = [];
          try {
            __wjs2_h2_data(this.__session.__id, this.id, __b64enc(flat));
          } catch {
            this.__detachFromSession();
          }
        }
        if (!this.__waitTrailers && this.__session?.__id) {
          try {
            __wjs2_h2_end(this.__session.__id, this.id, "[]");
          } catch {
            this.__detachFromSession();
          }
        }
      }
    }
    cb();
  }
  sendTrailers(trailers) {
    // node core.js 门序（同服务端 sendTrailers 注）。
    if (this.destroyed || this.closed) throw __code("ERR_HTTP2_INVALID_STREAM");
    if (this.__trailersSent) throw __code("ERR_HTTP2_TRAILERS_ALREADY_SENT");
    if (!this.__opened || !this.__waitTrailers || !this.__wantTrailersFired) {
      throw __code("ERR_HTTP2_TRAILERS_NOT_READY");
    }
    this.__trailersSent = true;
    const t = [];
    for (const [k, v] of Object.entries(trailers ?? {})) t.push([k, String(v)]);
    __wjs2_h2_open_trailers(this.__session.__id, this.id, JSON.stringify(t));
    return this;
  }
  __onResponse(headers, flags) {
    this.__responseReceived = true;
    // CONNECT 隧道递延补发（见 __connectDeferEnd）：抑 EOS 期攒体（如有）先拼发
    // 再补 END（2xx 即隧道半关、非 2xx 语义闭合）。
    // 会话已亡/流已毁即弃（隧道流永不 EOS、随会话收尾口径）。
    if (this.__deferEnd) {
      this.__deferEnd = false;
      const __sid = this.__session?.__id;
      if (__sid && !this.destroyed) {
        try {
          if (this.__pendingBody.length > 0) {
            let __total = 0;
            for (const __b of this.__pendingBody) __total += __b.length;
            const __flat = new Uint8Array(__total);
            let __off = 0;
            for (const __b of this.__pendingBody) { __flat.set(__b, __off); __off += __b.length; }
            this.__pendingBody = [];
            __wjs2_h2_data(__sid, this.id, __b64enc(__flat));
          }
          __wjs2_h2_end(__sid, this.id, "[]");
        } catch {
          this.__detachFromSession();
        }
      }
    }
    // endAfterHeaders 是请求侧标志（HEADERS 帧是否带 END_STREAM），
    // 不由应答 flags 改写（endafterheaders 套件：GET 分开 end 即 false）。
    this.emit("response", headers, flags);
  }
  __onData(u8) { this.push(Buffer.from(u8)); }
  __onTrailers(t) { this.emit("trailers", t); }
  __onEnd() {
    this.push(null);
    // node onStreamClose：客户端流 push(null) 后 read(0)——无人读的空体应答也能发 'end'
    // （缓冲有数据时 read(0) 不消费，照旧等用户读）。
    if (this.readableFlowing !== true) this.read(0);
    // node onStreamClose 口径：可读侧尚有数据（或无人消费）时等 'end' 再销毁——
    // 提前销毁会吞缓冲数据并抢先派 'close'（short-stream 套件点名 close 在 end 后）。
    if (this.writableEnded && !this.destroyed) {
      this.once("end", () => { if (!this.destroyed) this.destroy(); });
    }
  }
  __onAborted(rstCode) {
    __clientOnAborted(this, rstCode);
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
    if (options.silent !== true) { /* 无 PRIORITY 帧底座（偏差记档） */ }
    return this;
  }
  close(code = 0, cb) {
    if (typeof code === "function") { cb = code; code = 0; }
    __validateStreamClose(code, cb);
    if (!this.destroyed) {
      if (typeof cb === "function") this.once("close", cb);
      this.__ended = true;
      const rstErr = __h2RstError(code);
      if (rstErr !== undefined) {
        this.rstCode = code;
        this.emit("error", rstErr); // 非零非 CANCEL 即本地流错误，先于 close。
      } else if ((code ?? 0) !== 0) {
        this.rstCode = code;
      }
      if (this.__opened && this.__session.__id) {
        __wjs2_h2_reset(this.__session.__id, this.id, code ?? 0);
      }
      this.destroy();
    } else if (typeof cb === "function") {
      queueMicrotask(cb);
    }
    return this;
  }
  destroy(err, code, cb) {
    if (typeof err === "number") { cb = code; code = err; err = undefined; }
    if (typeof code === "function") { cb = code; code = 0; }
    if (typeof cb === "function") this.once("close", cb);
    if (this.destroyed) return this;
    // node：流销毁 → RST(CANCEL) 通知对端（未收到应答且已出线的流；
    // aborted 套件：client.destroy() 后服务端 req 须见 'aborted'）。
    // c2 轮：下方曾有无 RST 的重复 destroy 定义将其遮蔽，致 RST 永不发出——已删。
    if (this.__opened && !this.__responseReceived && this.__session?.__id) {
      __wjs2_h2_reset(this.__session.__id, this.id, 8);
    }
    this.__detachFromSession();
    super.destroy(err);
    return this;
  }
  __detachFromSession() {
    const s = this.__session;
    if (s && s.__streams) {
      s.__streams.delete(this.id);
      const pi = s.__pendingOpens ? s.__pendingOpens.indexOf(this) : -1;
      if (pi !== -1) s.__pendingOpens.splice(pi, 1);
      if (typeof s.__gracefulWait === "function") s.__gracefulWait();
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
    if (ms > 0 && !this.destroyed) {
      this.__timeoutTimer = setTimeout(() => {
        this.__timeoutTimer = undefined;
        this.emit("timeout");
      }, ms);
    }
    return this;
  }
}

class ClientHttp2Session extends EventEmitter {
  constructor(authority, options) {
    super();
    this.__id = 0;
    this.__seq = 1;
    this.__streams = new Map();
    // 已终结流 tombstone：迟到重复终点静默丢弃，禁回落会话 error。
    this.__goneStreams = new Set();
    this.__authority = authority;
    this.__options = options ?? {};
    this.destroyed = false;
    this.closed = false;
    this.type = 1; // NGHTTP2_SESSION_CLIENT
    this.encrypted = false;
    this.connecting = true;
    this.__settings = { ...__DEFAULT_SETTINGS, ...(options?.settings !== undefined ? __validateSettings(options.settings, "options.settings") : {}) };
    this.__remoteSettings = { ...__DEFAULT_SETTINGS };
    this.__pendingSettingsAck = false;
    this.__outstandingSettings = 0;
    this.__maxOutstandingSettings = options?.maxOutstandingSettings ?? Infinity;
    // node 缺省 maxOutstandingPings = 2（ping.js 超额即 CANCEL）
    this.__maxOutstandingPings = options?.maxOutstandingPings ?? 2;
    this.__pendingOpens = [];
    this.state = {
      effectiveLocalWindowSize: 65535,
      effectiveRemoteWindowSize: 65535,
      localWindowSize: 65535,
      remoteWindowSize: 65535,
      outboundQueueSize: 0,
      deflateDynamicTableSize: 4096,
      inflateDynamicTableSize: 4096,
    };
    this.__ev = this.__ev.bind(this);
  }
  get socket() {
    if (this[kSocket] === undefined) {
      const s = this;
      const sock = new EventEmitter();
      sock.connecting = true;
      sock.destroyed = false;
      sock.readable = true;
      sock.writable = true;
      sock.remoteAddress = s.__host;
      sock.remotePort = s.__port;
      sock.localAddress = undefined;
      sock.localPort = undefined;
      sock.on("newListener", (ev) => {
        if (ev === "close" && s.closed && !sock.destroyed) {
          queueMicrotask(() => sock.emit("close"));
        }
      });
      sock.destroy = (err) => {
        if (sock.destroyed) return;
        sock.destroyed = true;
        if (err) sock.emit("error", err);
        s.destroy();
      };
      sock.end = () => { s.close(); return sock; };
      sock.write = () => true;
      sock.setTimeout = (m, cb) => { s.setTimeout(m, cb); return sock; };
      sock.address = () => ({ address: s.__host, port: s.__port, family: s.__host?.includes(":") ? "IPv6" : "IPv4" });
      this[kSocket] = sock;
    }
    return this[kSocket];
  }
  get alpnProtocol() { return this.__secure ? "h2" : "h2c"; }
  get localSettings() { return this.__settings; }
  get remoteSettings() { return this.__remoteSettings; }
  get pendingSettingsAck() { return this.__pendingSettingsAck; }
  get originSet() { return this.__secure ? [] : undefined; }
  get unrefed() { return false; }
  setTimeout(msecs, callback) {
    if (typeof msecs !== "number") throw __code("ERR_INVALID_ARG_TYPE", "msecs", "number", msecs);
    if (callback !== undefined && typeof callback !== "function") {
      throw __code("ERR_INVALID_ARG_TYPE", "callback", "function", callback);
    }
    if (typeof callback === "function") this.once("timeout", callback);
    const ms = Number(msecs) || 0;
    // kTimeout 形状与服务端会话一致（compat-socket 套件同口径）。
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
  ref() { if (this.__id) __wjs2_net_ref(this.__id); return this; }
  unref() { if (this.__id) __wjs2_net_unref(this.__id); return this; }
  settings(settings = {}, cb) {
    const validated = __validateSettings(settings);
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
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
    if (typeof code === "object" && code !== null) {
      const o = code;
      code = o.errorCode ?? 0;
      lastStreamID = o.lastStreamID ?? 0;
      opaqueData = o.opaqueData;
    }
    this.__goaway = { code, lastStreamID, opaqueData };
    setImmediate(() => this.close());
    return this;
  }
  setNextStreamID(id) {
    if (this.destroyed) throw __code("ERR_HTTP2_INVALID_SESSION");
    // node core.js 口径：非数 ARG_TYPE；整数 (0, 2^32]，文案逐字
    //（setNextStreamID-errors 套件点名 `> 0 and <= 4294967295`）。
    if (typeof id !== "number") throw __code("ERR_INVALID_ARG_TYPE", "id", "number", id);
    if (!Number.isInteger(id) || id <= 0 || id > 4294967295) {
      throw new codes.ERR_OUT_OF_RANGE("id", "> 0 and <= 4294967295", id);
    }
    this.__seq = id;
    return this;
  }
  altsvc(alt, origin) {
    if (typeof alt === "string" || alt === undefined) return this;
    throw __code("ERR_INVALID_ARG_TYPE", "alt", "string", alt);
  }
  origin(...origins) { return this; }
  __start(port, host) {
    this.__host = host;
    this.__port = port;
    // kSocket 即刻物化（node：connect() 即有 socket；套件 connect() 返回后
    // 直探 client[kSocket]，懒 getter 拿到 undefined）
    void this.socket;
    // node createConnection：用户 Duplex 为传输建会话（隧道），不 dial。
    const __factory = this.__options.createConnection;
    if (typeof __factory === "function") {
      let __duplex;
      try { __duplex = __factory(); } catch (e) {
        this.connecting = false;
        queueMicrotask(() => { if (!this.destroyed) this.emit("error", e); });
        return this;
      }
      this.__id = Number(__wjs2_h2_client_adopt(this));
      // 隧道写出绕过请求流写门（CONNECT 应答后 duplex.write 即
      // WRITE_AFTER_END）：native data 直发外层流。
      const __outerConn = __duplex.__session ? __duplex.__session.__id : 0;
      const __outerId = __duplex.id;
      this.__bridge = __h2BridgeDuplex(this, __duplex, this.__id,
        (i, b) => __wjs2_h2_client_adopt_data(i, b),
        (i) => __wjs2_h2_client_adopt_close(i),
        (b64) => __wjs2_h2_data(__outerConn, __outerId, b64));
      return this;
    }
    const wire = {};
    if (this.__options.tls !== undefined || this.__secure) {
      const t = this.__options.tls ?? this.__options;
      wire.tls = {};
      if (t.ca !== undefined) wire.tls.ca = String(t.ca);
      if (t.rejectUnauthorized !== undefined) wire.tls.rejectUnauthorized = !!t.rejectUnauthorized;
      if (t.servername !== undefined) wire.tls.servername = String(t.servername);
    }
    // node：connect 经 net.connect，自定义 lookup 优先解析（promisify-
    // connect-error 套件：lookup 错原样 error；base16 回归）。
    const __lk = this.__options.lookup;
    if (typeof __lk === "function") {
      const self = this;
      let __done = false;
      try {
        __lk(host, { family: 0, hints: 0 }, (err, address) => {
          if (__done) return;
          __done = true;
          if (err) {
            self.connecting = false;
            queueMicrotask(() => { if (!self.destroyed) self.emit("error", err); });
            return;
          }
          self.__id = Number(__wjs2_h2_connect(String(address ?? host), port, JSON.stringify(wire), self));
        });
      } catch (e) {
        this.connecting = false;
        queueMicrotask(() => { if (!this.destroyed) this.emit("error", e); });
      }
      return this;
    }
    this.__id = Number(__wjs2_h2_connect(host, port, JSON.stringify(wire), this));
    return this;
  }
  __ev(kind, payload) {
    switch (kind) {
      case "connect":
        this.connecting = false;
        if (this[kSocket]) {
          this[kSocket].connecting = false;
          queueMicrotask(() => this[kSocket].emit("connect"));
        }
        for (const pst of this.__pendingOpens.splice(0)) pst.__openNow(null, !pst.__endStream);
        this.emit("connect", this, null);
        break;
      case "remoteSettings": {
        // 服务端 SETTINGS（SnoopIo 线上窥探；hyper 无回调）——node 口径：
        // 缺省全集 + 线上值覆盖；ECP 1→0 即连接级 PROTOCOL_ERROR
        //（cant-turn-off：remoteSettings 不再发，直接走协议错误）。
        const o = JSON.parse(payload);
        const upd = __decodeRemotePairs(JSON.parse(o.payload ?? "[]"));
        if (__maybeProtoPoison(this, upd)) break;
        if (upd.enableConnectProtocol === true) this.__sawECP1 = true;
        this.__remoteSettings = { ...this.__remoteSettings, ...upd };
        this.emit("remoteSettings", this.__remoteSettings);
        break;
      }
      case "goawayError": {
        // 服务端 GOAWAY 非零码。11 = header 超限（too-large-headers）；
        // 其余 = 会话级销毁（propagate-session-destroy-code，见 h2_head.js）。
        const o = JSON.parse(payload);
        const inner = o.payload === undefined ? o : JSON.parse(o.payload);
        const code = Number(inner.code);
        if (code === 11) {
          __goawayStreamErrors(this, inner);
          break;
        }
        __goawaySessionDestroy(this, code);
        break;
      }
      case "response": {
        const o = JSON.parse(payload);
        const st = this.__streams.get(Number(o.streamId));
        if (!st) break;
        const head = JSON.parse(o.payload);
        const headers = __pairsToObj(JSON.parse(head.headers));
        headers[":status"] = head.status;
        st.__onResponse(headers, head.flags ?? 4);
        break;
      }
      case "data": {
        const o = JSON.parse(payload);
        const st = this.__streams.get(Number(o.streamId));
        if (!st) break;
        st.__onData(__b64dec(o.payload));
        break;
      }
      case "trailers": {
        const o = JSON.parse(payload);
        const st = this.__streams.get(Number(o.streamId));
        if (!st) break;
        st.__onTrailers(__pairsToObj(JSON.parse(o.payload)));
        break;
      }
      case "end": {
        const o = JSON.parse(payload);
        const sid = Number(o.streamId);
        const st = this.__streams.get(sid);
        if (!st) break;
        st.__onEnd();
        this.__streams.delete(sid);
        this.__goneStreams.add(sid);
        break;
      }
      case "aborted": {
        const o = JSON.parse(payload);
        const sid = Number(o.streamId);
        const st = this.__streams.get(sid);
        if (!st) break;
        st.__onAborted(o.payload);
        this.__streams.delete(sid);
        this.__goneStreams.add(sid);
        break;
      }
      case "error": {
        // 已销毁会话的迟到错误（如握手失败竞态）吞掉（node：destroy 后不事件）
        if (this.destroyed) break;
        const o = JSON.parse(payload);
        const inner = o.payload === undefined ? o : JSON.parse(o.payload);
        const sid = o.streamId === undefined ? undefined : Number(o.streamId);
        const err = __h2Err(inner.code ?? "ERR_HTTP2_STREAM_ERROR", inner.msg ?? "");
        const st = sid !== undefined ? this.__streams.get(sid) : undefined;
        if (st) {
          // 流级错误：派发后即销毁（node：RST 后流走 destroy → 'close'；
          // 否则 close 永不到，fd-invalid 类套件挂死）。无监听器回落会话
          //（旧口径保留，error-order 系套件依赖）。
          if (inner.rst !== undefined && inner.rst !== null) st.rstCode = Number(inner.rst);
          // 传输层死亡先 aborted 后 error（探针序）；流 RST 错误不发 aborted。
          if (inner.code === "ECONNRESET" && !st.aborted) {
            st.aborted = true;
            st.emit("aborted");
          }
          if (st.listenerCount("error") > 0) st.emit("error", err);
          else this.emit("error", err);
          st.destroy();
          this.__goneStreams.add(sid);
        } else if (sid !== undefined && this.__goneStreams.has(sid)) {
          break; // 迟到重复终点：静默丢弃。
        } else {
          this.emit("error", err);
        }
        break;
      }
      case "close":
        this.closed = true;
        this.connecting = false;
        if (this[kSocket] && !this[kSocket].destroyed) {
          this[kSocket].destroyed = true;
          queueMicrotask(() => this[kSocket].emit("close"));
        }
        this.emit("close");
        break;
      case "adoptData": { // 通道桥写出落用户 Duplex。
        const raw = String(JSON.parse(payload).payload ?? "");
        if (this.__bridge) this.__bridge.write(raw.slice(raw.indexOf(" ") + 1));
        break;
      }
    }
  }
  request(headers, options) {
    // node：请求选项带优先级字段即弃用告警（DEP0194，RFC 9113 废止优先级信令；进程级一次）。
    if (options && (options.weight !== undefined || options.parent !== undefined ||
        options.exclusive !== undefined || options.silent !== undefined)) {
      __h2RequestPriorityDeprecate();
    }
    // node：closed（GOAWAY 后）新建流同步抛 ERR_HTTP2_GOAWAY_SESSION；
    // destroyed 会话上 request() 不同步抛——返回一个异步 error
    // ERR_HTTP2_INVALID_SESSION + 'close' 的流（client-destroy 套件）
    if (this.closed && !this.destroyed) throw __code("ERR_HTTP2_GOAWAY_SESSION");
    if (this.destroyed) {
      const sid = this.__seq;
      this.__seq += 2;
      const h = { ":method": "GET", ":path": "/", ":scheme": this.__secure ? "https" : "http", ":authority": this.__authority ?? "" };
      const st = new ClientHttp2Stream(this, sid, h, options);
      process.nextTick(() => {
        if (st.destroyed) return;
        st.emit("error", __code("ERR_HTTP2_INVALID_SESSION"));
        st.destroy();
      });
      return st;
    }
    __validateRequestOptions(options);
    // node core.js request() 逐字门序：CONNECT（无 :protocol）须带 :authority 且禁 :scheme/:path；
    // 其余补缺省伪头；再经 mapToHeaders 口径校验（名 token、单值头重复）。
    const h = { ...headers };
    if (h[":method"] === undefined) h[":method"] = "GET";
    const isConnect = h[":method"] === "CONNECT" && h[":protocol"] === undefined;
    if (!isConnect) {
      if (h[":authority"] === undefined && h.host === undefined) h[":authority"] = this.__authority;
      if (h[":scheme"] === undefined) h[":scheme"] = this.__secure ? "https" : "http";
      if (h[":path"] === undefined) h[":path"] = "/";
      // host-only 请求：用户未给 :authority，nghttp2 线上即省略；
      // h2 底座强制要求 authority 只能线上传、服务端再剥（serverrequest-host 套件）。
      if (h[":authority"] === undefined && h.host !== undefined) h["x-wjs-no-authority"] = "1";
    } else {
      if (h[":authority"] === undefined) throw __code("ERR_HTTP2_CONNECT_AUTHORITY");
      if (h[":scheme"] !== undefined) throw __code("ERR_HTTP2_CONNECT_SCHEME");
      if (h[":path"] !== undefined) throw __code("ERR_HTTP2_CONNECT_PATH");
    }
    __mapToHeadersCheck(h);
    // nghttp2 口径：:path 含 CTL/空格即 PROTOCOL_ERROR（unescaped-path 套件；
    // hyper 建包错文案不对齐，此处先验，流级 error + close）。
    if (typeof h[":path"] === "string" && /[\x00-\x20\x7f]/.test(h[":path"])) {
      const sid = this.__seq;
      this.__seq += 2;
      const bst = new ClientHttp2Stream(this, sid, h, options);
      process.nextTick(() => {
        if (bst.destroyed) return;
        bst.emit("error", __h2Err("ERR_HTTP2_STREAM_ERROR", "Stream closed with error code NGHTTP2_PROTOCOL_ERROR"));
        bst.destroy();
      });
      return bst;
    }
    const pseudoKeys = Object.keys(h).filter((k) => k.startsWith(":"));
    const canonical = [":method", ":scheme", ":authority", ":path"];
    const differs = pseudoKeys.length !== canonical.length ||
      pseudoKeys.some((k, i) => k !== canonical[i]);
    if (differs) h["x-wjs-pho"] = pseudoKeys.join(",");
    const sid = this.__seq;
    this.__seq += 2; // 客户端单数流（RFC 7540 口径）
    const st = new ClientHttp2Stream(this, sid, h, options);
    this.__streams.set(sid, st);
    // options.signal：abort → ABORT_ERR 销毁（AbortSignal 套件）
    const signal = options?.signal;
    if (signal) {
      const onAbort = () => {
        if (!st.destroyed) {
          st.emit("error", __code("ABORT_ERR"));
          st.destroy();
        }
      };
      if (signal.aborted) {
        // node：预中止信号 → 同 tick destroyed（destroy 套件），error 下一跳
        st.destroy();
        process.nextTick(() => st.emit("error", __code("ABORT_ERR")));
      } else {
        // 经 addAbortListener（listenerCount 可见）
        const disposable = addAbortListener(signal, onAbort);
        st.once("close", () => disposable[Symbol.dispose]());
      }
    }
    // node：request 即开流（HEADERS 即时出线；POST 等体随写随走、
    // 收尾随 end）。endStream（GET/HEAD/DELETE 缺省、显式选项）单发
    // 随头 fin；余者流式（首块随头、后块增量、END 随 end）。
    //（旧 deferred 攒包到 end 才开流，无 end 的上传永不开——server-socket-destroy。）
    const m = String(st.sentHeaders[":method"]);
    const endStream = options?.endStream ?? (m === "GET" || m === "HEAD" || m === "DELETE");
    st.__endStream = endStream;
    if (this.connecting && !this.closed) this.__pendingOpens.push(st);
    else st.__openNow(null, !endStream);
    if (endStream) st.end();
    return st;
  }
  // node：client.close() 优雅关（GOAWAY）：新流被拒、已收到应答的流继续、
  // 未收到应答的流报 ERR_HTTP2_GOAWAY_SESSION。本实现以 100ms 宽限扫描近似
  // （响应在途的流大多在窗口内完成；真机无此延迟——偏差记档）
  close(cb) {
    if (typeof cb === "function") this.once("close", cb);
    if (this.closed) return this;
    this.closed = true;
    this.connecting = false;
    // 未出线的 open 直接取消（node：close before connect，对端不可见流）
    for (const pst of this.__pendingOpens.splice(0)) {
      pst.emit("error", __code("ERR_HTTP2_GOAWAY_SESSION"));
      pst.destroy();
    }
    this.__gracefulWait();
    if (!this.destroyed) {
      setTimeout(() => {
        if (this.destroyed) return;
        for (const [, st] of this.__streams) {
          if (!st.__responseReceived) {
            st.emit("error", __code("ERR_HTTP2_GOAWAY_SESSION"));
            st.destroy();
          } else if (st.readableEnded && !st.destroyed) {
            // 已消费完的应答流随会话收尾（close 轮：close() 后无残留引用续命）。
            st.destroy();
          } else if (!st.destroyed && String(st.sentHeaders?.[":method"] ?? "").toUpperCase() === "CONNECT") {
            // CONNECT 隧道流永不 EOS，随会话收尾（proxy-over-h2 套件点名），
            // 否则 close() 永等排空；他法慢体不受影响。
            st.destroy();
          }
        }
        this.__gracefulWait();
      }, 100);
    }
    return this;
  }
  __gracefulWait() {
    // 仅在 close()/destroy() 发起后、流已排空时硬关；单纯流 detach
    // （响应完成）不得关会话——否则后续请求报 INVALID_SESSION
    if ((this.closed || this.destroyed) && !this.destroyed && this.__streams.size === 0 && this.__id) {
      this.destroyed = true;
      __wjs2_net_destroy(this.__id);
      this.__id = 0;
    }
  }
  destroy(code, cb) {
    if (typeof code === "function") { cb = code; code = 0; }
    if (typeof cb === "function") this.once("close", cb);
    if (!this.destroyed) {
      this.destroyed = true;
      this.closed = true;
      if (code && typeof code === "object") this.emit("error", code);
      // 在途流：RST 对端（防 server 侧 service 永挂）；本地只发 'close'，
      // 无 'error'/'aborted'（真机实测 p5/p6：destroy 后完成/未完成流皆仅 close）。
      // c2：拆流关连递延——服务端 close/finish 回调同 tick 内的 destroy 与在途应答
      // 竞速时，先让已在线字节落地（response/data/end），再拆（write block1/2；
      // node 同场景下投递先赢）。destroyed/closed 仍同步置位；pendingOpens
      // 保持同步（本地无句柄残留）。RST 一并递延：同步 RST 会 abort 在途读任务，
      // 反杀清洁 END 应答的投递（block2）。
      for (const pst of this.__pendingOpens.splice(0)) {
        pst.emit("error", __code("ERR_HTTP2_STREAM_CANCEL"));
        pst.destroy();
      }
      setTimeout(() => {
        for (const [, st] of this.__streams) {
          if (!st.__responseReceived && this.__id) {
            __wjs2_h2_reset(this.__id, st.id, 8);
          }
          // 经 __onAborted 收尾（先 aborted，有缓冲即递 end 后 close；
          // 真机 destroy 探针序；直接 destroy 会吞 end，client-socket-destroy）。
          if (!st.destroyed) st.__onAborted("");
        }
        if (this.__id) {
          __wjs2_net_destroy(this.__id);
          this.__id = 0;
        }
      }, 50);
    }
    return this;
  }
}

export function createServer(options, listener) {
  if (typeof options === "function") { listener = options; options = undefined; }
  // node：options 须为对象（null 亦拒；函数形已在上行转 listener）。
  if (options !== undefined && (typeof options !== "object" || options === null)) {
    throw __code("ERR_INVALID_ARG_TYPE", "options", "object", options);
  }
  const s = new Http2Server(options ?? {}, false);
  if (typeof listener === "function") s.on("request", listener);
  return s;
}
export function createSecureServer(options, listener) {
  if (options !== undefined && (typeof options !== "object" || options === null)) {
    throw __code("ERR_INVALID_ARG_TYPE", "options", "object", options);
  }
  const s = new Http2Server(options ?? {}, true);
  if (typeof listener === "function") s.on("request", listener);
  return s;
}
export function connect(authority, options, listener) {
  let url = null;
  let host, port, secure;
  if (typeof authority === "string") {
    url = new URL(authority);
  } else if (authority !== null && typeof authority === "object") {
    const o = authority;
    if (typeof o.href === "string" && typeof o.protocol === "string" && typeof o.hostname === "string") {
      url = new URL(o.href); // URL 实例
    } else {
      const proto = String(o.protocol ?? "http:").toLowerCase();
      if (!proto.endsWith(":")) throw __code("ERR_HTTP2_INVALID_PROTOCOL", proto, ":");
      secure = proto === "https:";
      host = String(o.authority ?? o.hostname ?? o.host ?? "localhost");
      port = o.port !== undefined ? Number(o.port) : (secure ? 443 : 80);
      if (typeof options === "function") { listener = options; options = {}; }
      const session = new ClientHttp2Session(host, { ...(options ?? {}), tls: secure ? (options ?? {}) : undefined });
      session.__secure = secure;
      wireSessionSignal(session, options);
      if (typeof listener === "function") session.once("connect", listener);
      session.__start(port, host);
      return session;
    }
  } else {
    options = authority ?? {};
  }
  if (typeof options === "function") { listener = options; options = {}; }
  // node connect()：仅 http:/https:，余者同步抛 UNSUPPORTED_PROTOCOL。
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw __code("ERR_HTTP2_UNSUPPORTED_PROTOCOL", url.protocol);
  }
  secure = url.protocol === "https:";
  const session = new ClientHttp2Session(url.host, { ...(options ?? {}), tls: secure ? (options ?? {}) : undefined });
  session.__secure = secure;
  wireSessionSignal(session, options);
  if (typeof listener === "function") session.once("connect", listener);
  port = url.port ? Number(url.port) : (secure ? 443 : 80);
  session.__start(port, url.hostname);
  return session;
}

// node core 原文 `assignFunctionName('connect', ...)`：只正名不绑名（命名函数
// 表达式会遮蔽外层 `connect` 致自递归；test-util-promisify-custom-names 点名）。
connect[Symbol.for("nodejs.util.promisify.custom")] = Object.defineProperty(
  function (authority, options) {
    return new Promise((resolve, reject) => {
      const server = connect(authority, options, () => {
        server.removeListener("error", reject);
        return resolve(server);
      });
      server.once("error", reject);
    });
  }, 'name', { value: 'connect', configurable: true });

// connect(options.signal)：abort → 会话 ABORT_ERR 销毁（AbortSignal 套件）
function wireSessionSignal(session, options) {
  const signal = options?.signal;
  if (!signal) return;
  const onAbort = () => session.destroy(__code("ABORT_ERR"));
  if (signal.aborted) {
    process.nextTick(onAbort);
    return;
  }
  const disposable = addAbortListener(signal, onAbort);
  session.once("close", () => disposable[Symbol.dispose]());
}

export function getDefaultSettings() {
  return { ...__DEFAULT_SETTINGS, customSettings: {} };
}
export function getPackedSettings(settings) {
  const s = __validateSettings(settings ?? {});
  const out = [];
  const push = (id, val) => {
    out.push((id >> 8) & 0xff, id & 0xff, (val >>> 24) & 0xff, (val >>> 16) & 0xff, (val >>> 8) & 0xff, val & 0xff);
  };
  if (s.headerTableSize !== undefined) push(0x1, s.headerTableSize);
  if (s.enablePush !== undefined) push(0x2, s.enablePush ? 1 : 0);
  if (s.maxConcurrentStreams !== undefined) push(0x3, s.maxConcurrentStreams);
  if (s.initialWindowSize !== undefined) push(0x4, s.initialWindowSize);
  if (s.maxFrameSize !== undefined) push(0x5, s.maxFrameSize);
  // id 0x6：maxHeaderListSize 优先，缺省回落 maxHeaderSize（真机对拍）
  if (s.maxHeaderListSize !== undefined) push(0x6, s.maxHeaderListSize);
  else if (s.maxHeaderSize !== undefined) push(0x6, s.maxHeaderSize);
  if (s.enableConnectProtocol !== undefined) push(0x8, s.enableConnectProtocol ? 1 : 0);
  if (s.customSettings) {
    for (const kRaw of Object.keys(s.customSettings)) {
      const id = Number(kRaw);
      const v = Number(s.customSettings[kRaw]);
      if (!Number.isInteger(id) || id < 0 || id > 0xffff) throw __settingErr("customSettings", kRaw, false);
      if (!Number.isInteger(v) || v < 0 || v > 0xffffffff) throw __settingErr("customSettings", v, false);
    }
    if (Object.keys(s.customSettings).length > 10) throw __code("ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS");
    for (const k of Object.keys(s.customSettings).map(Number).sort((a, b) => a - b)) {
      push(k, Number(s.customSettings[String(k)]));
    }
  }
  return Buffer.from(out);
}
export function getUnpackedSettings(buf, options) {
  if (!Buffer.isBuffer(buf) && !(buf instanceof ArrayBuffer) && (!ArrayBuffer.isView(buf) || buf instanceof DataView)) {
    throw __code("ERR_INVALID_ARG_TYPE", "buf", ["Buffer", "TypedArray"], buf);
  }
  const validate = options?.validate === true;
  let u8;
  if (Buffer.isBuffer(buf)) u8 = buf;
  else if (buf instanceof ArrayBuffer) u8 = new Uint8Array(buf);
  else if (buf instanceof Uint8Array) u8 = buf;
  else u8 = Buffer.from(buf); // Uint16Array 等：Buffer.from 元素截断语义（node 同）
  if (u8.length % 6 !== 0) throw __code("ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH");
  const out = {};
  for (let i = 0; i < u8.length; i += 6) {
    const id = (u8[i] << 8) | u8[i + 1];
    const val = ((u8[i + 2] << 24) | (u8[i + 3] << 16) | (u8[i + 4] << 8) | u8[i + 5]) >>> 0;
    switch (id) {
      case 0x1: if (validate) __validateSetting("headerTableSize", val); out.headerTableSize = val; break;
      case 0x2: out.enablePush = val !== 0; break;
      case 0x3: if (validate) __validateSetting("maxConcurrentStreams", val); out.maxConcurrentStreams = val; break;
      case 0x4: if (validate) __validateSetting("initialWindowSize", val); out.initialWindowSize = val; break;
      case 0x5: __validateSetting("maxFrameSize", val); out.maxFrameSize = val; break;
      case 0x6:
        if (validate) __validateSetting("maxHeaderListSize", val);
        out.maxHeaderListSize = val;
        out.maxHeaderSize = val;
        break;
      case 0x8: out.enableConnectProtocol = val !== 0; break;
      default: if (val > 0) out.customSettings = { ...(out.customSettings ?? {}), [String(id)]: val };
    }
  }
  return out;
}
function __validateSetting(name, val) {
  const spec = __SETTING_RANGES[name];
  if (spec !== undefined && spec !== "boolean" && (val < spec[0] || val > spec[1])) {
    throw __settingErr(name, val, false);
  }
}
export const sensitiveHeaders = Symbol("sensitiveHeaders");

