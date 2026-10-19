//! `node:internal/tls/common`（Node `lib/internal/tls/common.js` 按需移植，MIT）。
//!
//! 仅落地套件用到的 `translatePeerCertificate`（legacy 证书对象整形，纯函数）。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Node.js contributors. MIT.
// Port of node lib/internal/tls/common.js (translatePeerCertificate only).
function translatePeerCertificate(c) {
  if (!c)
    return null;

  if (c.issuerCertificate != null && c.issuerCertificate !== c) {
    c.issuerCertificate = translatePeerCertificate(c.issuerCertificate);
  }
  if (c.infoAccess != null) {
    const info = c.infoAccess;
    c.infoAccess = { __proto__: null };

    // XXX: More key validation?
    info.replace(/([^\n:]*):([^\n]*)(?:\n|$)/g,
                 (all, key, val) => {
                   if (val.charCodeAt(0) === 0x22) {
                     // The translatePeerCertificate function is only
                     // used on internally created legacy certificate
                     // objects, and any value that contains a quote
                     // will always be a valid JSON string literal,
                     // so this should never throw.
                     val = JSON.parse(val);
                   }
                   if (key in c.infoAccess)
                     c.infoAccess[key].push(val);
                   else
                     c.infoAccess[key] = [val];
                 });
  }
  return c;
}
export { translatePeerCertificate };
export default { translatePeerCertificate };
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn source_exports_translate() {
        assert!(super::SOURCE.contains("function translatePeerCertificate(c)"));
    }
}
