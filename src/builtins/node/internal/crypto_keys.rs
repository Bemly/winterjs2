//! `node:internal/crypto/keys`（webcrypto 套件直引面，MIT）。
//!
//! 仅出 `getCryptoKeyHandle`（derivekey/encap 套件点名）。本仓 CryptoKey 为
//! JS 对象，以 `__wjs2_cryptoKeyInfo` 读槽，句柄为轻量门面。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Subset of node:internal/crypto/keys needed by webcrypto suites.
function getCryptoKeyHandle(key) {
  const info = (typeof globalThis.__wjs2_cryptoKeyInfo === "function")
    ? globalThis.__wjs2_cryptoKeyInfo(key)
    : null;
  const material = info?.material;
  return {
    getSymmetricKeySize() {
      return material?.length ?? 0;
    },
    export() {
      return material ? material.slice().buffer : new ArrayBuffer(0);
    },
  };
}

export { getCryptoKeyHandle };
export default { getCryptoKeyHandle };
"#;
