//! `node:internal/util/inspect`（Node `lib/internal/util/inspect.js` 子集移植，MIT）。
//!
//! 忠实移植：`format`/`formatWithOptionsInternal`（占位符逐字）、
//! `identicalSequenceRange`、`stripVTControlCharacters`+`ansi` 正则、
//! `getStringWidth`（非 Intl 路径）、字符串转义与数字格式化。
//!
//! 偏差（按 9a 验收需要裁剪，逐条记录）：
//! - `inspect` 为务实重写（非 3000 行逐字）：支持 depth/circular/customInspect/
//!   getters/maxArrayLength/maxStringLength/类型形状（数组洞/Map/Set/TypedArray/
//!   Date/RegExp/Error/函数/类名/`[Symbol]` 键）、**breakLength/compact 换行**
//!   （`reduceToSingleString` 数值 compact 路 + `isBelowBreakLength`，数组分组
//!   `groupArrayElements` 未做；长数组按逐项换行——真机为分组多项一行）、
//!   revoked Proxy 兜底 `<Revoked Proxy>`（抛错后补探，真机 C++ 探测无此误判）、
//!   Promise 结算态经 `__wjs2_promise_state/result` 展开（fulfilled
//!   展值、rejected 标 `<rejected>`、pending 保形）、ArrayBuffer 不打
//!   `[Uint8Contents]`、numericSeparator 恒关、showProxy/showHidden 忽略。
//! - 颜色：`colors:true` 时 primitives/special 按 node 样式表上色（`inspect.styles`
//!   子集）；RegExp 高亮（`highlightRegExp` 分组着色器）未做，整体按 `red`
//!   处理。错误消息路径全部无色，与 9a 断言无交。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of node:internal/util/inspect (subset; deviations documented in module docs).
const inspectDefaultOptions = {
  showHidden: false, depth: 2, colors: false, customInspect: true,
  showProxy: false,
  maxArrayLength: 100, maxStringLength: 10000, breakLength: 80,
  compact: 3, sorted: false, getters: false, numericSeparator: false,
};
const stylizeNoColor = (str, _style) => str;
// 颜色表（node inspect.colors 全表 + inspect.styles 子集；真机口径：
// key 不上色、value 按类型上色；特殊标记走 special/cyan）。
// `inspectColors` 另供 `node:util` styleText 复用（与真机 `inspect.colors` 同键）。
const __ansiColors = {
  reset: [0, 0], bold: [1, 22], dim: [2, 22], italic: [3, 23],
  underline: [4, 24], blink: [5, 25], inverse: [7, 27], hidden: [8, 28],
  strikethrough: [9, 29], doubleunderline: [21, 24],
  black: [30, 39], red: [31, 39], green: [32, 39], yellow: [33, 39],
  blue: [34, 39], magenta: [35, 39], cyan: [36, 39], white: [37, 39],
  bgBlack: [40, 49], bgRed: [41, 49], bgGreen: [42, 49], bgYellow: [43, 49],
  bgBlue: [44, 49], bgMagenta: [45, 49], bgCyan: [46, 49], bgWhite: [47, 49],
  framed: [51, 54], overlined: [53, 55],
  gray: [90, 39], redBright: [91, 39], greenBright: [92, 39],
  yellowBright: [93, 39], blueBright: [94, 39], magentaBright: [95, 39],
  cyanBright: [96, 39], whiteBright: [97, 39],
  bgGray: [100, 49], bgRedBright: [101, 49], bgGreenBright: [102, 49],
  bgYellowBright: [103, 49], bgBlueBright: [104, 49],
  bgMagentaBright: [105, 49], bgCyanBright: [106, 49], bgWhiteBright: [107, 49],
  // 别名（真机 defineColorAlias 静态等价；getter/setter 联动语义未移植——
  // 用户改 `inspect.colors.gray` 不连带 `grey`，偏差记档）。
  grey: [90, 39], blackBright: [90, 39], bgGrey: [100, 49], bgBlackBright: [100, 49],
  faint: [2, 22], crossedout: [9, 29], strikeThrough: [9, 29], crossedOut: [9, 29],
  conceal: [8, 28], swapColors: [7, 27], swapcolors: [7, 27], doubleUnderline: [21, 24],
};
const inspectColors = __ansiColors;
const __ansiStyles = {
  special: 'cyan', number: 'yellow', bigint: 'yellow', boolean: 'yellow',
  undefined: 'gray', null: 'bold', string: 'green', symbol: 'green',
  date: 'magenta', regexp: 'red', module: 'underline',
};
function __stylize(str, style, ctx) {
  if (!ctx || !ctx.colors) return str;
  const color = __ansiColors[__ansiStyles[style] ?? style];
  if (!color) return str;
  return `\u001B[${color[0]}m${str}\u001B[${color[1]}m`;
}
// 可见长（colors 计入 ANSI 转义时剥除，真机 isBelowBreakLength 口径）。
function __visibleLength(ctx, s) {
  if (ctx && ctx.colors) {
    try { s = stripVTControlCharacters(s); } catch {}
  }
  return s.length;
}
// 真机 isBelowBreakLength 移植（breakLength Infinity 即单行，除非已有换行）。
function __isBelowBreakLength(ctx, output, start, base) {
  if (ctx.breakLength === Infinity) {
    if (base !== '' && String(base).includes('\n')) return false;
    for (let i = 0; i < output.length; i++) {
      if (typeof output[i] === 'string' && output[i].includes('\n')) return false;
    }
    return true;
  }
  let totalLength = output.length + start;
  if (totalLength + output.length > ctx.breakLength) return false;
  for (let i = 0; i < output.length; i++) {
    totalLength += __visibleLength(ctx, output[i]);
    if (totalLength > ctx.breakLength) return false;
  }
  return base === '' || !String(base).includes('\n');
}
// 真机 reduceToSingleString 移植（数值 compact 路 + compact:true 路；数组分组
// groupArrayElements 未做，长数组逐项换行——偏差见模块头）。
function __reduceEntries(ctx, output, lvl, braces, base) {
  const compact = ctx.compact;
  if (compact !== true) {
    if (typeof compact === 'number' && compact >= 1) {
      // 真机起算：entries 数 + 当前缩进 + 括号 + base + 10。
      const start = output.length + lvl + braces[0].length + String(base).length + 10;
      if (__isBelowBreakLength(ctx, output, start, base)) {
        const joined = output.join(', ');
        if (!joined.includes('\n')) {
          return `${base ? `${base} ` : ''}${braces[0]} ${joined} ${braces[1]}`;
        }
      }
    }
    const indentation = '\n' + ' '.repeat(lvl);
    return `${base ? `${base} ` : ''}${braces[0]}${indentation}  ` +
      `${output.join(`,${indentation}  `)}${indentation}${braces[1]}`;
  }
  if (__isBelowBreakLength(ctx, output, 0, base)) {
    return `${braces[0]}${base ? ` ${base}` : ''} ${output.join(', ')} ${braces[1]}`;
  }
  const indentation = '\n' + ' '.repeat(lvl);
  return `${braces[0]}${base ? ` ${base}` : ''}${indentation}  ` +
    `${output.join(`,${indentation}  `)}${indentation}${braces[1]}`;
}
const ansi = new RegExp(
  '(?:\\u001B\\][\\s\\S]*?(?:\\u0007|\\u001B\\u005C|\\u009C))' +
  '|[\\u001B\\u009B][[\\]()#;?]*' +
  '(?:\\d{1,4}(?:[;:]\\d{0,4})*)?' +
  '[\\dA-PR-TZcf-nq-uy=><~]', 'g',
);
const kCustom = Symbol.for('nodejs.util.inspect.custom');

