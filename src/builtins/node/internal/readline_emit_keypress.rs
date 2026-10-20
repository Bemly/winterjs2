//! `node:internal/readline/emitKeypressEvents`——按键解码面移植
//!（`lib/internal/readline/emitKeypressEvents.js` + `utils.js emitKeys`
//! 生成器大表内联解码，MIT；真机按键形状逐项对拍，见 tests/node/readline.rs）。
/// 内嵌 ESM 源（零 native，纯形）。
pub const SOURCE: &str = r#"// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node lib/internal/readline/emitKeypressEvents.js (decode inlined).
import errors from 'node:internal/errors';

const {
  codes: {
    ERR_INVALID_ARG_TYPE,
  },
} = errors;

// ── 按键解码（真机按键形状逐项对拍）────────────────────────────────────
// 事件：emit('keypress', s, key)，key = { sequence, name, ctrl, meta, shift, code? }。
// s：可打印/单字节即原文；`\x1b[`/`\x1bO` 转义形即 undefined。
function __ctrlName(code) {
  return String.fromCharCode(code + 96);
}
function __decodeKeys(buf, state) {
  // buf: 待解字节数组；state: { esc: 待定转义串 }。返回 { evs, rest }，
  // rest 为未消费尾（截断 UTF-8），由调用方留待下包。
  const evs = [];
  let i = 0;
  const takeUtf8 = () => {
    const b0 = buf[i];
    let len = 1;
    if ((b0 & 0x80) === 0) len = 1;
    else if ((b0 & 0xe0) === 0xc0) len = 2;
    else if ((b0 & 0xf0) === 0xe0) len = 3;
    else if ((b0 & 0xf8) === 0xf0) len = 4;
    else { i++; return { sequence: String.fromCharCode(b0), name: String.fromCharCode(b0), ctrl: false, meta: false, shift: false }; }
    if (i + len > buf.length) return null;
    const slice = buf.slice(i, i + len);
    i += len;
    const sequence = new TextDecoder().decode(new Uint8Array(slice));
    // node 口径（emitKeys 末段）：字母数字名小写 + 大写即 shift；其余无名
    //（单字符仍按键原串派发，由调用方按 s 落盘）。
    if (/^[0-9A-Za-z]$/.test(sequence)) {
      return { sequence, name: sequence.toLowerCase(), ctrl: false,
               meta: false, shift: /^[A-Z]$/.test(sequence) };
    }
    // node 口径：空格具名（`' '` → name 'space'，余标点/宽字符无名）。
    if (sequence === ' ') {
      return { sequence, name: 'space', ctrl: false, meta: false, shift: false };
    }
    return { sequence, name: undefined, ctrl: false, meta: false, shift: false };
  };
  while (i < buf.length) {
    if (state.esc === '\x1b' && buf[i] === 0x1b) {
      // node 口径（emitKeys 双 esc 合并）：`\x1b\x1b` 后续字符并入同一序列
      //（`\x1b\x1b ` → meta space），此处只续存不定稿。
      state.esc = '\x1b\x1b';
      i++;
      continue;
    }
    if (state.esc !== '') {
      state.esc += String.fromCharCode(buf[i]);
      const e = state.esc;
      const mEsc = /^\x1b(\[\[?|O)(.*)$/.exec(e);
      if (mEsc !== null && /(?:[A-Za-z~^$])$/.test(e) && e.length > 2) {
        // 完整转义（node emitKeys 修饰解析口径）：cmd 剥前缀后按数字形/
        // 字母形提修饰位，code 去修饰（`[2P`→code `[P` + shift）。
        const pfx = mEsc[1];
        const cmd = mEsc[2];
        const codePfx = pfx === 'O' ? 'O' : pfx;
        let code, modifier = 0, m2;
        if ((m2 = /^(?:(\d\d?)(?:;(\d))?([~^$])|(\d{3}~))$/.exec(cmd)) !== null) {
          if (m2[4]) code = codePfx + m2[4];
          else { code = codePfx + m2[1] + m2[3]; modifier = (m2[2] || 1) - 1; }
        } else if ((m2 = /^((\d;)?(\d))?([A-Za-z])$/.exec(cmd)) !== null) {
          code = codePfx + m2[4];
          modifier = (m2[3] || 1) - 1;
        } else {
          code = codePfx + cmd;
        }
        state.esc = '';
        i++; // 消费终字节（漏加即重解一遍，10c-2 实测抓到）
        const key = { sequence: e, name: undefined, ctrl: !!(modifier & 4),
                      meta: !!(modifier & 10), shift: !!(modifier & 1), code };
        switch (code) {
          case '[P': key.name = 'f1'; break;
          case '[Q': key.name = 'f2'; break;
          case '[R': key.name = 'f3'; break;
          case '[S': key.name = 'f4'; break;
          case 'OP': key.name = 'f1'; break;
          case 'OQ': key.name = 'f2'; break;
          case 'OR': key.name = 'f3'; break;
          case 'OS': key.name = 'f4'; break;
          case '[11~': key.name = 'f1'; break;
          case '[12~': key.name = 'f2'; break;
          case '[13~': key.name = 'f3'; break;
          case '[14~': key.name = 'f4'; break;
          case '[200~': key.name = 'paste-start'; break;
          case '[201~': key.name = 'paste-end'; break;
          case '[[A': key.name = 'f1'; break;
          case '[[B': key.name = 'f2'; break;
          case '[[C': key.name = 'f3'; break;
          case '[[D': key.name = 'f4'; break;
          case '[[E': key.name = 'f5'; break;
          case '[15~': key.name = 'f5'; break;
          case '[17~': key.name = 'f6'; break;
          case '[18~': key.name = 'f7'; break;
          case '[19~': key.name = 'f8'; break;
          case '[20~': key.name = 'f9'; break;
          case '[21~': key.name = 'f10'; break;
          case '[23~': key.name = 'f11'; break;
          case '[24~': key.name = 'f12'; break;
          case '[A': key.name = 'up'; break;
          case '[B': key.name = 'down'; break;
          case '[C': key.name = 'right'; break;
          case '[D': key.name = 'left'; break;
          case '[E': key.name = 'clear'; break;
          case '[F': key.name = 'end'; break;
          case '[H': key.name = 'home'; break;
          case 'OA': key.name = 'up'; break;
          case 'OB': key.name = 'down'; break;
          case 'OC': key.name = 'right'; break;
          case 'OD': key.name = 'left'; break;
          case 'OE': key.name = 'clear'; break;
          case 'OF': key.name = 'end'; break;
          case 'OH': key.name = 'home'; break;
          case '[1~': key.name = 'home'; break;
          case '[2~': key.name = 'insert'; break;
          case '[3~': key.name = 'delete'; break;
          case '[4~': key.name = 'end'; break;
          case '[5~': key.name = 'pageup'; break;
          case '[6~': key.name = 'pagedown'; break;
          case '[[5~': key.name = 'pageup'; break;
          case '[[6~': key.name = 'pagedown'; break;
          case '[7~': key.name = 'home'; break;
          case '[8~': key.name = 'end'; break;
          case '[a': key.name = 'up'; key.shift = true; break;
          case '[b': key.name = 'down'; key.shift = true; break;
          case '[c': key.name = 'right'; key.shift = true; break;
          case '[d': key.name = 'left'; key.shift = true; break;
          case '[e': key.name = 'clear'; key.shift = true; break;
          case '[2$': key.name = 'insert'; key.shift = true; break;
          case '[3$': key.name = 'delete'; key.shift = true; break;
          case '[5$': key.name = 'pageup'; key.shift = true; break;
          case '[6$': key.name = 'pagedown'; key.shift = true; break;
          case '[7$': key.name = 'home'; key.shift = true; break;
          case '[8$': key.name = 'end'; key.shift = true; break;
          case 'Oa': key.name = 'up'; key.ctrl = true; break;
          case 'Ob': key.name = 'down'; key.ctrl = true; break;
          case 'Oc': key.name = 'right'; key.ctrl = true; break;
          case 'Od': key.name = 'left'; key.ctrl = true; break;
          case 'Oe': key.name = 'clear'; key.ctrl = true; break;
          case '[2^': key.name = 'insert'; key.ctrl = true; break;
          case '[3^': key.name = 'delete'; key.ctrl = true; break;
          case '[5^': key.name = 'pageup'; key.ctrl = true; break;
          case '[6^': key.name = 'pagedown'; key.ctrl = true; break;
          case '[7^': key.name = 'home'; key.ctrl = true; break;
          case '[8^': key.name = 'end'; key.ctrl = true; break;
          case '[Z': key.name = 'tab'; key.shift = true; break;
          default: key.name = 'undefined'; break;
        }
        evs.push([undefined, key]);
        continue;
      }
      if (/^\x1b([\[O]|\[\[)?$/.test(e) ||
          /^\x1b(\[[0-9;]*|O[0-9]|\[\[[0-9;]*)$/.test(e)) {
        i++;
        continue; // 等更多字节
      }
      if (/^\x1b\[[0-9;]*$/.test(e) || e === '\x1b[' || e === '\x1bO' || e === '\x1b') {
        i++;
        continue; // 等更多字节
      }
      if (e.length === 3 && e[0] === '\x1b' && e[1] === '\x1b') {
        // `\x1b\x1b` 前缀形：末字符按 meta 规则具名（node 双 esc 合并同）。
        const ch3 = e[2];
        const sp3 = { '\n': 'enter', '\r': 'return', '\t': 'tab', '\b': 'backspace', '\x7f': 'backspace', ' ': 'space', '\x1b': 'escape' }[ch3];
        let name3;
        if (sp3 !== undefined) name3 = sp3;
        else if (/^[0-9A-Za-z]$/.test(ch3)) name3 = ch3.toLowerCase();
        else name3 = undefined;
        state.esc = '';
        i++;
        evs.push([undefined, { sequence: e, name: name3, ctrl: false, meta: true,
                       shift: /^[A-Z]$/.test(ch3) }]);
        continue;
      }
      if (e.length === 2 && e[0] === '\x1b') {
        // meta + 单字符（node emitKeys 末段口径）：\n/\r/\t/\x7f/空格/esc 具名 +
        // meta；字母数字名小写（大写即 shift）；其余无名 + meta。
        const ch = e[1];
        const special = { '\n': 'enter', '\r': 'return', '\t': 'tab', '\b': 'backspace', '\x7f': 'backspace', ' ': 'space', '\x1b': 'escape' }[ch];
        let name;
        if (special !== undefined) name = special;
        else if (/^[0-9A-Za-z]$/.test(ch)) name = ch.toLowerCase();
        else name = undefined;
        state.esc = '';
        i++;
        evs.push([undefined, { sequence: e, name, ctrl: false, meta: true,
                       shift: /^[A-Z]$/.test(ch) }]);
        continue;
      }
      // 不是转义：吐出 `\x1b` 本体，余下重解
      state.esc = '';
      evs.push(['\x1b', { sequence: '\x1b', name: 'escape', ctrl: false, meta: false, shift: false }]);
      continue;
    }
    const b = buf[i];
    if (b === 0x1b) {
      state.esc = '\x1b';
      i++;
      continue;
    }
    if (b === 0x0d) {
      i++;
      evs.push(['\r', { sequence: '\r', name: 'return', ctrl: false, meta: false, shift: false }]);
      continue;
    }
    if (b === 0x0a) {
      i++;
      evs.push(['\n', { sequence: '\n', name: 'enter', ctrl: false, meta: false, shift: false }]);
      continue;
    }
    if (b === 0x09) {
      i++;
      evs.push(['\t', { sequence: '\t', name: 'tab', ctrl: false, meta: false, shift: false }]);
      continue;
    }
    if (b === 0x7f || b === 0x08) {
      // node 口径：`\b`(0x08)/`\x7f` 一律 backspace（\x08 不作 ctrl+h；
      // 删左行 observed 同 C-h，经 _ttyWrite backspace 分支同效）。
      i++;
      const sq = b === 0x7f ? '\x7f' : '\b';
      evs.push([sq, { sequence: sq, name: 'backspace', ctrl: false, meta: false, shift: false }]);
      continue;
    }
    if (b >= 0x01 && b <= 0x1a) {
      i++;
      const ch = String.fromCharCode(b);
      evs.push([ch, { sequence: ch, name: __ctrlName(b), ctrl: true, meta: false, shift: false }]);
      continue;
    }
    const k = takeUtf8();
    if (k === null) break; // 等更多字节
    evs.push([k.sequence, k]);
  }
  return { evs, rest: buf.slice(i) };
}

function emitKeypressEvents(stream, iface) {
  if (stream === undefined || stream === null || typeof stream.on !== 'function') {
    throw new ERR_INVALID_ARG_TYPE('stream', 'object', stream);
  }
  if (stream.__wjs2Keypress) return undefined;
  const state = { esc: '', pending: [] };
  stream.__wjs2Keypress = state;
  const flushEsc = () => {
    state.escTimer = undefined;
    const pend = state.esc;
    if (pend === '') return;
    state.esc = '';
    // node 口径（triggerEscape：悬空 esc 超时按 escape 结算，s 用 undefined）。
    stream.emit('keypress', undefined,
      { sequence: pend, name: 'escape', ctrl: false, meta: true, shift: false });
  };
  const feed = (chunk) => {
    const bytes = typeof chunk === 'string'
      ? Array.from(new TextEncoder().encode(chunk))
      : Array.from(chunk instanceof Uint8Array ? chunk
        : chunk instanceof ArrayBuffer ? new Uint8Array(chunk)
        : ArrayBuffer.isView(chunk) ? new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength)
        : new TextEncoder().encode(String(chunk)));
    const all = state.pending.concat(bytes);
    state.pending = [];
    const { evs, rest } = __decodeKeys(all, state);
    state.pending = rest;
    for (const [s, key] of evs) {
      stream.emit('keypress', s, key);
    }
    // node 口径（emitKeypressEvents）：块尾悬空 esc 起定时，超时仍无后续
    // 即按 escape 结算（escapeCodeTimeout，缺省 500）。
    try { if (state.escTimer !== undefined) clearTimeout(state.escTimer); } catch {}
    state.escTimer = undefined;
    if (state.esc !== '') {
      let t = 500;
      try {
        const c = iface?.escapeCodeTimeout;
        if (typeof c === 'number' && Number.isFinite(c)) t = c;
      } catch {}
      state.escTimer = setTimeout(flushEsc, t);
    }
  };
  stream.on('data', feed);
  return undefined;
}

export { emitKeypressEvents };
export default emitKeypressEvents;
"#;
