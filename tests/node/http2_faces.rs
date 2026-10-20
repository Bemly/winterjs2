//! tests/node/http2_faces.rs — node:http2 R19 C2/C3 新面黑盒（wave-3 自续；
//! 与 http2.rs 同域，按 §0.9 方法边界切分：SOURCE 随实现走，黑盒随域聚）。
use crate::helpers::*;

#[test]
fn http2_extended_connect_faces() {
    // connect-method-extended：服务端 settings.enableConnectProtocol →
    // hyper Builder 开旗 → 线上 SETTINGS{8:1} → 客户端 remoteSettings 见旗 →
    // CONNECT+:protocol 出线 → 服务端 stream 头含 :protocol/:path → 应答收尾。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "e.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer({ settings: { enableConnectProtocol: true } });
server.on("stream", (stream, headers) => {
  console.log("method", headers[":method"]);
  console.log("protocol", headers[":protocol"]);
  console.log("path", headers[":path"]);
  stream.respond();
  stream.end("ok");
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  client.on("remoteSettings", (s) => {
    console.log("ecp", s.enableConnectProtocol);
    const req = client.request({ ":method": "CONNECT", ":protocol": "foo" });
    req.on("end", () => console.log("end", req.rstCode));
    req.on("close", () => { client.close(); server.close(); });
    req.resume();
    req.end();
  });
});
"#,
    );
    for line in [
        "ecp true",
        "method CONNECT",
        "protocol foo",
        "path /",
        "end 0",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_connect_protocol_off_poison() {
    // cant-turn-off：ECP 开后关 = 连接级 PROTOCOL_ERROR（node 原样）——
    // 服务端发坏 SETTINGS{8:0}（hyper 无 mid-connection API，直写线）；
    // 客户端先见 1 再见 0 即会话 error + 在途流 error；服务端不再建流。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer({ settings: { enableConnectProtocol: true } });
server.on("stream", () => console.log("BAD-STREAM"));
server.on("session", (session) => {
  session.settings({ enableConnectProtocol: false });
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  client.on("remoteSettings", (s) => {
    console.log("ecp", s.enableConnectProtocol);
    const req = client.request({ ":method": "CONNECT", ":protocol": "foo" });
    req.on("error", (e) => console.log("req-err", e.code, e.message));
    req.on("close", () => server.close());
    req.resume();
    req.end();
  });
  client.on("error", (e) => console.log("cli-err", e.code, e.message));
  client.on("close", () => client.close());
});
"#,
    );
    for line in [
        "ecp true",
        "req-err ERR_HTTP2_ERROR Protocol error",
        "cli-err ERR_HTTP2_ERROR Protocol error",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.contains("BAD-STREAM"), "poisoned conn must not emit stream; out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_too_large_headers_faces() {
    // too-large-headers：maxHeaderListSize/maxHeaderSize（别名）→ hyper
    // Builder 限头表 → 超限 h2 线上 GOAWAY(ENHANCE_YOUR_CALM)，node 线上
    // RST_STREAM——客户端翻成同码同文流错误 + rstCode 11；服务端无流。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "l.mjs",
        r#"
import http2 from "node:http2";
for (const key of ["maxHeaderListSize", "maxHeaderSize"]) {
  const server = http2.createServer({ settings: { [key]: 100 } });
  server.on("stream", () => console.log("BAD-STREAM", key));
  server.listen(0, () => {
    const client = http2.connect(`http://localhost:${server.address().port}`);
    client.on("remoteSettings", () => {
      const req = client.request({ "foo": "a".repeat(1000) });
      req.on("error", (e) => console.log(key, "req-err", e.code, e.message));
      req.on("close", () => {
        console.log(key, "rst", req.rstCode);
        client.close();
        server.close();
      });
      req.resume();
      req.end();
    });
    client.on("error", () => {});
  });
}
"#,
    );
    for line in [
        "maxHeaderListSize req-err ERR_HTTP2_STREAM_ERROR Stream closed with error code NGHTTP2_ENHANCE_YOUR_CALM",
        "maxHeaderListSize rst 11",
        "maxHeaderSize req-err ERR_HTTP2_STREAM_ERROR Stream closed with error code NGHTTP2_ENHANCE_YOUR_CALM",
        "maxHeaderSize rst 11",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.contains("BAD-STREAM"), "oversize headers must not reach service; out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_client_streaming_upload_faces() {
    // cancel-while-client-reading：POST 首 write 微任务后开流（同 tick 内
    // end() 仍走单发）；无 end() 的上传照样出线，服务端收块即见 data。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "u.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.resume();
  stream.on("data", (c) => console.log("srv-data", c.length));
  stream.on("end", () => {
    console.log("srv-end");
    stream.respond();
    stream.end("ok");
  });
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request({ ":method": "POST" });
  req.on("response", () => console.log("cli-response"));
  req.on("end", () => { client.close(); server.close(); });
  req.resume();
  req.write(Buffer.alloc(10));
  setTimeout(() => req.end(), 20);
});
"#,
    );
    for line in ["srv-data 10", "srv-end", "cli-response"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_stream_destroy_event_order_faces() {
    // stream-destroy-event-order：client req.close(2) 即本地 error + close；
    // 对端 RST(2) 经 wire 窥探翻服务端 aborted → error → close（hyper 吞对端
    // RST，无窥探即服务端 error 永不到）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "o.mjs",
        r#"
import http2 from "node:http2";
let client;
let req;
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.on("error", (e) => {
    console.log("srv-err", e.code);
    client.close();
    stream.on("close", () => {
      console.log("srv-close");
      server.close();
    });
  });
  req.close(2);
});
server.listen(0, () => {
  client = http2.connect(`http://localhost:${server.address().port}`);
  req = client.request();
  req.resume();
  req.on("error", (e) => {
    console.log("cli-err", e.code, e.message);
    req.on("close", () => console.log("cli-close"));
  });
});
"#,
    );
    for line in [
        "cli-err ERR_HTTP2_STREAM_ERROR Stream closed with error code NGHTTP2_INTERNAL_ERROR",
        "cli-close",
        "srv-err ERR_HTTP2_STREAM_ERROR",
        "srv-close",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_server_socket_destroy_faces() {
    // server-socket-destroy：会话 kSocket.destroy 即 session.destroy——
    // socket/stream/session/server 'close' 全发，服务端流 'aborted' 先行；
    // 客户端 req 'aborted' + ECONNRESET（node 二者择一，此处走 error 路）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "d.mjs",
        r#"
import http2, { constants } from "node:http2";
import { kSocket } from "node:internal/http2/util";
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.respond();
  stream.write("test");
  const socket = stream.session[kSocket];
  socket.on("close", () => console.log("sock-close"));
  stream.on("close", () => console.log("stream-close"));
  server.on("close", () => console.log("server-close"));
  stream.session.on("close", () => {
    console.log("sess-close");
    server.close();
  });
  stream.on("aborted", () => console.log("stream-aborted"));
  socket.destroy();
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  client.on("close", () => console.log("cli-close"));
  const req = client.request({ ":method": "POST" });
  req.on("aborted", () => console.log("req-aborted"));
  req.on("error", (e) => console.log("req-err", e.code));
  req.on("close", () => console.log("req-close"));
  req.resume();
  req.end("hello");
});
"#,
    );
    for line in [
        "sock-close",
        "stream-aborted",
        "stream-close",
        "sess-close",
        "server-close",
        "req-aborted",
        "req-err ECONNRESET",
        "req-close",
        "cli-close",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_propagate_session_destroy_code_faces() {
    // propagate-session-destroy-code：session.destroy(7) → GOAWAY(7) 先行
    // + 逐流 RST(7)；双侧流 SESSION_ERROR + rstCode 7，会话 error 同文，
    // destroyed 置位而 closed 保持 false 直到 close。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "g.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("session", (session) => {
  session.on("error", (e) => console.log("srv-sess-err", e.code, e.message));
  session.on("close", () => console.log("srv-sess-close"));
  session.on("stream", (stream) => {
    stream.on("error", (e) => console.log("srv-stream-err", e.code, e.message, stream.rstCode));
    session.destroy(7);
  });
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  client.on("error", (e) => console.log("cli-err", e.code, e.message));
  client.on("close", () => console.log("cli-close"));
  const req = client.request({ ":path": "/" });
  req.on("error", (e) => console.log("cli-stream-err", e.code, e.message, req.rstCode));
  req.on("close", () => {
    console.log("cli-stream-close", client.destroyed === true || client.closed === true);
    server.close();
  });
  req.resume();
  req.end();
});
"#,
    );
    for line in [
        "srv-stream-err ERR_HTTP2_SESSION_ERROR Session closed with error code 7 7",
        "srv-sess-err ERR_HTTP2_SESSION_ERROR Session closed with error code 7",
        "srv-sess-close",
        "cli-stream-err ERR_HTTP2_SESSION_ERROR Session closed with error code 7 7",
        "cli-err ERR_HTTP2_SESSION_ERROR Session closed with error code 7",
        "cli-close",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_client_socket_destroy_faces() {
    // client-socket-destroy：会话 socket.destroy 即会话 destroy——
    // 服务端流 aborted（须在 close 前）；客户端 req aborted → end → close
    //（无 end 即挂；直接 destroy 会吞 end）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "d.mjs",
        r#"
import http2 from "node:http2";
const { kSocket } = require("node:internal/http2/util");
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.on("aborted", () => console.log("srv-aborted"));
  stream.on("close", () => console.log("srv-close"));
  stream.on("error", () => {});
  stream.respond();
  stream.write("x");
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  client.on("close", () => console.log("cli-close"));
  const req = client.request();
  req.on("response", () => {
    client[kSocket].destroy();
  });
  req.resume();
  req.on("aborted", () => console.log("req-aborted"));
  req.on("end", () => console.log("req-end"));
  req.on("close", () => {
    console.log("req-close");
    server.close();
  });
  req.on("error", () => {});
  req.end();
});
"#,
    );
    for line in [
        "srv-aborted",
        "srv-close",
        "req-aborted",
        "req-end",
        "req-close",
        "cli-close",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_server_listen_callback_faces() {
    // client-request-listeners-warning：server.listen(cb) 即随机端口 + 回调
    //（首参函数形；旧实现误作端口致 hang）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "l.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer((req, res) => res.end("ok"));
server.listen(() => {
  const port = server.address().port;
  console.log("port", port > 0);
  const client = http2.connect(`http://localhost:${port}`);
  const req = client.request({ ":path": "/" });
  req.on("response", () => console.log("resp"));
  req.on("end", () => {
    console.log("end");
    client.close();
    server.close();
  });
  req.resume();
  req.end();
});
"#,
    );
    for line in ["port true", "resp", "end"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_socket_proxy_handler_for_has_faces() {
    // socket-proxy-handler-for-has：会话 socket 自带 encrypted；
    // request socket 代理 has 陷阱（stream 或会话 socket 含即 true）。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "h.mjs",
        &format!(
            r#"
import {{ createSecureServer, connect }} from "node:http2";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
const server = createSecureServer({{ key, cert }}, (req, res) => {{
  console.log("encrypted", req.socket.encrypted);
  console.log("has", "encrypted" in req.socket);
  res.end();
}});
server.listen(0, "127.0.0.1", () => {{
  const port = server.address().port;
  const sess = connect(`https://127.0.0.1:${{port}}`, {{ ca: cert }});
  sess.on("error", () => {{}});
  sess.on("connect", () => {{
    const st = sess.request({{ ":path": "/" }});
    st.on("response", () => console.log("resp"));
    st.on("end", () => {{ sess.close(); server.close(); }});
    st.resume();
    st.end();
  }});
}});
"#
        ),
    );
    for line in ["encrypted true", "has true", "resp"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_sendfile_range_slice() {
    // respondWithFile offset/length：常规文件自动 content-length（node 覆写口径），
    // 体为 [offset, offset+length) 切片（range 套件行为面）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "r.mjs",
        r#"
import http2 from "node:http2";
import fs from "node:fs";
fs.writeFileSync("data.txt", "0123456789abcdef");
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.respondWithFile("data.txt", { "content-type": "text/plain" }, {
    offset: 8,
    length: 3,
    statCheck(stat, headers) { headers["x-size"] = stat.size; },
  });
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request();
  req.on("response", (h) => {
    console.log("len", h["content-length"]);
    console.log("xsize", h["x-size"]);
  });
  req.setEncoding("utf8");
  let body = "";
  req.on("data", (c) => { body += c; });
  req.on("end", () => {
    console.log("body", JSON.stringify(body));
    client.close();
    server.close();
  });
  req.end();
});
"#,
    );
    for line in ["len 3", "xsize 16", "body \"89a\""] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_sendfile_pipe_body() {
    // respondWithFile FIFO：无 offset/length 即流到 EOF（node offset=-1/length=-1
    // 口径），statCheck 照调、无 content-length（with-pipe 套件行为面）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "p.mjs",
        r#"
import http2 from "node:http2";
import fs from "node:fs";
import child_process from "node:child_process";
const mk = child_process.spawnSync("mkfifo", ["f.pipe"]);
if (mk.error) { console.log("skip-nomkfifo"); }
else {
  const server = http2.createServer();
  server.on("stream", (stream) => {
    stream.respondWithFile("f.pipe", { "content-type": "text/plain" }, {
      onError: (e) => console.log("onerror", e.code),
      statCheck: () => console.log("statcheck"),
    });
  });
  server.listen(0, () => {
    const client = http2.connect(`http://localhost:${server.address().port}`);
    const req = client.request();
    req.on("response", (h) => console.log("status", h[":status"]));
    req.setEncoding("utf8");
    let body = "";
    req.on("data", (c) => { body += c; });
    req.on("end", () => {
      console.log("body", JSON.stringify(body));
      client.close();
      server.close();
    });
    req.end();
  });
  fs.open("f.pipe", "w", (e, fd) => {
    if (e) { console.log("wopen-err", e.code); return; }
    fs.writeSync(fd, "Hello, world!\n");
    fs.closeSync(fd);
  });
}
"#,
    );
    for line in ["statcheck", "status 200", "body \"Hello, world!\\n\""] {
        assert!(out.lines().any(|l| l == line) || out.lines().any(|l| l == "skip-nomkfifo"),
            "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_sendfile_pipe_offset_noseek() {
    // respondWithFile FIFO + offset：onError(ERR_HTTP2_SEND_FILE_NOSEEK)，
    // statCheck 不调，用户可另行 404（error-pipe-offset 套件行为面）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "q.mjs",
        r#"
import http2 from "node:http2";
import fs from "node:fs";
import child_process from "node:child_process";
const mk = child_process.spawnSync("mkfifo", ["f.pipe"]);
if (mk.error) { console.log("skip-nomkfifo"); }
else {
  const server = http2.createServer();
  server.on("stream", (stream) => {
    stream.respondWithFile("f.pipe", { "content-type": "text/plain" }, {
      offset: 10,
      onError: (e) => {
        console.log("onerror", e.code, e.message);
        stream.respond({ ":status": 404 });
        stream.end();
      },
      statCheck: () => console.log("statcheck-must-not-fire"),
    });
  });
  server.listen(0, () => {
    const client = http2.connect(`http://localhost:${server.address().port}`);
    const req = client.request();
    req.on("response", (h) => console.log("status", h[":status"]));
    req.on("end", () => { client.close(); server.close(); });
    req.end();
  });
  fs.writeFile("f.pipe", "Hello, world!\n", () => {});
}
"#,
    );
    for line in [
        "onerror ERR_HTTP2_SEND_FILE_NOSEEK Offset or length can only be specified for regular files",
        "status 404",
    ] {
        assert!(out.lines().any(|l| l == line) || out.lines().any(|l| l == "skip-nomkfifo"),
            "missing {line}; out: {out}");
    }
    assert!(!out.lines().any(|l| l == "statcheck-must-not-fire"), "statCheck fired; out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_sendfile_dir_sendfile_error() {
    // respondWithFile 目录：onError(ERR_HTTP2_SEND_FILE)，statCheck 不调
    //（error-dir 套件行为面）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "d.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.respondWithFile(".", { "content-type": "text/plain" }, {
    onError: (e) => {
      console.log("onerror", e.code, e.message);
      stream.respond({ ":status": 404 });
      stream.end();
    },
    statCheck: () => console.log("statcheck-must-not-fire"),
  });
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request();
  req.on("response", (h) => console.log("status", h[":status"]));
  req.on("end", () => { client.close(); server.close(); });
  req.end();
});
"#,
    );
    for line in ["onerror ERR_HTTP2_SEND_FILE Directories cannot be sent", "status 404"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.lines().any(|l| l == "statcheck-must-not-fire"), "statCheck fired; out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_sendfile_fd_validation_faces() {
    // respondWithFD 校验面（fd-errors 套件行为面）：fd 双形外即 ARG_TYPE、
    // options 三键错型即 ARG_VALUE、204 禁体、已发头、已销毁。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "v.mjs",
        r#"
import http2 from "node:http2";
import fs from "node:fs";
fs.writeFileSync("a.txt", "hello");
const fd = fs.openSync("a.txt", "r");
const server = http2.createServer();
server.on("stream", (stream) => {
  for (const bad of [true, {}, [], null, undefined]) {
    try {
      stream.respondWithFD(bad, { "content-type": "text/plain" });
      console.log("fd-nothrow", String(bad));
    } catch (e) { console.log("fd-throw", e.code); }
  }
  for (const [k, v] of [["offset", "x"], ["length", "x"], ["statCheck", "x"]]) {
    try {
      stream.respondWithFD(fd, {}, { [k]: v });
      console.log("opt-nothrow", k);
    } catch (e) { console.log("opt-throw", k, e.code); }
  }
  for (const s of [204, 304]) {
    try {
      stream.respondWithFD(fd, { ":status": s });
      console.log("status-nothrow", s);
    } catch (e) { console.log("status-throw", s, e.code); }
  }
  stream.respond();
  try { stream.respondWithFD(fd, {}); console.log("sent-nothrow"); }
  catch (e) { console.log("sent-throw", e.code); }
  stream.destroy();
  try { stream.respondWithFD(fd, {}); console.log("dead-nothrow"); }
  catch (e) { console.log("dead-throw", e.code); }
  try { stream.respondWithFile("a.txt"); console.log("deadfile-nothrow"); }
  catch (e) { console.log("deadfile-throw", e.code); }
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request();
  req.on("error", () => {});
  req.on("close", () => { client.close(); server.close(); });
  req.end();
});
"#,
    );
    for line in [
        "fd-throw ERR_INVALID_ARG_TYPE",
        "opt-throw offset ERR_INVALID_ARG_VALUE",
        "opt-throw length ERR_INVALID_ARG_VALUE",
        "opt-throw statCheck ERR_INVALID_ARG_VALUE",
        "status-throw 204 ERR_HTTP2_PAYLOAD_FORBIDDEN",
        "status-throw 304 ERR_HTTP2_PAYLOAD_FORBIDDEN",
        "sent-throw ERR_HTTP2_HEADERS_SENT",
        "dead-throw ERR_HTTP2_INVALID_STREAM",
        "deadfile-throw ERR_HTTP2_INVALID_STREAM",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.lines().any(|l| l.contains("nothrow")), "unexpected nothrow; out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_socket_close_unordered_teardown() {
    // socket-close 套件行为面：net 明文 socket 手动注入 secure server
    // （emit connection→服务端 TLS 握手→会话），颠倒拆链（先毁裸 socket、
    // 后关会话）不挂死不 segfault，客户端正常收尾。
    let dir = assert_fs::TempDir::new().unwrap();
    let (cert_path, key_path) = write_self_signed(&dir);
    let out = run_fs_file(
        &dir,
        "s.mjs",
        &format!(
            r#"
import net from "node:net";
import h2 from "node:http2";
import fs from "node:fs";
const key = fs.readFileSync({key_path:?}, "utf8");
const cert = fs.readFileSync({cert_path:?}, "utf8");
let serverRawSocket;
let serverH2Session;
const netServer = net.createServer((socket) => {{
  serverRawSocket = socket;
  h2Server.emit("connection", socket);
}});
const h2Server = h2.createSecureServer({{ key, cert }}, (req, res) => {{
  res.writeHead(200);
  res.end();
}});
h2Server.on("session", (s) => {{ serverH2Session = s; }});
h2Server.on("error", () => {{}});
netServer.listen(0, "127.0.0.1", () => {{
  const client = h2.connect(`https://127.0.0.1:${{netServer.address().port}}`, {{ rejectUnauthorized: false }});
  client.on("error", () => {{}});
  client.on("close", () => {{ console.log("client-close"); netServer.close(); }});
  const req = client.request({{ ":method": "GET", ":path": "/" }});
  req.on("error", () => {{}});
  req.on("response", (h) => {{
    console.log("status", h[":status"]);
    setTimeout(() => {{
      serverRawSocket.destroy();
      setTimeout(() => {{ serverH2Session.close(); }}, 10);
    }}, 10);
  }});
  req.resume();
  req.end();
}});
"#
        ),
    );
    for line in ["status 200", "client-close"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_socket_destroy_delayed_flush() {
    // destroy-delayed 套件行为面：res.end 紧接 setImmediate 杀 socket，
    // 应答字节必须落盘（优雅排空收尾命令），客户端收 response+end+close。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "d.mjs",
        r#"
import http2 from "node:http2";
const app = http2.createServer((req, res) => {
  res.end("hello");
  setImmediate(() => req.socket.destroy());
});
app.listen(0, () => {
  const session = http2.connect(`http://localhost:${app.address().port}`);
  const request = session.request({ ":path": "/", ":method": "get" });
  request.once("response", (h) => console.log("status", h[":status"]));
  request.on("data", () => {});
  request.on("end", () => console.log("end"));
  request.on("error", () => console.log("req-error"));
  request.on("close", () => {
    console.log("close");
    session.close();
    app.close();
  });
  request.end();
});
"#,
    );
    for line in ["status 200", "end", "close"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.lines().any(|l| l == "req-error"), "unexpected error; out: {out}");
    dir.close().unwrap();
}

#[test]
fn http2_session_close_flush() {
    // session.close 紧接应答的优雅对照面：同上不断链，保证优雅排空语义。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "c.mjs",
        r#"
import http2 from "node:http2";
const app = http2.createServer((req, res) => {
  res.end("hello");
  setImmediate(() => req.stream.session.close());
});
app.listen(0, () => {
  const session = http2.connect(`http://localhost:${app.address().port}`);
  const request = session.request({ ":path": "/", ":method": "get" });
  request.once("response", (h) => console.log("status", h[":status"]));
  request.on("data", () => {});
  request.on("end", () => console.log("end"));
  request.on("close", () => {
    console.log("close");
    session.close();
    app.close();
  });
  request.end();
});
"#,
    );
    for line in ["status 200", "end", "close"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}
