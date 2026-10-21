//! `node:internal/crypto/util`——`internal/crypto/util.js` 的测试侧最小面移植（MIT）。
//! 出套件点名的四个纯函数（Node `lib/internal/crypto/util.js` 子集）：
//! getOpenSSLSecLevel（sec-level 套件直引；返回我方实际执行档 0 = 无强制）+
//! bigIntArrayToUnsignedInt/normalizeAlgorithm/validateKeyOps（webcrypto 套件直引）。
/// 内嵌 ESM 源（零 native，纯形）。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Port of the tested surface of node lib/internal/crypto/util.js.
function getOpenSSLSecLevel() {
  // 本仓无 OpenSSL 安全级别强制（恒 0；BoringSSL 真机同值，OpenSSL 真机为 1/2，
  // 行为差记档——secLevel 仅测试调参用，不入门限逻辑）。
  return 0;
}
function bigIntArrayToUnsignedInt(data) {
  let v = 0;
  for (const b of data) {
    v = v * 256 + (b & 255);
    if (v > 0xffffffff) {
      throw new DOMException(
        "algorithm.publicExponent must fit in an unsigned 32-bit integer",
        "OperationError");
    }
  }
  return v >>> 0;
}
function normalizeAlgorithm(algorithm, op) {
  if (typeof algorithm === "string") return { name: algorithm };
  if (algorithm === null || typeof algorithm !== "object") return { name: String(algorithm) };
  return { ...algorithm };
}
function validateKeyOps(ops) {
}
export { getOpenSSLSecLevel, bigIntArrayToUnsignedInt, normalizeAlgorithm, validateKeyOps };
export default { getOpenSSLSecLevel, bigIntArrayToUnsignedInt, normalizeAlgorithm, validateKeyOps };
"#;
