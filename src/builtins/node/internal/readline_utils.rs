//! `node:internal/readline/utils`——`internal/readline/utils` 的 CSI/切分纯件移植。
/// 源：nodejs/node（MIT）对应 internal 件的最小对位实现。
/// 口径：primordials 还原为直接调用（internal 面免防篡改）；先落 CSI 标签 +
/// 四常量 + `charLengthLeft/At`（csi 套件 + 补全/刷新列宽用）；`emitKeys`
/// 生成器（按键解码大表）随 R-C1 另件落。
pub const SOURCE: &str = r#"const kUTF16SurrogateThreshold = 0x10000; // 2 ** 16
const kEscape = '\x1b';

function CSI(strings, ...args) {
  let ret = `${kEscape}[`;
  for (let n = 0; n < strings.length; n++) {
    ret += strings[n];
    if (n < args.length)
      ret += args[n];
  }
  return ret;
}

CSI.kEscape = kEscape;
CSI.kClearToLineBeginning = CSI`1K`;
CSI.kClearToLineEnd = CSI`0K`;
CSI.kClearLine = CSI`2K`;
CSI.kClearScreenDown = CSI`0J`;

// TODO(BridgeAR): Treat combined characters as single character, i.e,
// 'a\u0301' and '\u0301a' (both have the same visual output).
// Check Canonical_Combining_Class in
// http://userguide.icu-project.org/strings/properties
function charLengthLeft(str, i) {
  if (i <= 0)
    return 0;
  if ((i > 1 &&
      str.codePointAt(i - 2) >= kUTF16SurrogateThreshold) ||
      str.codePointAt(i - 1) >= kUTF16SurrogateThreshold) {
    return 2;
  }
  return 1;
}

function charLengthAt(str, i) {
  if (str.length <= i) {
    // Pretend to move to the right. This is necessary to autocomplete while
    // moving to the right.
    return 1;
  }
  return str.codePointAt(i) >= kUTF16SurrogateThreshold ? 2 : 1;
}

function commonPrefix(strings) {
  if (strings.length === 0) {
    return '';
  }
  if (strings.length === 1) {
    return strings[0];
  }
  const sorted = [...strings].sort();
  const min = sorted[0];
  const max = sorted[sorted.length - 1];
  for (let i = 0; i < min.length; i++) {
    if (min[i] !== max[i]) {
      return min.slice(0, i);
    }
  }
  return min;
}

export { charLengthAt, charLengthLeft, commonPrefix, CSI };
export default { charLengthAt, charLengthLeft, commonPrefix, CSI };
"#;
