//! tests/node/tls.rs — 对齐 src/builtins/node/tls.rs（node:tls）。

use crate::helpers::*;

#[test]
fn tls_echo_loopback() {
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "p.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
// node 口径：无 key/cert 也可建服务（握手时才失败 → tlsClientError）。
try {{ tls.createServer({{}}); console.log("no-cert ok"); }} catch (e) {{ console.log("no-cert", e.constructor.name); }}
const server = tls.createServer({{ key, cert }});
server.on("secureConnection", (sock) => {{
  console.log("srv-secure", sock.encrypted, sock.authorized);
  sock.on("data", (c) => sock.write("tls-echo:" + c));
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  // ca 校验路径：authorized 为 true
  const cli = tls.connect({{ port, host: "127.0.0.1", ca: cert }}, () => {{
    console.log("cli-secure", cli.encrypted, cli.authorized, cli.authorizationError === null);
    cli.write("hello-tls");
  }});
  cli.on("data", (c) => {{
    console.log("cli-data", String(c));
    cli.end();
  }});
  cli.on("close", () => {{
    // rejectUnauthorized:false 路径：连上但未授权
    const cli2 = tls.connect({{ port, host: "127.0.0.1", rejectUnauthorized: false }}, () => {{
      console.log("cli2-secure", cli2.encrypted, cli2.authorized, cli2.authorizationError !== null);
      cli2.end();
    }});
    cli2.on("close", () => server.close());
    cli2.on("error", () => {{}});
  }});
  cli.on("error", () => {{}});
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    assert!(out.contains("no-cert ok"), "out: {out}");
    assert!(out.contains("cli-secure true true true"), "out: {out}");
    assert!(out.contains("srv-secure true false"), "out: {out}"); // node：服务端未 requestCert 即 authorized=false（真机对拍）
    assert!(out.contains("cli-data tls-echo:hello-tls"), "out: {out}");
    assert!(out.contains("cli2-secure true false true"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn tls_errors() {
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "p.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
// 坏 PEM 同步抛（真机 26.8.2：OpenSSL DECODER 错，非 TypeError）
try {{ tls.createServer({{ key: "nope", cert }}).listen(0); }} catch (e) {{ console.log("badkey", e.code, e.message); }}
const server = tls.createServer({{ key, cert }});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  // 自签无 ca：握手失败，错误提 certificate（不断具体码，hermetic 口径）
  const a = tls.connect({{ port, host: "127.0.0.1" }});
  a.on("error", (e) => {{
    console.log("selfsign", e.code, /certificate/i.test(e.message));
    // 拒连：code 为 string（具体码平台相关，不断言值）
    const b = tls.connect({{ port: 1, host: "127.0.0.1", rejectUnauthorized: false }});
    b.on("error", (e2) => {{
      console.log("refused", typeof e2.code, b.destroyed === true);
      server.close();
    }});
  }});
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    assert!(out.contains("badkey ERR_OSSL_UNSUPPORTED error:1E08010C:DECODER routines::unsupported"), "out: {out}");
    // 真机 DEPTH_ZERO_SELF_SIGNED_CERT（自签 depth-0 终结；真机对拍 strict-probe QB/QC）。
    assert!(out.contains("selfsign DEPTH_ZERO_SELF_SIGNED_CERT true"), "out: {out}");
    assert!(out.contains("refused string true"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}
// ── Phase 9d-7：node:http2 ────

#[test]
fn tls_x509_v1_certificates() {
    // P1（2026-09-25）：X.509 v1 证书（node fixtures agent* 同形，OpenSSL 照收、webpki 拒）——
    // 服务端出示 + 客户端经 ca 校验（v1 兜底：issuer 验签 + 有效期 + CN 主机名）。
    let dir = assert_fs::TempDir::new().unwrap();
    let fx = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tls/");
    let out = run_fs_file(
        &dir,
        "v1.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync("{fx}v1-key.pem", "utf8");
const cert = fs.readFileSync("{fx}v1-cert.pem", "utf8");
const ca = fs.readFileSync("{fx}ca-cert.pem", "utf8");
const server = tls.createServer({{ key, cert }}, (s) => s.end("v1-ok"));
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  // 正常：servername=localhost 对上 CN，ca 验签通过。
  const a = tls.connect({{ port, host: "127.0.0.1", servername: "localhost", ca }}, () => {{
    console.log("v1-authorized", a.authorized);
  }});
  a.on("data", (c) => console.log("v1-data", String(c)));
  a.on("close", () => {{
    // 报错：主机名不符（CN=localhost vs other.test）。
    const b = tls.connect({{ port, host: "127.0.0.1", servername: "other.test", ca }});
    b.on("error", (e) => {{
      console.log("v1-badname", e.code === "ERR_TLS_CERT_ALTNAME_INVALID");
      // 边界：错的 ca（自身证书当 ca）→ 签发者不认识。
      const c = tls.connect({{ port, host: "127.0.0.1", servername: "localhost", ca: cert }});
      c.on("error", (e2) => {{ console.log("v1-badca", e2.code === "UNABLE_TO_VERIFY_LEAF_SIGNATURE"); server.close(); }});
    }});
  }});
}});
"#
        ),
    );
    for line in ["v1-authorized true", "v1-data v1-ok", "v1-badname true", "v1-badca true"] {
        assert!(out.contains(line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn tls_socket_surface() {
    // P2（2026-09-25）：TLSSocket 建在 net.Socket 上（流面继承）+ 握手信息查询 + 顶层 API。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "s.mjs",
        &format!(
            r#"
import tls from "node:tls";
import net from "node:net";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = tls.createServer({{ key, cert }}, (s) => s.pipe(s));
server.listen(0, "127.0.0.1", () => {{
  const c = tls.connect({{ port: server.address().port, host: "127.0.0.1", ca: cert }}, () => {{
    console.log("isnet", c instanceof net.Socket, typeof c.pipe, typeof c.setTimeout);
    console.log("proto", c.getProtocol(), c.getCipher().version === c.getProtocol(), typeof c.getCipher().name);
    const pc = c.getPeerCertificate();
    console.log("peer", Buffer.isBuffer(pc.raw), typeof pc.fingerprint256, /127\.0\.0\.1/.test(pc.subjectaltname ?? ""));
    c.end("ping");
  }});
  c.setEncoding("utf8");
  c.on("data", (d) => console.log("echo", d));
  c.on("close", () => server.close());
}});
// 正常：CA 列表 / SecureContext / 身份校验
const cas = tls.getCACertificates("system");
console.log("ca", Array.isArray(cas), cas.every((p) => p.startsWith("-----BEGIN CERTIFICATE-----")));
console.log("ctx", typeof tls.createSecureContext({{ key, cert }}).context);
console.log("ident-ok", tls.checkServerIdentity("a.example.com", {{ subjectaltname: "DNS:*.example.com" }}) === undefined);
// 报错：身份不符 / 非法 type
console.log("ident-bad", tls.checkServerIdentity("b.other.com", {{ subjectaltname: "DNS:a.example.com" }}).code);
try {{ tls.getCACertificates("nope"); }} catch (e) {{ console.log("ca-bad", e.code); }}
// 边界：未给 key/cert 也可建服务（node 口径）
console.log("nocert", tls.createServer({{}}) instanceof tls.Server);
"#
        ),
    );
    for line in [
        "isnet true function function",
        "proto TLSv1.3 true string",
        "peer true string true",
        "echo ping",
        "ca true true",
        "ctx object",
        "ident-ok true",
        "ident-bad ERR_TLS_CERT_ALTNAME_INVALID",
        "ca-bad ERR_INVALID_ARG_VALUE",
        "nocert true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn tls_peer_certificate_pubkey_material() {
    // N1：legacy 19 键——RSA modulus/exponent/bits + EC asn1Curve/nistCurve/bits +
    // pubkey（RSA 取 SPKI 整段、EC 取点字节）；N/A 键显式 undefined 不缺席。
    let out = run_fs_file(
        &assert_fs::TempDir::new().unwrap(),
        "s.mjs",
        r#"
import { X509Certificate } from "node:crypto";
const RSA = `-----BEGIN CERTIFICATE-----
MIIC6jCCAdKgAwIBAgIUaN+vftnBZXfz3MBulRhR01+kaq4wDQYJKoZIhvcNAQEL
BQAwFTETMBEGA1UEAwwKbjFyc2EudGVzdDAeFw0yNjEwMjAxMzA3MThaFw0yNjEw
MjIxMzA3MThaMBUxEzARBgNVBAMMCm4xcnNhLnRlc3QwggEiMA0GCSqGSIb3DQEB
AQUAA4IBDwAwggEKAoIBAQC5km8B1houPkBjrc/51z/9DcdWokppQxdtdLKAT+Qe
qV+uSb+P0lMDvqJYn7ZeSULm3wc1f8b/LLfyf4O9jBsBTR0j2I1m6Byr3HWhCaF8
XyaeTt002t3MPwAtbrv7rkAF3YakgDOdAbCbNCJ//C63FuS9rASgeyJ6EUYZg8gS
j4PWyQNCJHjeBHRd4IBlMWlpTHrhOJLVLDAZidMkZOsFNdI7yWUz0QJYWLVwJ6Ff
ro2XDzbCDtidwALWst1T6GdWlL3bCYCJqWUAVvStmCT0Gx0Rt6OCBAY2Jqq/+52g
g4gP3Rxlww0kpqyluMeJ5JiuxEy4PiyehYMMgsXHC7HpAgMBAAGjMjAwMB0GA1Ud
DgQWBBRYXXH+bBpWJp3UeeZt8rEFOirCXjAPBgNVHRMBAf8EBTADAQH/MA0GCSqG
SIb3DQEBCwUAA4IBAQCRvRsptGAH2FM4NmFucGRkGt1x9BzmElWCPBiibhsaPCVn
CmF/A99zLTV4Hwq5o1oIOeCd//mvZQR9p8YUo3RN35FgkeFFxSzPuZNSHu/CMD/f
2TGWWHr1vh1+/7bGDixEg+VlVCqW0tc6Ura4UZc6a1htWPFZ99ficgC6xDgAhG1O
4VcWksScvplWt921PQQFtWMdzHJn3ty4FLfAlfST4sseoXoskgdOnX39FDFMzWAz
ilp0xT/z1OgjOJ+MZx+Ooli2ZHoRtWT8qMkvPnaRLuFPoWcVDeOJ7xCv+32O1SAR
knJVGia4eYgFRkK6k1o6cMFzimsmNbizkyLM4hQp
-----END CERTIFICATE-----`;
const EC = `-----BEGIN CERTIFICATE-----
MIIBXDCCAQKgAwIBAgIUQtKwLQdCGGkYDqtpykn0yfzpb40wCgYIKoZIzj0EAwIw
FDESMBAGA1UEAwwJbjFlYy50ZXN0MB4XDTI2MTAyMDEzMDcxOFoXDTI2MTAyMjEz
MDcxOFowFDESMBAGA1UEAwwJbjFlYy50ZXN0MFkwEwYHKoZIzj0CAQYIKoZIzj0D
AQcDQgAEx8Pt/7BV54e8cKGdz9fnGrcFherMtcsRfBmbtORbqxb4kiSL4JCc4ohP
YpRIMxeW75HzwjuTBtSMHyTpeSX/76MyMDAwHQYDVR0OBBYEFLukrMBRUTfqGBmU
60tutZp55JdEMA8GA1UdEwEB/wQFMAMBAf8wCgYIKoZIzj0EAwIDSAAwRQIgVHuX
wTqTIrfpS4vDX0pLLMgSl7SSU4i6xQdVSX8ZdtkCIQC0R9cplTTBhx+a9bQ1dolE
mH8YMtIgHKRp+fIzIGwitA==
-----END CERTIFICATE-----`;
// 正常：RSA 七键形（interp：modulus 大写 512、exponent 0x10001、SPKI 头 0x30）。
const rsa = new X509Certificate(RSA).toLegacyObject();
console.log("rsa", Object.keys(rsa).length, rsa.bits, rsa.exponent, rsa.asn1Curve, rsa.nistCurve, rsa.ext_key_usage, rsa.modulus.length, rsa.pubkey.length, rsa.pubkey[0], /^[0-9A-F]+$/.test(rsa.modulus));
// 正常：EC 七键形（点头 0x04、65B）。
const ec = new X509Certificate(EC).toLegacyObject();
console.log("ec", ec.bits, ec.asn1Curve, ec.nistCurve, ec.exponent, ec.modulus, ec.pubkey.length, ec.pubkey[0]);
// 报错：坏块抛 PEM 行错码。
try { new X509Certificate("zz"); console.log("bad NO-THROW"); } catch (e) { console.log("bad", e.code); }
// 边界：19 键定序与真机一致。
console.log("order", JSON.stringify(Object.keys(rsa)) === JSON.stringify(["subject","issuer","subjectaltname","infoAccess","ca","modulus","exponent","pubkey","bits","valid_from","valid_to","fingerprint","fingerprint256","fingerprint512","ext_key_usage","serialNumber","raw","asn1Curve","nistCurve"]));
"#,
    );
    for line in [
        "rsa 19 2048 0x10001 undefined undefined undefined 512 294 48 true",
        "ec 256 prime256v1 P-256 undefined undefined 65 4",
        "bad ERR_OSSL_PEM_NO_START_LINE",
        "order true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
}

#[test]
fn tls_secure_context_validation() {
    // P2（2026-09-26）：SecureContext/createSecureContext/configSecureContext 按 node 逐字移植——
    // 选项校验、OpenSSL 可观察报错（未知方法 / no cipher match / 密钥不配对）、Server 构造器校验、
    // tls.Server 无 new 可调、rootCertificates 只读、setDefaultCACertificates 报错面。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "v.js",
        &format!(
            r#"
"use strict";
const tls = require("tls");
const fs = require("fs");
const crypto = require("crypto");
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const code = (f) => {{ try {{ f(); return "ok"; }} catch (e) {{ return e.code ?? e.message; }} }};
// 正常：合法选项建上下文，旧读取面仍给 PEM；Server 无 new 可调。
const sc = tls.createSecureContext({{ key, cert, ciphers: "ECDHE-RSA-AES128-GCM-SHA256:RSA@SECLEVEL=0" }});
console.log("ctx", typeof sc.context.key, sc.context.cert.includes("BEGIN CERTIFICATE"), tls.Server({{ key, cert }}) instanceof tls.Server);
// 报错：类型/方法/套件/密钥不配对/SNI 回调/servername 为 IP。
const other = crypto.generateKeyPairSync("rsa", {{ modulusLength: 2048 }}).privateKey.export({{ type: "pkcs8", format: "pem" }});
console.log("errs", [
  code(() => tls.createSecureContext({{ ciphers: 1 }})),
  code(() => tls.createSecureContext({{ secureProtocol: "blargh" }})),
  code(() => tls.createSecureContext({{ ciphers: "FOOBARBAZ" }})),
  code(() => tls.createSecureContext({{ key: other, cert }})),
  code(() => tls.createServer({{ SNICallback: 42 }})),
  code(() => tls.connect({{ port: 1, servername: "127.0.0.1" }})),
].join(","));
// 边界：rootCertificates 只读且无尾换行；bundled 即同一数组；非 PEM 全无效 / PEM 坏块两种报错。
console.log("root", code(() => {{ tls.rootCertificates = 0; }}), tls.getCACertificates("bundled") === tls.rootCertificates,
  tls.rootCertificates.every((c) => c.endsWith("\n-----END CERTIFICATE-----")));
console.log("setca", code(() => tls.setDefaultCACertificates(["nope"])),
  code(() => tls.setDefaultCACertificates(["-----BEGIN CERTIFICATE-----\nxx\n-----END CERTIFICATE-----"])),
  code(() => tls.setDefaultCACertificates([1])));
// EC PARAMETERS 块在前的密钥 PEM 可解析（OpenSSL 跳过参数块）。
const ec = crypto.generateKeyPairSync("ec", {{ namedCurve: "prime256v1" }}).privateKey.export({{ type: "sec1", format: "pem" }});
console.log("ecparams", crypto.createPrivateKey("-----BEGIN EC PARAMETERS-----\nBggqhkjOPQMBBw==\n-----END EC PARAMETERS-----\n" + ec).asymmetricKeyType);
"#
        ),
    );
    assert!(out.contains("ctx string true true"), "out: {out}");
    assert!(
        out.contains("errs ERR_INVALID_ARG_TYPE,ERR_TLS_INVALID_PROTOCOL_METHOD,ERR_SSL_NO_CIPHER_MATCH,ERR_OSSL_X509_KEY_VALUES_MISMATCH,ERR_INVALID_ARG_TYPE,ERR_INVALID_ARG_VALUE"),
        "out: {out}"
    );
    assert!(out.lines().any(|l| l.starts_with("root ") && l.ends_with(" true true") && !l.starts_with("root ok")), "out: {out}");
    assert!(out.contains("setca ERR_CRYPTO_OPERATION_FAILED ERR_OSSL_PEM_ASN1_LIB ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("ecparams ec"), "out: {out}");
}

#[test]
fn tls_alpn_negotiation() {
    // ALPN 静态协商（alpn-server-client 套件 Test1/4 口径；callback 另案）。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "a.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = tls.createServer({{ key, cert, ALPNProtocols: ["a", "b", "c"] }}, (sock) => {{
  console.log("srv-alpn", sock.alpnProtocol);
  sock.destroy();
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const tryOne = (protos, tag, next) => {{
    const cli = tls.connect({{ port, host: "127.0.0.1", ca: cert, ALPNProtocols: protos }}, () => {{
      console.log(tag, cli.alpnProtocol);
      cli.destroy();
      next();
    }});
    cli.on("error", (e) => {{ console.log(tag, "err", e.code); next(); }});
  }};
  tryOne(["a", "b", "c"], "c1", () =>
    tryOne(["c", "b", "e"], "c2", () =>
      tryOne(["x", "y"], "c3", () => server.close())));
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    assert!(out.contains("srv-alpn a"), "out: {out}");
    assert!(out.contains("c1 a"), "out: {out}");
    assert!(out.contains("c2 b"), "out: {out}");
    assert!(out.contains("c3 err ERR_SSL_TLSV1_ALERT_NO_APPLICATION_PROTOCOL"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn tls_absent_sni_reports_false() {
    // node 真机：客户端 servername 缺席（''/IP 直连）即不发 SNI，
    // 服务端 socket.servername 为 false（非 null）；显式名则原样透传。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "s.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = tls.createServer({{ key, cert }}, (sock) => {{
  console.log("srv-sni", JSON.stringify(sock.servername));
  sock.on("data", () => sock.end());
  sock.on("error", () => {{}});
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  // 正常：显式 servername 透传（DNS 名，webpki 合法形）。
  const a = tls.connect({{ port, host: "127.0.0.1", servername: "example.com", rejectUnauthorized: false }}, () => {{
    a.write("x");
    a.on("data", () => {{}});
    a.on("end", () => {{
      a.destroy();
      // 边界：显式 '' 即缺席（SNI 禁发），服务端见 false。
      const b = tls.connect({{ port, host: "127.0.0.1", servername: "", rejectUnauthorized: false }}, () => {{
        b.write("y");
        b.on("data", () => {{}});
        b.on("end", () => {{ b.destroy(); server.close(); }});
      }});
      b.on("error", () => {{}});
    }});
  }});
  a.on("error", () => {{}});
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 2500);
"#
        ),
    );
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.contains(&"srv-sni \"example.com\""), "out: {out}");
    assert!(lines.contains(&"srv-sni false"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn tls_client_absent_sni_is_false() {
    // node _finishInit 口径：客户端无 SNI（IP 直连/显式 ''）即 servername=false（非 null，
    // 更不回灌 host）；显式名原样；setServername 非串挂 Received 后缀、服务端禁签发。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "s.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = tls.createServer({{ key, cert }}, (sock) => {{
  sock.on("data", () => sock.end());
  sock.on("error", () => {{}});
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  // 正常：显式 servername 客户端自见原样。
  const a = tls.connect({{ port, host: "127.0.0.1", servername: "example.com", rejectUnauthorized: false }}, () => {{
    console.log("cli-sni", JSON.stringify(a.servername));
    a.destroy();
    // 边界：IP 直连无 SNI → false。
    const b = tls.connect({{ port, host: "127.0.0.1", rejectUnauthorized: false }}, () => {{
      console.log("cli-nosni", JSON.stringify(b.servername));
      b.destroy();
      // 边界：显式 '' 即缺席 → false。
      const c = tls.connect({{ port, host: "127.0.0.1", servername: "", rejectUnauthorized: false }}, () => {{
        console.log("cli-empty", JSON.stringify(c.servername));
        c.destroy(); server.close();
      }});
      c.on("error", () => {{}});
    }});
    b.on("error", () => {{}});
  }});
  a.on("error", () => {{}});
  // 报错：setServername 非串挂 Received 后缀；服务端禁签发。
  const t = (f) => {{ try {{ f(); return "NO-THROW"; }} catch (e) {{ return e.code + "|" + e.message; }} }};
  console.log("set-undef", t(() => a.setServername(undefined)));
  console.log("set-num", t(() => a.setServername(1)));
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 2500);
"#
        ),
    );
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.contains(&"cli-sni \"example.com\""), "out: {out}");
    assert!(lines.contains(&"cli-nosni false"), "out: {out}");
    assert!(lines.contains(&"cli-empty false"), "out: {out}");
    assert!(out.contains("set-undef ERR_INVALID_ARG_TYPE|The \"name\" argument must be of type string. Received undefined"), "out: {out}");
    assert!(out.contains("set-num ERR_INVALID_ARG_TYPE|The \"name\" argument must be of type string. Received type number (1)"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn tls_verify_verdict_precedes_flush() {
    // node onConnectSecure 口径：裁决（捕获错/身份校验）先于应用字节上网；
    // 须销毁即预连写随 socket 弃置——服务端零字节（checkServerIdentity 套件幻影请求根因）。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "v.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
let srvBytes = 0;
const server = tls.createServer({{ key, cert }}, (sock) => {{
  sock.on("data", (c) => {{ srvBytes += c.length; sock.write("echo:" + c); }});
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  // 报错：自签无 ca → 链错硬失败，服务端零字节（预连写不上网）。
  const step1 = () => {{
    const c = tls.connect({{ port, host: "127.0.0.1" }});
    c.write("nope"); // 预连写：裁决失败即随 socket 弃置
    c.on("data", () => {{}});
    c.on("error", (e) => {{
      console.log("untrusted", e.code, "srvbytes", srvBytes);
      c.destroy();
      step2();
    }});
  }};
  // 边界：名错 + 自定义 CSI 放行 → authorized 且数据通。
  const step2 = () => {{
    const c = tls.connect({{ port, host: "127.0.0.1", ca: cert, servername: "mismatch.invalid",
      checkServerIdentity: () => undefined }}, () => {{
      console.log("csi-pass", c.authorized, c.authorizationError);
      c.write("hi");
    }});
    c.on("data", (d) => {{ console.log("echo", String(d), "srvbytes", srvBytes); c.destroy(); step3(); }});
    c.on("error", (e) => console.log("unexpected", e.code));
  }};
  // 报错：名错 + 自定义 CSI 落错 → error，服务端无新增字节。
  const step3 = () => {{
    const before = srvBytes;
    const c = tls.connect({{ port, host: "127.0.0.1", ca: cert, servername: "mismatch.invalid",
      checkServerIdentity: () => new Error("nope") }});
    c.write("nope2"); // 预连写：裁决失败即随 socket 弃置
    c.on("data", () => {{}});
    c.on("error", (e) => {{
      console.log("csi-fail", e.message, c.authorized, "srvbytes", srvBytes, "delta", srvBytes - before);
      c.destroy();
      server.close();
    }});
  }};
  step1();
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 2500);
"#
        ),
    );
    assert!(out.contains("untrusted DEPTH_ZERO_SELF_SIGNED_CERT srvbytes 0"), "out: {out}");
    assert!(out.contains("csi-pass true null"), "out: {out}");
    assert!(out.contains("echo echo:hi srvbytes 2"), "out: {out}");
    assert!(out.contains("csi-fail nope false srvbytes 2 delta 0"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

/// rcgen CA 双套 + 无 SAN 终端证书（CN=leaf.test，node agent1 形）+ 自签证书（hermetic）。
fn write_ca_no_san_leaf(dir: &assert_fs::TempDir) -> (String, String, String, String, String, String) {
    fn mk_ca(cn: &str) -> rcgen::CertifiedIssuer<'static, rcgen::KeyPair> {
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        let k = rcgen::KeyPair::generate().unwrap();
        rcgen::CertifiedIssuer::self_signed(p, k).unwrap()
    }
    fn put(dir: &assert_fs::TempDir, name: &str, content: &str) -> String {
        let p = dir.path().join(name);
        std::fs::write(&p, content).unwrap();
        p.to_string_lossy().into_owned()
    }
    let ca = mk_ca("wjs-verify-ca");
    let ca2 = mk_ca("wjs-verify-ca2");
    let mut leaf_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    leaf_params.distinguished_name.push(rcgen::DnType::CommonName, "leaf.test");
    let leaf_key = rcgen::KeyPair::generate().unwrap();
    let leaf_key_pem = leaf_key.serialize_pem();
    let leaf = rcgen::CertifiedIssuer::signed_by(leaf_params, leaf_key, &ca).unwrap();
    let ss = rcgen::generate_simple_self_signed(vec!["leaf.test".to_string()]).unwrap();
    (
        put(dir, "ca.pem", &ca.pem()),
        put(dir, "ca2.pem", &ca2.pem()),
        put(dir, "leaf-key.pem", &leaf_key_pem),
        put(dir, "leaf-cert.pem", &leaf.pem()),
        put(dir, "ss-key.pem", &ss.signing_key.serialize_pem()),
        put(dir, "ss-cert.pem", &ss.cert.pem()),
    )
}

#[test]
fn tls_cert_verify_names_and_self_signed() {
    // node 校验三码（真机 test-https-strict 全断言 + strict-probe QA/QB/QC2 对拍）：
    // v3 无 SAN 证书 CN 回落（对上放行/错位 ALTNAME）、错 CA 链错 UNABLE、
    // 自签 depth-0 终结 DEPTH_ZERO（名对错/存活硬失败皆此码）。
    let dir = assert_fs::TempDir::new().unwrap();
    let (ca_p, ca2_p, lk_p, lc_p, sk_p, sc_p) = write_ca_no_san_leaf(&dir);
    let out = run_fs_file(
        &dir,
        "v.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const ca = fs.readFileSync("{ca_p}", "utf8");
const ca2 = fs.readFileSync("{ca2_p}", "utf8");
const leafKey = fs.readFileSync("{lk_p}", "utf8");
const leafCert = fs.readFileSync("{lc_p}", "utf8");
const ssKey = fs.readFileSync("{sk_p}", "utf8");
const ssCert = fs.readFileSync("{sc_p}", "utf8");
const srv = tls.createServer({{ key: leafKey, cert: leafCert }}, (s) => {{
  s.on("data", (c) => s.write("ok:" + c));
  s.on("error", () => {{}});
}});
srv.listen(0, "127.0.0.1", () => {{
  const port = srv.address().port;
  // 正常：无 SAN + CN 对上 → 授权通过（真机 val1 形）。
  const a = tls.connect({{ port, host: "127.0.0.1", servername: "leaf.test", ca, rejectUnauthorized: false }}, () => {{
    console.log("match", a.authorized, a.authorizationError);
    a.end();
  }});
  a.on("error", () => {{}});
  a.on("close", () => {{
    // 报错：名错位 → ALTNAME（连接存活，reject=false；真机 inv1 形）。
    const b = tls.connect({{ port, host: "127.0.0.1", servername: "other.test", ca, rejectUnauthorized: false }}, () => {{
      console.log("mismatch", b.authorized, b.authorizationError);
      b.end();
    }});
    b.on("error", () => {{}});
    b.on("close", () => {{
      // 报错：错 CA → UNABLE（真机 inv1-ca2 形）。
      const c = tls.connect({{ port, host: "127.0.0.1", servername: "leaf.test", ca: ca2, rejectUnauthorized: false }}, () => {{
        console.log("wrongca", c.authorized, c.authorizationError);
        c.end();
      }});
      c.on("error", () => {{}});
      c.on("close", () => {{
        // 报错：缺省 reject + 名错位 → 硬失败 ALTNAME（真机 inv1-ca1 硬失败形）。
        const d = tls.connect({{ port, host: "127.0.0.1", servername: "other.test", ca }});
        d.on("error", (e) => {{
          console.log("strict-name", e.code);
          stepSelfSigned();
        }});
      }});
    }});
  }});
}});
const stepSelfSigned = () => {{
  const srv2 = tls.createServer({{ key: ssKey, cert: ssCert }}, (s) => {{
    s.on("error", () => {{}});
    s.write("hi");
  }});
  srv2.listen(0, "127.0.0.1", () => {{
    const port2 = srv2.address().port;
    // 正常/报错：自签 + reject=false → 存活但 DEPTH_ZERO（真机 QC2）。
    const e = tls.connect({{ port: port2, host: "127.0.0.1", servername: "leaf.test", ca, rejectUnauthorized: false }}, () => {{
      console.log("selfsigned-soft", e.authorized, e.authorizationError);
      e.end();
    }});
    e.on("error", () => {{}});
    e.on("close", () => {{
      // 报错：缺省 reject → 硬失败 DEPTH_ZERO（真机 QB）。
      const f = tls.connect({{ port: port2, host: "127.0.0.1", servername: "leaf.test", ca }});
      f.on("error", (er) => {{
        console.log("selfsigned-hard", er.code);
        srv.close();
        srv2.close();
      }});
    }});
  }});
  srv2.on("close", () => console.log("srv2-close"));
}};
srv.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 2500);
"#
        ),
    );
    for line in [
        "match true null",
        "mismatch false ERR_TLS_CERT_ALTNAME_INVALID",
        "wrongca false UNABLE_TO_VERIFY_LEAF_SIGNATURE",
        "strict-name ERR_TLS_CERT_ALTNAME_INVALID",
        "selfsigned-soft false DEPTH_ZERO_SELF_SIGNED_CERT",
        "selfsigned-hard DEPTH_ZERO_SELF_SIGNED_CERT",
        "srv-close",
        "srv2-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn tls_check_server_identity_unfqdn_and_wildcards() {
    // node lib/tls.js checkServerIdentity 原文移植面（unfqdn/RFC6125/IP 规范化）——
    // 正常：尾点 CN 双向放行、部分通配（*b.a.com）、IPv6 展开形等价、CN 数组；
    // 报错：双层通配解析（c.b.a.com vs *.a.com）、裸 TLD 通配、xn-- 逐字、空标签/坏字符；
    // 边界：IDN 全角点分词、URI-only 无 DNS 名、非法 v4 SAN 空表、主机尾点消息归一。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "i.mjs",
        r#"
import tls from "node:tls";
const t = (host, cert) => {
  const e = tls.checkServerIdentity(host, cert);
  return e === undefined ? "pass" : e.reason;
};
const rows = [
  // 正常：CN 尾点双向等价。
  ["a.com", { subject: { CN: "a.com." } }, "pass"],
  ["a.com.", { subject: { CN: "a.com" } }, "pass"],
  ["a.com.", { subject: { CN: "a.com." } }, "pass"],
  // 正常：部分通配（非首段 * 形，node 逐字保留）。
  ["b.a.com", { subjectaltname: "DNS:*b.a.com", subject: {} }, "pass"],
  ["a-cb.a.com", { subjectaltname: "DNS:*b.a.com", subject: {} }, "pass"],
  ["b.a.com", { subject: { CN: "*.a.com" } }, "pass"],
  // 正常：IPv6 展开大写等价 + CN 数组。
  ["2001:db8::1", { subject: {}, subjectaltname: "IP Address:2001:DB8:0:0:0:0:0:1" }, "pass"],
  ["foo.com", { subject: { CN: ["foo.com", "bar.com"] } }, "pass"],
  // 报错：跨层/裸 TLD/xn-- 内嵌通配/空标签/坏字符。
  ["c.b.a.com", { subjectaltname: "DNS:*.a.com", subject: {} }, "Host: c.b.a.com. is not in the cert's altnames: DNS:*.a.com"],
  ["a.com", { subjectaltname: "DNS:*.com", subject: { CN: "b.com" } }, "Host: a.com. is not in the cert's altnames: DNS:*.com"],
  ["xn--bcher-kva.example.com", { subject: { CN: "xn--*.example.com" } }, "Host: xn--bcher-kva.example.com. is not cert's CN: xn--*.example.com"],
  ["bad.x.example.com", { subject: { CN: "bad..example.com" } }, "Host: bad.x.example.com. is not cert's CN: bad..example.com"],
  ["x.example.com", { subjectaltname: "DNS:bad label.com", subject: {} }, "Host: x.example.com. is not in the cert's altnames: DNS:bad label.com"],
  // 报错：IDN 全角点按分隔符分词（4 段 vs 3 段失配）。
  ["foo。bar.example.com", { subjectaltname: "DNS:*.example.com", subject: {} }, "Host: foo。bar.example.com. is not in the cert's altnames: DNS:*.example.com"],
  // 边界：URI-only 即无 DNS 名；非法 v4 SAN 规范化为空表；主机尾点消息归一。
  ["a.b.a.com", { subjectaltname: "URI:http://a.b.a.com/", subject: {} }, "Cert does not contain a DNS name"],
  ["8.8.8.8", { subject: { CN: "8.8.8.8" }, subjectaltname: "IP Address:8.8.08.8" }, "IP: 8.8.8.8 is not in the cert's list: "],
  ["a.com.", { subject: { CN: "b.com" } }, "Host: a.com. is not cert's CN: b.com"],
];
let bad = 0;
for (const [h, c, want] of rows) {
  const got = t(h, c);
  if (got !== want) { console.log("MISMATCH", JSON.stringify(h), "got", JSON.stringify(got), "want", JSON.stringify(want)); bad++; }
}
console.log(bad === 0 ? "ident-ok" : "ident-bad " + bad);
"#,
    );
    assert!(out.lines().any(|l| l == "ident-ok"), "out: {out}");
    dir.close().unwrap();
}

/// rcgen 三级链（root→mid→leaf，CN 分别 wjs-root/mid/leaf；leaf SAN leaf.test）。
fn write_three_level_chain(dir: &assert_fs::TempDir) -> (String, String, String, String) {
    fn mk_ca(cn: &str) -> rcgen::CertifiedIssuer<'static, rcgen::KeyPair> {
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        let k = rcgen::KeyPair::generate().unwrap();
        rcgen::CertifiedIssuer::self_signed(p, k).unwrap()
    }
    fn put(dir: &assert_fs::TempDir, name: &str, content: &str) -> String {
        let p = dir.path().join(name);
        std::fs::write(&p, content).unwrap();
        p.to_string_lossy().into_owned()
    }
    let root = mk_ca("wjs-root");
    let mut mid_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    mid_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    mid_params.distinguished_name.push(rcgen::DnType::CommonName, "wjs-mid");
    let mid = rcgen::CertifiedIssuer::signed_by(
        mid_params,
        rcgen::KeyPair::generate().unwrap(),
        &root,
    )
    .unwrap();
    let mut leaf_params = rcgen::CertificateParams::new(vec!["leaf.test".to_string()]).unwrap();
    leaf_params.distinguished_name.push(rcgen::DnType::CommonName, "wjs-leaf");
    let leaf_key = rcgen::KeyPair::generate().unwrap();
    let leaf_key_pem = leaf_key.serialize_pem();
    let leaf = rcgen::CertifiedIssuer::signed_by(leaf_params, leaf_key, &mid).unwrap();
    (
        put(dir, "root.pem", &root.pem()),
        put(dir, "mid.pem", &mid.pem()),
        put(dir, "leaf-key.pem", &leaf_key_pem),
        put(dir, "leaf-cert.pem", &leaf.pem()),
    )
}

#[test]
fn tls_server_chain_extra_and_anchor() {
    // node 服务端出示链口径（cert + ca 作 extra chain）+ getPeerCertificate(true) 信任锚补齐：
    // 正常：三级链授信存活，issuerCertificate 逐级链接、根自链；
    // 边界：无 ca 出示即末级自链（单证书形不崩）。
    let dir = assert_fs::TempDir::new().unwrap();
    let (root_p, mid_p, lk_p, lc_p) = write_three_level_chain(&dir);
    let out = run_fs_file(
        &dir,
        "c.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const root = fs.readFileSync("{root_p}", "utf8");
const mid = fs.readFileSync("{mid_p}", "utf8");
const leafKey = fs.readFileSync("{lk_p}", "utf8");
const leafCert = fs.readFileSync("{lc_p}", "utf8");
const srv = tls.createServer({{ key: leafKey, cert: leafCert, ca: mid }}, (s) => {{
  s.on("data", (c) => s.write("ok:" + c));
  s.on("error", () => {{}});
}});
srv.listen(0, "127.0.0.1", () => {{
  const port = srv.address().port;
  // 正常：三级链完整，授权通过且锚链三级。
  const a = tls.connect({{ port, host: "127.0.0.1", servername: "leaf.test", ca: root }}, () => {{
    console.log("auth", a.authorized);
    const full = a.getPeerCertificate(true);
    const m = full.issuerCertificate, r = m.issuerCertificate;
    console.log("leaf", full.subject.CN);
    console.log("mid", m.subject.CN);
    console.log("root", r.subject.CN, r.issuerCertificate === r);
    console.log("flat", a.getPeerCertificate().subject.CN);
    a.end();
  }});
  a.on("error", (e) => console.log("unexpected", e.code));
  a.on("close", () => {{
    // 边界：服务端无 ca 出示（单证书）：客户端 ca 仍可补锚，不崩。
    const srv2 = tls.createServer({{ key: leafKey, cert: leafCert }}, (s) => s.on("error", () => {{}}));
    srv2.listen(0, "127.0.0.1", () => {{
      const b = tls.connect({{ port: srv2.address().port, host: "127.0.0.1", servername: "leaf.test",
        ca: root + "\n" + mid, rejectUnauthorized: false }}, () => {{
        const f = b.getPeerCertificate(true);
        console.log("solo", f.subject.CN, f.issuerCertificate.subject.CN);
        b.end();
        srv.close();
        srv2.close();
      }});
      b.on("error", () => {{}});
    }});
  }});
}});
srv.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 2500);
"#
        ),
    );
    for line in [
        "auth true",
        "leaf wjs-leaf",
        "mid wjs-mid",
        "root wjs-root true",
        "flat wjs-leaf",
        "solo wjs-leaf wjs-mid",
        "srv-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.contains("unexpected"), "out: {out}");
    dir.close().unwrap();
}
