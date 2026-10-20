//! `node:internal/readline/callbacks`——`lib/internal/readline/callbacks.js`
//! 逐字移植（MIT；顶层光标件：直接写流转义序列，无流/空参即回 true）。
//! `node:readline` 由此重导出（lib/readline.js 同构）；tty.WriteStream 方法面
//! 由 stream.js 侧另行实现，不经此处。
/// 内嵌 ESM 源（零 native，纯形）。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/internal/readline/callbacks.js.
import { CSI } from 'node:internal/readline/utils';
import { validateFunction } from 'node:internal/validators';
import errors from 'node:internal/errors';

const {
  codes: {
    ERR_INVALID_ARG_VALUE,
    ERR_INVALID_CURSOR_POS,
  },
} = errors;

/**
 * moves the cursor to the x and y coordinate on the given stream
 */
function cursorTo(stream, x, y, callback) {
  if (callback !== undefined) {
    validateFunction(callback, 'callback');
  }
  if (typeof y === 'function') {
    callback = y;
    y = undefined;
  }
  if (Number.isNaN(x)) throw new ERR_INVALID_ARG_VALUE('x', x);
  if (Number.isNaN(y)) throw new ERR_INVALID_ARG_VALUE('y', y);
  if (stream == null || (typeof x !== 'number' && typeof y !== 'number')) {
    if (typeof callback === 'function') process.nextTick(callback, null);
    return true;
  }
  if (typeof x !== 'number') throw new ERR_INVALID_CURSOR_POS();
  const data = typeof y !== 'number' ? CSI`${x + 1}G` : CSI`${y + 1};${x + 1}H`;
  return stream.write(data, callback);
}

/**
 * moves the cursor relative to its current location
 */
function moveCursor(stream, dx, dy, callback) {
  if (callback !== undefined) {
    validateFunction(callback, 'callback');
  }
  if (stream == null || !(dx || dy)) {
    if (typeof callback === 'function') process.nextTick(callback, null);
    return true;
  }
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
  return stream.write(data, callback);
}

/**
 * clears the current line the cursor is on:
 *   -1 for left of the cursor
 *   +1 for right of the cursor
 *    0 for the entire line
 */
function clearLine(stream, dir, callback) {
  if (callback !== undefined) {
    validateFunction(callback, 'callback');
  }
  if (stream === null || stream === undefined) {
    if (typeof callback === 'function') process.nextTick(callback, null);
    return true;
  }
  const type =
    dir < 0 ? CSI.kClearToLineBeginning : dir > 0 ? CSI.kClearToLineEnd : CSI.kClearLine;
  return stream.write(type, callback);
}

/**
 * clears the screen from the current position of the cursor down
 */
function clearScreenDown(stream, callback) {
  if (callback !== undefined) {
    validateFunction(callback, 'callback');
  }
  if (stream === null || stream === undefined) {
    if (typeof callback === 'function') process.nextTick(callback, null);
    return true;
  }
  return stream.write(CSI.kClearScreenDown, callback);
}

export {
  clearLine,
  clearScreenDown,
  cursorTo,
  moveCursor,
};
export default {
  clearLine,
  clearScreenDown,
  cursorTo,
  moveCursor,
};
"#;
