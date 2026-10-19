//! `node:internal/url`（Node `lib/internal/url.js` 按需移植，MIT）。
//!
//! 仅落地套件用到的 `isURL` 判形（真机原文逐字，见 `~/node-test/lib/internal/url.js`
//! `function isURL`）：WHATWG URL 实例 ↔ legacy `url.parse()` 产物/裸对象。
//! 其余（URLContext/`isURLInstance`/parse 系）随用随补。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Node.js contributors. MIT.
// Port of node lib/internal/url.js (isURL only).
// We check for `auth` and `path` attribute to distinguish legacy url instance with
// WHATWG URL instance.
function isURL(self) {
  return Boolean(self?.href && self.protocol && self.auth === undefined && self.path === undefined);
}
export { isURL };
export default { isURL };
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn source_exports_is_url() {
        assert!(super::SOURCE.contains("function isURL(self)"));
    }
}
