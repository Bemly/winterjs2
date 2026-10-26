//! tests/node/tls/identity.rs — tls identity 面（对齐 src/builtins/node/tls.rs；§0.9 按面分块）。

use crate::helpers::*;
use assert_fs::prelude::*;


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

