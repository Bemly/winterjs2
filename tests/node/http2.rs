//! tests/node/http2.rs — 对齐 src/builtins/node/http2.rs（node:http2）。

use crate::helpers::*;

#[test]
fn http2_compat_header_faces() {
    // C 轮壳面：头名归一（trim+小写）/writeHead 元组与平铺/setNextStreamID 校验/
    // kTimeout 与 alpnProtocol/sendDate 关/同步 address（正常+报错+边界）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "hf.mjs",
        r#"
import http2 from "node:http2";
import { kTimeout } from "node:internal/timers";
const server = http2.createServer((req, res) => {
  console.log("req-eah", req.stream.endAfterHeaders);
  res.sendDate = false;
  res.setHeader("X-Foo", "1");
  console.log("has", res.hasHeader("x-foo"), res.hasHeader(" X-FOO\t"));
  console.log("get", res.getHeader("X-FOO "));
  try { res.setHeader(); } catch (e) { console.log("set-missing", e.code); }
  try { res.writeHead(200, ["a"]); } catch (e) { console.log("odd", e.code); }
  res.writeHead(200, [["x-a", "1"], ["x-a", "2"], ["X-B", 7]]);
  res.end("ok");
});
server.listen(0, () => {
  const port = server.address().port;
  console.log("sync-addr", port > 0);
  const client = http2.connect(`http://localhost:${port}`);
  client.on("error", () => {});
  client.on("connect", () => {
    console.log("alpn", client.alpnProtocol);
    client.setTimeout(321);
    console.log("kto", client[kTimeout]._idleTimeout);
    try { client.setNextStreamID(2 ** 32); } catch (e) { console.log("nid", e.code, e.name); }
    try { client.setNextStreamID("x"); } catch (e) { console.log("nid-type", e.code, e.name); }
    const req = client.request({ ":path": "/" });
    req.on("response", (h) => {
      console.log("h-xa", h["x-a"], "h-xb", h["x-b"], "h-date", "date" in h);
      console.log("st", h[":status"]);
    });
    req.on("end", () => { client.close(); server.close(); });
    req.resume();
    req.end();
  });
});
"#,
    );
    assert!(out.contains("sync-addr true"), "out: {out}");
    assert!(out.contains("alpn h2c"), "out: {out}");
    assert!(out.contains("kto 321"), "out: {out}");
    assert!(out.contains("nid ERR_OUT_OF_RANGE RangeError"), "out: {out}");
    assert!(out.contains("nid-type ERR_INVALID_ARG_TYPE TypeError"), "out: {out}");
    assert!(out.contains("req-eah true"), "out: {out}");
    assert!(out.contains("has true true"), "out: {out}");
    assert!(out.contains("get 1"), "out: {out}");
    assert!(out.contains("set-missing ERR_INVALID_ARG_TYPE"), "out: {out}");
    assert!(out.contains("odd ERR_INVALID_ARG_VALUE"), "out: {out}");
    assert!(out.contains("h-xa 1, 2"), "out: {out}");
    assert!(out.contains("h-xb 7"), "out: {out}");
    assert!(out.contains("h-date false"), "out: {out}");
    assert!(out.contains("st 200"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_cleartext() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import http2, { createServer, connect, constants } from "node:http2";
console.log("const", constants.HTTP2_HEADER_METHOD === ":method", constants.NGHTTP2_NO_ERROR === 0);
const server = createServer();
server.on("request", (req, res) => {
  let b = "";
  req.on("data", (c) => (b += c));
  req.on("end", () => {
    res.setHeader("x-r", req.url);
    res.writeHead(req.url === "/missing" ? 404 : 200);
    res.end("h2:" + req.method + ":" + b);
  });
});
server.listen(0, "127.0.0.1", () => {
  const port = server.address().port;
  const sess = connect(`http://127.0.0.1:${port}`);
  sess.on("error", () => {});
  sess.on("connect", () => {
    // 两流同 session 并发（多路复用；到达序不定，收集排序后断言）
    const got = [];
    const maybeDone = () => {
      if (got.length === 4) {
        got.sort();
        console.log("mux", got.join("|"));
        sess.close();
      }
    };
    for (const [path, body] of [["/a", "one"], ["/missing", "two"]]) {
      const st = sess.request({ ":method": "POST", ":path": path });
      st.on("response", (h) => got.push(`h${h[":status"]}`));
      let b = "";
      st.on("data", (c) => (b += c));
      st.on("end", () => { got.push(`b${b}`); maybeDone(); });
      st.on("error", () => {});
      st.end(body);
    }
  });
  sess.on("close", () => server.close());
});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 1500);
"#,
    );
    assert!(out.contains("const true true"), "out: {out}");
    assert!(out.contains("mux bh2:POST:one|bh2:POST:two|h200|h404"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_secure() {
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "p.mjs",
        &format!(
            r#"
import {{ createSecureServer, connect }} from "node:http2";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
try {{ const s0 = createSecureServer({{}}); console.log("no-throw", typeof s0.listen); }} catch (e) {{ console.log("no-cert", e.constructor.name); }}
const server = createSecureServer({{ key, cert }}, (req, res) => {{
  res.end("secure-h2:" + req.url);
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const sess = connect(`https://127.0.0.1:${{port}}`, {{ ca: cert }});
  sess.on("error", (e) => console.log("sess-err", e.code));
  sess.on("connect", () => {{
    const st = sess.request({{ ":path": "/s" }});
    st.on("response", (h) => console.log("h", h[":status"]));
    let b = "";
    st.on("data", (c) => (b += c));
    st.on("end", () => {{ console.log("b", b); sess.close(); }});
    st.on("error", () => {{}});
    st.end();
  }});
  sess.on("close", () => server.close());
}});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 1500);
"#
        ),
    );
    // 真机 node 26 实测：createSecureServer({}) 不抛（缺 key/cert 不在构造期校验）
    assert!(out.contains("no-throw function"), "out: {out}");
    assert!(!out.contains("no-cert"), "out: {out}");
    assert!(out.contains("h 200"), "out: {out}");
    assert!(out.contains("b secure-h2:/s"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_errors_boundary() {
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import { createServer, connect } from "node:http2";
// 拒连：code 为 string（平台相关，不断值）
const bad = connect("http://127.0.0.1:1");
bad.on("error", (e) => {
  console.log("refused", typeof e.code);
  const server = createServer((req, res) => {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => res.end("ok:" + b.length));
  });
  server.listen(0, "127.0.0.1", () => {
    const port = server.address().port;
    const sess = connect(`http://127.0.0.1:${port}`);
    sess.on("error", () => {});
    sess.on("connect", () => {
      // 边界：空体 GET + 1MB 体往返
      const g = sess.request({ ":path": "/e" });
      g.on("response", (h) => console.log("empty-h", h[":status"]));
      let eb = "";
      g.on("data", (c) => (eb += c));
      g.on("end", () => {
        console.log("empty-b", eb);
        const big = "ab".repeat(524288);
        const st = sess.request({ ":method": "POST", ":path": "/big" });
        let rb = "";
        st.on("data", (c) => (rb += c));
        st.on("end", () => {
          console.log("big", rb === "ok:" + big.length);
          sess.close();
        });
        st.on("error", () => {});
        st.end(big);
      });
      g.on("error", () => {});
      g.end();
    });
    sess.on("close", () => server.close());
  });
  server.on("close", () => console.log("srv-close"));
});
setTimeout(() => console.log("end-ok"), 2500);
"#,
    );
    assert!(out.contains("refused string"), "out: {out}");
    assert!(out.contains("empty-h 200"), "out: {out}");
    assert!(out.contains("empty-b ok:0"), "out: {out}");
    assert!(out.contains("big true"), "out: {out}");
    assert!(out.contains("srv-close"), "out: {out}");
    assert!(out.contains("end-ok"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_streaming() {
    // 10f http2 流式化：服务端分块写（write/write/end 增量下发）+ 客户端 POST 体
    // + trailer 往返 + 报错（writeHead 双调 ERR_HTTP2_HEADERS_SENT）+ 边界空体。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import { createServer, connect } from "node:http2";
const server = createServer();
server.on("request", (req, res) => {
  if (req.url === "/stream") {
    res.writeHead(200, { "x-a": "1" });
    res.write("chunk1-");
    setTimeout(() => { res.write("chunk2-"); setTimeout(() => res.end("chunk3"), 20); }, 20);
  } else if (req.url === "/echo") {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => res.end("echo:" + b));
  } else if (req.url === "/trailer") {
    res.writeHead(200);
    res.addTrailers({ "x-t": "1" });
    res.end("t-body");
  } else if (req.url === "/double") {
    // flushHeaders 即发头（__sendHead 置 headersSent），其后 writeHead 必抛 HEADERS_SENT。
    res.flushHeaders();
    let code = "NO-THROW";
    try { res.writeHead(200); } catch (e) { code = e.code ?? "no-code"; }
    res.end("gate:" + code);
  } else {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => res.end("empty:" + b.length));
  }
});
server.listen(0, "127.0.0.1", () => {
  const port = server.address().port;
  const sess = connect(`http://127.0.0.1:${port}`);
  sess.on("error", () => {});
  const get = (path, wantTrailers) => new Promise((resolve, reject) => {
    const st = sess.request({ ":path": path });
    st.on("response", (h) => {});
    let b = "";
    let trailers = null;
    st.on("data", (c) => (b += c));
    st.on("trailers", (t) => { trailers = t; });
    st.on("end", () => resolve({ b, trailers }));
    st.on("error", reject);
    st.end();
  });
  const post = (path, body) => new Promise((resolve, reject) => {
    const st = sess.request({ ":method": "POST", ":path": path });
    let b = "";
    st.on("data", (c) => (b += c));
    st.on("end", () => resolve(b));
    st.on("error", reject);
    st.end(body);
  });
  sess.on("connect", async () => {
    try {
      const s = await get("/stream");
      console.log("stream", s.b === "chunk1-chunk2-chunk3");
      const e = await post("/echo", "hello-h2");
      console.log("echo", e === "echo:hello-h2");
      const t = await get("/trailer");
      console.log("trailer", t.b === "t-body", t.trailers !== null && t.trailers["x-t"] === "1");
      const d = await get("/double");
      console.log("gate", d.b === "gate:ERR_HTTP2_HEADERS_SENT");
      const z = await get("/empty");
      console.log("empty", z.b === "empty:0");
      console.log("done");
      sess.close();
    } catch (e) {
      console.log("fail", e && e.code, e && e.message);
      sess.close();
    }
  });
  sess.on("close", () => server.close());
});
server.on("close", () => console.log("srv-close"));
setTimeout(() => console.log("end-ok"), 2500);
"#,
    );
    for line in [
        "stream true",
        "echo true",
        "trailer true true",
        "gate true",
        "empty true",
        "done",
        "srv-close",
        "end-ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing line: {line}\nout: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_trailers_with_body() {
    // P1（2026-09-25）：有体 + waitForTrailers 双向 trailer（修前 ChanBody EndPending 不登记
    // waker，trailer 永不出线两端互等）；trailers 事件序 data → trailers → end；
    // sendTrailers 门序 NOT_READY / ALREADY_SENT / INVALID_STREAM（真机逐项）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "t.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("stream", (stream) => {
  const ev = [];
  stream.on("data", (c) => ev.push("data:" + c));
  stream.on("trailers", (h) => ev.push("trailers:" + h["x-up"]));
  stream.on("end", () => { console.log("srv-order", ev.join(",")); });
  try { stream.sendTrailers({}); } catch (e) { console.log("gate-early", e.code); }
  stream.respond({ ":status": 200 }, { waitForTrailers: true });
  stream.on("wantTrailers", () => {
    stream.sendTrailers({ "x-down": "d" });
    try { stream.sendTrailers({}); } catch (e) { console.log("gate-twice", e.code); }
  });
  stream.end("body");
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request({ ":path": "/", ":method": "POST" }, { waitForTrailers: true });
  req.on("wantTrailers", () => req.sendTrailers({ "x-up": "u" }));
  req.on("trailers", (h) => console.log("cli-trailers", h["x-down"]));
  req.resume();
  req.on("close", () => {
    try { req.sendTrailers({}); } catch (e) { console.log("gate-closed", e.code); }
    client.close();
    server.close();
  });
  req.end("up");
});
"#,
    );
    for line in [
        "srv-order data:up,trailers:u",
        "gate-early ERR_HTTP2_TRAILERS_NOT_READY",
        "gate-twice ERR_HTTP2_TRAILERS_ALREADY_SENT",
        "cli-trailers d",
        "gate-closed ERR_HTTP2_INVALID_STREAM",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_lookup_and_promisify_custom() {
    // base16回归：connect 尊选自定义 lookup（错原样 error）+ promisify.custom
    //（node internal/http2/core.js 原文口径；正常 connect 面由 P1 trailer 测试覆盖）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import http2 from "node:http2";
import util from "node:util";
const error = new Error("Unable to resolve hostname");
const lookup = (h, o, cb) => cb(error);
const sess = http2.connect("http://hostname", { lookup });
sess.on("error", (e) => console.log("raw-error", e.message === "Unable to resolve hostname"));
const connect = util.promisify(http2.connect);
connect("http://hostname", { lookup }).then(
    () => console.log("FAIL resolved"),
    (e) => console.log("rejected", e.message === "Unable to resolve hostname"));
"#,
    );
    assert!(out.contains("raw-error true"), "lookup error passthrough: {out}");
    assert!(out.contains("rejected true"), "promisify.custom rejects: {out}");
    assert!(!out.contains("FAIL"), "must not resolve: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_compat_expect_routing() {
    // c2：expect 路由（node onServerStream 逐字）：checkExpectation 监听 +
    // 无人监听 417 缺省；100-continue 无监听走 request（auto-100 记账，线形 bound）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "e.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("checkExpectation", (req, res) => {
  console.log("checkExpectation", req.headers.expect);
  res.statusCode = 417;
  res.end();
});
server.on("request", (req, res) => {
  console.log("request", req.headers.expect);
  res.end("ok");
});
server.listen(0, () => {
  const port = server.address().port;
  const get = (headers) => new Promise((resolve) => {
    const c = http2.connect(`http://localhost:${port}`);
    const req = c.request(headers);
    req.on("response", (h) => {
      let b = "";
      req.on("data", (d) => (b += d));
      req.on("end", () => { c.close(); resolve(`${h[":status"]}:${b}`); });
    });
    req.resume();
    req.end();
  });
  (async () => {
    console.log("r1", await get({ ":path": "/", "expect": "meoww" }));
    console.log("r2", await get({ ":path": "/", "expect": "100-continue" }));
    server.close();
  })();
});
"#,
    );
    for line in [
        "checkExpectation meoww",
        "r1 417:",
        "request 100-continue",
        "r2 200:ok",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_compat_connect_default() {
    // c2：CONNECT 缺省 405（旧 501 误植；node onServerStream 逐字）+
    // 有监听走 connect 事件。node setupCompat 口径：compat 路由（connect/405）
    // 只在 'request' 监听器出现过后接线——两 dummy request 监听为接线用，
    // CONNECT 本体不走 request（connect-method 套件：纯 stream 监听不 405）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "c.mjs",
        r#"
import http2 from "node:http2";
const s1 = http2.createServer();
s1.on("request", (req, res) => res.end("r"));
s1.listen(0, () => {
  const c = http2.connect(`http://localhost:${s1.address().port}`);
  const req = c.request({ ":method": "CONNECT", ":authority": "x" });
  req.on("response", (h) => console.log("default", h[":status"]));
  req.on("end", () => { c.close(); s1.close(); });
  req.resume();
  req.end();
});
const s2 = http2.createServer();
s2.on("request", (req, res) => res.end("r"));
s2.on("connect", (req, res) => {
  console.log("connect-ev", req.headers[":method"]);
  res.statusCode = 200;
  res.end();
});
s2.listen(0, () => {
  const c = http2.connect(`http://localhost:${s2.address().port}`);
  const req = c.request({ ":method": "CONNECT", ":authority": "y" });
  req.on("response", (h) => console.log("handled", h[":status"]));
  req.on("end", () => { c.close(); s2.close(); });
  req.resume();
  req.end();
});
"#,
    );
    for line in ["default 405", "connect-ev CONNECT", "handled 200"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_early_hints_validation() {
    // c2：writeEarlyHints 校验（link BigInt → ARG_VALUE；非对象 → ARG_TYPE；
    // 空数组 link → false）+ writeInformation 门（101/200/字符串 → STATUS_INVALID；
    // 终后 writeContinue → false）。抛错经 handler 内捕获回传（uncaught 投递系 infra）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "v.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer((req, res) => {
  const out = [];
  try { res.writeEarlyHints({ link: BigInt(100) }); } catch (e) { out.push("link-bigint:" + e.code); }
  try { res.writeEarlyHints("x"); } catch (e) { out.push("hints-str:" + e.code); }
  console.log("empty-link", res.writeEarlyHints({ link: [] }));
  for (const t of [101, 200, "100"]) {
    try { res.writeInformation(t, {}); } catch (e) { out.push("info:" + e.code); }
  }
  res.end("pre");
  console.log("wc-after-end", res.writeContinue());
  console.log("gates", out.join(","));
  res.on("finish", () => server.close());
});
server.listen(0, () => {
  const c = http2.connect(`http://localhost:${server.address().port}`);
  const req = c.request({ ":path": "/" });
  req.resume();
  req.on("end", () => { c.close(); });
  req.end();
});
"#,
    );
    for line in [
        "empty-link false",
        "wc-after-end false",
        "gates link-bigint:ERR_INVALID_ARG_VALUE,hints-str:ERR_INVALID_ARG_TYPE,info:ERR_HTTP2_STATUS_INVALID,info:ERR_HTTP2_STATUS_INVALID,info:ERR_HTTP2_STATUS_INVALID",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_write_after_destroy() {
    // c2：destroy 后 write 静默 false 且回调不触发（真机 p1）；
    // 流关（response 存活）后 write 回调 INVALID_STREAM（真机 p18）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "w.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("request", (req, res) => {
  res.destroy();
  let cb = "no";
  const ret = res.write("asd", () => { cb = "yes"; });
  console.log("silent", ret === false && cb === "no", res.destroyed === true);
  server.close();
});
server.listen(0, () => {
  const c = http2.connect(`http://localhost:${server.address().port}`);
  const req = c.request({ ":path": "/" });
  req.on("error", () => {});
  req.on("close", () => { c.close(); });
  req.resume();
  req.end();
});
const s2 = http2.createServer();
s2.on("request", (req, res) => {
  res.write("x");
  res.stream.close(0, () => {
    res.write("y", (err) => console.log("closed-cb", err && err.code));
  });
});
s2.listen(0, () => {
  const c = http2.connect(`http://localhost:${s2.address().port}`);
  const req = c.request({ ":path": "/" });
  req.on("error", () => {});
  req.on("close", () => { c.close(); s2.close(); });
  req.resume();
  req.end();
});
"#,
    );
    for line in ["silent true true", "closed-cb ERR_HTTP2_INVALID_STREAM"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_socket_session_backing() {
    // c2：request socket 未知属性与 session socket 互通（socket-set 套件；
    // node socketProxyHandler 默认分支走 session[kSocket]）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "s.mjs",
        r#"
import http2 from "node:http2";
import net from "node:net";
const server = http2.createServer();
server.on("request", (req, res) => {
  console.log("instanceof", req.socket instanceof net.Socket);
  console.log("same", req.socket === res.socket);
  console.log("init", req.stream.session.socket._isProcessing === undefined);
  req.socket._isProcessing = true;
  console.log("shared", req.stream.session.socket._isProcessing === true);
  res.stream.destroy();
});
server.listen(0, () => {
  const c = http2.connect(`http://localhost:${server.address().port}`);
  const req = c.request({ ":path": "/" });
  req.on("error", () => {});
  req.on("close", () => { c.close(); server.close(); });
  req.resume();
  req.end();
});
"#,
    );
    for line in ["instanceof true", "same true", "init true", "shared true"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_head_end_stream_on_headers() {
    // C3 HEAD 块：END_STREAM 随头出线（node：writeHead 即发 flags=5 的 HEADERS，
    // end(chunk) 仅账面收尾；native respond {endStream:true} 同形）。
    // 回归守卫：普通 GET 空体 end 仍走头/尾分离（flags=4 + data + end），
    // 禁同 tick 空 END 合并（同款合并曾致 173 回退）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "h.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer((req, res) => {
  if (req.headers[":method"] === "HEAD") {
    res.writeHead(200, { foo: "bar" });
    res.end("data", () => console.log("head-finish-cb"));
  } else {
    res.end("hello");
  }
});
server.listen(0, () => {
  const port = server.address().port;
  const c = http2.connect(`http://localhost:${port}`);
  const h = c.request({ ":path": "/", ":method": "HEAD" });
  h.on("response", (headers, flags) => console.log("head-flags", flags, headers.foo));
  h.on("data", () => console.log("head-DATA-UNEXPECTED"));
  h.on("end", () => {
    console.log("head-end");
    const g = c.request({ ":path": "/" });
    let body = "";
    g.on("response", (headers, flags) => console.log("get-flags", flags));
    g.on("data", (ch) => { body += ch; });
    g.on("end", () => {
      console.log("get-body", body);
      const s2 = http2.createServer();
      s2.on("stream", (stream) => {
        stream.respond({ ":status": 200 }, { endStream: true });
      });
      s2.listen(0, () => {
        const c2 = http2.connect(`http://localhost:${s2.address().port}`);
        const n = c2.request({ ":path": "/" });
        n.on("response", (headers, flags) => console.log("native-flags", flags));
        n.on("data", () => console.log("native-DATA-UNEXPECTED"));
        n.on("end", () => {
          console.log("native-end");
          c2.close();
          s2.close();
          c.close();
          server.close();
        });
        n.end();
      });
    });
    g.end();
  });
  h.end();
});
"#,
    );
    for line in [
        "head-flags 5 bar",
        "head-end",
        "head-finish-cb",
        "get-flags 4",
        "get-body hello",
        "native-flags 5",
        "native-end",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(
        !out.lines().any(|l| l.contains("UNEXPECTED")),
        "HEAD/none body leaked DATA; out: {out}"
    );
    dir.close().unwrap();
}

