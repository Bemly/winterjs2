//! `node:internal/crypto/util`（webcrypto 套件直引面，MIT）。
//!
//! 仅出套件点名的三个纯函数（Node `lib/internal/crypto/util.js` 子集）。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Subset of node:internal/crypto/util needed by webcrypto suites.
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

export { bigIntArrayToUnsignedInt, normalizeAlgorithm, validateKeyOps };
export default { bigIntArrayToUnsignedInt, normalizeAlgorithm, validateKeyOps };
"#;
