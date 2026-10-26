//! tests/node/tls/sni.rs — tls sni 面（对齐 src/builtins/node/tls.rs；§0.9 按面分块）。

use crate::helpers::*;
use assert_fs::prelude::*;


#[test]
fn tls_snicallback_option_validation() {
    // node wrap.js:1030-1041 口径：服务端 + 显式 SNICallback 即校验函数形；
    // 正常：函数形双路（createServer/TLSSocket）皆收；
    // 报错：四件非函数（'fhqwhgads'/42/{}/[]) 双路皆抛 ERR_INVALID_ARG_TYPE；
    // 边界：客户端形缺席即静默忽略（不抛）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "v.mjs",
        r#"
import tls from "node:tls";
import net from "node:net";
const bad = ["fhqwhgads", 42, {}, []];
for (const v of bad) {
  try { tls.createServer({ SNICallback: v }); console.log("srv-nothrow", JSON.stringify(v)); }
  catch (e) { console.log("srv-throw", e.code, JSON.stringify(v)); }
  try { new tls.TLSSocket(new net.Socket(), { isServer: true, SNICallback: v }); console.log("sock-nothrow", JSON.stringify(v)); }
  catch (e) { console.log("sock-throw", e.code, JSON.stringify(v)); }
}
// 正常：函数形双路皆收。
const fn1 = (name, cb) => cb(null, null);
tls.createServer({ SNICallback: fn1 });
new tls.TLSSocket(new net.Socket(), { isServer: true, SNICallback: fn1 });
console.log("fn-ok");
// 边界：客户端形 SNICallback 缺席即忽略。
new tls.TLSSocket(new net.Socket(), { SNICallback: 42 });
console.log("cli-ok");
setTimeout(() => console.log("end-ok"), 500);
"#,
    );
    for v in ["\"fhqwhgads\"", "42", "{}", "[]"] {
        assert!(out.contains(&format!("srv-throw ERR_INVALID_ARG_TYPE {v}")), "out: {out}");
        assert!(out.contains(&format!("sock-throw ERR_INVALID_ARG_TYPE {v}")), "out: {out}");
    }
    assert!(out.contains("fn-ok"), "out: {out}");
    assert!(out.contains("cli-ok"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}


/// SNI 证书选择（addContext 下沉；node wrap.js:1697 默认 SNICallback 口径）：
/// 正常：精确命中/通配命中/未命中回落缺省（服务端 getCertificate 同步跟随选中证）；
/// 报错：空名 ERR_TLS_REQUIRED_SERVER_NAME、非串 TypeError；
/// 边界：listen 后追加即时生效（不重启；usage-order 的可达子集——requestCert 面另案）。
fn write_sni_cert(dir: &assert_fs::TempDir, file: &str, san: &str, cn: &str) -> (String, String) {
    let mut params = rcgen::CertificateParams::new(vec![san.to_string()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, cn.to_string());
    let kp = rcgen::KeyPair::generate().unwrap();
    let cert_pem = params.self_signed(&kp).unwrap().pem();
    let key_pem = kp.serialize_pem();
    let cert_path = dir.child(format!("{file}-cert.pem"));
    cert_path.write_str(&cert_pem).unwrap();
    let key_path = dir.child(format!("{file}-key.pem"));
    key_path.write_str(&key_pem).unwrap();
    (
        cert_path.path().to_string_lossy().into_owned(),
        key_path.path().to_string_lossy().into_owned(),
    )
}

#[test]
fn tls_sni_context_selects_certificate() {
    let dir = assert_fs::TempDir::new().unwrap();
    let (d_cert, d_key) = write_sni_cert(&dir, "d", "sni-default.test", "sni-default");
    let (a_cert, a_key) = write_sni_cert(&dir, "a", "a.example.com", "sni-a");
    let out = run_fs_file(
        &dir,
        "s.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const dKey = fs.readFileSync({d_key:?}, "utf8");
const dCert = fs.readFileSync({d_cert:?}, "utf8");
const aKey = fs.readFileSync({a_key:?}, "utf8");
const aCert = fs.readFileSync({a_cert:?}, "utf8");
// 报错：空名 / 非串。
try {{ tls.createServer({{}}).addContext("", {{ key: aKey, cert: aCert }}); console.log("empty-nothrow"); }}
catch (e) {{ console.log("empty-throw", e.code); }}
try {{ tls.createServer({{}}).addContext(42, {{ key: aKey, cert: aCert }}); console.log("num-nothrow"); }}
catch (e) {{ console.log("num-throw", e.constructor.name); }}
const server = tls.createServer({{ key: dKey, cert: dCert }}, (s) => {{
  s.on("data", (c) => s.write("ok:" + c));
  s.on("error", () => {{}});
}});
server.addContext("a.example.com", {{ key: aKey, cert: aCert }});
server.addContext("*.test.com", tls.createSecureContext({{ key: aKey, cert: aCert }}));
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const dial = (servername, ca, label, done) => {{
    const c = tls.connect({{ port, host: "127.0.0.1", servername, ca, rejectUnauthorized: false }}, () => {{
      const peer = c.getPeerCertificate();
      console.log(label, "peer", peer.subject.CN, peer.subjectaltname);
      c.end();
    }});
    c.on("error", (e) => console.log(label, "unexpected", e.code));
    c.on("close", done);
  }};
  // 正常：精确命中 a（ca 用 a 链自授信，授权通过）。
  dial("a.example.com", aCert, "exact", () => {{
    // 边界：listen 后追加即时生效。
    server.addContext("late.example.com", {{ key: aKey, cert: aCert }});
    dial("late.example.com", aCert, "late", () => {{
      // 正常：未命中回落缺省 d。
      dial("unknown.example.com", dCert, "fallback", () => {{
        server.close();
      }});
    }});
  }});
}});
server.on("secureConnection", (s) => console.log("srv-cert", s.getCertificate().subject.CN));
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 3000);
"#
        ),
    );
    for line in [
        "empty-throw ERR_TLS_REQUIRED_SERVER_NAME",
        "num-throw TypeError",
        "srv-cert sni-a",
        "srv-cert sni-default",
        "srv-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    for frag in [
        "exact peer sni-a DNS:a.example.com",
        "late peer sni-a DNS:a.example.com",
        "fallback peer sni-default DNS:sni-default.test",
    ] {
        assert!(out.contains(frag), "missing {frag}; out: {out}");
    }
    assert!(!out.contains("unexpected"), "out: {out}");
    assert!(!out.contains("nothrow"), "out: {out}");
    dir.close().unwrap();
}

