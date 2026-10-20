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
