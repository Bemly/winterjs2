//! tests/node/http2_tunnel.rs — node:http2 CONNECT 隧道面黑盒（C 轮 px11 收尾；
//! 与 http2.rs/http2_faces.rs 同域，按 §0.9 方法边界切分：隧道单建一文件）。
use crate::helpers::*;

#[test]
fn http2_connect_deferred_end_patient() {
    // CONNECT 抑 EOS（4.322 对侧·4.326）：应答前 DATA 经升级桥递延补发；
    // 耐心对端（等上行字节到才回包）下隧道双向通。immediate-FIN 形仍输竞速，见偏离注。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "t.mjs",
        r#"
import net from "node:net";
import http2 from "node:http2";
const { HTTP2_HEADER_METHOD, HTTP2_HEADER_AUTHORITY } = http2.constants;
setTimeout(() => { console.log("HANG"); process.exit(1); }, 8000).unref();
const server = net.createServer((socket) => {
  let data = "";
  socket.setEncoding("utf8");
  socket.on("data", (c) => {
    data += c;
    if (data === "hello") socket.end("hello");
  });
  socket.on("end", () => console.log(data === "hello" ? "TARGET-OK" : "TARGET-MISMATCH"));
});
server.listen(0, () => {
  const port = server.address().port;
  const proxy = http2.createServer();
  proxy.on("stream", (stream, headers) => {
    if (headers[HTTP2_HEADER_METHOD] !== "CONNECT") return;
    const auth = new URL(`tcp://${headers[HTTP2_HEADER_AUTHORITY]}`);
    const socket = net.connect(auth.port, auth.hostname, () => {
      stream.respond();
      socket.pipe(stream);
      stream.pipe(socket);
    });
    socket.on("error", () => {});
  });
  proxy.listen(0, () => {
    const client = http2.connect(`http://localhost:${proxy.address().port}`);
    const req = client.request({
      [HTTP2_HEADER_METHOD]: "CONNECT",
      [HTTP2_HEADER_AUTHORITY]: `localhost:${port}`,
    });
    let data = "";
    req.setEncoding("utf8");
    req.on("data", (c) => (data += c));
    req.on("end", () => {
      console.log(data === "hello" ? "CLIENT-OK" : "CLIENT-MISMATCH");
      client.close();
      proxy.close();
      server.close();
    });
    req.end("hello");
  });
});
"#,
    );
    for line in ["TARGET-OK", "CLIENT-OK"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.contains("HANG"), "out:\n{out}");
    dir.close().unwrap();
}

#[test]
fn http2_tunnel_teardown_clean() {
    // 隧道拆除级联：内外层全关后进程干净退出（泵半关续读不得把收尾拖成永生，4.321 对侧）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "t.mjs",
        r#"
import h2 from "node:http2";
setTimeout(() => { console.log("HANG"); process.exit(1); }, 8000).unref();
const server = h2.createServer();
server.listen(0, () => {
  const proxyClient = h2.connect(`http://localhost:${server.address().port}`);
  const request = proxyClient.request({ ":method": "CONNECT", ":authority": "x:80" });
  request.on("response", () => {
    const nested = h2.connect("http://x", { createConnection: () => request });
    nested.on("connect", () => {
      const preq = nested.request();
      preq.on("response", (h) => console.log("status", h[":status"]));
      preq.on("end", () => {
        console.log("end");
        nested.close();
        proxyClient.close();
        server.close();
        console.log("all-closed");
      });
      preq.end();
    });
  });
});
server.once("connect", (req, res) => {
  res.writeHead(200);
  server.emit("connection", res.stream);
});
server.once("request", (req, res) => {
  res.writeHead(204);
  res.end();
});
"#,
    );
    for line in ["status 204", "end", "all-closed"] {
        assert!(out.lines().any(|l| l == line), "missing {line}; out: {out}");
    }
    assert!(!out.contains("HANG"), "out:\n{out}");
    dir.close().unwrap();
}
