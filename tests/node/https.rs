//! tests/node/https.rs — 对齐 src/builtins/node/https.rs（node:https）。

use crate::helpers::*;

#[test]
fn https_loopback() {
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "p.mjs",
        &format!(
            r#"
import https from "node:https";
import http from "node:http";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = https.createServer({{ key, cert }}, (req, res) => {{
  let b = "";
  req.on("data", (c) => (b += c));
  req.on("end", () => res.end("secure:" + req.method + ":" + b));
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const r = https.request(
    {{ port, host: "127.0.0.1", path: "/s", method: "POST", ca: cert }},
    (res) => {{
      let b = "";
      res.on("data", (c) => (b += c));
      res.on("end", () => {{
        console.log("post", res.statusCode, b);
        https.get(`https://127.0.0.1:${{port}}/g?x=1`, {{ ca: cert }}, (res2) => {{
          let b2 = "";
          res2.on("data", (c) => (b2 += c));
          res2.on("end", () => {{
            console.log("get", res2.statusCode, b2);
            server.close();
          }});
        }}).on("error", () => {{}});
      }});
    }}
  );
  r.on("error", () => {{}});
  r.end("secret");
  // 10f G3：协议错已错误码化（message 为 node 原文），断言改走 e.code（§4.36）。
  try {{ http.get("https://x/"); }} catch (e) {{ console.log("proto", e.code, e.name); }}
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    assert!(out.contains("proto ERR_INVALID_PROTOCOL TypeError"), "out: {out}");
    assert!(out.contains("post 200 secure:POST:secret"), "out: {out}");
    assert!(out.contains("get 200 secure:GET:"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn https_keepalive_reuse() {
    // 10b：https 随行（同帧层 + tls 底座）——2 请求 1 连接。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "k.mjs",
        &format!(
            r#"
import https, {{ Agent }} from "node:https";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
let conns = 0;
const server = https.createServer({{ key, cert }}, (req, res) => {{
  req.on("data", () => {{}});
  req.on("end", () => res.end("s-ok"));
}});
server.on("connection", () => {{ conns++; }});
server.listen(0, "127.0.0.1", async () => {{
  const port = server.address().port;
  const agent = new Agent({{ keepAlive: true }});
  const one = () => new Promise((resolve, reject) => {{
    const r = https.get({{ port, host: "127.0.0.1", path: "/", ca: cert, agent }}, (res) => {{
      let b = "";
      res.on("data", (c) => (b += c));
      res.on("end", () => resolve(b + ":" + r.reusedSocket));
    }});
    r.on("error", reject);
  }});
  console.log("first", await one());
  console.log("second", await one());
  console.log("conns", conns);
  agent.destroy();
  server.close();
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    for line in [
        "first s-ok:false",
        "second s-ok:true",
        "conns 1",
        "srv-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn https_sethost_false_no_host() {
    // setHost:false/0/null 即禁自动 Host（node internal/http/client.js 420 行）；
    // 无 Host 的 1.1 请求服务端 requireHostHeader 400，不派发 request。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "s.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
let seen = 0;
const server = https.createServer({{ key, cert }}, (req, res) => {{
  seen++;
  res.end("host:" + req.headers.host);
}});
server.listen(0, "127.0.0.1", async () => {{
  const port = server.address().port;
  const one = (opts) => new Promise((resolve) => {{
    const r = https.get({{ port, host: "127.0.0.1", ca: cert, ...opts }}, (res) => {{
      let b = "";
      res.on("data", (c) => (b += c));
      res.on("end", () => resolve(res.statusCode + ":" + b));
    }});
    r.on("error", (e) => resolve("err:" + e.code));
  }});
  const d = await one({{}});
  const f = await one({{ setHost: false }});
  const z = await one({{ setHost: 0 }});
  const n = await one({{ setHost: null }});
  console.log("default", d);
  console.log("false", f);
  console.log("zero", z);
  console.log("null", n);
  console.log("seen", seen);
  server.close();
}});
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    assert!(
        out.lines().any(|l| l.starts_with("default 200:host:127.0.0.1")),
        "missing default line\nout: {out}"
    );
    for line in ["false 400:", "zero 400:", "null 400:", "seen 1", "end-ok"] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn https_reject_unauthorized_env_default() {
    // NODE_TLS_REJECT_UNAUTHORIZED=0 即 rejectUnauthorized 缺省 false
    //（node internal/options.js + tls/wrap.js）；自签名无 ca 照通，告警一次。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "e.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
process.env.NODE_TLS_REJECT_UNAUTHORIZED = "0";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = https.createServer({{ key, cert }}, (req, res) => res.end("insecure-ok"));
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const r = https.get({{ port, host: "127.0.0.1", path: "/" }}, (res) => {{
    let b = "";
    res.on("data", (c) => (b += c));
    res.on("end", () => {{
      console.log("status", res.statusCode + ":" + b);
      server.close();
    }});
  }});
  r.on("error", (e) => console.log("err", e.code));
}});
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    assert!(out.lines().any(|l| l == "status 200:insecure-ok"), "out: {out}");
    assert!(out.lines().any(|l| l == "end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn https_host_header_servername() {
    // servername 归一（node internal/http/agent.js normalizeServerName）：
    // Host 头进 TLS 校验名；IP 头不隐式 SNI（回落连接 host）。
    // 自签 SAN=127.0.0.1：host 头 127.0.0.1 即验过，example.com 即错名。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "n.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = https.createServer({{ key, cert }}, (req, res) => res.end("ok"));
server.listen(0, "127.0.0.1", async () => {{
  const port = server.address().port;
  const one = (headers) => new Promise((resolve) => {{
    const r = https.get(
      {{ port, host: "127.0.0.1", ca: cert, rejectUnauthorized: false, headers }},
      (res) => {{
        res.resume();
        res.on("end", () => resolve(
          "auth:" + res.connection.authorizationError + " sni:" + (r.socket && r.socket.servername)
        ));
      }}
    );
    r.on("error", (e) => resolve("err:" + e.code));
  }});
  console.log("loop", await one({{ host: "127.0.0.1" }}));
  console.log("name", await one({{ host: "example.com" }}));
  server.close();
}});
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    // name 半段：servername 已转发（sni:example.com 与真机一致）；
    // 授权码系 tls  lane 映射事项（v3 无 SAN 名错须 ERR_TLS_CERT_ALTNAME_INVALID，
    // 另案记档），此处不断言 auth。
    assert!(
        out.lines().any(|l| l.starts_with("loop auth:null")),
        "missing loop line\nout: {out}"
    );
    assert!(out.lines().any(|l| l == "end-ok"), "out: {out}");
    assert!(
        out.lines().any(|l| l.starts_with("name auth:") && l.ends_with("sni:example.com")),
        "missing name sni line\nout: {out}"
    );
    dir.close().unwrap();
}

#[test]
fn https_max_requests_drop_request() {
    // 超 maxRequestsPerSocket 的管线请求走正常管线回 503（首响 200 先行，
    // dropRequest(request, socket) 派发一次；node internal/http/server.js）。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "d.mjs",
        &format!(
            r#"
import https from "node:https";
import tls from "node:tls";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
let drops = 0;
const server = https.createServer({{ key, cert }}, (req, res) => res.end("ok"));
server.on("dropRequest", () => {{ drops++; }});
server.maxRequestsPerSocket = 1;
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const s = tls.connect({{ port, host: "127.0.0.1", rejectUnauthorized: false }}, () => {{
    let buf = "";
    s.setEncoding("utf8");
    s.on("data", (c) => {{ buf += c; }});
    s.on("close", () => {{
      const i200 = buf.indexOf("HTTP/1.1 200 OK");
      const i503 = buf.indexOf("HTTP/1.1 503 Service Unavailable");
      console.log("order", i200, i503);
      console.log("drops", drops);
      server.close();
    }});
    s.write("GET /a HTTP/1.1\r\nHost: x\r\nConnection: keep-alive\r\n\r\n");
    s.write("GET /b HTTP/1.1\r\nHost: x\r\nConnection: keep-alive\r\n\r\n");
  }});
}});
setTimeout(() => console.log("end-ok"), 2000);
"#
        ),
    );
    assert!(
        out.lines().any(|l| {{
            let rest = l.strip_prefix("order ").unwrap_or("-");
            let mut it = rest.split_whitespace().filter_map(|x| x.parse::<i64>().ok());
            matches!((it.next(), it.next()), (Some(a), Some(b)) if a >= 0 && b > a)
        }}),
        "no 200-then-503 order\nout: {out}"
    );
    assert!(out.lines().any(|l| l == "drops 1"), "out: {out}");
    assert!(out.lines().any(|l| l == "end-ok"), "out: {out}");
    dir.close().unwrap();
}
