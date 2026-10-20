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
fn https_server_options_type_gate() {
    // N1：new Server/createServer 首参类型门（node https.js validateObject
    // 口径，mirror http.rs __serverOptions）：'foo'/42/true/[]（含数组）一律
    // ERR_INVALID_ARG_TYPE('options')；null/undefined 缺省、函数=监听器。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "g.mjs",
        r#"
import https from "node:https";
const bad = ["foo", 42, true, []];
for (const v of bad) {
  for (const [tag, fn] of [["new", (o) => new https.Server(o)], ["create", (o) => https.createServer(o)]]) {
    try { fn(v); console.log(tag, JSON.stringify(v), "NO-THROW"); }
    catch (e) { console.log(tag, JSON.stringify(v), e.code); }
  }
}
for (const [tag, v] of [["null", null], ["undef", undefined]]) {
  try { https.createServer(v); console.log("create", tag, "ok"); }
  catch (e) { console.log("create", tag, e.code); }
}
try { https.createServer((req, res) => res.end("x")); console.log("create fn ok"); }
catch (e) { console.log("create fn", e.code); }
setTimeout(() => console.log("end-ok"), 500);
"#,
    );
    for v in [r#""foo""#, "42", "true", "[]"] {
        for tag in ["new", "create"] {
            assert!(
                out.lines().any(|l| l == format!("{tag} {v} ERR_INVALID_ARG_TYPE")),
                "missing gate line: {tag} {v}\nout: {out}"
            );
        }
    }
    for line in ["create null ok", "create undef ok", "create fn ok", "end-ok"] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
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