function stripVTControlCharacters(str) {
  if (typeof str !== 'string') throw new TypeError('str must be a string');
  if (str.indexOf('\u001B') === -1 && str.indexOf('\u009B') === -1) return str;
  return str.replace(ansi, '');
}

function isFullWidthCodePoint(code) {
  return code >= 0x1100 && (
    code <= 0x115f || code === 0x2329 || code === 0x232a ||
    (code >= 0x2e80 && code <= 0x3247 && code !== 0x303f) ||
    (code >= 0x3250 && code <= 0x4dbf) ||
    (code >= 0x4e00 && code <= 0xa4c6) ||
    (code >= 0xa960 && code <= 0xa97c) ||
    (code >= 0xac00 && code <= 0xd7a3) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0xfe10 && code <= 0xfe19) ||
    (code >= 0xfe30 && code <= 0xfe6b) ||
    (code >= 0xff01 && code <= 0xff60) ||
    (code >= 0xffe0 && code <= 0xffe6) ||
    (code >= 0x1b000 && code <= 0x1b001) ||
    (code >= 0x1f200 && code <= 0x1f251) ||
    (code >= 0x1f300 && code <= 0x1f64f) ||
    (code >= 0x20000 && code <= 0x3fffd));
}

function isZeroWidthCodePoint(code) {
  return code <= 0x1f || (code >= 0x7f && code <= 0x9f) ||
    (code >= 0x300 && code <= 0x36f) ||
    (code >= 0x200b && code <= 0x200f) ||
    (code >= 0x20d0 && code <= 0x20ff) ||
    (code >= 0xfe00 && code <= 0xfe0f) ||
    (code >= 0xfe20 && code <= 0xfe2f) ||
    (code >= 0xe0100 && code <= 0xe01ef);
}

