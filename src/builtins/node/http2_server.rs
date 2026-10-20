//! http2 服务端域（监听选项 + h2_listen；对齐 http2.rs；纯搬移）。

use mozjs::jsval::JSVal;
use crate::jsapi_glue::{report_error, value_to_string, wrap_cx, Frame};
use crate::state;
use super::http2::{ensure_provider, opt_num, serve_conn, set_rval_str};
use super::net::{NetEvent, NetKind};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// 监听选项 JSON：`{tls?: {cert, key}, settings?: {enableConnectProtocol?, maxHeaderListSize?, maxHeaderSize?}}`（h2c 缺省）。
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct ListenOpts {
    tls: Option<TlsServerOpts>,
    settings: Option<ServerH2Settings>,
}

/// createServer settings 直传子集（hyper Builder 消费；其余键忽略）。
#[derive(Debug, Default, serde::Deserialize, Clone, Copy)]
#[serde(default)]
struct ServerH2Settings {
    #[serde(rename = "enableConnectProtocol")]
    enable_connect_protocol: Option<bool>,
    #[serde(rename = "maxHeaderListSize")]
    max_header_list_size: Option<u32>,
    #[serde(rename = "maxHeaderSize")]
    max_header_size: Option<u32>,
}

#[derive(Debug, Default, serde::Deserialize, Clone)]
#[serde(default)]
struct TlsServerOpts {
    cert: Option<String>,
    key: Option<String>,
}

