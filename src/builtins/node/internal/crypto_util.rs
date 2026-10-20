//! `node:internal/crypto/util`——`internal/crypto/util.js` 的测试侧最小面移植（MIT）。
//! 仅 `getOpenSSLSecLevel`（套件直引形；返回我方实际执行档 0 = 无强制）。
//! 其余（ccmlabel 等）套件未覆盖，不列。
/// 内嵌 ESM 源（零 native，纯形）。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of the getOpenSSLSecLevel surface of node lib/internal/crypto/util.js.
function getOpenSSLSecLevel() {
  // 本仓无 OpenSSL 安全级别强制（恒 0；BoringSSL 真机同值，OpenSSL 真机为 1/2，
  // 行为差记档——secLevel 仅测试调参用，不入门限逻辑）。
  return 0;
}

export { getOpenSSLSecLevel };
export default { getOpenSSLSecLevel };
"#;