function getStringWidth(str, removeControlChars = true) {
  let width = 0;
  if (removeControlChars) str = stripVTControlCharacters(str);
  str = str.normalize('NFC');
  for (const char of str) {
    const code = char.codePointAt(0);
    if (isFullWidthCodePoint(code)) width += 2;
    else if (!isZeroWidthCodePoint(code)) width++;
  }
  return width;
}

// ── format（逐字移植；颜色路径以 NoColor 恒等收束）────────────────────────
function formatNumberNoColor(number) {
  return Object.is(number, -0) ? '-0' : `${number}`;
}
// hasBuiltInToString（10f 真机逐字移植：%s 遇内建 toString 走 inspect，
// 自定义 toString 走 String()；proxy/不可探测对象按原文简化）。
const __builtInObjects = new Set(
  Object.getOwnPropertyNames(globalThis).filter((e) => /^[A-Z][a-zA-Z0-9]+$/.test(e)),
);
// 10f：真机集是启动快照（`Buffer`/`URL`/`URLSearchParams` 等迟装全局不在其内，
// `%s` 对它们走 String()）；本仓预置全局早已齐，需显式剔除以对齐（实测校准）。
for (const late of ['Buffer', 'URL', 'URLSearchParams']) __builtInObjects.delete(late);
function __hasBuiltInToString(value) {
  if ((typeof value !== 'object' && typeof value !== 'function') || value === null) return true;
  let checkToString = true;
  if (typeof value.toString !== 'function') {
    if (typeof value[Symbol.toPrimitive] !== 'function') return true;
    if (Object.prototype.hasOwnProperty.call(value, Symbol.toPrimitive)) return false;
    checkToString = false;
  } else if (Object.prototype.hasOwnProperty.call(value, 'toString')) {
    return false;
  } else if (typeof value[Symbol.toPrimitive] !== 'function') {
    checkToString = true;
  } else if (Object.prototype.hasOwnProperty.call(value, Symbol.toPrimitive)) {
    return false;
  }
  let pointer = value;
  for (;;) {
    pointer = Object.getPrototypeOf(pointer);
    if (pointer === null) return true;
    if ((checkToString && Object.prototype.hasOwnProperty.call(pointer, 'toString')) ||
        Object.prototype.hasOwnProperty.call(pointer, Symbol.toPrimitive)) {
      break;
    }
  }
  const descriptor = Object.getOwnPropertyDescriptor(pointer, 'constructor');
  return descriptor !== undefined &&
    typeof descriptor.value === 'function' &&
    __builtInObjects.has(descriptor.value.name);
}
function formatBigIntNoColor(bigint) {
  return `${bigint}n`;
}
function tryStringify(obj) {
  try { return JSON.stringify(obj); } catch { return '[Circular]'; }
}