/// `__wjs2_h2_listen(port, host, optsJson, target)` → server id。
pub unsafe extern "C" fn h2_listen(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 || !frame.arg(3).is_object() {
        report_error(&mut cx, "TypeError: h2 listen internals missing target");
        return false;
    }
    let Some(port) = opt_num(&frame, 0) else {
        report_error(&mut cx, "TypeError: h2 listen: port must be a number");
        return false;
    };
    let host = value_to_string(&mut cx, frame.arg(1));
    let opts: ListenOpts =
        serde_json::from_str(&value_to_string(&mut cx, frame.arg(2))).unwrap_or_default();
    // createServer settings → hyper Builder（connect-method-extended/too-large-headers；
    // node maxHeaderSize 即 maxHeaderListSize 别名，maxHeaderListSize 优先）。
    let srv = {
        let s = opts.settings.unwrap_or_default();
        super::http2::ServerConnSettings {
            enable_connect_protocol: s.enable_connect_protocol.unwrap_or(false),
            max_header_list_size: s.max_header_list_size.or(s.max_header_size),
        }
    };
    let target = frame.arg(3);
    ensure_provider();
    // TLS 配置预检（fail fast；h2c 无配置）
    let tls_cfg: Option<std::sync::Arc<rustls::ServerConfig>> = match opts.tls {
        None => None,
        Some(t) => {
            let (Some(cert), Some(key)) = (t.cert, t.key) else {
                report_error(&mut cx, "TypeError: http2 secure server needs { key, cert }");
                return false;
            };
            match crate::builtins::node::tls::server_config_h2(&cert, &key) {
                Ok(c) => Some(std::sync::Arc::new(c)),
                Err(e) => {
                    report_error(&mut cx, &e);
                    return false;
                }
            }
        }
    };
    let Some((id, ev_tx)) = state::net_alloc() else {
        report_error(&mut cx, "OperationError: net driver not installed");
        return false;
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        report_error(&mut cx, "OperationError: no async runtime for h2 listen");
        return false;
    };
    // 同步 bind（node 口径：listen(0) 当 tick 即可 address().port；
    // 仅 'listening' 事件递延）。失败走原异步错误通道（id:0 占位）。
    let std_bound = std::net::TcpListener::bind((host.as_str(), port as u16));
    let picked_port: u16 = match &std_bound {
        Ok(l) => l.local_addr().map(|a| a.port()).unwrap_or(0),
        Err(_) => 0,
    };
    let mut cmd_rx = state::net_socket_add(id, target);
    set_rval_str(&mut cx, &frame, &format!("{id}:{picked_port}"));
    handle.spawn(async move {
        let listener = match std_bound {
            Ok(std_l) => {
                // from_std 要求非阻塞（tokio 不接管 blocking fd）。
                let _ = std_l.set_nonblocking(true);
                match tokio::net::TcpListener::from_std(std_l) {
                    Ok(t) => t,
                    Err(e) => {
                        let code = crate::builtins::node::fs::io_code(&e);
                        let _ = ev_tx.send(NetEvent {
                            id,
                            kind: NetKind::ServerError { code: code.into(), msg: format!("{code}: {e}") },
                        });
                        let _ = ev_tx.send(NetEvent { id, kind: NetKind::ServerClose });
                        return;
                    }
                }
            }
            Err(e) => {
                let code = crate::builtins::node::fs::io_code(&e);
                let _ = ev_tx.send(NetEvent {
                    id,
                    kind: NetKind::ServerError { code: code.into(), msg: format!("{code}: {e}") },
                });
                let _ = ev_tx.send(NetEvent { id, kind: NetKind::ServerClose });
                return;
            }
        };
        let local = listener
            .local_addr()
            .unwrap_or_else(|_| "0.0.0.0:0".parse::<std::net::SocketAddr>().expect("literal addr"));
        let _ = ev_tx.send(NetEvent {
            id,
            kind: NetKind::Listening { addr: local.ip().to_string(), port: local.port() },
        });
        // 存活连接表（server close 时逐个 Close；conn 退出经 done 通道摘除）
        let mut live: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let (done_tx, mut done_rx) =
            tokio::sync::mpsc::unbounded_channel::<u64>();
        let acceptor = tls_cfg.map(tokio_rustls::TlsAcceptor::from);
        loop {
            tokio::select! {
                acc = listener.accept() => {
                    let Ok((stream, peer)) = acc else { continue };
                    let (conn_id, conn_cmd_rx) = state::net_conn_add();
                    // conn 复用 server 的 target（connClose/aborted 事件在
                    // ServerClose purge 后仍可达；node：server.close 不杀活连接）
                    if let Some(t) = state::net_target(id) {
                        state::net_target_add(conn_id, t);
                    }
                    live.insert(conn_id);
                    let ev2 = ev_tx.clone();
                    let done2 = done_tx.clone();
                    if let Some(acc) = acceptor.clone() {
                        let fut = async move {
                            match acc.accept(stream).await {
                                Err(_) => {
                                    state::net_purge(conn_id);
                                    let _ = done2.send(conn_id);
                                }
                                Ok(tls) => {
                                    serve_conn(tls, srv, id, conn_id, peer, ev2, conn_cmd_rx).await;
                                    let _ = done2.send(conn_id);
                                }
                            }
                        };
                        tokio::spawn(fut);
                    } else {
                        let fut = async move {
                            serve_conn(stream, srv, id, conn_id, peer, ev2, conn_cmd_rx).await;
                            let _ = done2.send(conn_id);
                        };
                        tokio::spawn(fut);
                    }
                }
                done = done_rx.recv() => {
                    if let Some(cid) = done {
                        live.remove(&cid);
                    }
                }
                _ = cmd_rx.recv() => break,
            }
        }
        // node 默认：server.close() 不杀活连接，各 conn 由对端关闭后自行退出
        let _ = ev_tx.send(NetEvent { id, kind: NetKind::ServerClose });
    });
    true
}

// ── 共享 IO（服务端：直写 SETTINGS 帧 + 握手 SETTINGS 落线检测）────────
// http2.rs serve_conn 用（纯搬移；servo 式集中边界外唯一例外见 §6）。
// ── 共享 IO（服务端：连接级协议错误需绕过 hyper 直写 SETTINGS 帧；
// hyper 无 mid-connection SETTINGS API，cant-turn-off 套件要求线上有
// SETTINGS{ENABLE_CONNECT_PROTOCOL:0}。poll 期串行（conn future 只由
// serve_conn 的 select 驱动），Mutex 仅解决所有权共享，无实质竞争。
#[derive(Debug)]
pub(crate) struct SharedIo<T> {
    inner: Arc<tokio::sync::Mutex<T>>,
    /// 发出侧握手 SETTINGS 是否已落线（cant-turn-off：坏帧必须排它之后；
    /// serve_conn 预轮询凭此旗收敛，不与握手竞速）。
    hs_sent: Arc<std::sync::atomic::AtomicBool>,
    /// 发出侧帧重组缓冲（写分片跨 poll_write 时拼帧）。
    out_obs: Vec<u8>,
}

impl<T> Clone for SharedIo<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            hs_sent: Arc::clone(&self.hs_sent),
            out_obs: Vec::new(),
        }
    }
}

