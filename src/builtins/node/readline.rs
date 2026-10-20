//! `node:readline`（Node `lib/readline.js` 语义移植，MIT；plan 9j：vite 顶层 import）。
//!
//! 忠实面（真机 node 26.8.2 逐项对过）：`createInterface`（options/positional 双形态；
//! 无 input 即无码 TypeError）、行提交（`line` 事件/question/异步迭代器）、
//! history（terminal 下 recent-first、去空去连续重、historySize 上限）、
//! `emitKeypressEvents`（按键形状逐项对拍：sequence/name/ctrl/meta/shift/code）、
//! 终端编辑 Emacs 子集（见下）、pause/resume 转调 + 标志、`close`（input end
//! 自关/幂等）、`prompt`/`setPrompt` 回 undefined。
//!
//! Emacs 子集（terminal 下按键驱动）：可打印插入、Backspace/C-h 左删、
//! Delete/C-d 右删（空行 C-d 即关）、C-u 整行杀、C-k 杀至行尾、C-a/C-e 行首尾、
//! C-b/C-f 左右、C-n/C-p 历史、Up/Down 历史、Home/End、Enter 提交、词跳/词删
//! （meta-b/f/d/backspace）、Tab 补全（kTabComplete/kTabCompleter）、
//! C-c（SIGINT 有监听则发、无则关）、C-l 清屏重绘；其余 ctrl/meta 忽略。
//! 非终端 = 纯行 splitter（`\r\n`/`\n`/`\r`/`\u2028`/`\u2029` + crlfDelay 暂留）。
//!
//! 偏差（记档）：
//! - 无 SIGTSTP/SIGCONT（C-z 忽略）。
//! - `write(data, key)` 有 key 时只走按键动作（真机 `write("ab",{left})` 无输出同款）。
//! - 历史回退到顶丢弃编辑中行（真机保留，角落差）。
//! - 行渲染/补全为 node kRefreshLine/kTabComplete 逐字（colorize/multiline/
//!   beforeEdit 着色面略）；question 无 AbortSignal（promises 面仅覆盖已决态）。

/// 内嵌 ESM 源（零 native，纯形）。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/readline.js (see module docs for deviations).
import { EventEmitter, on as __eventsOn } from 'node:events';
import { kFirstEventParam } from 'node:internal/events/symbols';
import { charLengthAt, charLengthLeft, commonPrefix } from 'node:internal/readline/utils';
import { clearLine, clearScreenDown, cursorTo, moveCursor } from 'node:internal/readline/callbacks';
import { emitKeypressEvents } from 'node:internal/readline/emitKeypressEvents';
import { stripVTControlCharacters } from 'node:util';
import { getStringWidth, inspect } from 'node:internal/util/inspect';
import { validateArray, validateNumber, validateUint32 } from 'node:internal/validators';
import { AbortError } from 'node:internal/errors';
import errors from 'node:internal/errors';

const {
  codes: {
    ERR_INVALID_ARG_TYPE,
    ERR_INVALID_ARG_VALUE,
    ERR_USE_AFTER_CLOSE,
  },
} = errors;

// Tab 补错文案（`__tabErrorText` 偏离见 _tabComplete 注释）。
function __tabErrorText(err) {
  if (err instanceof Error && typeof err.stack === 'string') {
    return `${err.name}: ${err.message}\n${err.stack}`;
  }
  return inspect(err);
}

// node 口径（ReplHistory kNormalizeLineEndings `'\r'→'\n'`）：history 行内
// `\r` 为多行折叠存形，导航展示时逆转回 `\n`（单行无变）。
function __undisplayHistory(stored) {
  return stored.split('\r').reverse().join('\n');
}

// node 口径（lib/readline.js _ttyWriteDumb 逐字）：dumb 终端简化写——
// 无行编辑（s 直拼）、回车门与主路径同（sawReturnAt + crlfDelay 合并）。
function __ttyWriteDumb(s, key) {
  if (key === undefined || key === null) key = {};
  if (key.name === 'escape') return;
  if (this._sawReturnAt && key.name !== 'enter') this._sawReturnAt = 0;
  if (key.ctrl) {
    if (key.name === 'c') {
      if (this.listenerCount('SIGINT') > 0) this.emit('SIGINT');
      else this.close();
      return;
    } else if (key.name === 'd') {
      this.close();
      return;
    }
  }
  switch (key.name) {
    case 'return':
      this._sawReturnAt = Date.now();
      this._commitLine();
      break;
    case 'enter':
      if (this._sawReturnAt === 0 ||
          Date.now() - this._sawReturnAt > this.crlfDelay) {
        this._commitLine();
      }
      this._sawReturnAt = 0;
      break;
    default:
      if (typeof s === 'string' && s) {
        this.line += s;
        this.cursor += s.length;
        this._writeOut(s);
      }
  }
}