function formatWithOptionsInternal(inspectOptions, args) {
  const first = args[0];
  let a = 0;
  let str = '';
  let join = '';
  // 10f：numericSeparator 同步 format 的数字输出（真机口径；options 缺省守卫）。
  const __sepOn = !!(inspectOptions && inspectOptions.numericSeparator);
  const __num = (n) => __sepOn ? __addNumericSeparator(formatNumberNoColor(n)) : formatNumberNoColor(n);
  const __big = (b) => {
    const s = formatBigIntNoColor(b);
    return __sepOn ? __addNumericSeparator(s.slice(0, -1)) + 'n' : s;
  };

  if (typeof first === 'string') {
    if (args.length === 1) return first;
    let tempStr;
    let lastPos = 0;
    for (let i = 0; i < first.length - 1; i++) {
      if (first.charCodeAt(i) === 37) { // '%'
        const nextChar = first.charCodeAt(++i);
        if (a + 1 !== args.length) {
          switch (nextChar) {
            case 115: { // 's'
              const tempArg = args[++a];
              if (typeof tempArg === 'number') tempStr = __num(tempArg);
              else if (typeof tempArg === 'bigint') tempStr = __big(tempArg);
              else if (typeof tempArg !== 'object' ||
                       tempArg === null ||
                       !__hasBuiltInToString(tempArg)) {
                tempStr = String(tempArg);
              } else {
                tempStr = inspect(tempArg, { ...inspectOptions, compact: 3, colors: false, depth: 0 });
              }
              break;
            }
            case 106: tempStr = tryStringify(args[++a]); break; // 'j'
            case 100: { // 'd'
              const tempNum = args[++a];
              if (typeof tempNum === 'bigint') tempStr = __big(tempNum);
              else if (typeof tempNum === 'symbol') tempStr = 'NaN';
              else tempStr = __num(Number(tempNum));
              break;
            }
            case 79: tempStr = inspect(args[++a], inspectOptions); break; // 'O'
            case 111: // 'o'
              tempStr = inspect(args[++a], { ...inspectOptions, showHidden: true, showProxy: true, depth: 4 });
              break;
            case 105: { // 'i'
              const tempInteger = args[++a];
              if (typeof tempInteger === 'bigint') tempStr = __big(tempInteger);
              else if (typeof tempInteger === 'symbol') tempStr = 'NaN';
              else tempStr = __num(Number.parseInt(tempInteger));
              break;
            }
            case 102: { // 'f'
              const tempFloat = args[++a];
              if (typeof tempFloat === 'symbol') tempStr = 'NaN';
              else tempStr = __num(Number.parseFloat(tempFloat));
              break;
            }
            case 99: a += 1; tempStr = ''; break; // 'c'
            case 37: // '%'
              str += first.slice(lastPos, i);
              lastPos = i + 1;
              continue;
            default: continue; // 非法占位符原样保留
          }
          if (lastPos !== i - 1) str += first.slice(lastPos, i - 1);
          str += tempStr;
          lastPos = i + 1;
        } else if (nextChar === 37) {
          str += first.slice(lastPos, i);
          lastPos = i + 1;
        }
      }
    }
    if (lastPos !== 0) {
      a++;
      join = ' ';
      if (lastPos < first.length) str += first.slice(lastPos);
    }
  }

  while (a < args.length) {
    const value = args[a];
    str += join;
    str += typeof value !== 'string' ? inspect(value, inspectOptions) : value;
    join = ' ';
    a++;
  }
  return str;
}

function format(...args) {
  // 10f：合入活默认表（`defaultOptions.numericSeparator` 突变对 format 可见，真机口径）。
  return formatWithOptionsInternal({ ...inspectDefaultOptions }, args);
}

function formatWithOptions(inspectOptions, ...args) {
  if (inspectOptions === null || Array.isArray(inspectOptions) || typeof inspectOptions !== 'object') {
    throw new TypeError('The "inspectOptions" argument must be of type Object.' +
      `${inspectOptions === null ? ' Received null' : Array.isArray(inspectOptions) ? ' Received an instance of Array' : ` Received type ${typeof inspectOptions}`}`);
  }
  return formatWithOptionsInternal(inspectOptions, args);
}

// ── identicalSequenceRange（逐字移植；events enhanceStackTrace 用）────────
function identicalSequenceRange(a, b) {
  for (let i = 0; i < a.length - 3; i++) {
    const pos = b.indexOf(a[i]);
    if (pos !== -1) {
      const rest = b.length - pos;
      if (rest > 3) {
        let len = 1;
        const maxLen = Math.min(a.length - i, rest);
        while (maxLen > len && a[i + len] === b[pos + len]) len++;
        if (len > 3) return [len, i];
      }
    }
  }
  return [0, 0];
}

