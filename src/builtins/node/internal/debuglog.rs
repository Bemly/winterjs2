//! `node:internal/debuglog`——`internal/util/debuglog` 最小面（`debuglog` 恒关闭记档 +
//! `formatTime` 逐字移植，console 计时/套件直调用）。
/// 源：nodejs/node（MIT）对应 internal 件的最小对位实现；偏差见 docs/plan.md Phase 9b。
/// `formatTime` 逐字译自 `lib/internal/util/debuglog.js`（primordials 还原为直接调用，
/// 本模块 internal 面免防篡改，见 internal/mod.rs 口径）。
pub const SOURCE: &str = r#"function debuglog(_set, cb) {
  const fn = function debug() {};
  // Node 异步升级回调（同步会 TDZ：`let debug = debuglog('stream', fn => debug = fn)`）
  if (typeof cb === 'function') queueMicrotask(() => cb(fn));
  return fn;
}
// lib/internal/util/debuglog.js `formatTime` 逐字移植（ms → 'x.xxxms' /
// 'x.xxxs' / 'm:ss.mmm (m:ss.mmm)' / 'h:mm:ss.mmm (h:mm:ss.mmm)'；console
// `time/timeLog/timeEnd` 与 test-console-formatTime 套件直调同一实现）。
function __pad(value) {
  return String(value).padStart(2, '0');
}
const kSecond = 1000;
const kMinute = 60 * kSecond;
const kHour = 60 * kMinute;
function formatTime(ms) {
  let hours = 0;
  let minutes = 0;
  let seconds = 0;
  if (ms >= kSecond) {
    if (ms >= kMinute) {
      if (ms >= kHour) {
        hours = Math.floor(ms / kHour);
        ms = ms % kHour;
      }
      minutes = Math.floor(ms / kMinute);
      ms = ms % kMinute;
    }
    seconds = ms / kSecond;
  }
  if (hours !== 0 || minutes !== 0) {
    [seconds, ms] = seconds.toFixed(3).split('.', 2);
    const res = hours !== 0 ? `${hours}:${__pad(minutes)}` : minutes;
    return `${res}:${__pad(seconds)}.${ms} (${hours !== 0 ? 'h:m' : ''}m:ss.mmm)`;
  }
  if (seconds !== 0) {
    return `${seconds.toFixed(3)}s`;
  }
  return `${Number(ms.toFixed(3))}ms`;
}
export { debuglog, formatTime };
export default { debuglog, formatTime };
"#;
