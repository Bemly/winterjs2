  // 事件循环派发钩子（Rust dispatch 调用；kind/data 均为字符串）
  __ev(kind, payload) {
    switch (kind) {
      case "connect": {
        try {
          const o = JSON.parse(payload || "{}");
          // serde SocketAddr → "ip:port"（IPv6 为 "[ip]:port"）
          // adopt-TCP 不回填（真机 fd 复用：local 恒为 bound 值；OS 重分漂移时以预置为准）。
          if (typeof o.local === "string" && this.__adoptPort === undefined) {
            const m = o.local.match(/^\[?([^\]]+?)\]?:(\d+)$/);
            if (m) { this.localAddress = m[1]; this.localPort = Number(m[2]); }
          }
          if (this.localAddress !== undefined && this.localAddress !== null)
            this.localFamily = String(this.localAddress).includes(":") ? "IPv6" : "IPv4";
        } catch {}
        // 远端面在此发布（UDS 恒 undefined；TCP 取 __realConnect 存的目标）。
        this.remoteAddress = this.__targetHost ?? undefined;
        this.remotePort = this.__targetPort ?? undefined;
        this.remoteFamily = this.remoteAddress === undefined ? undefined
          : (String(this.remoteAddress).includes(":") ? "IPv6" : "IPv4");
        // node 'lookup' 事件（dns-error/dns-lookup 套件）：系统 DNS 成功即发
        //（err null + 解析 ip/family/host；IP 字面直连不发，真机实测）。
        try {
          const __th = this.__targetHost;
          const __op = JSON.parse(payload || "{}").peer;
          if (typeof __op === "string" && typeof __th === "string" && !isIP(__th)) {
            const __m = __op.match(/^\[?([^\]]+?)\]?:(\d+)$/);
            if (__m) this.emit("lookup", null, __m[1], __m[1].includes(":") ? 6 : 4, __th);
          }
        } catch { /* 坏载荷即无事件 */ }
        this.__connected = true;
        this.readable = true; this.writable = true;
        // perf_hooks 'net' connect 条目（仅 TCP 成功；detail 主机端口；桥缺席即跳过）。
        if (this.__perfStart !== undefined && this.__udsTarget == null) {
          const __ps = this.__perfStart; this.__perfStart = undefined;
          try {
            const __now = (typeof performance !== "undefined" && typeof performance.now === "function")
              ? performance.now() : Date.now();
            const __rec = globalThis.__wjs2_perfRecord;
            if (typeof __rec === "function" && this.__targetHost != null) {
              __rec({ name: "connect", entryType: "net", startTime: __ps, duration: Math.max(0, __now - __ps),
                detail: { host: this.__targetHost, port: this.__targetPort } });
            }
          } catch { /* 计时失败不阻连接 */ }
        }
        const pend = this.__pendW; this.__pendW = [];
        this.__pendBytes = 0;
        for (const [u8, cb2] of pend) {
          this.__nativeWrite(u8);
          // bytesWritten 已在 write 时同步计入 base，此处只补写队列记账。
          this.__sockQAdd(u8.length);
          if (cb2) queueMicrotask(cb2);
        }
        // 递延 finish：end() 早调即此刻补发（buffersize 套件；单发守卫）。
        if (this.__finishPending && !this.destroyed && !this.__finishEmitted) {
          this.__finishPending = false;
          this.__finishEmitted = true;
          queueMicrotask(() => this.emit("finish"));
        }
        if (this.__endAfterFlush) {
          this.__endAfterFlush = false;
          if (this.__id) __wjs2_net_end(this.__id);
        }
        // HE 成功：拆回落钩（此后 close 走正常路径）。
        this.__heOnErr = null; this.__heSeq = null;
        this.emit("connect");
        // node 口径：connect 后同步发 'ready'（onconnection 原文；已接受端不走
        // 此分支故不发）。旧"暂不发射"记档（§4.126 并行 -10 强相关）作废：
        // 欠账清零要求语义到位；§4.126 禁并行压力负载后常规验证无复现。
        this.emit("ready");
        break;
      }
      case "data": {
        const u8 = __b64dec(payload);
        this.__ingestData(u8);
        break;
      }
      case "wrote": {
        // 写落盘回执（reuse-drained：Rust write_all 完成即发；递减记账，
        // 归零且有背压待发即 'drain'）。
        this.__sockQDrain(Number(payload) || 0);
        break;
      }
      case "end": {
        this.readable = false;
        this.__peerFin = true;
        // 暂存先行（FIN 前到的字节先于 end 交付；无监听即留待迟挂冲刷）。
        try { this.__flushData(); } catch { /* 监听抛错不阻收尾 */ }
        // 池化空闲 socket 见 FIN 即销毁（半关不可复用；否则写端永活、条目永泄，
        // 10b https 保活案；Node 同样把 end 掉的 socket 踢出池）。
        if (this.__inPool) {
          this.destroy();
          break;
        }
        // setEncoding 残余字节 flush（分包切断的多字节尾在 end 前补齐）
        if (this.__dec) {
          const rest = this.__dec.end();
          if (rest) this.emit("data", rest);
        }
        this.emit("end");
        // （自动回 FIN 由构造期内部门执行，用户监听之后触发；此处不重复。）
        // G11 半开案：allowHalfOpen 持有半开即不再续命（真机同款——读停转后
        // 空闲句柄不 ref 循环；k9/k12 实证：ref() 也留不住，真机照常退出）。
        // 递延一轮：end 监听内同步 destroy/auto-end 的走正常收尾通道，此处只收
        // 无动作的稳定半开；写侧仍可用，Close 派发照常 purge 结算。
        // raw-close 补（2026-10-21，http2 lane）：end 监听内已同步 end()
        // （__ended 真，shutdown 在途、close 待定）即非闲置——照 node 口径须
        // 续命至 close，不摘（摘即 loop 先排空、服务端 FIN 到不了，cleanup 件）。
        if (this.allowHalfOpen && !this.destroyed && this.__id) {
          const __s = this;
          queueMicrotask(() => {
            if (__s.allowHalfOpen && !__s.destroyed && __s.__id && __s.__peerFin && !__s.__ended) {
              try { __wjs2_net_halfhold(__s.__id); } catch { /* entry gone 即无事 */ }
            }
          });
        }
        break;
      }
      case "error": {
        const o = JSON.parse(payload);
        const se = __netErr(o.code, o.msg);
        this.__hadError = true;
        // 已销毁 socket 的迟到 teardown 噪声不 chạm 用户监听（真机口径：destroy 后
        // 底层 RST/EOF 竞速错不再派发；write-after-close 套件双块并发下必现 flaky）。
        if (this.destroyed) break;
        // node connect 系标配：syscall + errno（uv 负值；ENOENT=-2/EACCES=-13/ECONNREFUSED=-61
        // /ENOTSOCK=-38/EADDRNOTAVAIL=-49；未知 -4094；ENOTFOUND 系 getaddrinfo -3008）。
        se.syscall = "connect";
        se.errno = { ENOENT: -2, EACCES: -13, ECONNREFUSED: -61, ENOTSOCK: -38, EADDRNOTAVAIL: -49, EINVAL: -22, EADDRINUSE: -48 }[o.code] ?? -4094;
        if (o.code === "ENOTFOUND") {
          se.syscall = "getaddrinfo";
          se.errno = -3008;
        }
        // node connect 错误消息形："connect CODE <target>"（target=host:port 或 path）。
        // native msg 已是 "CODE: <os>"，此处按目标重塑（expectsError 逐字断言面）；
        // getaddrinfo 形保留原文（dns-error 套件 node 口径）。
        // 服务端 accepted 面读错形："read CODE"（reset 系列套件逐字）。
        if (typeof o.msg === "string" && !o.msg.startsWith("connect ") && !o.msg.startsWith("IP(") &&
            !o.msg.startsWith("getaddrinfo ") && !o.msg.startsWith("read ")) {
          if (o.code === "ECONNRESET" && this.server !== undefined && this.server !== null) {
            se.message = `read ${o.code}`;
            se.syscall = "read";
          } else {
            const rh = this.__targetHost ?? this.remoteAddress;
            const rp = this.__targetPort ?? this.remotePort;
            const tgt = this.__udsTarget ?? ((rh !== undefined && rh !== null && rp !== undefined && rp !== null) ? `${rh}:${rp}` : null);
            if (tgt) se.message = `connect ${o.code} ${tgt}`;
          }
        }
        // HE 串行回落：中间地址的连接失败被钩吞（不落用户监听），close 后重试。
        if (this.__heOnErr) { this.__heLast = se; break; }
        // node 'lookup' 失败面（dns-error 套件）：DNS 错先发 lookup(err) 再发 error；
        // 仅连接期（未连通）+ DNS 码 + 非 IP 字面。
        if (!this.__connected && (o.code === "ENOTFOUND" || String(o.code).startsWith("EAI_"))) {
          const __lh = this.__targetHost;
          if (typeof __lh === "string" && !isIP(__lh)) {
            try { this.emit("lookup", se, undefined, undefined, __lh); } catch { /* 监听抛错不阻 */ }
          }
        }
        // node：socket 错误经 destroy(err) 递送——'error' 监听里 destroyed 已为 true
        // （修前 false；tls/net 回环套件 `socket.destroyed` 断言现形）。
        this.destroyed = true; this.writable = false; this.readable = false;
        this.emit("error", se);
        break;
      }
      case "close":
        // HE：失败尝试的 close → 推进下一地址（或末位失败收口）。
        if (this.__heOnErr) { this.__heAdvance(); break; }
        this.destroyed = true; this._handle = null; this.__dataBuf = [];
        this.__closeEmitted = true;
        this.emit("close", this.__hadError === true); break;
    }
  }
  // node 口径：pending = 尚无可用句柄——连接中 true、连接完成 false、
  // close 后**仍为 true**（test-net-connect-buffer 'close' 处理器点名）。
  // node 口径：net.Socket 亦有 writableEnded（remove-header 套件点名
  // response.socket.writableEnded；end() 后 true）。
  // node 口径 writableLength（socket 写队列积压 + 连接前缓冲；reuse-drained
  // 套件点名 req.socket.writableLength === 0 / > 0）。
  get writableLength() { return (this.__sockQ ?? 0) + (this.__pendBytes ?? 0); }
  get writableEnded() { return this.__ended === true; }
  get pending() { return !this.__connected || this.destroyed; }
  get connecting() { return !this.__connected && !this.destroyed && this.__id > 0; }
  // socket-connecting 套件： legacy `_connecting` 与 connecting 同值。
  get _connecting() { return this.connecting; }
  get readyState() {
    if (this.destroyed) return "closed";
    // 真机：new Socket() 未连接即 "open"（构造 readable/writable 初始真；connect 前即 open）。
    // 连接中（__id 已发）才 "opening"。
    if (!this.__connected && this.__id) return "opening";
    if (this.readable && this.writable) return "open";
    return this.readable ? "readOnly" : "writeOnly";
  }
  // node Socket async iterable（for await over data；end/close 终结、error 拒绝）
  [Symbol.asyncIterator]() {
    const st = { q: [], wake: null, done: false, err: null };
    const push = (fn, v) => { st[fn] && 0; };
    const onData = (c) => { st.q.push({ v: c }); if (st.wake) { st.wake(); st.wake = null; } };
    const onDone = () => { st.done = true; if (st.wake) { st.wake(); st.wake = null; } };
    const onErr = (e) => { st.err = e; st.done = true; if (st.wake) { st.wake(); st.wake = null; } };
    this.on("data", onData);
    this.on("end", onDone);
    this.on("close", onDone);
    this.once("error", onErr);
    const cleanup = () => { this.off("data", onData); this.off("end", onDone); this.off("close", onDone); this.off("error", onErr); };
    return {
      next: () => new Promise((resolve, reject) => {
        const step = () => {
          if (st.q.length) resolve({ value: st.q.shift().v, done: false });
          else if (st.err) { const e = st.err; st.err = null; cleanup(); reject(e); }
          else if (st.done) { cleanup(); resolve({ done: true }); }
          else st.wake = step;
        };
        step();
      }),
      return: () => { cleanup(); this.destroy(); return Promise.resolve({ done: true }); },
      throw: (e) => { cleanup(); this.destroy(); return Promise.reject(e); },
    };
  }
  // node 口径（lib/net.js writeGeneric + stream Writable.write）：
  // destroyed/!writable → 有 cb 走 cb(err)+error 事件（返回 false），无 cb 才同步抛；
  // err 形状：write after end / connect 未完成 → ERR_STREAM_WRITE_AFTER_END（writableLength 0），
  // 其余 destroyed → ERR_STREAM_DESTROYED。§4.119 同源：包装/状态检查不吞场景口径。
  __writeErr(cb2) {
    // node 口径：已销毁恒 DESTROYED（即使 __ended 真——echo 套件 close 后写）；
    // 未销毁 + 已 end 即 WRITE_AFTER_END。
    const code = this.destroyed ? "ERR_STREAM_DESTROYED" : "ERR_STREAM_WRITE_AFTER_END";
    const e = __netErr(code, this.destroyed ? "Cannot call write after a stream was destroyed" : "write after end");
    if (typeof cb2 === "function") { queueMicrotask(() => { try { cb2.call(this, e); } catch {} }); return false; }
    throw e;
  }
  write(data, enc, cb) {
    const cb2 = typeof enc === "function" ? enc : cb;
    // 真机逐字（writable.js _write）：仅 null → ERR_STREAM_NULL_VALUES（undefined 落
    // ARG_TYPE 'Received undefined'）；chunk 类型校验先于 after-end/destroyed 状态检查。
    if (data === null) {
      const e = new TypeError("May not write null values to stream");
      e.code = "ERR_STREAM_NULL_VALUES";
      if (typeof cb2 === "function") { queueMicrotask(() => { try { cb2.call(this, e); } catch {} }); return false; }
      throw e;
    }
    const u8 = __chunkU8(data, typeof enc === "string" ? enc : undefined);
    // node 口径：已销毁即 DESTROYED（劣后 chunk 校验、优先 EPIPE——echo 套件
    // close 后写同步抛，真机同）。
    if (this.destroyed) return this.__writeErr(cb2);
    // net 口径（write-after-end-nt 套件真机形）：本地已 end 且对端已 FIN 后再写
    // → Error EPIPE 'This socket has been ended by the other party'——cb 与 error
    // 事件都下一 tick。自动收尾（peerFin + 非半开，自门递延落 __ended）同形
    // （write-after-fin-error 套件；'end' 处理器内写仍放行，真机 writable 真）。
    // 仅本地 end（G7 write-after-end 形 STREAM_WRITE_AFTER_END）与仅对端 FIN
    //（writable 套件 'end' 后写成功）不走此路。
    if (this.__peerFin === true && this.__ended === true && this.allowHalfOpen !== true) {
      const e = new Error("This socket has been ended by the other party");
      e.code = "EPIPE"; e.errno = 32; e.syscall = "write";
      if (typeof cb2 === "function") queueMicrotask(() => { try { cb2.call(this, e); } catch {} });
      queueMicrotask(() => this.emit("error", e));
      return false;
    }
    // 显式 end 后写（未销毁、无 EPIPE 条件）→ WRITE_AFTER_END（G7 形）。
    if (!this.writable) return this.__writeErr(cb2);
    // node 口径（write-after-close 套件双形，真机 26 实测均为异步 error 事件非同步抛）：
    // 已连接但 _handle 被置空后写 → ERR_SOCKET_CLOSED('Socket is closed')；
    // _handle.close() 后（柄关而对象在）写 → Error('write EBADF'，win 系 EPIPE)。
    if (this.__connected && this._handle === null) {
      const e2 = new Error("Socket is closed"); e2.code = "ERR_SOCKET_CLOSED";
      if (typeof cb2 === "function") queueMicrotask(() => { try { cb2.call(this, e2); } catch {} });
      queueMicrotask(() => this.emit("error", e2));
      return false;
    }
    if (this.__handleClosed === true) {
      const e = new Error(`write ${typeof process !== "undefined" && process.platform === "win32" ? "EPIPE" : "EBADF"}`);
      if (typeof cb2 === "function") queueMicrotask(() => { try { cb2.call(this, e); } catch {} });
      queueMicrotask(() => this.emit("error", e));
      return false;
    }
    this.__bwBase += u8.length;
    this.__bwSub(u8.length);
    if (!this.__connected) {
      // node 口径：连接完成前 write 缓冲（connect 完成时按序冲刷）
      this.__pendW.push([u8, cb2]);
      this.__pendBytes += u8.length;
      return u8.length + this.__pendBytes - u8.length <= this._writableState.highWaterMark;
    }
    this.__nativeWrite(u8);
    // 记账：底层同步写队列，无 flush 语义，回调即刻（排空/drain 见 __sockQAdd）。
    const __wret = this.__sockQAdd(u8.length);
    if (cb2) queueMicrotask(cb2);
    return __wret;
  }
