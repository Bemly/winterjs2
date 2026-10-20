    // 真机逐字（writable.js _write）：仅 null → ERR_STREAM_NULL_VALUES（undefined 落
    // ARG_TYPE 'Received undefined'）；chunk 类型校验先于 after-end/destroyed 状态检查。
    if (data === null) {
      const e = new TypeError("May not write null values to stream");
      e.code = "ERR_STREAM_NULL_VALUES";
      if (typeof cb2 === "function") { queueMicrotask(() => { try { cb2.call(this, e); } catch {} }); return false; }
      throw e;
    }
    const u8 = __chunkU8(data, typeof enc === "string" ? enc : undefined);
    if (this.destroyed || !this.writable) return this.__writeErr(cb2);
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