impl<T> SharedIo<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(inner)),
            hs_sent: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            out_obs: Vec::new(),
        }
    }

    pub(crate) fn hs_sent(&self) -> bool {
        self.hs_sent.load(std::sync::atomic::Ordering::Acquire)
    }

    /// 发出字节流：首个非 ACK 服务端 SETTINGS 即置 hs_sent。
    fn observe_out(&mut self, fresh: &[u8]) {
        if fresh.is_empty() || self.hs_sent() {
            return;
        }
        self.out_obs.extend_from_slice(fresh);
        if self.out_obs.len() > 65536 {
            self.out_obs.clear();
            return;
        }
        loop {
            if self.out_obs.len() < 9 {
                break;
            }
            let len = ((self.out_obs[0] as usize) << 16)
                | ((self.out_obs[1] as usize) << 8)
                | (self.out_obs[2] as usize);
            if self.out_obs.len() < 9 + len {
                break;
            }
            let ftype = self.out_obs[3];
            let flags = self.out_obs[4];
            let sid = u32::from_be_bytes([
                self.out_obs[5],
                self.out_obs[6],
                self.out_obs[7],
                self.out_obs[8],
            ]);
            if ftype == 0x4 && sid == 0 && flags & 0x1 == 0 {
                self.hs_sent.store(true, std::sync::atomic::Ordering::Release);
                self.out_obs.clear();
                break;
            }
            self.out_obs.drain(..9 + len);
        }
    }

    /// 毒化注入：直写 SETTINGS{ENABLE_CONNECT_PROTOCOL:0}（cant-turn-off；
    /// 调用时 conn future 已不再被轮询（循环退出后兜底）或正被主循环驱动
    /// （waiter），串行无竞争。不断连——node 同形：坏帧后 socket 照开，
    /// 收尾由客户端 teardown 触发。
    pub(crate) async fn inject_proto_error(&self)
    where
        T: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt as _;
        // SETTINGS len=6 type=0x4 flags=0 stream=0 id=0x8 val=0（node 线形逐字节对拍）。
        const BAD: &[u8] = &[
            0x00, 0x00, 0x06, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut guard = self.inner.lock().await;
        let _ = guard.write_all(BAD).await;
        let _ = guard.flush().await;
    }
}

/// poll 期取锁（毒化注入只在循环退出后发生，轮询期永不竞争；万一失手则
/// 自唤醒重轮询，不静默 Pending）。
fn lock_poll<'a, T>(
    m: &'a tokio::sync::Mutex<T>,
    cx: &mut Context<'_>,
) -> Option<tokio::sync::MutexGuard<'a, T>> {
    match m.try_lock() {
        Ok(g) => Some(g),
        Err(_) => {
            cx.waker().wake_by_ref();
            None
        }
    }
}

impl<T> hyper::rt::Read for SharedIo<T>
where
    T: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let mut guard = match lock_poll(&self.inner, cx) {
            Some(g) => g,
            None => return Poll::Pending,
        };
        // SAFETY: ReadBufCursor 的未填区即合法写区（TokioIo 同款）。
        let uninit = unsafe { buf.as_mut() };
        let mut tbuf = tokio::io::ReadBuf::uninit(uninit);
        futures::ready!(tokio::io::AsyncRead::poll_read(
            Pin::new(&mut *guard),
            cx,
            &mut tbuf
        ))?;
        let n = tbuf.filled().len();
        unsafe { buf.advance(n) };
        Poll::Ready(Ok(()))
    }
}

impl<T> hyper::rt::Write for SharedIo<T>
where
    T: tokio::io::AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let n_opt = {
            let mut guard = match lock_poll(&self.inner, cx) {
                Some(g) => g,
                None => return Poll::Pending,
            };
            match tokio::io::AsyncWrite::poll_write(Pin::new(&mut *guard), cx, buf) {
                Poll::Ready(Ok(n)) => Some(n),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => None,
            }
        };
        if let Some(n) = n_opt {
            self.as_mut().observe_out(&buf[..n]);
            Poll::Ready(Ok(n))
        } else {
            Poll::Pending
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let mut guard = match lock_poll(&self.inner, cx) {
            Some(g) => g,
            None => return Poll::Pending,
        };
        tokio::io::AsyncWrite::poll_flush(Pin::new(&mut *guard), cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let mut guard = match lock_poll(&self.inner, cx) {
            Some(g) => g,
            None => return Poll::Pending,
        };
        tokio::io::AsyncWrite::poll_shutdown(Pin::new(&mut *guard), cx)
    }
}
