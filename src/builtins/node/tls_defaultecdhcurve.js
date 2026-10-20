// node tls.checkServerIdentity：见 tls_context.js（校验算法族；导出经同模块拼接保持）。
export const DEFAULT_ECDH_CURVE = "auto";
export const DEFAULT_MIN_VERSION = "TLSv1.2";
export const DEFAULT_MAX_VERSION = "TLSv1.3";
export const DEFAULT_CIPHERS =
  "TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_128_GCM_SHA256:" +
  "ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384:" +
  "ECDHE-ECDSA-AES256-GCM-SHA384";
export function getCiphers() {
  return ["tls_aes_128_gcm_sha256", "tls_aes_256_gcm_sha384", "tls_chacha20_poly1305_sha256",
    "ecdhe-ecdsa-aes128-gcm-sha256", "ecdhe-ecdsa-aes256-gcm-sha384", "ecdhe-ecdsa-chacha20-poly1305",
    "ecdhe-rsa-aes128-gcm-sha256", "ecdhe-rsa-aes256-gcm-sha384", "ecdhe-rsa-chacha20-poly1305"];
}
export const CLIENT_RENEG_LIMIT = 3;
export const CLIENT_RENEG_WINDOW = 600;

export function createServer(options, cb) {
  return new Server(options, cb);
}
export function connect(...args) {
  const o = typeof args[0] === "object" && args[0] !== null ? args[0] : (typeof args[1] === "object" && args[1] !== null ? args[1] : (typeof args[2] === "object" && args[2] !== null ? args[2] : {}));
  // node exports.connect：缺省合并 → 回调/DH 校验 → createSecureContext(options)（套件/证书/密钥
  // 校验同 server）→ servername 禁 IP。
  const options = {
    ciphers: __api.DEFAULT_CIPHERS,
    checkServerIdentity: __api.checkServerIdentity,
    minDHSize: 1024,
    ...o,
  };
  __tlsVFunction(options.checkServerIdentity, "options.checkServerIdentity");
  __tlsVNumber(options.minDHSize, "options.minDHSize", 1);
  const context = options.secureContext || createSecureContext(options);
  if (options.servername && __isIP(options.servername)) {
    throw new __tlsC.ERR_INVALID_ARG_VALUE(
      "options.servername",
      options.servername,
      "Setting the TLS ServerName to an IP address is not permitted.",
    );
  }
  if (!o.secureContext) o.secureContext = context;
  const tlssock = new TLSSocket(undefined, { allowHalfOpen: !!o.allowHalfOpen });
  tlssock.__viaConnectApi = true;
  if (o.ALPNProtocols !== undefined) convertALPNProtocols(o.ALPNProtocols, tlssock);
  return tlssock.connect(...args);
}
// node 口径：`tls.Server(...)`/`SecureContext(...)` 无 new 可调（函数构造器自 new）；
// Proxy apply→construct，extends/instanceof 照常（connect-simple 等 9 件）。
const __callable = (C) => new Proxy(C, { apply(t, _this, args) { return new t(...args); } });
const __ServerCallable = __callable(Server);
export { connect as createConnection, TLSSocket, __ServerCallable as Server, SecureContext, convertALPNProtocols };
const __api = {
  TLSSocket, Server: __ServerCallable, SecureContext, createServer, connect, createConnection: connect,
  createSecureContext, getCACertificates, setDefaultCACertificates, convertALPNProtocols,
  checkServerIdentity, getCiphers, DEFAULT_ECDH_CURVE, DEFAULT_MIN_VERSION, DEFAULT_MAX_VERSION,
  DEFAULT_CIPHERS, CLIENT_RENEG_LIMIT, CLIENT_RENEG_WINDOW,
};
// node 口径：rootCertificates 为只读访问器（赋值在严格模式抛 TypeError，root-certificates 套件）。
Object.defineProperty(__api, "rootCertificates", {
  __proto__: null,
  configurable: false,
  enumerable: true,
  get: () => rootCertificates,
});
export default __api;
