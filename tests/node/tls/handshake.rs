//! tests/node/tls/handshake.rs — tls handshake 面（对齐 src/builtins/node/tls.rs；§0.9 按面分块）。

use crate::helpers::*;
use assert_fs::prelude::*;


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



#[test]
fn tls_connect_family_honored() {
    // node lib/net.js onlookup 口径：family 指定即先 lookup 定向再建连；
    // 正常：127.0.0.1 + family:4 经 lookup 直达，remoteAddress 为 IPv4；
    // 边界：字面 IP + family 不走 lookup（同步直连不断言 lookup 形）。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "f.mjs",
        &format!(
            r#"
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = tls.createServer({{ key, cert }});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const a = tls.connect({{ port, host: "127.0.0.1", family: 4, rejectUnauthorized: false }}, () => {{
    console.log("v4", a.remoteAddress, a.remoteFamily);
    const b = tls.connect({{ port, host: "localhost", family: 4, rejectUnauthorized: false }}, () => {{
      console.log("lname", b.remoteAddress);
      b.end();
      a.end();
      server.close();
    }});
    b.on("error", (e) => console.log("unexpected", e.code));
  }});
  a.on("error", (e) => console.log("unexpected", e.code));
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 2500);
"#
        ),
    );
    assert!(out.contains("v4 127.0.0.1 IPv4"), "out: {out}");
    assert!(out.contains("lname 127.0.0.1"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    assert!(!out.contains("unexpected"), "out: {out}");
    dir.close().unwrap();
}