// ── inspect（务实重写，偏差见模块头注）──────────────────────────────────
const strEscapeSequencesReplacer = /[\x00-\x1f\x27\x5c\x7f-\x9f]|[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/g;
const keyStrEscape = /[\x00-\x1f\x27\x5c\x7f-\x9f]/g;
const escapeSequences = new Map([
  [0, '\\x00'], [1, '\\x01'], [2, '\\x02'], [3, '\\x03'], [4, '\\x04'], [5, '\\x05'],
  [6, '\\x06'], [7, '\\x07'], [8, '\\b'], [9, '\\t'], [10, '\\n'], [11, '\\x0B'],
  [12, '\\f'], [13, '\\r'], [14, '\\x0e'], [15, '\\x0f'], [16, '\\x10'], [17, '\\x11'],
  [18, '\\x12'], [19, '\\x13'], [20, '\\x14'], [21, '\\x15'], [22, '\\x16'], [23, '\\x17'],
  [24, '\\x18'], [25, '\\x19'], [26, '\\x1a'], [27, '\\x1B'], [28, '\\x1c'], [29, '\\x1d'],
  [30, '\\x1e'], [31, '\\x1f'], [127, '\\x7f'],
]);

function escapeFn(s) {
  const code = s.charCodeAt(0);
  if (escapeSequences.has(code)) return escapeSequences.get(code);
  if (code < 0x20 || (code >= 0x7f && code <= 0x9f)) {
    return '\\x' + code.toString(16).padStart(2, '0');
  }
  return s; // 孤代理：原样（Node 为 \\u 转义，接受偏差）
}

function inspectString(value, ctx) {
  let trailer = '';
  if (value.length > ctx.maxStringLength) {
    const remaining = value.length - ctx.maxStringLength;
    value = value.slice(0, ctx.maxStringLength);
    trailer = `... ${remaining} more character${remaining > 1 ? 's' : ''}`;
  }
  return `'${value.replace(strEscapeSequencesReplacer, escapeFn)}'` + trailer;
}

function formatNumber(value) {
  return Object.is(value, -0) ? '-0' : `${value}`;
}

// numericSeparator（10f 真机口径）：纯十进制串的整数部自右、分数部自左每三位
// 加 `_`；指数形式不动；符号保留。
function __addNumericSeparator(s) {
  const m = s.match(/^(-?)(\d+)(?:\.(\d+))?$/);
  if (!m) return s;
  const grpR = (d) => d.replace(/\B(?=(\d{3})+$)/g, '_');
  const grpL = (d) => d.replace(/(\d{3})(?=\d)/g, '$1_');
  return m[1] + grpR(m[2]) + (m[3] === undefined ? '' : '.' + grpL(m[3]));
}

function formatPrimitive(value, ctx) {
  switch (typeof value) {
    case 'string': return __stylize(inspectString(value, ctx), 'string', ctx);
    case 'number':
      return __stylize(ctx.numericSeparator ? __addNumericSeparator(formatNumber(value)) : formatNumber(value), 'number', ctx);
    case 'bigint':
      return __stylize(ctx.numericSeparator ? __addNumericSeparator(`${value}`) + 'n' : `${value}n`, 'bigint', ctx);
    case 'boolean': return __stylize(`${value}`, 'boolean', ctx);
    case 'undefined': return __stylize('undefined', 'undefined', ctx);
    case 'symbol': return __stylize(value.toString(), 'symbol', ctx);
    default: return Object.is(value, null) ? __stylize('null', 'null', ctx) : 'unknown';
  }
}

function constructorName(obj) {
  let proto = Object.getPrototypeOf(obj);
  while (proto !== null) {
    const ctor = proto.constructor;
    if (typeof ctor === 'function' && ctor.name !== '') return ctor.name;
    proto = Object.getPrototypeOf(proto);
  }
  return null;
}

function getFunctionPrefix(fn) {
  if (fn.constructor?.name === 'AsyncFunction') return 'AsyncFunction';
  if (fn.constructor?.name === 'GeneratorFunction') return 'GeneratorFunction';
  if (fn.constructor?.name === 'AsyncGeneratorFunction') return 'AsyncGeneratorFunction';
  return 'Function';
}

function prefixOf(value) {
  const name = constructorName(value);
  if (name === null || name === 'Object') return '';
  if (name === 'Array') return '';
  return name + ' ';
}

// 普通对象（含 null 原型）主体：`{ a: 1 }` / 空键 `{}`。
function formatPlainObject(value, ctx, depth, seen) {
  const keys = Reflect.ownKeys(value);
  const entries = [];
  // 子项按深一层缩进格式化（多行嵌套对齐用；真机 indentationLvl 口径）。
  const lvl = ctx.__lvl ?? 0;
  ctx.__lvl = lvl + 2;
  try {
  for (const key of keys) {
    if (ctx.maxArrayLength !== Infinity && entries.length >= ctx.maxArrayLength) break;
    let desc;
    try { desc = Object.getOwnPropertyDescriptor(value, key); } catch { continue; }
    if (desc === undefined) continue;
    const name = typeof key === 'symbol' ? `[${key.toString()}]` : key;
    let valStr;
    if (desc.get !== undefined || desc.set !== undefined) {
      const hasGetter = desc.get !== undefined;
      const hasSetter = desc.set !== undefined;
      if (ctx.getters === true || ctx.getters === 'always') {
        try { valStr = inspect2(hasGetter ? desc.get.call(value) : undefined, ctx, depth - 1, seen); }
        catch (err) { valStr = `[Exception: ${err?.message ?? err}]`; }
      } else {
        valStr = __stylize(hasGetter && hasSetter ? '[Getter/Setter]' : hasGetter ? '[Getter]' : '[Setter]', 'special', ctx);
      }
    } else {
      valStr = inspect2(desc.value, ctx, depth - 1, seen);
    }
    const shown = typeof key === 'symbol' ? name :
      (/^[A-Za-z_$][\w$]*$/.test(name) ? name : JSON.stringify(name));
    entries.push(`${shown}: ${valStr}`);
  }
  } finally { ctx.__lvl = lvl; }
  // prefixOf 带尾空格（旧单行拼接用）；__reduceEntries 的 base 不带。
  const p = (prefixOf(value) || (Object.getPrototypeOf(value) === null ? '[Object: null prototype]' : '')).trimEnd();
  if (entries.length === 0) return p ? `${p} {}` : '{}';
  return __reduceEntries(ctx, entries, lvl, ['{', '}'], p);
}

// revoked Proxy 兜底（真机 `<Revoked Proxy>`；C++ getProxyDetails 的 JS 侧近似：
// 仅在已抛错后补探 getPrototypeOf，活 Proxy 正常路径零副作用）。
function __revokedFallback(value, ctx, err) {
  let __revoked = false;
  try { Object.getPrototypeOf(value); } catch { __revoked = true; }
  if (__revoked) return __stylize('<Revoked Proxy>', 'special', ctx);
  throw err;
}

function inspect2(value, ctx, depth, seen) {
  // 循环引用：Node 形 `​[Circular * n]`（n = 1 起；colors 下 special 上色）
  if (typeof value === 'object' && value !== null) {
    const idx = seen.indexOf(value);
    if (idx !== -1) return __stylize(`[Circular * ${idx + 1}]`, 'special', ctx);
  }
  // 10f：函数不走 primitives（Node formatValue 亦然；否则 '[Function]' 变 'unknown'）
  if ((typeof value !== 'object' && typeof value !== 'function') || value === null) return formatPrimitive(value, ctx);
  // 10f：depth<0 只截断容器（Array/普通对象/Map/Set/TypedArray/AB），Date/Error/
  // RegExp/Promise/函数照常展开（真机口径）；空容器仍显体，非空显 `[Prefix]`。
  try {
  if (depth < 0) {
    if (value instanceof Error || value instanceof Date || value instanceof RegExp ||
        value instanceof Promise || typeof value === 'function') {
      // 落空到下述正常格式化
    } else if (value instanceof Map) {
      return value.size === 0 ? 'Map(0) {}' : '[Map]';
    } else if (value instanceof Set) {
      return value.size === 0 ? 'Set(0) {}' : '[Set]';
    } else if (Array.isArray(value)) {
      if (value.length === 0) {
        const extra = Reflect.ownKeys(value).filter((k) => k !== 'length' &&
          !(typeof k === 'string' && /^[0-9]+$/.test(k)));
        if (extra.length === 0) return '[]';
      }
      return '[Array]';
    } else if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer ||
        (typeof SharedArrayBuffer === 'function' && value instanceof SharedArrayBuffer)) {
      const cn = constructorName(value);
      return `[${cn ?? 'ArrayBuffer'}]`;
    } else {
      let keys;
      try { keys = Reflect.ownKeys(value); } catch { return '[Object]'; }
      if (keys.length === 0) {
        // 10f：depth<0 且具 custom inspect 面即 '[Object]'（真机：负深度不调
        // custom，直接坍缩；custom-searchparams-inspect 套件点名）。
        if (ctx.customInspect && typeof value[kCustom] === 'function') return '[Object]';
        const p = prefixOf(value) || (Object.getPrototypeOf(value) === null ? '[Object: null prototype] ' : '');
        return p ? `${p}{}` : '{}';
      }
      const cn = constructorName(value);
      if (cn === null) return '[Object: null prototype]';
      if (cn === 'Object' || cn === '') return '[Object]';
      return `[${cn}]`;
    }
  }
  } catch (errD) {
    return __revokedFallback(value, ctx, errD);
  }

  // custom inspect（customInspect:false 关闭；返回自身则继续普通格式化；异常传播）
  try {
    if (ctx.customInspect && typeof value[kCustom] === 'function') {
      const ret = value[kCustom](depth, { ...ctx, stylize: (s, t) => __stylize(s, t, ctx) }, inspect);
      if (ret !== value) return ret;
    }
  } catch (err0) {
    return __revokedFallback(value, ctx, err0);
  }

  seen.push(value);
  try {
    if (value instanceof Error) return value.stack ?? `${value.name}: ${value.message}`;
    // 真机口径：colors 下 Date 整体 magenta（date 样），RegExp 整体 red
    //（真机 highlightRegExp 分组着色器未移植——偏差见模块头）。
    if (value instanceof Date) {
      return __stylize(Number.isNaN(value.getTime()) ? 'Invalid Date' : value.toISOString(), 'date', ctx);
    }
    if (value instanceof RegExp) return __stylize(value.toString(), 'regexp', ctx);
    if (typeof value === 'function') {
      // 真机口径：调原始 Function.prototype.toString（用户覆写实例 toString
      // 不得触发；test-console-not-call-toString 点名）。
      const name = value.name === '' ? ' (anonymous)' : `: ${value.name}`;
      if (Function.prototype.toString.call(value).startsWith('class')) {
        const proto = Object.getPrototypeOf(value.prototype);
        const ext = proto && proto.constructor && proto.constructor.name !== 'Object' && proto.constructor.name !== ''
          ? ` extends ${proto.constructor.name}` : '';
        // 真机口径：colors 下函数/类整体 special（cyan）。
        return __stylize(`[class${name === ' (anonymous)' ? ' (anonymous)' : ` ${value.name}`}${ext}]`, 'special', ctx);
      }
      return __stylize(`[${getFunctionPrefix(value)}${name}]`, 'special', ctx);
    }
    if (value instanceof Promise) return formatSettledPromise(value, ctx, depth, seen); // 结算态经 native 内态 API
    if (typeof globalThis.__wjs2_promise_state === 'function') {
      // 跨 compartment 的 CCW promise（`instanceof` 恒 false，§4.57）：按
      // toStringTag 认领后读内态（vm evaluate 跨域 completion 显示用）。
      let isP = false;
      try { isP = Object.prototype.toString.call(value) === '[object Promise]'; } catch { isP = false; }
      if (isP) return formatSettledPromise(value, ctx, depth, seen);
    }
    if (value instanceof Map) {
      const entries = [];
      let i = 0;
      const lvl = ctx.__lvl ?? 0;
      ctx.__lvl = lvl + 2;
      try {
      for (const [k, v] of value) {
        if (ctx.maxArrayLength !== Infinity && i >= ctx.maxArrayLength) break;
        entries.push(`${inspect2(k, ctx, depth - 1, seen)} => ${inspect2(v, ctx, depth - 1, seen)}`);
        i++;
      }
      } finally { ctx.__lvl = lvl; }
      let extra = '';
      if (ctx.maxArrayLength !== Infinity && value.size > i) {
        extra = `... ${value.size - i} more item${value.size - i > 1 ? 's' : ''}`;
      }
      if (extra !== '') entries.push(extra);
      const base = `Map(${value.size})`;
      if (entries.length === 0) return `${base} {}`;
      return __reduceEntries(ctx, entries, lvl, ['{', '}'], base);
    }
    if (value instanceof Set) {
      const entries = [];
      let i = 0;
      const lvl = ctx.__lvl ?? 0;
      ctx.__lvl = lvl + 2;
      try {
      for (const v of value) {
        if (ctx.maxArrayLength !== Infinity && i >= ctx.maxArrayLength) break;
        entries.push(inspect2(v, ctx, depth - 1, seen));
        i++;
      }
      } finally { ctx.__lvl = lvl; }
      let extra = '';
      if (ctx.maxArrayLength !== Infinity && value.size > i) {
        extra = `... ${value.size - i} more item${value.size - i > 1 ? 's' : ''}`;
      }
      if (extra !== '') entries.push(extra);
      const base = `Set(${value.size})`;
      if (entries.length === 0) return `${base} {}`;
      return __reduceEntries(ctx, entries, lvl, ['{', '}'], base);
    }
    if (value instanceof WeakMap || value instanceof WeakSet) return `${constructorName(value)} { <items unknown> }`;
    if (value instanceof ArrayBuffer || (typeof SharedArrayBuffer === 'function' && value instanceof SharedArrayBuffer)) {
      return `${constructorName(value)} { byteLength: ${value.byteLength} }`;
    }
    if (ArrayBuffer.isView(value)) {
      if (value instanceof DataView) {
        return `DataView { byteLength: ${value.byteLength}, byteOffset: ${value.byteOffset}, buffer: ArrayBuffer { byteLength: ${value.buffer.byteLength} } }`;
      }
      const entries = [];
      const limit = ctx.maxArrayLength === Infinity ? value.length : Math.min(value.length, ctx.maxArrayLength);
      for (let i = 0; i < limit; i++) entries.push(inspect2(value[i], ctx, depth - 1, seen));
      let extra = '';
      if (value.length > limit) {
        const more = value.length - limit;
        extra = `... ${more} more item${more > 1 ? 's' : ''}`;
      }
      const body = entries.length === 0 && extra === '' ? '[]' : `[ ${entries.join(', ')}${extra ? `, ${extra}` : ''} ]`;
      return `${constructorName(value)}(${value.length}) ${body}`;
    }
    if (Array.isArray(value)) {
      const name = constructorName(value);
      const p = name === 'Array' || name === null ? '' : `${name}(${value.length}) `;
      const entries = [];
      let holeCount = 0;
      const lvl = ctx.__lvl ?? 0;
      ctx.__lvl = lvl + 2;
      try {
      const limit = ctx.maxArrayLength === Infinity ? value.length : Math.min(value.length, ctx.maxArrayLength);
      for (let i = 0; i < limit; i++) {
        if (!(i in value)) { holeCount++; continue; }
        if (holeCount > 0) {
          entries.push(holeCount === 1 ? '<1 empty item>' : `<${holeCount} empty items>`);
          holeCount = 0;
        }
        entries.push(inspect2(value[i], ctx, depth - 1, seen));
      }
      if (holeCount > 0 && limit === value.length) {
        entries.push(holeCount === 1 ? '<1 empty item>' : `<${holeCount} empty items>`);
        holeCount = 0;
      }
      // 10f：数组自有非索引属性一并打印（`new Foobar(5)` 的 `aaa`，真机口径）。
      for (const k of Reflect.ownKeys(value)) {
        if (typeof k !== 'symbol') {
          const n = Number(k);
          if (Number.isInteger(n) && n >= 0 && n < 4294967295 && String(n) === k) continue;
          if (!Object.prototype.propertyIsEnumerable.call(value, k)) continue;
        } else if (!Object.prototype.propertyIsEnumerable.call(value, k)) continue;
        const shown = typeof k === 'symbol' ? `[${k.toString()}]` :
          (/^[A-Za-z_$][\w$]*$/.test(k) ? k : JSON.stringify(k));
        entries.push(`${shown}: ${inspect2(value[k], ctx, depth - 1, seen)}`);
      }
      } finally { ctx.__lvl = lvl; }
      let extra = '';
      if (value.length > (ctx.maxArrayLength === Infinity ? value.length : Math.min(value.length, ctx.maxArrayLength))) {
        const more = value.length - Math.min(value.length, ctx.maxArrayLength);
        extra = `... ${more} more item${more > 1 ? 's' : ''}`;
      }
      if (extra !== '') entries.push(extra);
      if (entries.length === 0 && p === '') return '[]';
      if (entries.length === 0) return `${p}[]`;
      if (p !== '') return __reduceEntries(ctx, entries, lvl, ['[', ']'], p.trimEnd());
      return __reduceEntries(ctx, entries, lvl, ['[', ']'], '');
    }
    // 普通对象
    return formatPlainObject(value, ctx, depth, seen);
  } catch (err2) {
    // 真机口径：revoked Proxy 显示 `<Revoked Proxy>`（issue-43095）。
    // 真机经 C++ 内 binding（getProxyDetails）无副作用探测；JS 侧无等价手段，
    // 此处仅在已抛错后补探 getPrototypeOf（活 Proxy 不到此分支，无副作用；
    // getPrototypeOf 自抛的活 Proxy 误判为 revoked——偏差记档）。
    return __revokedFallback(value, ctx, err2);
  } finally {
    seen.pop();
  }
}

