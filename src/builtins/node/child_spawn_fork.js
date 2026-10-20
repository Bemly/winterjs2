export function fork(modulePath, args, opts) {
  if (modulePath === undefined || modulePath === null ||
      (typeof modulePath !== "string" && !(modulePath instanceof URL))) {
    const err = new TypeError("The \"modulePath\" argument must be of type string or URL");
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  // args/options 归一（真机 fork 口径：缺席即 []/{}；纯对象回落 options；
  // 余下非数组即 ARG_TYPE/Array；options 数组与非对象即 ARG_TYPE）。
  if (args === undefined || args === null) { args = []; }
  else if (typeof args === "object" && !Array.isArray(args)) { opts = args; args = []; }
  else if (!Array.isArray(args)) { throw new ERR_INVALID_ARG_TYPE("args", "Array", args); }
  if (opts === undefined || opts === null) opts = {};
  else if (typeof opts !== "object" || Array.isArray(opts)) { throw new ERR_INVALID_ARG_TYPE("options", "object", opts); }
  if (modulePath instanceof URL && modulePath.protocol !== "file:") {
    const err = new TypeError("The \"modulePath\" argument must be a file URL");
    err.code = "ERR_INVALID_ARG_VALUE";
    throw err;
  }
  __nullCheck(String(modulePath instanceof URL ? modulePath.href : modulePath), "modulePath", "must be a string, Uint8Array, or URL without null bytes");
  for (let i = 0; i < (args || []).length; i++) __nullCheck(String(args[i]), `args[${i}]`);
  const o = __normForkOpts(opts);
  const modStr = modulePath instanceof URL ? modulePath.href : String(modulePath);
  const fileUrl = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(modStr) ? modStr : pathToFileURL(modStr).href;
  const argsArr = [...(args || [])].map(String);
  const proc = new ChildProcess();
  proc.__forkChild = true;
  proc.__killSignal = o.killSignal;
  proc.spawnfile = o.execPath;
  proc.spawnargs = [o.execPath, fileUrl, ...argsArr];
  // 预中止：线程不起（真机不起子进程同理），异步 error + exit/close(null, killSignal)
  //（fork-abort-signal 套件 exit mustCall(null, 'SIGTERM'/'SIGKILL')）。
  if (o.signal && o.signal.aborted) {
    const signame = o.killSigname;
    const reason = o.signal.reason;
    queueMicrotask(() => {
      proc.__emitAbort(reason);
      if (typeof proc.onexit === "function") proc.onexit(null, signame);
      if (typeof proc.onclose === "function") proc.onclose(null, signame);
    });
    return proc;
  }
  // silent 缓冲管形流（子输出进父端管道，见 child_fork.js）。
  if (o.silent) {
    proc.stdout = __forkPipeStream();
    proc.stderr = __forkPipeStream();
  }
  // ESM 系（.mjs/.mts/.cts）走 import，其余（.cjs/.js/无后缀）走 require
  // （CJS 双求值案：扩展名判定，无后缀按 CJS；type:module 包内 .js 经
  // require(esm) 面兜底，见 phase9k）。
  const __modLower = fileUrl.split("?")[0].toLowerCase();
  const __useImport = __modLower.endsWith(".mjs") || __modLower.endsWith(".mts") ||
    __modLower.endsWith(".cts");
  const src = __FORK_CHILD_SRC
    .replace("__FORK_MOD__", () => JSON.stringify(fileUrl))
    .replace("__FORK_ARGV__", () => JSON.stringify(argsArr))
    .replace("__USE_IMPORT__", () => (__useImport ? "true" : "false"));
  const worker = new Worker(src, { eval: true, __wjs2_forkChild: true, env: o.env });
  proc.__worker = worker;
  proc.__connected = true;
  // 非 silent（stdio 继承）：stdout/stderr 恒 null（真机 26 逐项：fork 未 silent
  // 时 `c.stdout === null` 三面全真——继承无管道句柄；silent 才挂管形流）。
  proc.stdin = null;
  if (!o.silent) {
    proc.stdout = null;
    proc.stderr = null;
  }
  proc.channel = { ref() {}, unref() {} };
  worker.on("message", (m) => {
    // 子输出信封分流（非 silent 丢弃）。
    if (__isForkIoEnvelope(m)) {
      const target = m.__wjs2_fork_io[0] === 1 ? proc.stdout : proc.stderr;
      if (target !== null && target !== undefined && typeof target.__push === "function") {
        // 子端发 base64 串（结构化克隆竞速案；Buffer 直发在退出沿丢件）。
        try {
          const raw = m.__wjs2_fork_io[1];
          target.__push(typeof raw === "string" ? Buffer.from(raw, "base64") : raw);
        } catch {}
      }
      return;
    }
    proc.__onForkMessage(m);
  });
  worker.on("error", (e) => {
    if (typeof proc.onerror === "function") proc.onerror(e);
  });
  worker.on("exit", (code) => proc.__onForkExit(code));
  // 退出收尾（清 timer + abort dispose；listenerCount 断言）。
  const __kDispose = Symbol.dispose ?? Symbol.for("Symbol.dispose");
  let __abortDispose = null;
  const __disposeAbort = () => {
    if (__abortDispose !== null) {
      try { __abortDispose[__kDispose](); } catch {}
      __abortDispose = null;
    }
  };
  // timeout 到点按 killSignal 杀；自然先退清 timer。
  if (o.timeoutMs > 0) {
    const __forkTmo = setTimeout(() => {
      if (proc.__exitDone) return;
      proc.__exitDone = true;
      try { worker.terminate(); } catch {}
      __disposeAbort();
      if (typeof proc.onexit === "function") proc.onexit(null, o.killSigname);
      if (typeof proc.onclose === "function") proc.onclose(null, o.killSigname);
    }, o.timeoutMs);
    const __rawForkExit = proc.__onForkExit.bind(proc);
    proc.__onForkExit = (code) => { clearTimeout(__forkTmo); __disposeAbort(); __rawForkExit(code); };
  } else {
    const __rawForkExit = proc.__onForkExit.bind(proc);
    proc.__onForkExit = (code) => { __disposeAbort(); __rawForkExit(code); };
  }
  // abort 接线（addAbortListener 供 listenerCount；各路径 dispose）。
  if (o.signal) {
    __abortDispose = addAbortListener(o.signal, () => {
      if (proc.__exitDone) return;
      proc.__exitDone = true;
      try { worker.terminate(); } catch { /* 已退即走下 */ }
      __disposeAbort();
      proc.__emitAbort(o.signal.reason);
      if (typeof proc.onexit === "function") proc.onexit(null, o.killSigname);
      if (typeof proc.onclose === "function") proc.onclose(null, o.killSigname);
    });
  }
  return proc;
}
export default { execSync, spawnSync, spawn, exec, execFile, execFileSync, fork, ChildProcess };
// live 注册表回填（mock 面；加载期一次）。
import __childInternal from "node:internal/child_process";
if (__childInternal !== null && typeof __childInternal === "object") {
  __childInternal.ChildProcess = ChildProcess;
  __childInternal.spawnSync = __spawnSyncInternal;
}
