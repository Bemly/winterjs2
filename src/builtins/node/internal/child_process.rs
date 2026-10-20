//! `node:internal/child_process`——live ChildProcess 面注册表 + `getValidStdio` 逐字移植。
//!
//! CJS 可变 exports 形（套件 mock `ChildProcess.prototype` 方法/重赋值
//! `spawnSync`；`require` 取 default 可变对象）：`node:child_process` 本体在
//! 加载时把 live 引用填入 `default` 注册表；公开 `spawnSync` 经注册表
//! call-time 分发（mock 可见，与 node lib/child_process.js 同层）。
/// 源：nodejs/node（MIT）`lib/internal/child_process.js` getValidStdio 族。
pub const SOURCE: &str = r#"
import errors from 'node:internal/errors';
import { inspect } from 'node:util';
const {
  codes: {
    ERR_INVALID_ARG_VALUE: { HideStackFramesError: ERR_INVALID_ARG_VALUE },
    ERR_IPC_ONE_PIPE,
    ERR_IPC_SYNC_FORK,
    ERR_INVALID_SYNC_FORK_INPUT,
  },
} = errors;

// 管道占位（结构 stub：getValidStdio 非 sync 形每 pipe 位建一个；
/// 真句柄由 spawn 侧自建，此处仅供形状断言 + 错误路径 close）。
class __PipeStub {
  close() {}
}
function stdioStringToArray(stdio, channel) {
  const options = [];
  switch (stdio) {
    case 'ignore':
    case 'overlapped':
    case 'pipe':
      options.push(stdio, stdio, stdio);
      break;
    case 'inherit':
      options.push(0, 1, 2);
      break;
    default:
      throw new ERR_INVALID_ARG_VALUE('stdio', stdio);
  }
  if (channel) options.push(channel);
  return options;
}
// 句柄嗅探（本仓流皆 JS 仿形，无原生句柄——一律落空走 fd/报错分支；
/// node 真机 `getHandleWrapType` 逐字位由底座补，见 handle 传递另案）。
function getHandleWrapType(_stream) {
  return undefined;
}
function getValidStdio(stdio, sync) {
  let ipc;
  let ipcFd;
  if (typeof stdio === 'string') {
    stdio = stdioStringToArray(stdio);
  } else if (!Array.isArray(stdio)) {
    throw new ERR_INVALID_ARG_VALUE('stdio', stdio);
  }
  while (stdio.length < 3) stdio.push(undefined);
  stdio = stdio.reduce((acc, stdio, i) => {
    function cleanup() {
      for (let i = 0; i < acc.length; i++) {
        if ((acc[i].type === 'pipe' || acc[i].type === 'ipc') && acc[i].handle) {
          acc[i].handle.close();
        }
      }
    }
    if (stdio === undefined || stdio === null) stdio = i < 3 ? 'pipe' : 'ignore';
    if (stdio === 'ignore') {
      acc.push({ type: 'ignore' });
    } else if (stdio === 'pipe' || stdio === 'overlapped' ||
               (typeof stdio === 'number' && stdio < 0)) {
      const a = {
        type: stdio === 'overlapped' ? 'overlapped' : 'pipe',
        readable: i === 0,
        writable: i !== 0,
      };
      if (!sync) a.handle = new __PipeStub();
      acc.push(a);
    } else if (stdio === 'ipc') {
      if (sync || ipc !== undefined) {
        cleanup();
        if (!sync) throw new ERR_IPC_ONE_PIPE();
        else throw new ERR_IPC_SYNC_FORK();
      }
      ipc = new __PipeStub();
      ipcFd = i;
      acc.push({ type: 'pipe', handle: ipc, ipc: true });
    } else if (stdio === 'inherit') {
      acc.push({ type: 'inherit', fd: i });
    } else if (typeof stdio === 'number' || (stdio !== null && typeof stdio === 'object' && typeof stdio.fd === 'number')) {
      acc.push({
        type: 'fd',
        fd: typeof stdio === 'number' ? stdio : stdio.fd,
      });
    } else if (getHandleWrapType(stdio) || (stdio !== null && typeof stdio === 'object' && (getHandleWrapType(stdio.handle) || getHandleWrapType(stdio._handle)))) {
      acc.push({ type: 'wrap', wrapType: 'unknown', handle: stdio, _stdio: stdio });
    } else if (ArrayBuffer.isView(stdio) || typeof stdio === 'string') {
      if (!sync) {
        cleanup();
        throw new ERR_INVALID_SYNC_FORK_INPUT(inspect(stdio));
      }
    } else {
      cleanup();
      throw new ERR_INVALID_ARG_VALUE('stdio', stdio);
    }
    return acc;
  }, []);
  return { stdio, ipc, ipcFd };
}
// live 注册表（`node:child_process` 加载时填入；mock 直接改本对象属性）。
const __registry = {
  ChildProcess: null,
  spawnSync: null,
  getValidStdio,
};
export { getValidStdio };
export default __registry;
"#;