#[test]
fn https_unix_socket_roundtrip() {
    // N2：socketPath→tls.connect path（双 openSocket 钩子）：agent:false 直连
    // 与显式 Agent 经 addRequest/createSocket 两路，UDS 上皆 200。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let sock_path = dir.path().join("t-https.sock");
    let out = run_fs_file(
        &dir,
        "u.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = https.createServer({{ key, cert }}, (req, res) => res.end("uds-ok"));
server.listen({sock_path:?}, () => {{
  const one = (opts) => new Promise((resolve) => {{
    const r = https.get(opts, (res) => {{
      let b = "";
      res.on("data", (c) => (b += c));
      res.on("end", () => resolve(res.statusCode + ":" + b));
    }});
    r.on("error", (e) => resolve("err:" + e.code));
  }});
  (async () => {{
    console.log("direct", await one({{ socketPath: {sock_path:?}, rejectUnauthorized: false }}));
    const agent = new https.Agent({{ keepAlive: true }});
    console.log("agent", await one({{ socketPath: {sock_path:?}, rejectUnauthorized: false, agent }}));
    agent.destroy();
    server.close();
  }})();
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 3000);
"#
        ),
    );
    for line in ["direct 200:uds-ok", "agent 200:uds-ok", "srv-close", "end-ok"] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn https_global_agent_reassignment() {
    // N2b：https.globalAgent 重赋后请求走新 agent（FLAVOR 活读；node
    // test-https-client-override-global-agent 口径：agent.getName({port})
    // 键同步落新 agent.sockets）。读全局态——用后复位。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "o.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
process.env.NODE_TLS_REJECT_UNAUTHORIZED = "0";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const orig = https.globalAgent;
const server = https.createServer({{ key, cert }}, (req, res) => {{
  res.writeHead(200);
  res.end("Hello, World!");
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const agent = new https.Agent();
  const name = agent.getName({{ port }});
  https.globalAgent = agent;
  console.log("swapped", https.globalAgent === agent);
  const req = https.get({{ port }});
  req.on("error", (e) => console.log("err", e.code));
  console.log("pooled", name in agent.sockets);
  req.on("close", () => {{
    https.globalAgent = orig;
    console.log("restored", https.globalAgent === orig);
    server.close();
  }});
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 3000);
"#
        ),
    );
    for line in [
        "swapped true",
        "pooled true",
        "restored true",
        "srv-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn https_concurrent_write_end_race() {
    // (4)：并发 raw 连接同拍 write+end——服务端 FIN 不得抢跑响应落盘
    //（net 自动回 FIN 曾致 3/4 空体；现 allowHalfOpen 收链 + end 收口）。
    // 经 agent.createConnection（含位置参变体）直验 node 套件形态。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "r.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = https.createServer({{ key, cert }}, (req, res) => res.end("hello world\n"));
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const agent = new https.Agent();
  let pending = 4;
  let bad = 0;
  const check = (socket, tag) => {{
    let result = "";
    socket.on("connect", () => {{
      socket.write("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");
      socket.end();
    }});
    socket.on("data", (c) => (result += c));
    socket.on("end", () => {{
      const ok = result.includes("200 OK") && result.includes("hello world");
      console.log(tag, ok ? "ok" : "EMPTY");
      if (!ok) bad++;
      if (--pending === 0) {{
        console.log(bad === 0 ? "ALL-OK" : "BAD:" + bad);
        agent.destroy();
        server.close();
      }}
    }});
    socket.on("error", (e) => console.log(tag, "err", e.code));
  }};
  check(agent.createConnection({{ port, host: "127.0.0.1", rejectUnauthorized: false }}), "obj");
  check(agent.createConnection(port, "127.0.0.1", {{ rejectUnauthorized: false }}), "pos");
  check(agent.createConnection(port, {{ rejectUnauthorized: false }}), "portopt");
  check(agent.createConnection({{ port, host: "127.0.0.1", rejectUnauthorized: false }}), "obj2");
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 5000);
"#
        ),
    );
    for line in ["obj ok", "pos ok", "portopt ok", "obj2 ok", "ALL-OK", "srv-close", "end-ok"] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn https_agent_free_event() {
    // (5)：响应回池即 agent 级 'free'（socket, options）——node
    // installListeners/onFree 口径；handler 内 freeSockets 已有键。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "f.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = https.createServer({{ key, cert }}, (req, res) => res.end("free-ok"));
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const agent = new https.Agent({{ keepAlive: true }});
  let frees = 0;
  agent.on("free", (sock, opts) => {{
    frees++;
    console.log("free", typeof sock.writable, Object.keys(agent.freeSockets).length > 0);
  }});
  const one = () => new Promise((resolve) => {{
    const r = https.get({{ port, host: "127.0.0.1", path: "/", ca: cert, agent }}, (res) => {{
      let b = "";
      res.on("data", (c) => (b += c));
      res.on("end", () => resolve(b + ":" + r.reusedSocket));
    }});
    r.on("error", (e) => resolve("err:" + e.code));
  }});
  (async () => {{
    console.log("first", await one());
    console.log("second", await one());
    console.log("frees", frees);
    agent.destroy();
    server.close();
  }})();
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 3000);
"#
        ),
    );
    for line in [
        "free boolean true",
        "first free-ok:false",
        "second free-ok:true",
        "frees 2",
        "srv-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn https_connect_lookup_family() {
    // connect-address-family 回归：https 建连须 honor family/lookup（tls
    // 直拨面只收 host/port，lookup 被吞即错连→握手失败/hang 交替）。
    // 正常：agent 路 + agent:false 路自定义 lookup（'dummy'→::1, family 6）；
    // 报错：lookup 回 err 即 req 'error'；边界：lookup 断言 family 透传。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "l.mjs",
        &format!(
            r#"
import https from "node:https";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = https.createServer({{ key, cert }}, (req, res) => res.end("fam-ok"));
server.listen(0, "::1", () => {{
  const port = server.address().port;
  const lookup6 = (addr, opts, cb) => {{
    console.log("lookup", addr, opts.family);
    cb(null, "::1", opts.family);
  }};
  const one = (useAgent) => new Promise((resolve) => {{
    const opts = {{
      host: "dummy", port, family: 6, rejectUnauthorized: false, lookup: lookup6,
    }};
    if (!useAgent) opts.agent = false;
    else opts.agent = new https.Agent();
    const r = https.get(opts, (res) => {{
      let b = "";
      res.on("data", (c) => (b += c));
      res.on("end", () => resolve("ok:" + b + ":" + r.socket.remoteAddress));
    }});
    r.on("error", (e) => resolve("err:" + (e.code || e.message)));
  }});
  (async () => {{
    console.log("agent", await one(true));
    console.log("noagent", await one(false));
    const bad = https.get({{
      host: "dummy", port, family: 6, rejectUnauthorized: false,
      agent: new https.Agent(),
      lookup: (_a, _o, cb) => cb(new Error("no-such-host")),
    }}, () => console.log("bad-unexpected"));
    bad.on("error", (e) => console.log("bad", e.message));
    setTimeout(() => {{ server.close(); }}, 500);
  }})();
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 6000);
"#
        ),
    );
    for line in [
        "lookup dummy 6",
        "agent ok:fam-ok:::1",
        "noagent ok:fam-ok:::1",
        "bad no-such-host",
        "srv-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}
