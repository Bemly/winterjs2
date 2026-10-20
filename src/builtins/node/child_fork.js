// fork silent 管道面（fork-ref 套件：silent 子输出须进父端管道）。
// 子端 shim 包装 stdout/stderr.write 逐块投递 `{__wjs2_fork_io: [fd, u8]}`；
// 父端按位喂缓冲流（`__push`/`__close`）。非 silent fork（流恒 null）信封丢弃。
// 本地直写保留（线程底座共享 stdio 偏差记档；silent 套件只断言收到不计数）。
function __forkPipeStream() {
  const listeners = {};
  let buf = [];
  let flowing = false;
  let enc = null;
  let closed = false;
  const emit = (ev, ...args) => {
    for (const l of [...(listeners[ev] || [])]) {
      try { l(...args); } catch {}
    }
  };
  const flush = () => {
    if (!flowing) return;
    while (buf.length > 0) {
      let c = buf.shift();
      if (enc !== null) c = c.toString(enc);
      emit("data", c);
    }
    if (closed) {
      emit("end");
      queueMicrotask(() => emit("close"));
    }
  };
  const api = {
    __push(data) {
      if (closed) return;
      buf.push(Buffer.isBuffer(data) ? data : Buffer.from(data));
      flush();
      return api;
    },
    __close() {
      if (closed) return api;
      closed = true;
      flush();
      return api;
    },
    on(ev, cb) {
      (listeners[ev] ||= []).push(cb);
      if (ev === "data") { flowing = true; flush(); }
      else if (ev === "readable" || ev === "end" || ev === "close") { flush(); }
      return api;
    },
    once(ev, cb) {
      const w = (...a) => { api.off(ev, w); cb(...a); };
      w.__wjs2_orig = cb;
      return api.on(ev, w);
    },
    off(ev, cb) {
      const l = listeners[ev];
      if (l) {
        let i = l.findIndex((f) => f === cb || f.__wjs2_orig === cb);
        while (i >= 0) { l.splice(i, 1); i = l.findIndex((f) => f === cb || f.__wjs2_orig === cb); }
      }
      return api;
    },
    removeListener(ev, cb) { return api.off(ev, cb); },
    removeAllListeners(ev) {
      if (ev !== undefined) delete listeners[ev];
      else for (const k of Object.keys(listeners)) delete listeners[k];
      return api;
    },
    pipe(dest, opts) {
      api.on("data", (d) => { try { dest.write(d); } catch {} });
      api.on("end", () => {
        if (!opts || opts.end !== false) {
          try { if (typeof dest.end === "function") dest.end(); } catch {}
        }
      });
      return dest;
    },
    unpipe() { return api; },
    setEncoding(e) { enc = e === null ? null : String(e); return api; },
    pause() { flowing = false; return api; },
    resume() { flowing = true; flush(); return api; },
    read() { return null; },
    destroy() { closed = true; return api; },
    get readable() { return !closed; },
    get destroyed() { return false; },
  };
  return api;
}
// fork 子输出信封判定（父端 worker message 分流用）。
function __isForkIoEnvelope(m) {
  return m !== null && typeof m === "object" && !Array.isArray(m) &&
    Array.isArray(m.__wjs2_fork_io);
}