// Promise 显示（真机 26.8.1：fulfilled 展值 `Promise { v }`、rejected 标
// `<rejected>`、pending 保形；内态经 `__wjs2_promise_state/result`，失败回落 pending）。
function formatSettledPromise(value, ctx, depth, seen) {
  let st = -1;
  try { st = globalThis.__wjs2_promise_state(value); } catch { st = -1; }
  if (st === 1) {
    try {
      const r = globalThis.__wjs2_promise_result(value);
      return `Promise { ${inspect2(r, ctx, depth - 1, seen)} }`;
    } catch { return 'Promise { <pending> }'; }
  }
  if (st === 2) return 'Promise { <rejected> }';
  return 'Promise { <pending> }';
}

function inspect(value, options) {
  const opts = typeof options === 'boolean' ? { ...inspectDefaultOptions, showHidden: options } :
    { ...inspectDefaultOptions, ...(options ?? {}) };
  // 10f：depth 语义与真机一致（剩余层数，null/undefined 表无穷；旧实现 +1 永不到 cut）。
  const startDepth = opts.depth ?? Infinity;
  return inspect2(value, opts, startDepth, []);
}

export {
  identicalSequenceRange,
  inspect,
  inspectColors,
  inspectDefaultOptions,
  stylizeNoColor,
  format,
  formatWithOptions,
  getStringWidth,
  stripVTControlCharacters,
  isZeroWidthCodePoint,
};
export default { identicalSequenceRange, inspect, inspectColors, inspectDefaultOptions, stylizeNoColor, format, formatWithOptions, getStringWidth, stripVTControlCharacters, isZeroWidthCodePoint };
"#;
