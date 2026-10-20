//! Web Streams（Readable/Writable/Transform/排队/压缩流）（prelude 分域；拼接顺序见 mod.rs）。
pub const STREAMS_JS: &str = r#"
// ---- Phase 3c-2: streams（纯 prelude 内存实现；默认 reader + BYOB）----
// BYOB 口径：`new ReadableStream({ type: "bytes", ... })` + `getReader({ mode: "byob" })`；
// `read(view)` 按 view 类型回同类前缀视图；`byobRequest.respond/respondWithNewView` 完整；
// 简化（文档记录）：done 时 value 为 undefined（非空视图）；respond 非元素对齐截断丢余量；
// 无 autoAllocateChunkSize；default reader 照常读字节流（Uint8Array 块）。
const __wjs2_rsState = new WeakMap();
function __wjs2_rsViewPrefix(r, n) {
  // 取 view 前 n 字节（元素对齐由调用方保证；DataView 按字节）。
  if (r.viewCtor === DataView) return new DataView(r.view.buffer, r.view.byteOffset, n);
  return new r.viewCtor(r.view.buffer, r.view.byteOffset, n / r.viewElem);
}
function __wjs2_rsByobFill(st) {
  // 用 byteQ 填充排队的 BYOB 读；closed/出错同样结算
  while (st.byobReads.length) {
    const r = st.byobReads[0];
    try { new Uint8Array(r.view.buffer, 0, 0); }
    catch { st.byobReads.shift(); if (st.byobReqRead === r) st.byobReqRead = null; r.reject(new TypeError("BYOB view is detached")); continue; }
    if (st.error !== undefined) { st.byobReads.shift(); if (st.byobReqRead === r) st.byobReqRead = null; r.reject(st.error); continue; }
    if (st.byteLen === 0) {
      // 闭流且排空：待读按 done 结算；若其为活跃 pull 请求，请求对象快照
      // 留用（close 后 respondWithNewView 仍须可达并走视图校验，
      // bad-buffers 套件点名；结算点在 respond 或 pull 收尾后的 pump）。
      if (st.closed) {
        st.byobReads.shift();
        if (st.byobReqRead === r) {
          st.byobReqSettled = { req: st.byobReq, r };
          st.byobReqRead = null;
        }
        r.resolve({ value: undefined, done: true }); continue;
      }
      break;
    }
    const n = Math.min(r.view.byteLength, st.byteLen);
    const take = n - (n % r.viewElem);
    if (take === 0) break;
    let off = take;
    for (const q of st.byteQ) {
      if (off === 0) break;
      const c = Math.min(q.length - q._off, off);
      new Uint8Array(r.view.buffer, r.view.byteOffset + (take - off), c).set(q.subarray(q._off, q._off + c));
      q._off += c; off -= c;
    }
    while (st.byteQ.length && st.byteQ[0]._off >= st.byteQ[0].length) st.byteQ.shift();
    st.byteLen -= take;
    st.byobReads.shift();
    r.resolve({ value: __wjs2_rsViewPrefix(r, take), done: false });
  }
}
function __wjs2_rsByobReq(st) {
  const r = st.byobReads[0];
  if (!r) return null;
  st.byobReqRead = r;
  st.byobReqSettled = null;
  return {
    get view() { return r.view; },
    respond(n) {
      n = Number(n);
      if (!Number.isInteger(n) || n < 0 || n > r.view.byteLength) throw new RangeError("respond: bad byte count");
      if ((st.byobReads[0] !== r || st.byobReq === null) && st.byobReqSettled?.r !== r) {
        throw new TypeError("respond: request is not active");
      }
      st.byobReqSettled = null;
      if (st.byobReads[0] !== r) return;
      st.byobReads.shift();
      st.byobReq = null;
      st.byobReqRead = null;
      // 非元素对齐截断（余量丢弃，见头注）
      const take = n - (n % r.viewElem);
      r.resolve({ value: __wjs2_rsViewPrefix(r, take), done: false });
      __wjs2_rsPump(st);
    },
    respondWithNewView(v) {
      if (!ArrayBuffer.isView(v)) throw new TypeError("respondWithNewView needs a view");
      // 已结算（close 顺带 done）的活跃请求仍可用（见 ByobFill 快照）。
      if ((st.byobReads[0] !== r || st.byobReq === null) && st.byobReqSettled?.r !== r) {
        throw new TypeError("respond: request is not active");
      }
      // node 口径（bad-buffers 套件点名码）：空视图 RangeError ARG_VALUE；
      // detached 视图 TypeError INVALID_STATE（文案 best-effort，码断言）。
      if (v.byteLength === 0) {
        const e = new RangeError(`The argument 'view' is invalid. Received ${v.constructor?.name ?? "Object"}(${v.length ?? v.byteLength})`);
        e.code = "ERR_INVALID_ARG_VALUE";
        throw e;
      }
      try { new Uint8Array(v.buffer, 0, 0); }
      catch {
        const e = new TypeError("Invalid state: View or Viewed ArrayBuffer is zero-length or detached");
        e.code = "ERR_INVALID_STATE";
        throw e;
      }
      r.view = v; r.viewCtor = v.constructor; r.viewElem = v.BYTES_PER_ELEMENT ?? 1;
    },
  };
}
function __wjs2_rsByteToQueue(st) {
  // default reader 读字节流：整块搬运（有 _off 余量的半块留给 BYOB，不拆）
  while (st.byteQ.length && st.byteQ[0]._off === 0) {
    const q = st.byteQ.shift();
    st.byteLen -= q.length;
    st.queue.push(q);
  }
}
function __wjs2_rsPull(st) {
  // 早退仍需 pump：关闭/出错后不再拉数据，但排队的取值/close 等待仍要结算
  // （read 消费掉末块后只调 pull，不 pump 即 closeWaiter 永挂）。
  // start 未决同样早退（byob 套件：start 内 await open，pull 须等 file 就绪，
  // 真机 pull 门控 start 结算；复位由构造器的 settle 回调做）。
  if (!st.reader || st.closed || st.error !== undefined || st.pulling || st.starting) { __wjs2_rsPump(st); return; }
  // pull 触发面（防微任务空转饿死事件循环，见 §4.27 追补）：
  // 只在新需求到达（read 推入等待）或有进展且需求还在（pump 尾）时调；
  // 无 pull 方法的源 + 挂起的读，eager 重拉即无限微任务链。
  st.pulling = true;
  st.pullProgress = false;
  // BYOB 读排队时带 byobRequest 进 pull（source 可直接写 view + respond）
  if (st.isBytes && st.byobReads.length && !st.byobReq) st.byobReq = __wjs2_rsByobReq(st);
  try {
    const r = st.source.pull ? st.source.pull(st.controller) : undefined;
    Promise.resolve(r).then(() => {
      st.pulling = false; st.byobReq = null;
      if (st.byobReqRead && st.byobReads[0] !== st.byobReqRead) st.byobReqRead = null;
      __wjs2_rsPump(st);
    }, (e) => {
      st.pulling = false; st.byobReq = null; st.byobReqRead = null; __wjs2_rsError(st, e);
    });
  } catch (e) { st.pulling = false; st.byobReq = null; st.byobReqRead = null; __wjs2_rsError(st, e); }
}
function __wjs2_rsPump(st) {
  __wjs2_rsByobFill(st);
  // default reader 读字节流：仅当有读等待才整块搬运；
  // close 等待另队（closeWaiters），此处只看取值等待。
  if (st.isBytes && st.pending.length) __wjs2_rsByteToQueue(st);
  while (st.pending.length && (st.queue.length || st.closed || st.error !== undefined)) {
    const { resolve, reject } = st.pending.shift();
    if (st.error !== undefined) { reject(st.error); continue; }
    if (st.queue.length) {
      const v = st.queue.shift();
      resolve({ value: v, done: false });
    } else { resolve({ value: undefined, done: true }); }
  }
  // close 等待：关闭且排空即 resolve，出错即 reject；绝不消费队列 chunk
  // （与取值等待分队：混队 FIFO 会让 .closed/eos 吞掉一个数据块；
  // kIsClosedPromise 同样排空感知——有队关闭无人读则永不结算，node 实测）。
  if (st.error !== undefined) {
    for (const w of st.closeWaiters.splice(0)) w.reject(st.error);
    for (const w of st.drainWaiters.splice(0)) w.reject(st.error);
  } else if (st.closed && !st.queue.length && !(st.isBytes && st.byteLen)) {
    for (const w of st.closeWaiters.splice(0)) w.resolve(undefined);
    for (const w of st.drainWaiters.splice(0)) w.resolve(undefined);
  }
  // pump 尾再拉：仅当需求还在且本轮有进展（enqueue/close/error 置 pullProgress）；
  // 干 pull（无进展）不再重拉——新需求到达时 read() 会拉。
  if (!st.closed && st.error === undefined && !st.pulling) {
    const demand = st.byobReads.length > 0 || st.pending.length > 0;
    if (demand && st.pullProgress) { st.pullProgress = false; __wjs2_rsPull(st); }
  }
}
function __wjs2_rsError(st, e) {
  // node 口径：close 后再 error 仍转 errored（abort-controller 套件点名），
  // 仅重复 error 才 no-op；出错清队列（读端后续读一律 reject）。
  if (st.error !== undefined) return;
  st.closed = true;
  st.error = e;
  st.queue.length = 0;
  st.byteQ.length = 0; st.byteLen = 0;
  __wjs2_rsByobFill(st);
  __wjs2_rsPump(st);
}
function __wjs2_rsController(stream, st) {
  if (st.isBytes) {
    return {
      get desiredSize() { return st.hwm - st.byteLen; },
      get byobRequest() { return st.byobReq ?? st.byobReqSettled?.req ?? null; },
      enqueue(chunk) {
        if (st.closed || st.error !== undefined) throw new TypeError("stream is not readable");
        if (!ArrayBuffer.isView(chunk)) throw new TypeError("byte stream chunk must be a view");
        const v = new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength);
        v._off = 0;
        st.byteQ.push(v);
        st.byteLen += v.length;
        st.pullProgress = true;
        __wjs2_rsPump(st);
      },
      close() {
        if (st.closed || st.error !== undefined) throw new TypeError("stream is not readable");
        st.closed = true;
        st.pullProgress = true;
        __wjs2_rsPump(st);
      },
      error(e) { __wjs2_rsError(st, e); },
    };
  }
  return {
    get desiredSize() { return st.hwm - st.queue.length; },
    enqueue(chunk) {
      if (st.closed || st.error !== undefined) throw new TypeError("stream is not readable");
      if (chunk === undefined) throw new TypeError("chunk must not be undefined");
      st.queue.push(chunk);
      st.pullProgress = true;
      __wjs2_rsPump(st);
    },
    close() {
      if (st.closed || st.error !== undefined) throw new TypeError("stream is not readable");
      st.closed = true;
      st.pullProgress = true;
      __wjs2_rsPump(st);
    },
    error(e) { __wjs2_rsError(st, e); },
  };
}
globalThis.ReadableStream = class ReadableStream {
  constructor(underlyingSource = {}, strategy) {
    const hwm = strategy && strategy.highWaterMark !== undefined ? Number(strategy.highWaterMark) : 1;
    const utype = underlyingSource ? underlyingSource.type : undefined;
    if (utype !== undefined && utype !== "bytes") throw new TypeError("ReadableStream type must be 'bytes'");
    const st = {
      queue: [], pending: [], closeWaiters: [], drainWaiters: [], closed: false, error: undefined,
      reader: null, pulling: false, pullProgress: false, hwm: Number.isNaN(hwm) ? 1 : hwm,
      source: underlyingSource, controller: null,
      isBytes: utype === "bytes", byteQ: [], byteLen: 0, byobReads: [], byobReq: null,
    };
    st.controller = __wjs2_rsController(this, st);
    __wjs2_rsState.set(this, st);
    // start 结算前不 pull（见 __wjs2_rsPull 门）；结算后若有待需求即补拉一轮。
    // 无 start 方法即不同步延迟（既有套件时序不变）。
    if (underlyingSource.start) {
      st.starting = true;
      try {
        const r = underlyingSource.start(st.controller);
        Promise.resolve(r).then(() => {
          st.starting = false;
          st.pullProgress = true;
          __wjs2_rsPump(st);
        }, (e) => { st.starting = false; __wjs2_rsError(st, e); });
      } catch (e) { st.starting = false; __wjs2_rsError(st, e); }
    } else {
      st.starting = false;
    }
  }
  get locked() { return !!__wjs2_rsState.get(this).reader; }
  cancel(reason) {
    const st = __wjs2_rsState.get(this);
    if (st.reader) { const e = new TypeError("stream is locked"); e.code = "ERR_INVALID_STATE"; throw e; }
    st.queue.length = 0; st.closed = true;
    st.byteQ.length = 0; st.byteLen = 0;
    const c = st.source.cancel ? st.source.cancel(reason) : undefined;
    __wjs2_rsPump(st);
    return Promise.resolve(c).then(() => undefined);
  }
  getReader(options) {
    const st = __wjs2_rsState.get(this);
    if (st.reader) { const e = new TypeError("stream is locked"); e.code = "ERR_INVALID_STATE"; throw e; }
    const mode = options ? options.mode : undefined;
    if (mode !== undefined && mode !== "byob") throw new TypeError(`Unknown reader mode '${mode}'`);
    const stream = this;
    if (mode === "byob") {
      if (!st.isBytes) throw new TypeError("getReader({ mode: 'byob' }) needs a byte stream");
      const reader = {
        get closed() {
          return new Promise((resolve, reject) => {
            if (st.error !== undefined) reject(st.error);
            else if (st.closed && !st.byteLen) resolve(undefined);
            else st.drainWaiters.push({ resolve: () => resolve(undefined), reject });
          });
        },
        read(view) {
          return new Promise((resolve, reject) => {
            if (!ArrayBuffer.isView(view)) { reject(new TypeError("BYOB read needs a view")); return; }
            // node 口径（bad-buffers 套件点名码与文案）：空视图/detached 背缓冲
            // 同一 TypeError ERR_INVALID_STATE。
            let viewBad = view.byteLength === 0;
            if (!viewBad) {
              try { new Uint8Array(view.buffer, 0, 0); }
              catch { viewBad = true; }
            }
            if (viewBad) {
              const e = new TypeError("Invalid state: View or Viewed ArrayBuffer is zero-length or detached");
              e.code = "ERR_INVALID_STATE";
              reject(e);
              return;
            }
            if (st.error !== undefined) { reject(st.error); return; }
            st.byobReads.push({
              view, viewCtor: view.constructor, viewElem: view.BYTES_PER_ELEMENT ?? 1,
              resolve, reject,
            });
            // 新需求即置进展旗（in-flight pull 完成后的 pump 尾凭此补拉；
            // 否则异步源下新读到达时 pulling 恒真、旗恒假，永不再拉——
            // byob 文件套件现形；default 读端同款已置）。
            st.pullProgress = true;
            __wjs2_rsByobFill(st);
            __wjs2_rsPull(st);
          });
        },
        releaseLock() { if (st.reader === reader) st.reader = null; },
        cancel(reason) {
          st.byteQ.length = 0; st.byteLen = 0; st.closed = true;
          const c = st.source.cancel ? st.source.cancel(reason) : undefined;
          if (st.reader === reader) st.reader = null;
          __wjs2_rsPump(st);
          return Promise.resolve(c).then(() => undefined);
        },
      };
      st.reader = reader;
      return reader;
    }
    const reader = {
      get closed() {
        return new Promise((resolve, reject) => {
          if (st.error !== undefined) reject(st.error);
          else if (st.closed && !st.queue.length) resolve(undefined);
          else st.drainWaiters.push({ resolve: () => resolve(undefined), reject });
        });
      },
      read() {
        return new Promise((resolve, reject) => {
          if (st.error !== undefined) { reject(st.error); return; }
          if (st.isBytes) __wjs2_rsByteToQueue(st);
          if (st.queue.length) {
            const v = st.queue.shift();
            resolve({ value: v, done: false });
            __wjs2_rsPull(st);
            return;
          }
          if (st.closed) { resolve({ value: undefined, done: true }); return; }
          st.pending.push({ resolve, reject });
          __wjs2_rsPull(st);
        });
      },
      releaseLock() { if (st.reader === reader) st.reader = null; },
      cancel(reason) {
        st.queue.length = 0; st.closed = true;
        st.byteQ.length = 0; st.byteLen = 0;
        const c = st.source.cancel ? st.source.cancel(reason) : undefined;
        if (st.reader === reader) st.reader = null;
        __wjs2_rsPump(st);
        return Promise.resolve(c).then(() => undefined);
      },
    };
    st.reader = reader;
    return reader;
  }
  pipeThrough(t, options) {
    this.pipeTo(t.writable, options);
    return t.readable;
  }
  async pipeTo(dest, options = {}) {
    const preventClose = !!(options && options.preventClose);
    const reader = this.getReader();
    const writer = dest.getWriter();
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        await writer.write(value);
      }
      if (!preventClose) await writer.close();
    } finally {
      reader.releaseLock();
      writer.releaseLock();
    }
  }
  tee() {
    const st = __wjs2_rsState.get(this);
    if (st.reader) { const e = new TypeError("stream is locked"); e.code = "ERR_INVALID_STATE"; throw e; }
    // 简化 tee：顺序读源，两分支各收一份（引用共享；无背压，见文档）。
    const q1 = [], q2 = [];
    const mkBranch = (q) => {
      const b = new ReadableStream({
        pull(c) {
          if (q.length) { c.enqueue(q.shift()); return; }
          if (done) { c.close(); return; }
          if (failed !== undefined) { c.error(failed); return; }
          waiters.push(() => {
            if (q.length) { try { c.enqueue(q.shift()); } catch {} return; }
            if (done) { try { c.close(); } catch {} return; }
            if (failed !== undefined) { try { c.error(failed); } catch {} }
          });
        },
        cancel() {},
      });
      return b;
    };
    let done = false, failed;
    const waiters = [];
    const wake = () => { for (const w of waiters.splice(0)) w(); };
    const r1 = mkBranch(q1), r2 = mkBranch(q2);
    const src = this.getReader();
    st.reader = null;
    // 源出错/收尾即主动结算分支（finished 只观测不读，无此即永挂；
    // 有未投递块的分支留待 pull 消费，不提前关——判 tee 数组非分支内队）。
    const settleBranches = () => {
      const pairs = [[r1, q1], [r2, q2]];
      for (const [b, q] of pairs) {
        const bst = __wjs2_rsState.get(b);
        if (failed !== undefined) { __wjs2_rsError(bst, failed); continue; }
        if (done && !q.length && !bst.queue.length && !bst.closed && bst.error === undefined) {
          bst.closed = true;
        }
      }
      for (const b of [r1, r2]) __wjs2_rsPump(__wjs2_rsState.get(b));
    };
    const loop = () => src.read().then(({ value, done: d }) => {
      if (d) { done = true; wake(); settleBranches(); return; }
      q1.push(value); q2.push(value);
      wake();
      loop();
    }, (e) => { failed = e; wake(); settleBranches(); });
    loop();
    return [r1, r2];
  }
  async *[Symbol.asyncIterator]() {
    const reader = this.getReader();
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) return;
        yield value;
      }
    } finally { reader.releaseLock(); }
  }
};
const __wjs2_wsState = new WeakMap();
globalThis.WritableStream = class WritableStream {
  constructor(underlyingSink = {}, strategy) {
    const hwm = strategy && strategy.highWaterMark !== undefined ? Number(strategy.highWaterMark) : 1;
    const st = {
      queue: [], writing: false, closed: false, errored: false, error: undefined,
      writer: null, hwm: Number.isNaN(hwm) ? 1 : hwm, sink: underlyingSink,
      closeReq: null,
    };
    __wjs2_wsState.set(this, st);
    const stream = this;
    st.controller = { error(e) { __wjs2_wsError(stream, e); } };
    try {
      const r = underlyingSink.start ? underlyingSink.start(st.controller) : undefined;
      Promise.resolve(r).catch((e) => __wjs2_wsError(this, e));
    } catch (e) { __wjs2_wsError(this, e); }
  }
  get locked() { return !!__wjs2_wsState.get(this).writer; }
  abort(reason) {
    const st = __wjs2_wsState.get(this);
    if (st.writer) { const e = new TypeError("stream is locked"); e.code = "ERR_INVALID_STATE"; throw e; }
    const a = st.sink.abort ? st.sink.abort(reason) : undefined;
    __wjs2_wsError(this, reason);
    return Promise.resolve(a).then(() => undefined);
  }
  close() {
    const st = __wjs2_wsState.get(this);
    if (st.writer) { const e = new TypeError("stream is locked"); e.code = "ERR_INVALID_STATE"; throw e; }
    return __wjs2_wsCloseReq(this);
  }
  getWriter() {
    const st = __wjs2_wsState.get(this);
    if (st.writer) { const e = new TypeError("stream is locked"); e.code = "ERR_INVALID_STATE"; throw e; }
    const stream = this;
    const writer = {
      get closed() {
        return new Promise((resolve, reject) => {
          if (st.errored) reject(st.error);
          else if (st.closed) resolve(undefined);
          else st.closeWaiters.push({ resolve, reject });
        });
      },
      get desiredSize() { return st.hwm - st.queue.length; },
      get ready() { return Promise.resolve(); },
      write(chunk) {
        // 空值不拦截：转交 sink 校验（CompressionStream 需带码拒绝，
        // 原生 WS 按 spec 收 undefined；旧守卫无码一律拒是错的）。
        if (st.errored) return Promise.reject(st.error);
        if (st.closed) return Promise.reject(new TypeError("stream is closed"));
        return new Promise((resolve, reject) => {
          st.queue.push({ chunk, resolve, reject });
          __wjs2_wsPump(stream);
        });
      },
      close() { return __wjs2_wsCloseReq(stream); },
      abort(reason) {
        const a = st.sink.abort ? st.sink.abort(reason) : undefined;
        __wjs2_wsError(stream, reason);
        return Promise.resolve(a).then(() => undefined);
      },
      releaseLock() { if (st.writer === writer) st.writer = null; },
    };
    st.closeWaiters = st.closeWaiters || [];
    st.writer = writer;
    return writer;
  }
};
function __wjs2_wsError(stream, e) {
  const st = __wjs2_wsState.get(stream);
  if (st.errored) return;
  st.errored = true;
  st.error = e;
  for (const q of st.queue.splice(0)) q.reject(e);
  if (st.closeReq) { const c = st.closeReq; st.closeReq = null; c.reject(e); }
  for (const w of (st.closeWaiters || []).splice(0)) w.reject(e);
}
function __wjs2_wsCloseReq(stream) {
  const st = __wjs2_wsState.get(stream);
  return new Promise((resolve, reject) => { st.closeReq = { resolve, reject }; __wjs2_wsPump(stream); });
}
function __wjs2_wsPump(stream) {
  const st = __wjs2_wsState.get(stream);
  if (st.writing || st.errored) return;
  const item = st.queue.shift();
  if (!item) {
    if (st.closeReq && !st.writing) {
      const c = st.closeReq; st.closeReq = null;
      const done = () => { st.closed = true; c.resolve(undefined); for (const w of (st.closeWaiters || []).splice(0)) w.resolve(undefined); };
      try {
        Promise.resolve(st.sink.close ? st.sink.close() : undefined).then(done, (e) => { __wjs2_wsError(stream, e); });
      } catch (e) { __wjs2_wsError(stream, e); }
    }
    return;
  }
  st.writing = true;
  try {
    Promise.resolve(st.sink.write ? st.sink.write(item.chunk, st.controller) : undefined).then(
      () => { st.writing = false; item.resolve(undefined); __wjs2_wsPump(stream); },
      (e) => { st.writing = false; item.reject(e); __wjs2_wsError(stream, e); __wjs2_wsPump(stream); },
    );
  } catch (e) { st.writing = false; item.reject(e); __wjs2_wsError(stream, e); }
}
// ---- QueuingStrategy 双类（Web 全局；WHATWG streams。真机 26 口径：highWaterMark
// 是原型 getter 非自有键、size 是可枚举 accessor 且全实例共享同一函数、构造器
// ARG_TYPE 文案 + highWaterMark 缺失 ERR_MISSING_OPTION；size 对 undefined/null
// 抛 TypeError、其余回 chunk.byteLength（原始值/普通对象 → undefined）；10f）----
const __wjs2_qsState = new WeakMap();
function __wjs2_qsArg(init) {
  if (init === null) return "null";
  if (typeof init === "string") return `type string ('${init}')`;
  if (typeof init === "number") return `type number (${init})`;
  if (typeof init === "function") return "type function";
  return `type ${typeof init}`;
}
const __wjs2_qsSizeBL = (chunk) => {
  if (chunk === undefined || chunk === null) throw new TypeError("chunk must not be undefined or null");
  return chunk.byteLength;
};
const __wjs2_qsSizeCount = () => 1;
globalThis.ByteLengthQueuingStrategy = class ByteLengthQueuingStrategy {
  constructor(init) {
    if (init === null || (typeof init !== "object" && typeof init !== "function")) {
      const err = new TypeError(`The "init" argument must be of type object. Received ${__wjs2_qsArg(init)}`);
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    if (init.highWaterMark === undefined) {
      const err = new TypeError("init.highWaterMark is required");
      err.code = "ERR_MISSING_OPTION";
      throw err;
    }
    __wjs2_qsState.set(this, init.highWaterMark);
  }
  get highWaterMark() { return __wjs2_qsState.get(this); }
  get size() { return __wjs2_qsSizeBL; }
};
globalThis.CountQueuingStrategy = class CountQueuingStrategy {
  constructor(init) {
    if (init === null || (typeof init !== "object" && typeof init !== "function")) {
      const err = new TypeError(`The "init" argument must be of type object. Received ${__wjs2_qsArg(init)}`);
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }
    if (init.highWaterMark === undefined) {
      const err = new TypeError("init.highWaterMark is required");
      err.code = "ERR_MISSING_OPTION";
      throw err;
    }
    __wjs2_qsState.set(this, init.highWaterMark);
  }
  get highWaterMark() { return __wjs2_qsState.get(this); }
  get size() { return __wjs2_qsSizeCount; }
};
globalThis.TransformStream = class TransformStream {
  constructor(transformer = {}, writableStrategy, readableStrategy) {
    let rsCtrl;
    const readable = new ReadableStream({
      start(c) { rsCtrl = c; },
    }, readableStrategy);
    const writable = new WritableStream({
      write: (chunk, c) => transformer.transform
        ? transformer.transform(chunk, {
            enqueue: (out) => rsCtrl.enqueue(out),
            get desiredSize() { return rsCtrl.desiredSize; },
            terminate() { rsCtrl.close(); },
          })
        : rsCtrl.enqueue(chunk),
      close: () => {
        if (transformer.flush) {
          return Promise.resolve(transformer.flush({
            enqueue: (out) => rsCtrl.enqueue(out),
            get desiredSize() { return rsCtrl.desiredSize; },
            terminate() { rsCtrl.close(); },
          })).then(() => rsCtrl.close());
        }
        rsCtrl.close();
      },
      abort: (r) => rsCtrl.error(r),
    }, writableStrategy);
    try {
      const r = transformer.start ? transformer.start({
        enqueue: (out) => rsCtrl.enqueue(out),
        get desiredSize() { return rsCtrl.desiredSize; },
        terminate() { rsCtrl.close(); },
      }) : undefined;
      Promise.resolve(r).catch((e) => rsCtrl.error(e));
    } catch (e) { rsCtrl.error(e); }
    this.readable = readable;
    this.writable = writable;
  }
};
// ---- TextEncoderStream / TextDecoderStream（whatwg webstreams-encoding 套件）----
// 薄封装：编码侧 TransformStream + TextEncoder（chunk 字符串化后编码）；
// 解码侧同 + 有状态 TextDecoder（跨片多字节正确；label/options 校验复用
// TextDecoder 码）。真机同款：无 Symbol.toStringTag（node 26.8.1 双侧 undefined）。
const __wjs2_tesState = new WeakMap();
function __wjs2_tesBrand(self) {
  const st = __wjs2_tesState.get(self);
  if (st === undefined) throw new TypeError("Cannot read private member #handle from an object whose class did not declare it");
  return st;
}
// 取值器品牌门 + node 形栈帧（webstreams-encoding 套件 stack 正则点名
// `at get <name>`；引擎原生栈为 SpiderMonkey 形 `get <name>@…`，此处前置
// 一行 node 方言帧，指向真实调用的取值器）。
function __wjs2_tesGet(self, getter, pick) {
  try {
    return pick(__wjs2_tesBrand(self));
  } catch (e) {
    try {
      const cur = typeof e.stack === "string" ? e.stack : String(e);
      e.stack = `    at get ${getter} (winterjs2:encoding)\n${cur}`;
    } catch {}
    throw e;
  }
}
globalThis.TextEncoderStream = class TextEncoderStream {
  constructor() {
    const enc = new TextEncoder();
    const ts = new TransformStream({
      transform(chunk, c) { c.enqueue(enc.encode(String(chunk))); },
    });
    __wjs2_tesState.set(this, { ts });
  }
  get encoding() { return __wjs2_tesGet(this, "encoding", () => "utf-8"); }
  get readable() { return __wjs2_tesGet(this, "readable", (s) => s.ts.readable); }
  get writable() { return __wjs2_tesGet(this, "writable", (s) => s.ts.writable); }
};
globalThis.TextDecoderStream = class TextDecoderStream {
  constructor(label = "utf-8", options = undefined) {
    // 选项形校验（undefined/null 缺省；标量即 ARG_TYPE，encoding 套件点名文案）；
    // label 校验复用 TextDecoder（含 unknown-label 的 NOT_SUPPORTED 码）。
    if (options !== undefined && options !== null &&
        (typeof options !== "object" && typeof options !== "function")) {
      const shown = typeof options === "string" ? `'${options}'` : String(options);
      const e = new TypeError(`The "options" argument must be of type object. Received type ${typeof options} (${shown})`);
      e.code = "ERR_INVALID_ARG_TYPE";
      throw e;
    }
    const dec = new TextDecoder(label, options ?? undefined);
    const ts = new TransformStream({
      transform(chunk, c) {
        const out = dec.decode(chunk, { stream: true });
        if (out) c.enqueue(out);
      },
      flush(c) {
        const out = dec.decode();
        if (out) c.enqueue(out);
      },
    });
    __wjs2_tesState.set(this, { ts, label: dec.encoding, fatal: dec.fatal, ignoreBOM: dec.ignoreBOM });
  }
  get encoding() { return __wjs2_tesGet(this, "encoding", (s) => s.label); }
  get fatal() { return __wjs2_tesGet(this, "fatal", (s) => s.fatal); }
  get ignoreBOM() { return __wjs2_tesGet(this, "ignoreBOM", (s) => s.ignoreBOM); }
  get readable() { return __wjs2_tesGet(this, "readable", (s) => s.ts.readable); }
  get writable() { return __wjs2_tesGet(this, "writable", (s) => s.ts.writable); }
};
// ---- CompressionStream / DecompressionStream（10f 欠账 G9-3：Web 全局 +
// node:stream/web；真机 26.8.2 对拍——不继承 TransformStream（proto 链独立，
// instanceof TransformStream false）、format 枚举校验 TypeError（文案逐字）、
// 解压侧尾垃圾/截断错误落 readable（pipeThrough 场景 Array.fromAsync 可见
// reject），junk = TypeError ERR_TRAILING_JUNK_AFTER_STREAM_END（node:zlib
// 引擎 junk 码同文复用））----
const __wjs2_csState = new WeakMap();
// Web 扩展 `zstd`（ruzstd 底座；编码恒 Fastest，见 node/zlib.rs 头注）：
// 引擎侧 ZstdEnc=10/ZstdDec=11 已就绪，此处只加格式表（finish 档走 flag 2）。
const __CS_KINDS = { gzip: 2, deflate: 0, "deflate-raw": 1, brotli: 8, zstd: 10 };
const __DS_KINDS = { gzip: 6, deflate: 3, "deflate-raw": 4, brotli: 9, zstd: 11 };
function __wjs2_csFinishFlag(kind) { return kind <= 7 ? 4 : 2; }
function __wjs2_makeCSClass(name, kinds, reject) {
  const cls = class {
    constructor(format) {
      if (new.target === undefined) {
        throw new TypeError(`Failed to construct '${name}': Please use the 'new' operator, this DOM object constructor cannot be called as a function.`);
      }
      const fmt = String(format);
      const kind = kinds[fmt];
      if (kind === undefined) {
        // node 口径（webstreams-compression 套件点名码；文案原已逐字）。
        const e = new TypeError(`Failed to construct '${name}': 1st argument '${fmt}' is not a valid enum value of type CompressionFormat.`);
        e.code = "ERR_INVALID_ARG_VALUE";
        throw e;
      }
      // brotli 压缩档位对齐真机默认 11；zlib 族 -1（引擎 clamp 默认）。
      const lv = kind === 8 ? 11 : -1;
      const id = __wjs2_zlib_stream_new(kind, lv, null, -1, reject ? 1 : 0);
      let ctrl = null;
      let closed = false; // readable 已 close/error
      let freed = false;
      let done = false;   // 引擎已 StreamEnd（后续写入即尾垃圾）
      let failErr = null; // 首错（close-after-fail 拒此错，corrupt 套件点名）
      const free = () => { if (!freed) { freed = true; __wjs2_zlib_stream_free(id); } };
      const fail = (err) => { if (!closed) { closed = true; failErr = err; ctrl.error(err); } };
      const feed = (u8, flag) => {
        const r = JSON.parse(__wjs2_zlib_stream_feed(id, u8 ?? null, flag));
        if (r.code !== undefined) {
          // node 口径：数据错一律 TypeError（空文案）+ 引擎码（corrupt 套件点名双侧同错）。
          const err = new TypeError();
          err.code = r.code;
          fail(err);
          return false;
        }
        done = r.d === true;
        const out = __wjs2_zlib_stream_out(id);
        if (!closed && out.length) ctrl.enqueue(out);
        return true;
      };
      const readable = new ReadableStream({
        start(c) { ctrl = c; },
        cancel() { closed = true; free(); },
      });
      const toU8 = (chunk) => {
        // node 口径（bad-chunks 套件矩阵）：string 编码收；ArrayBuffer/普通视图收；
        // SharedArrayBuffer（含其背视图）拒 ERR_INVALID_ARG_TYPE；null 拒
        // ERR_STREAM_NULL_VALUES；其余拒 ERR_INVALID_ARG_TYPE。读写双侧同错
        // （sink 抛 → write 拒绝 + fail 落 readable）。
        const bad = (code) => {
          const e = new TypeError("The provided value is not of type '(ArrayBuffer or ArrayBufferView)'");
          e.code = code;
          throw e;
        };
        if (typeof chunk === 'string') return new TextEncoder().encode(chunk);
        if (chunk instanceof ArrayBuffer) return new Uint8Array(chunk);
        if (typeof SharedArrayBuffer !== 'undefined' && chunk instanceof SharedArrayBuffer) {
          bad('ERR_INVALID_ARG_TYPE');
        }
        if (ArrayBuffer.isView(chunk)) {
          if (typeof SharedArrayBuffer !== 'undefined' && chunk.buffer instanceof SharedArrayBuffer) {
            bad('ERR_INVALID_ARG_TYPE');
          }
          return new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength);
        }
        if (chunk === null) bad('ERR_STREAM_NULL_VALUES');
        bad('ERR_INVALID_ARG_TYPE');
      };
      const writable = new WritableStream({
        write(chunk) {
          if (closed || freed) return;
          // 坏块：fail 落 readable + 抛出拒 write（双侧同错，bad-chunks 套件点名）。
          let u8;
          try { u8 = toU8(chunk); } catch (e) { fail(e); throw e; }
          if (done) {
            // done 后写入 = 尾垃圾（type-error 套件 [valid, empty] case；
            // 错误落 readable 而非 write 拒绝——pipeTo 语义下后者走 cancel）
            const err = new TypeError("Trailing junk found after the end of the compressed stream");
            err.code = "ERR_TRAILING_JUNK_AFTER_STREAM_END";
            fail(err);
            return;
          }
          feed(u8, 0);
        },
        close() {
          // fail 后再 close 拒首错（corrupt 套件点名 close 拒绝；正常关走下）。
          if (closed || freed) { if (failErr) return Promise.reject(failErr); return; }
          const ok = feed(null, __wjs2_csFinishFlag(kind));
          if (ok) { closed = true; ctrl.close(); }
          free();
        },
        abort(reason) { if (!closed) { closed = true; ctrl.error(reason); } free(); },
      });
      __wjs2_csState.set(this, { readable, writable });
    }
    get readable() {
      const s = __wjs2_csState.get(this);
      if (s === undefined) throw new TypeError("Cannot read private member #transform from an object whose class did not declare it");
      return s.readable;
    }
    get writable() {
      const s = __wjs2_csState.get(this);
      if (s === undefined) throw new TypeError("Cannot read private member #transform from an object whose class did not declare it");
      return s.writable;
    }
  };
  Object.defineProperty(cls.prototype, Symbol.toStringTag, { value: name, configurable: true });
  return cls;
}
globalThis.CompressionStream = __wjs2_makeCSClass("CompressionStream", __CS_KINDS, false);
globalThis.DecompressionStream = __wjs2_makeCSClass("DecompressionStream", __DS_KINDS, true);
// node 内部流互操作（internal/streams/end-of-stream eosWeb）：实例需
// `Symbol.for('nodejs.webstream.isClosedPromise')`（{promise} 形）。
// node 语义：延迟物化、结算随流关闭/出错；此处以 closeWaiters 直供
// （泵在排空关闭/出错时结算）。注意 node 26 已无流级 `.closed`，不补
// （reader/writer 级 .closed 照旧），保持同形。
{
  const kIsClosedPromise = Symbol.for('nodejs.webstream.isClosedPromise');
  Object.defineProperty(globalThis.ReadableStream.prototype, kIsClosedPromise, {
    get() {
      const st = __wjs2_rsState.get(this);
      return { promise: new Promise((resolve, reject) => {
        if (st.error !== undefined) reject(st.error);
        else if (st.closed && !st.queue.length && !st.byteLen) resolve(undefined);
        else { st.closeWaiters.push({ resolve: () => resolve(undefined), reject }); __wjs2_rsPump(st); }
      }) };
    },
    configurable: true,
  });
  Object.defineProperty(globalThis.WritableStream.prototype, kIsClosedPromise, {
    get() {
      const st = __wjs2_wsState.get(this);
      return { promise: new Promise((resolve, reject) => {
        if (st.errored) reject(st.error);
        else if (st.closed) resolve(undefined);
        else { st.closeWaiters = st.closeWaiters || []; st.closeWaiters.push({ resolve, reject }); }
      }) };
    },
    configurable: true,
  });
  // addAbortSignal 互操作：controller.error 直达（读端经控制器 error，
  // 写端经控制器 error；字节流控制器无 error 法即 no-op，真机同款）。
  const kControllerErrorFunction = Symbol.for('nodejs.webstream.controllerErrorFunction');
  Object.defineProperty(globalThis.ReadableStream.prototype, kControllerErrorFunction, {
    value(error) {
      const st = __wjs2_rsState.get(this);
      // node 同款：仅 default 控制器直达 error，字节流保持 no-op。
      if (st.isBytes) return;
      const c = st.controller;
      if (c && typeof c.error === 'function') c.error(error);
    },
    configurable: true,
    writable: true,
  });
  Object.defineProperty(globalThis.WritableStream.prototype, kControllerErrorFunction, {
    value(error) { __wjs2_wsError(this, error); },
    configurable: true,
    writable: true,
  });
}
"#;
