//! tests/node/net_rawclose.rs — node:net 半关收尾面黑盒（C 轮 raw-close 收尾；
//! 与 net.rs 同域，按 §0.9 方法边界切分：半关单建一文件）。
use crate::common::*;
use crate::helpers::*;

#[test]
fn net_rawclose_end_after_fin_survives() {
    // raw-close 补（4.324 对侧）：对端已 FIN（半开）后才调 end，
    // halfhold 迟摘/已摘即装回续命至 close（node：半开 end 后句柄仍活至 close）。
    // end 监听内同步 end（__ended 真）即非闲置，不摘（摘即 loop 排空收尾丢包）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "rc.mjs",
        r#"
import net from "node:net";
setTimeout(() => { console.log("HANG"); process.exit(1); }, 8000).unref();
// 形一：end 监听内同步 end（微任务递延 halfhold 前已 __ended，不摘）。
{
  const srv = net.createServer((s) => {
    s.on("data", () => { s.end("bye"); });
    s.on("close", () => srv.close(() => console.log("a-srv-close")));
  });
  await new Promise((r) => srv.listen(0, "127.0.0.1", r));
  await new Promise((resolve) => {
    const c = net.connect({ port: srv.address().port, host: "127.0.0.1", allowHalfOpen: true });
    c.on("data", () => {});
    c.on("end", () => { console.log("a-cli-end"); c.end(); });
    c.on("close", () => { console.log("a-cli-close"); resolve(); });
    c.write("hi");
  });
}
// 形二：迟调 end（halfhold 已摘，end 时装回续命，close 照到）。
{
  const srv = net.createServer((s) => {
    s.on("data", () => { s.end("bye"); });
    s.on("close", () => srv.close(() => console.log("b-srv-close")));
  });
  await new Promise((r) => srv.listen(0, "127.0.0.1", r));
  await new Promise((resolve) => {
    const c = net.connect({ port: srv.address().port, host: "127.0.0.1", allowHalfOpen: true });
    c.on("data", () => {});
    c.on("end", () => { console.log("b-cli-end"); setTimeout(() => c.end(), 50); });
    c.on("close", () => { console.log("b-cli-close"); resolve(); });
    c.write("hi");
  });
}
"#,
    );
    for line in ["a-cli-end", "a-cli-close", "a-srv-close", "b-cli-end", "b-cli-close", "b-srv-close"] {
        assert!(out.lines().any(|l| l == line), "missing `{line}`; out:\n{out}");
    }
    assert!(!out.contains("HANG"), "out:\n{out}");
    dir.close().unwrap();
}
