//! `node:readline/promises`（Node `lib/readline/promises.js` + 对应用到的
//! `lib/internal/readline/promises.js` Readline 部分的语义移植，MIT）。
//! 结构偏离（记档）：node 分公/内两件（`readline/promises` + `internal/readline/promises`），
//! 本仓合一件（零可观察差；internal 件不单列——套件直引面未覆盖它）。
/// 内嵌 ESM 源（零 native，纯形）。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/readline/promises.js (+ the used part of
// lib/internal/readline/promises.js; see module docs for deviations).
import { Interface as _Interface } from 'node:readline';
import { CSI } from 'node:internal/readline/utils';
import { inspect } from 'node:internal/util/inspect';
import { validateAbortSignal, validateBoolean, validateInteger } from 'node:internal/validators';
import { addAbortListener } from 'node:internal/events/abort_listener';
import { AbortError } from 'node:internal/errors';
import utilsStreams from 'node:internal/streams/utils';
import errors from 'node:internal/errors';

const { isWritable } = utilsStreams;
const {
  codes: { ERR_INVALID_ARG_TYPE },
} = errors;

const {
  kClearToLineBeginning,
  kClearToLineEnd,
  kClearLine,
  kClearScreenDown,
} = CSI;

// lib/internal/readline/promises.js Readline（链式游标写器；autoCommit 面同）。
class Readline {
  #autoCommit = false;
  #stream;
  #todo = [];
  constructor(stream, options = undefined) {
    if (!isWritable(stream))
      throw new ERR_INVALID_ARG_TYPE('stream', 'Writable', stream);
    this.#stream = stream;
    if (options?.autoCommit != null) {
      validateBoolean(options.autoCommit, 'options.autoCommit');
      this.#autoCommit = options.autoCommit;
    }
  }
  cursorTo(x, y = undefined) {
    validateInteger(x, 'x');
    if (y != null) validateInteger(y, 'y');
    const data = y == null ? CSI`${x + 1}G` : CSI`${y + 1};${x + 1}H`;
    if (this.#autoCommit) process.nextTick(() => this.#stream.write(data));
    else this.#todo.push(data);
    return this;
  }
  moveCursor(dx, dy) {
    if (dx || dy) {
      validateInteger(dx, 'dx');
      validateInteger(dy, 'dy');
      let data = '';
      if (dx < 0) {
        data += CSI`${-dx}D`;
      } else if (dx > 0) {
        data += CSI`${dx}C`;
      }
      if (dy < 0) {
        data += CSI`${-dy}A`;
      } else if (dy > 0) {
        data += CSI`${dy}B`;
      }
      if (this.#autoCommit) process.nextTick(() => this.#stream.write(data));
      else this.#todo.push(data);
    }
    return this;
  }
  clearLine(dir) {
    validateInteger(dir, 'dir', -1, 1);
    const type =
      dir < 0 ? kClearToLineBeginning : dir > 0 ? kClearToLineEnd : kClearLine;
    if (this.#autoCommit) process.nextTick(() => this.#stream.write(type));
    else this.#todo.push(type);
    return this;
  }
  clearScreenDown() {
    if (this.#autoCommit) process.nextTick(() => this.#stream.write(kClearScreenDown));
    else this.#todo.push(kClearScreenDown);
    return this;
  }
  commit() {
    const data = this.#todo.join('');
    this.#todo = [];
    // node 原文形（executor 内直写：_write 同步抛即 executor 抛转 reject）。
    return new Promise((resolve) => {
      this.#stream.write(data, resolve);
    });
  }
  rollback() {
    this.#todo = [];
    return this;
  }
}

// lib/readline/promises.js Interface（node 逐字：promise 化 question 全形）。
// AbortSignal：已决即 AbortError（cause 系 reason）；决议/中止互斥，
// 中止走基类 _questionCancel；应答即 dispose 监听（removeEventListener 可观测）。
class Interface extends _Interface {
  // node 口径（internal/interface kTabComplete await 形）：promises 面恒异步
  // 决议（基类 public 同步回调形与此相反；unicode 套件微任务断言依赖此）。
  // 错文案与基类同（`__tabErrorText` 逻辑内联——跨件不导出，偏离记档见基类）。
  async _tabComplete(lastKeypressWasTab) {
    this.pause();
    const string = this.line.slice(0, this.cursor);
    let value;
    try {
      value = await this.completer(string);
    } catch (err) {
      try {
        const tail = (err instanceof Error && typeof err.stack === 'string')
          ? `${err.name}: ${err.message}\n${err.stack}`
          : inspect(err);
        this._writeOut(`Tab completion error: ${tail}`);
      } catch {}
      try { this.resume(); } catch {}
      return;
    }
    try { this.resume(); } catch {}
    try { this._tabCompleter(lastKeypressWasTab, value); }
    catch (err) {
      try {
        const tail = (err instanceof Error && typeof err.stack === 'string')
          ? `${err.name}: ${err.message}\n${err.stack}`
          : inspect(err);
        this._writeOut(`Tab completion error: ${tail}`);
      } catch {}
    }
  }
  question(query, options = {}) {
    // node lib/readline/promises.js 逐字（基类经原型方法挂载——本仓无 kQuestion
    // 符号；addAbortListener 惰性取与 node 同形，此处顶层直引等价）。
    return new Promise((resolve, reject) => {
      let cb = resolve;
      if (options?.signal) {
        validateAbortSignal(options.signal, 'options.signal');
        if (options.signal.aborted) {
          return reject(new AbortError(undefined, { cause: options.signal.reason }));
        }
        const onAbort = () => {
          this._questionCancel();
          reject(new AbortError(undefined, { cause: options.signal.reason }));
        };
        const disposable = addAbortListener(options.signal, onAbort);
        const SymbolDispose = Symbol.dispose ?? Symbol.for('Symbol.dispose');
        cb = (answer) => {
          disposable[SymbolDispose]();
          resolve(answer);
        };
      }
      this._questionReject = reject;
      _Interface.prototype.question.call(this, query, cb);
    });
  }
}

function createInterface(input, output, completer, terminal) {
  return new Interface(input, output, completer, terminal);
}

export { Interface, Readline, createInterface };
export default { Interface, Readline, createInterface };
"#;