class Interface extends EventEmitter {
  constructor(input, output, completer, terminal) {
    super();
    // P2-repl：`new Interface(options)` 归一（node internal/readline 180 行
    // `if (input?.input)` 口径；createInterface 同判定，见下）。
    if (input !== undefined && input !== null && typeof input === 'object' && !Array.isArray(input) &&
        (input.input !== undefined || input.terminal !== undefined || input.completer !== undefined ||
         input.prompt !== undefined || input.history !== undefined || input.historySize !== undefined ||
         input.removeHistoryDuplicates !== undefined ||
         input.escapeCodeTimeout !== undefined || input.tabSize !== undefined ||
         input.crlfDelay !== undefined)) {
      const o = input;
      var __prompt = o.prompt;
      var __history = o.history;
      var __historySize = o.historySize;
      var __removeDup = o.removeHistoryDuplicates;
      var __escapeTimeout = o.escapeCodeTimeout;
      var __tabSize = o.tabSize;
      var __crlfDelay = o.crlfDelay;
      input = o.input;
      output = o.output ?? output;
      completer = o.completer ?? completer;
      terminal = o.terminal ?? terminal;
    }
    // node 口径（InterfaceConstructor back-compat）：terminal 缺省看合并后 output.isTTY。
    if (terminal === undefined && output !== null && output !== undefined) {
      terminal = !!output.isTTY;
    }
    this.input = input ?? null;
    this.output = output ?? null;
    // node 口径（InterfaceConstructor 228 行）：completer 须函数或缺省。
    if (completer !== undefined && typeof completer !== 'function') {
      throw new ERR_INVALID_ARG_VALUE('completer', completer);
    }
    this.completer = completer;
    this.terminal = !!terminal;
    // node 口径（kDecoder StringDecoder）：非终端按字节流拼合 UTF-8（见 _onInputData）。
    this._decoder = new TextDecoder();
    // node 口径（ReplHistory kValidateOptions）：history 缺省或数组、
    // historySize 缺省或数（min 0；-1/NaN 即 OUT_OF_RANGE）。
    if (__history !== undefined) validateArray(__history, 'history');
    if (__historySize !== undefined) validateNumber(__historySize, 'size', 0);
    this.history = [];
    this.historySize = 30;
    if (__historySize !== undefined) this.historySize = __historySize;
    this.removeHistoryDuplicates = !!__removeDup;
    this._prompt = '> ';
    if (__prompt !== undefined) this._prompt = String(__prompt);
    // node 口径（internal/interface 170/200-209 行）：escapeCodeTimeout 缺省 500；
    // 显式值须有限数，否则 ERR_INVALID_ARG_VALUE（报错值取缺省 500，真机同）。
    this.escapeCodeTimeout = 500;
    if (__escapeTimeout !== undefined) {
      if (Number.isFinite(__escapeTimeout)) this.escapeCodeTimeout = __escapeTimeout;
      else throw new ERR_INVALID_ARG_VALUE('input.escapeCodeTimeout', this.escapeCodeTimeout);
    }
    this.tabSize = 8;
    if (__tabSize !== undefined) {
      // node 口径（InterfaceConstructor 选项段）：tabSize 须正 uint32。
      validateUint32(__tabSize, 'tabSize', true);
      this.tabSize = __tabSize;
    }
    // node 口径（InterfaceConstructor 末段）：缺省/假值即 100，否则与 100 取大
    //（Infinity 透传；0/''/NaN 即 100——`??` 会漏，逐字对齐）。
    this.crlfDelay = __crlfDelay ? Math.max(100, __crlfDelay) : 100;
    this.isCompletionEnabled = true;
    this._previousKey = null;
    this._sawReturnAt = 0;
    this.closed = false;
    this.paused = false;
    this.line = '';
    this.cursor = terminal ? 0 : undefined;
    this._historyIndex = -1;
    this._substringSearch = null;
    this._oldPrompt = null;
    this._questionCb = null;
    // node 口径（kQuestionReject 符号槽）：promises 层 question 挂 reject，
    // C-c/C-d 无 SIGINT 监听时经此 AbortError 结算（见 _ttyWrite）。
    this._questionReject = null;
    this._lineBuf = '';
    this._lineIter = undefined;
    this._onData = (b) => this._onInputData(b);
    this._onKey = (s, k) => this._ttyWriteImpl(s, k);
    // node 口径（onend）：EOF 先把残留行直接发 'line'（不走 question/history），再关；
    // 有限 crlfDelay 下暂留的尾部孤 `\r` 视作行结束符去尾。
    this._onEnd = () => {
      // node 口径（ontermend）：终端下 EOF 先把已键入行径 'line' 发出（不经
      // _submit，不写 history），再关。
      if (this.terminal && typeof this.line === 'string' && this.line.length > 0) {
        this.emit('line', this.line);
      }
      if (!this.terminal) {
        // 流式解码收尾（残留截断序列按 U+FFFD 冲刷，StringDecoder.end 同）。
        try { this._lineBuf += this._decoder.decode(); } catch {}
      }
      if (!this.terminal && this._lineBuf.length > 0) {
        let tail = this._lineBuf;
        if (this.crlfDelay !== Infinity) tail = tail.replace(/\r$/, '');
        this._lineBuf = '';
        this.emit('line', tail);
      }
      this.close();
    };
    // node 口径：输入错透传 interface 'error'（input-onerror 套件）。
    this._onError = (e) => this.emit('error', e);
    if (this.terminal) {
      emitKeypressEvents(input, this);
      input.on('keypress', this._onKey);
      // node 口径（lib/readline.js 117 行）：dumb 终端实例级覆写简化写
      //（无行编辑；回车即交，ctrl-c/d 特例保留）。
      try {
        if (typeof process !== 'undefined' && process.env?.TERM === 'dumb') {
          this._ttyWrite = (s, key) => __ttyWriteDumb.call(this, s, key);
        }
      } catch {}
    } else {
      input.on('data', this._onData);
    }
    input.on('error', this._onError);
    input.on('end', this._onEnd);
    if (this.terminal) {
      // node 口径：终端起服即 rawMode(true)；构造尾 resume（input.resume）。
      if (typeof input.setRawMode === 'function') input.setRawMode(true);
    }
    try { input.resume(); } catch { /* ignore */ }
  }
  // ── dumb 模式：纯行 splitter（node lineEnding 口径：
  // `/\r?\n|\r(?!\n)|\u2028|\u2029/` 全量转义形；跨包 `\r\n` 经 sawReturnAt 门合并）。
  _onInputData(chunk) {
    if (this.closed) return;
    let s;
    if (typeof chunk === 'string') {
      s = chunk;
    } else {
      const u8 = chunk instanceof Uint8Array ? chunk
        : chunk instanceof ArrayBuffer ? new Uint8Array(chunk)
        : ArrayBuffer.isView(chunk) ? new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength)
        : new TextEncoder().encode(String(chunk));
      // node 口径（kDecoder StringDecoder 流式）：跨包截断 UTF-8 序列须拼合
      //（逐包独立 decode 会产 U+FFFD，split-bytes 套件钉住）。
      s = this._decoder.decode(u8, { stream: true });
    }
    // node 口径（kNormalWrite kSawReturnAt 门）：上包尾孤 `\r` 且间隔
    // ≤crlfDelay、本包首字符即 `\n` 时吞之（`\r\n` 跨包合并；Infinity 恒吞）。
    // 有限暂留（holdCR）不取——node 即时切分，超时 `\n` 另起空行（200ms 件钉住）。
    if (this._sawReturnAt && Date.now() - this._sawReturnAt <= this.crlfDelay) {
      if (s.codePointAt(0) === 10) s = s.slice(1);
      this._sawReturnAt = 0;
    }
    if (s.endsWith('\r')) this._sawReturnAt = Date.now();
    this._lineBuf += s;
    if (this.paused) return;
    this._drainLines();
  }
  _drainLines() {
    const parts = this._lineBuf.split(/\r?\n|\r(?!\n)|\u2028|\u2029/);
    this._lineBuf = parts.pop();
    for (const line of parts) this._submit(line);
  }
  // ── 提交（两模式共用）──
  _submit(line) {
    if (this.terminal) {
      // P2-repl：多行历史倒序存（node internal/readline/utils reverseString 口径：
      // history 文件单行格式使然；单行无变）。
      const histLine = line.split('\n').reverse().join('\r');
      // node 口径（ReplHistory.addHistory）：空行/纯空白/historySize 0 即
      // 直返（不落 history、不发 'history' 事件）；'history' 事件携 live 数组
      //（监听方改动落盘但不改当行——finalLine 先取后发）。
      if (histLine.length > 0 && histLine.trim().length > 0 && this.historySize > 0) {
        // P2-repl：removeHistoryDuplicates 即清全表同行（node 口径；缺省仅去连续重）。
        if (this.removeHistoryDuplicates) {
          this.history = this.history.filter((h) => h !== histLine);
        }
        if (this.history[0] !== histLine) {
          this.history.unshift(histLine);
          if (this.history.length > this.historySize) this.history.pop();
        }
        this.emit('history', this.history);
      }
      this._historyIndex = -1;
      if (this.output !== null) {
        try { this.output.write('\r\n'); } catch { /* ignore */ }
      }
    }
    this.line = '';
    this.cursor = this.terminal ? 0 : undefined;
    const cb = this._questionCb;
    this._questionCb = null;
    if (cb !== null && cb !== undefined) {
      this.setPrompt(this._oldPrompt);
      cb(line);
    } else {
      this.emit('line', line);
    }
  }
  // 提交当行（return/enter/dumb 共用：清行→_submit；_submit 内写 history）。
  _commitLine() {
    const line = this.line;
    this.line = '';
    this.cursor = 0;
    this._submit(line);
  }
  // ── 终端编辑 ──
  _writeOut(s) {
    if (this.output !== null && this.output !== undefined) {
      try { this.output.write(s); } catch { /* ignore */ }
    }
  }
  // node 口径（interface kRefreshLine 逐字；colorize/multiline 面本仓无）：
  // 行尾下沉→行首→清屏→prompt+行→空行补格→光标归位→上移差值。
  _refreshLine() {
    if (this.output === null || this.output === undefined) return;
    const line = this._prompt + this.line;
    const dispPos = this._getDisplayPos(line);
    const lineCols = dispPos.cols;
    const lineRows = dispPos.rows;
    const cursorPos = this.getCursorPos();
    const prevRows = this.prevRows || 0;
    if (prevRows > 0) {
      moveCursor(this.output, 0, -prevRows);
    }
    cursorTo(this.output, 0);
    clearScreenDown(this.output);
    this._writeOut(this._prompt + this.line);
    if (lineCols === 0) {
      this._writeOut(' ');
    }
    cursorTo(this.output, cursorPos.cols);
    const diff = lineRows - cursorPos.rows;
    if (diff > 0) {
      moveCursor(this.output, 0, -diff);
    }
    this.prevRows = cursorPos.rows;
  }
  // node 口径（interface kInsertString 精简：补全前缀直写，不整行重绘）。
  _insertString(c) {
    if (this.cursor < this.line.length) {
      const beg = this.line.slice(0, this.cursor);
      const end = this.line.slice(this.cursor, this.line.length);
      this.line = beg + c + end;
    } else {
      this.line += c;
    }
    this.cursor += c.length;
    this._writeOut(c);
  }
  // node 口径（interface kTabComplete/kTabCompleter；beforeEdit/着色/多行面略）。
  // 偏离记档：node 原文 `Tab completion error: ${inspect(err)}`；本仓
  // Error.stack 天生无首行（引擎无头栈，uncaught 打印另拼首行；且 node 锁定
  // `inspect(err)===err.stack`，inspect 侧不可补），此处按 node inspect 形
  // 补 `${name}: ${message}` 首行（stack 缺失回落 inspect；public/promises
  // 两层复用 `__tabErrorText`）。
  // 调度口径（public 层：同步回调/同步回值零跳——undefined-columns/tab-complete
  // 同步断言；仅 thenable 排异步。promises 层另见覆写——内部 await 形恒异步）。
  _tabComplete(lastKeypressWasTab) {
    this.pause();
    const string = this.line.slice(0, this.cursor);
    let settled = false;
    const fail = (err) => {
      try { this._writeOut(`Tab completion error: ${__tabErrorText(err)}`); } catch {}
      try { this.resume(); } catch {}
    };
    const go = (value) => {
      try { this.resume(); } catch {}
      try { this._tabCompleter(lastKeypressWasTab, value); }
      catch (e) { fail(e); }
    };
    const settle = (err, value, sync) => {
      if (settled) return;
      settled = true;
      if (err) { fail(err); return; }
      if (sync) { go(value); return; }
      if (value !== null && (typeof value === 'object' || typeof value === 'function') &&
          typeof value.then === 'function') {
        try { value.then((v) => go(v), (e) => fail(e)); }
        catch (e) { fail(e); }
        return;
      }
      // node 口径（lib/readline.js public 包装：单参 completer 包成
      // `(v, cb) => cb(null, completer(v))`——同步回值同步回调、零跳；
      // 仅 thenable 排异步。undefined-columns/tab-complete 同步断言依赖此）。
      go(value);
    };
    let inCall = false;
    let ret;
    try {
      inCall = true;
      ret = this.completer(string, (err, value) => settle(err, value, inCall));
      inCall = false;
    } catch (err) { inCall = false; settle(err, undefined, true); return; }
    if (ret !== undefined) settle(null, ret, false);
  }
  _tabCompleter(lastKeypressWasTab, { 0: completions, 1: completeOn }) {
    if (!completions || completions.length === 0) {
      return;
    }
    const prefix = commonPrefix(completions.filter((e) => e !== ''));
    if (prefix.startsWith(completeOn) && prefix.length > completeOn.length) {
      this._insertString(prefix.slice(completeOn.length));
      return;
    } else if (!completeOn.startsWith(prefix)) {
      this.line = this.line.slice(0, this.cursor - completeOn.length) + prefix +
        this.line.slice(this.cursor, this.line.length);
      this.cursor = this.cursor - completeOn.length + prefix.length;
      this._refreshLine();
      return;
    }
    if (!lastKeypressWasTab) {
      return;
    }
    const completionsWidth = completions.map((e) => getStringWidth(e));
    const width = Math.max(...completionsWidth) + 2;
    let maxColumns = Math.floor(this.columns / width) || 1;
    if (maxColumns === Infinity) {
      maxColumns = 1;
    }
    let output = '\r\n';
    let lineIndex = 0;
    let whitespace = 0;
    for (let i = 0; i < completions.length; i++) {
      const completion = completions[i];
      if (completion === '' || lineIndex === maxColumns) {
        output += '\r\n';
        lineIndex = 0;
        whitespace = 0;
      } else {
        output += ' '.repeat(whitespace);
      }
      if (completion !== '') {
        output += completion;
        whitespace = width - completionsWidth[i];
        lineIndex++;
      } else {
        output += '\r\n';
      }
    }
    if (lineIndex !== 0) {
      output += '\r\n\r\n';
    }
    this._writeOut(output);
    this._refreshLine();
  }
  // node 口径（interface kInsertString 逐字；beforeEdit/colorize 面略）：
  // 补全选择态直写；常态行中改写整行重绘、行尾直写（除非换行才重绘）。
  _insert(s) {
    if (!this.isCompletionEnabled) {
      if (this.cursor < this.line.length) {
        this.line = this.line.slice(0, this.cursor) + s + this.line.slice(this.cursor);
      } else {
        this.line += s;
      }
      this.cursor += s.length;
      this._writeOut(s);
      return;
    }
    if (this.cursor < this.line.length) {
      this.line = this.line.slice(0, this.cursor) + s +
        this.line.slice(this.cursor, this.line.length);
      this.cursor += s.length;
      this._refreshLine();
    } else {
      const oldPos = this.getCursorPos();
      this.line += s;
      this.cursor += s.length;
      const newPos = this.getCursorPos();
      if (oldPos.rows < newPos.rows) {
        this._refreshLine();
      } else {
        this._writeOut(s);
      }
    }
  }
  // node 口径（kDeleteLeft/kDeleteRight 经 charLength 系码点步进，astral 不裂）。
  _deleteLeft() {
    if (this.cursor === 0) return;
    const n = charLengthLeft(this.line, this.cursor);
    this.line = this.line.slice(0, this.cursor - n) + this.line.slice(this.cursor);
    this.cursor -= n;
    this._refreshLine();
  }
  _deleteRight() {
    if (this.cursor >= this.line.length) return;
    this.line = this.line.slice(0, this.cursor) +
      this.line.slice(this.cursor + charLengthAt(this.line, this.cursor));
    this._refreshLine();
  }
  // node 口径（ReplHistory.navigateToPrevious/navigateToNext，经 kHistoryPrev/
  // kHistoryNext）：按 search 前缀过滤（不符或与当行相同即跳过）；到头回 search
  // 本身；门外（空表/越界）直接返回。search 由 _ttyWrite 首段定（首按即定连按保持）。
  _historyGo(dir) {
    const search = this._substringSearch ?? '';
    if (dir > 0) {
      if (this.history.length === 0 || this.history.length === this._historyIndex) return;
      let i = this._historyIndex + 1;
      while (i < this.history.length &&
             (!this.history[i].startsWith(search) || this.line === this.history[i])) i++;
      this._historyIndex = i;
      this.line = i === this.history.length ? search : __undisplayHistory(this.history[i]);
    } else {
      if (this._historyIndex <= -1 || this.history.length === 0) return;
      let i = this._historyIndex - 1;
      while (i >= 0 &&
             (!this.history[i].startsWith(search) || this.line === this.history[i])) i--;
      this._historyIndex = i;
      this.line = i === -1 ? search : __undisplayHistory(this.history[i]);
    }
    this.cursor = this.line.length;
    this._refreshLine();
  }
  // node 口径（kMoveCursor）：统一光标运动。dx=0 即返；钳制界内；
  // 界内同行即相对移动（单写），跨行才整行重绘（±Infinity 到头）。
  _moveCursor(dx) {
    if (dx === 0) return;
    const oldPos = this.getCursorPos();
    this.cursor += dx;
    if (this.cursor < 0) this.cursor = 0;
    else if (this.cursor > this.line.length) this.cursor = this.line.length;
    const newPos = this.getCursorPos();
    if (oldPos.rows === newPos.rows) {
      moveCursor(this.output, newPos.cols - oldPos.cols, 0);
    } else {
      this._refreshLine();
    }
  }
  // node 口径（interface kWordLeft/kWordRight）：词跳（meta+b/f）。
  _wordLeft() {
    if (this.cursor > 0) {
      const leading = this.line.slice(0, this.cursor);
      const reversed = [...leading].reverse().join('');
      const match = /^\s*(?:[^\w\s]+|\w+)?/.exec(reversed);
      this._moveCursor(-match[0].length);
    }
  }
  _wordRight() {
    if (this.cursor < this.line.length) {
      const trailing = this.line.slice(this.cursor);
      const match = /^(?:\s+|[^\w\s]+|\w+)\s*/.exec(trailing);
      this._moveCursor(match[0].length);
    }
  }
  // node 口径（interface kDeleteWordLeft/kDeleteWordRight）。
  _deleteWordLeft() {
    if (this.cursor > 0) {
      const leading = this.line.slice(0, this.cursor);
      const reversed = [...leading].reverse().join('');
      const match = /^\s*(?:[^\w\s]+|\w+)?/.exec(reversed);
      const cut = leading.slice(0, leading.length - match[0].length);
      this.line = cut + this.line.slice(this.cursor);
      this.cursor = cut.length;
      this._refreshLine();
    }
  }
  _deleteWordRight() {
    if (this.cursor < this.line.length) {
      const trailing = this.line.slice(this.cursor);
      const match = /^(?:\s+|\W+|\w+)\s*/.exec(trailing);
      this.line = this.line.slice(0, this.cursor) + trailing.slice(match[0].length);
      this._refreshLine();
    }
  }
  _ttyWrite(s, key) {
    // node 口径（lib/readline.js 兼容别名）：dumb 实例覆写仅改此入口，
    // 按键/write 主路径恒走全量实现（见 _ttyWriteImpl）。
    return this._ttyWriteImpl(s, key);
  }
  _ttyWriteImpl(s, key) {
    // node 口径（kTtyWrite 无 paused 门：暂停期按键不丢，tab 补全 pause/resume
    // 桥接背靠背按键全靠它；close 仍挡）。
    if (this.closed) return;
    if (key === undefined || key === null) key = {};
    const previousKey = this._previousKey;
    this._previousKey = key;
    // node 口径（kTtyWrite 修饰链首段）：ctrl+shift 仅退格/删除有义
    //（行首/行尾杀；kill ring 面略，见偏差），余下无动作。
    if (key.ctrl && key.shift) {
      if (key.name === 'backspace') {
        this.line = this.line.slice(this.cursor);
        this.cursor = 0;
        this._refreshLine();
      } else if (key.name === 'delete') {
        this.line = this.line.slice(0, this.cursor);
        this._refreshLine();
      }
      return;
    }
    // node 口径（kTtyWrite 首段 substring 搜索臂）：首按 up/down 即按当行
    //（0..cursor）定 search，连按保持；他键清零（index 到顶顺带回 -1）。
    if ((key.name === 'up' || key.name === 'down') && !key.ctrl && !key.meta && !key.shift) {
      if (this._substringSearch === null) {
        this._substringSearch = this.line.slice(0, this.cursor);
      }
    } else if (this._substringSearch !== null) {
      this._substringSearch = null;
      if (this.history.length === this._historyIndex) this._historyIndex = -1;
    }
    if (key.ctrl) {
      const n = key.name;
      if (n === 'c') {
        if (this.listenerCount('SIGINT') > 0) this.emit('SIGINT');
        else {
          this.close();
          // node 口径：待决 question 即 AbortError 结算。
          if (typeof this._questionReject === 'function') {
            const rej = this._questionReject;
            this._questionReject = null;
            try { rej(new AbortError('Aborted with Ctrl+C')); } catch {}
          }
        }
        return;
      }
      if (n === 'd') {
        if (this.cursor === 0 && this.line.length === 0) {
          this.close();
          if (typeof this._questionReject === 'function') {
            const rej = this._questionReject;
            this._questionReject = null;
            try { rej(new AbortError('Aborted with Ctrl+D')); } catch {}
          }
          return;
        }
        this._deleteRight();
        return;
      }
      if (n === 'h') { this._deleteLeft(); return; }
      if (n === 'u') { this.line = ''; this.cursor = 0; this._refreshLine(); return; }
      if (n === 'k') { this.line = this.line.slice(0, this.cursor); this._refreshLine(); return; }
      if (n === 'a') { this._moveCursor(-Infinity); return; }
      if (n === 'e') { this._moveCursor(Infinity); return; }
      if (n === 'b') { this._moveCursor(-charLengthLeft(this.line, this.cursor)); return; }
      if (n === 'f') { this._moveCursor(charLengthAt(this.line, this.cursor)); return; }
      if (n === 'n') { this._historyGo(-1); return; } // node: C-n 下一条（新向）
      if (n === 'p') { this._historyGo(1); return; } // node: C-p 上一条（旧向）
      if (n === 'l') { this._writeOut('\x1b[2J\x1b[0;0H'); this._refreshLine(); return; }
      if (n === 'w') {
        const left = this.line.slice(0, this.cursor);
        const cut = left.replace(/[^\s]+\s*$/, '');
        this.line = cut + this.line.slice(this.cursor);
        this.cursor = cut.length;
        this._refreshLine();
        return;
      }
      // node 口径（ctrl 分支后段）：ctrl+左右即词跳、ctrl+退格/删除即词删。
      if (n === 'left') { this._wordLeft(); return; }
      if (n === 'right') { this._wordRight(); return; }
      if (n === 'backspace') { this._deleteWordLeft(); return; }
      if (n === 'delete') { this._deleteWordRight(); return; }
      return;
    }
    if (key.meta) {
      // node 口径（interface meta 分支）：b/f 词跳；d/delete 右词删；
      // backspace 左词删。
      if (key.name === 'b') { this._wordLeft(); return; }
      if (key.name === 'f') { this._wordRight(); return; }
      if (key.name === 'd' || key.name === 'delete') { this._deleteWordRight(); return; }
      if (key.name === 'backspace') { this._deleteWordLeft(); return; }
      return;
    }
    // node 口径（kTtyWrite 无修饰分支前）：sawReturn 计时遇他键清零
    //（`\n` 紧跟 `\r` 才合并，见上）。
    if (this._sawReturnAt && key.name !== 'enter') this._sawReturnAt = 0;
    switch (key.name) {
      // node 口径（kTtyWrite 回车门 + kSawReturnAt）：`\r` 即交并计时；
      // `\n` 仅超时（>crlfDelay）才交——`\r\n` 合并为一行（Infinity 恒合）。
      case 'return': {
        this._sawReturnAt = Date.now();
        this._commitLine();
        return;
      }
      case 'enter': {
        if (this._sawReturnAt === 0 ||
            Date.now() - this._sawReturnAt > this.crlfDelay) {
          this._commitLine();
        }
        this._sawReturnAt = 0;
        return;
      }
      case 'backspace': this._deleteLeft(); return;
      case 'delete': this._deleteRight(); return;
      case 'left': this._moveCursor(-charLengthLeft(this.line, this.cursor)); return;
      case 'right': this._moveCursor(charLengthAt(this.line, this.cursor)); return;
      case 'home': this._moveCursor(-Infinity); return;
      case 'end': this._moveCursor(Infinity); return;
      case 'up': this._historyGo(1); return;
      case 'down': this._historyGo(-1); return;
      case 'tab':
        // node 口径（public _ttyWrite tab 形）：有 completer 即补全
        //（连按二次展表）；无则落入 default 作普通字符插入（非忽略）。
        if (typeof this.completer === 'function' && this.isCompletionEnabled) {
          const lastKeypressWasTab =
            previousKey && previousKey.name === 'tab';
          this._tabComplete(lastKeypressWasTab);
          return;
        }
        // falls through（无 completer → default 普通字符分支）
      default:
        // node 口径（_ttyWrite 末段）：整串按行结束符切分，段逐个插入，
        // 段间即提交（kLine 同构：history + 发射 + 清空）。
        if (typeof s === 'string' && s) {
          const parts = s.split(/\r?\n|\r(?!\n)|\u2028|\u2029/);
          for (let j = 0; j < parts.length; j++) {
            if (parts[j] !== '') this._insert(parts[j]);
            if (j < parts.length - 1) this._submit(this.line);
          }
        }
        return;
    }
  }
  setPrompt(prompt) {
    this._prompt = String(prompt);
  }
  getPrompt() {
    return this._prompt;
  }
  prompt(preserveCursor) {
    // node 口径：暂停即 resume；终端非 dumb 即 cursor 归零 + 整行重绘，
    // 否则直写 prompt（dumb/非终端）。
    if (this.paused) this.resume();
    if (this.terminal && !(typeof process !== 'undefined' && process.env?.TERM === 'dumb')) {
      if (!preserveCursor) this.cursor = 0;
      this._refreshLine();
    } else if (this.output !== null && this.output !== undefined) {
      try { this.output.write(this._prompt); } catch { /* ignore */ }
    }
    return undefined;
  }
  question(query, cb) {
    // node 口径（kQuestion）：query 即新 prompt（getCursorPos 随之）；
    // 重入即重显；应答后复旧 prompt（见 _submit）。
    if (this.closed) throw new ERR_USE_AFTER_CLOSE('readline');
    if (this._questionCb) {
      this.prompt();
    } else {
      this._oldPrompt = this._prompt;
      this.setPrompt(query);
      this._questionCb = cb ?? null;
      this.prompt();
    }
  }
  // node 口径（kQuestionCancel）：弃置待决问题（复旧 prompt + 清行换行）。
  _questionCancel() {
    if (this._questionCb) {
      this._questionCb = null;
      this.setPrompt(this._oldPrompt);
      this.line = '';
      this.cursor = 0;
      this._writeOut('\r\n');
    }
  }
  write(data, key) {
    // P2-repl：关后写抛 ERR_USE_AFTER_CLOSE（真机逐字）；写即 resume（真机口径）；
    // 非终端写即喂行缓冲并排空（node kNormalWrite：write 是入流，不是纯回显）。
    if (this.closed) {
      throw new ERR_USE_AFTER_CLOSE('readline');
    }
    if (this.paused) this.resume();
    if (key !== undefined && key !== null) {
      this._ttyWriteImpl(undefined, typeof key === 'object' ? key : { name: String(key) });
      return undefined;
    }
    if (this.terminal) {
      // node 口径（write 终端分支）：直走全量实现（dumb 覆写只拦外部直调）。
      if (data !== undefined && data !== null) {
        this._ttyWriteImpl(String(data), {});
      }
    } else {
      if (data !== undefined && data !== null) {
        this._lineBuf += String(data);
        this._drainLines();
      }
    }
    return undefined;
  }
  pause() {
    if (this.closed) throw new ERR_USE_AFTER_CLOSE('readline');
    if (!this.paused) {
      this.paused = true;
      try { this.input.pause(); } catch { /* ignore */ }
      this.emit('pause');
    }
    return this;
  }
  resume() {
    if (this.closed) throw new ERR_USE_AFTER_CLOSE('readline');
    if (this.paused) {
      this.paused = false;
      try { this.input.resume(); } catch { /* ignore */ }
      if (!this.terminal) this._drainLines();
      this.emit('resume');
    }
    return this;
  }
  close() {
    if (this.closed) return undefined;
    // node 口径：先 pause（input.pause + 'pause' 派发），终端再 setRawMode(false)。
    try { this.pause(); } catch { /* ignore */ }
    if (this.terminal) {
      try {
        if (typeof this.input.setRawMode === 'function') this.input.setRawMode(false);
      } catch { /* ignore */ }
    }
    this.closed = true;
    try { this.input.removeListener('data', this._onData); } catch { /* ignore */ }
    try { this.input.removeListener('keypress', this._onKey); } catch { /* ignore */ }
    try { this.input.removeListener('error', this._onError); } catch { /* ignore */ }
    try { this.input.removeListener('end', this._onEnd); } catch { /* ignore */ }
    this.emit('close');
    return undefined;
  }
  // node 口径（kHistoryAccessorDescriptors.historyIndex）：导航下标直通
  //（-1 即当行；up 到顶滞留 length，由他键臂回 -1）。
  get historyIndex() { return this._historyIndex; }
  set historyIndex(v) { this._historyIndex = v; }
  // node 口径（internal/interface get columns）：输出列宽，无则 Infinity。
  get columns() {
    try {
      const c = this.output?.columns;
      if (c) return c;
    } catch {}
    return Infinity;
  }
  // node 口径（lib/readline.js getCursorPos）：prompt + 光标前行串的显示列/行。
  getCursorPos() {
    const before = String(this._prompt ?? '') + String(this.line ?? '').slice(0, this.cursor ?? 0);
    return this._getDisplayPos(before);
  }
  _getDisplayPos(str) {
    let offset = 0;
    const col = this.columns;
    let rows = 0;
    str = stripVTControlCharacters(str);
    for (const char of str) {
      if (char === '\n') {
        rows += Math.ceil(offset / col) || 1;
        offset = 0;
        continue;
      }
      if (char === '\t') {
        offset += this.tabSize - (offset % this.tabSize);
        continue;
      }
      const width = getStringWidth(char, false);
      if (width === 0 || width === 1) {
        offset += width;
      } else {
        if ((offset + 1) % col === 0) {
          offset++;
        }
        offset += 2;
      }
    }
    const cols = offset % col;
    rows += (offset - cols) / col;
    return { cols, rows };
  }
  [Symbol.asyncIterator]() {
    // node 口径（internal/readline/interface.js）：缓存 EventEmitter.on(this,
    // 'line', {close:['close'], highWaterMark:1024, 第一参直传})——watermarkData
    // 符号属性 + 背压 pause/resume 随附，不自造迭代器。
    if (this._lineIter === undefined) {
      this._lineIter = __eventsOn(this, 'line', {
        close: ['close'],
        highWaterMark: 1024,
        [kFirstEventParam]: true,
      });
    }
    return this._lineIter;
  }
}