#[test]
fn http2_server_close_idle_connection() {
    // C3 backlog：server.close() 主动关闲置连接（无在途流会话立即收尾，
    // 有流会话自然排空）；会话侧 'stream' 在服务端 'stream'/'request' 之后；
    // 双向尽后流先 'finish'（writableFinished 置位）再 'close'；
    // 整洁收尾不 abort 待消费的 req（req 'end' 照旧到）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "c.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("session", (session) => {
  session.on("stream", (stream) => {
    console.log("session-stream");
    stream.respond({ ":status": 200 });
    stream.end("hello");
    stream.on("close", () => console.log("stream-close", stream.writableFinished === true));
  });
  session.on("close", () => console.log("session-close"));
});
server.on("close", () => console.log("server-close"));
server.listen(0, () => {
  const c = http2.connect(`http://localhost:${server.address().port}`);
  c.on("close", () => console.log("client-close"));
  const req = c.request({ ":path": "/" });
  req.setEncoding("utf8");
  let data = "";
  req.on("data", (ch) => { data += ch; });
  req.on("end", () => {
    console.log("body", data);
    setImmediate(() => server.close());
  });
  req.end();
  req.resume();
});
const s2 = http2.createServer((req, res) => {
  req.on("end", () => console.log("req-end"));
  res.end();
});
s2.listen(0, () => {
  const c = http2.connect(`http://localhost:${s2.address().port}`);
  const q = c.request({ ":path": "/", ":method": "HEAD" });
  q.on("end", () => { c.close(); s2.close(); });
  q.end();
  q.resume();
});
"#,
    );
    for line in [
        "session-stream",
        "stream-close true",
        "session-close",
        "server-close",
        "client-close",
        "body hello",
        "req-end",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

