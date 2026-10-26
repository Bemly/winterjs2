//! tests/node/http2_server_opts.rs — node:http2 服务端选项面黑盒（R19 wave-3 L-C）。
//! custom Req/Res 类（options-server-request/response）+ server 超时透传
//! session（server-timeout 双形 + 无人监听即 destroy）。
use crate::helpers::*;

#[test]
fn http2_custom_server_classes_faces() {
    // node initializeOptions 口径（core.js:3673-3674）：options.Http2ServerRequest /
    // Http2ServerResponse 缺省为基类；用户子类实例化且方法可见。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "c.mjs",
        r#"
import http2 from "node:http2";
class MyReq extends http2.Http2ServerRequest {
  getUserAgent() { return this.headers["user-agent"] || "unknown"; }
}
class MyRes extends http2.Http2ServerResponse {
  status(code) { return this.writeHead(code, { "content-type": "text/plain" }); }
}
const server = http2.createServer(
  { Http2ServerRequest: MyReq, Http2ServerResponse: MyRes },
  (req, res) => {
    console.log("ua", req.getUserAgent());
    console.log("req-instance", req instanceof MyReq);
    console.log("res-instance", res instanceof MyRes);
    res.status(200);
    res.end("ok");
  },
);
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request({ ":path": "/", "user-agent": "node-test" });
  req.on("response", () => console.log("resp"));
  req.resume();
  req.on("end", () => { client.destroy(); server.close(); });
});
"#,
    );
    for line in ["ua node-test", "req-instance true", "res-instance true", "resp"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_server_timeout_faces() {
    // node connectionListener 口径（core.js:3604-3605/3545-3551）：新会话按
    // server.timeout 武装；到点 server.emit('timeout', session)，无人监听即 destroy。
    // 双形（setTimeout/纯赋值）+ 无人监听 destroy 三断言。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "t.mjs",
        r#"
import http2 from "node:http2";
let pending = 3;
const done = () => { if (--pending === 0) console.log("all-timeout"); };
// 形一：setTimeout，handler 收 session 即 close。
const s1 = http2.createServer();
s1.setTimeout(50);
s1.on("timeout", (session) => {
  console.log("t1", typeof session.close);
  session.close();
  done();
});
s1.listen(0, () => {
  const c = http2.connect(`http://localhost:${s1.address().port}`);
  c.on("close", () => { c.destroy(); s1.close(); });
});
// 形二：纯赋值，handler 收 session 即 close。
const s2 = http2.createServer();
s2.timeout = 50;
s2.on("timeout", (session) => {
  console.log("t2", typeof session.close);
  session.close();
  done();
});
s2.listen(0, () => {
  const c = http2.connect(`http://localhost:${s2.address().port}`);
  c.on("close", () => { c.destroy(); s2.close(); });
});
// 边界：无人监听 → session.destroy，客户端照样见 close。
const s3 = http2.createServer();
s3.setTimeout(50);
s3.listen(0, () => {
  const c = http2.connect(`http://localhost:${s3.address().port}`);
  c.on("close", () => { console.log("t3-destroyed-close"); c.destroy(); s3.close(); done(); });
});
"#,
    );
    for line in ["t1 function", "t2 function", "t3-destroyed-close", "all-timeout"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_stream_server_bookkeeping_faces() {
    // 服务端流记账面（sent-headers/stream-client 套件的非 wire 部分）：
    // sentHeaders 含数字形 ':status'；wantTrailers→sendTrailers→sentTrailers；
    // inspect 呈 `Http2Stream {state/readableState/writableState}`（wire 1xx
    // 下发属 B7 interim bound，本面不依赖）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "b.mjs",
        r#"
import http2 from "node:http2";
import util from "node:util";
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.respond({ abc: "xyz" }, { waitForTrailers: true });
  console.log("status", stream.sentHeaders[":status"], typeof stream.sentHeaders[":status"]);
  console.log("abc", stream.sentHeaders.abc);
  const insp = util.inspect(stream);
  console.log("insp-head", insp.startsWith("Http2Stream {"));
  console.log("insp-state", / {2}state:/.test(insp));
  console.log("insp-rs", / {2}readableState:/.test(insp));
  console.log("insp-ws", / {2}writableState:/.test(insp));
  stream.on("wantTrailers", () => {
    stream.sendTrailers({ xyz: "abc" });
  });
  stream.end();
  stream.on("close", () => {
    console.log("trailers", stream.sentTrailers && stream.sentTrailers.xyz);
    server.close();
  });
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request();
  req.resume();
  req.on("close", () => { client.close(); });
});
"#,
    );
    for line in [
        "status 200 number",
        "abc xyz",
        "insp-head true",
        "insp-state true",
        "insp-rs true",
        "insp-ws true",
        "trailers abc",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_server_graceful_drain_faces() {    // graceful-close 套件面：server.close() 后在途流不受影响，排空后拆会话，
    // 服务端 'close' 到达且客户端收全量（node closeAllSessions + kMaybeDestroy 口径）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "g.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("stream", (stream) => {
  server.close();
  setImmediate(() => {
    stream.respond({ "content-type": "text/plain", ":status": 200 });
    const chunk = Buffer.alloc(64 * 1024, "x");
    for (let i = 0; i < 16; i++) stream.write(chunk);
    stream.end();
  });
  stream.on("close", () => {
    console.log("stream-close", stream.readableEnded, stream.writableFinished);
  });
});
server.on("close", () => console.log("server-close"));
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request({ ":path": "/" });
  let n = 0;
  req.on("data", (c) => { n += c.length; });
  req.on("close", () => {
    console.log("req-close", n, req.readableEnded);
    client.close();
  });
  req.end();
});
"#,
    );
    for line in [
        "stream-close true true",
        "server-close",
        "req-close 1048576 true",
    ] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_server_options_no_mutation_faces() {
    // dont-override 套件面：server options 只浅拷后默认，不改用户原对象。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "o.mjs",
        r#"
import http2 from "node:http2";
const options = {};
const server = http2.createServer(options);
console.log("keys", JSON.stringify(Object.keys(options)));
server.on("stream", (stream) => {
  stream.respond({ ":status": 200 });
  stream.end("ok");
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request({ ":path": "/" });
  req.resume();
  req.on("end", () => { client.destroy(); server.close(); console.log("done"); });
});
"#,
    );
    for line in ["keys []", "done"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    dir.close().unwrap();
}

#[test]
fn http2_server_wanttrailers_no_listener_faces() {
    // no-wanttrailers-listener 套件面：无监听即自动发空 trailers 自然关闭，
    // 不挂死（node onStreamTrailers 口径：emit 为 false 即 sendTrailers({})）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "w.mjs",
        r#"
import http2 from "node:http2";
const server = http2.createServer();
server.on("stream", (stream) => {
  stream.respond(undefined, { waitForTrailers: true });
  stream.end("ok");
});
server.listen(0, () => {
  const client = http2.connect(`http://localhost:${server.address().port}`);
  const req = client.request();
  req.on("trailers", () => console.log("BAD-trailers"));
  req.resume();
  req.on("close", () => {
    console.log("req-close");
    server.close();
    client.close();
  });
});
"#,
    );
    for line in ["req-close"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.contains("BAD-trailers"), "unexpected trailers; out: {out}");
    dir.close().unwrap();
}