function createInterface(input, output, completer, terminal) {
  // node 口径（lib/readline.js createInterface）：直转 Interface，
  // 选项归一全在构造器（旧有的 prompt/historySize 构造后复写已冗余——构造器内已施，删）。
  if (input !== undefined && input !== null && typeof input === 'object' && !Array.isArray(input) &&
      (input.input !== undefined || input.terminal !== undefined || input.completer !== undefined ||
       input.prompt !== undefined || input.history !== undefined || input.historySize !== undefined ||
       input.removeHistoryDuplicates !== undefined ||
       input.escapeCodeTimeout !== undefined || input.tabSize !== undefined ||
       input.crlfDelay !== undefined)) {
    return new Interface(input, output, completer, terminal);
  }
  if (input === undefined || input === null || typeof input.on !== 'function') {
    throw new TypeError('input.on is not a function');
  }
  if (terminal === undefined) {
    terminal = output != null ? !!output.isTTY : !!input.isTTY;
  }
  return new Interface(input, output, completer, terminal);
}

// 顶层光标件经 `node:internal/readline/callbacks` 重导出（lib/readline.js 同构）。

export {
  Interface,
  createInterface,
  emitKeypressEvents,
  cursorTo,
  clearLine,
  clearScreenDown,
  moveCursor,
};
export default {
  Interface,
  createInterface,
  emitKeypressEvents,
  cursorTo,
  clearLine,
  clearScreenDown,
  moveCursor,
};
"#;
