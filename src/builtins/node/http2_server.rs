//! http2 服务端域（监听选项 + h2_listen；对齐 http2.rs；纯搬移）。

use mozjs::jsval::JSVal;
use crate::jsapi_glue::{report_error, value_to_string, wrap_cx, Frame};
use crate::state;
use super::http2::{b64, ensure_provider, opt_num, serve_conn, set_rval_str};
use super::net::{NetEvent, NetKind};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::collections::HashMap;

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

// ── 共享 IO（双向 wire 窥探 + 帧注入）────────────────────────────────────
// hyper 无 mid-connection SETTINGS/RST API，下列套件要求线形：
// - cant-turn-off：服务端直写坏 SETTINGS{8:0}（wire-faithful）；
// - stream-destroy-event-order：对端 RST 需落 'aborted'/错误（hyper 吞掉 service
//   的对端 RST 信号——service future 被静默丢弃，无事件；只能线窥探）；
// - 精确 RST 码（close(code)/session.destroy(code)）：hyper 只发 CANCEL/
//   INTERNAL_ERROR，线注精确码。
// poll 期串行（conn future 只由驱动 select 轮询），Mutex 仅解决所有权共享。
// wire_streams：按序收录 HEADERS 流 id（双向去重）；JS sid↔wire 映射凭位序
// （hyper 按收序调 service，JS seq 逐个对应；pre-service 拒收即散列，另案记档）。
#[derive(Debug)]
pub(crate) struct SharedIo<T> {
    inner: Arc<tokio::sync::Mutex<T>>,
    /// 发出侧握手 SETTINGS 是否已落线（cant-turn-off：坏帧必须排它之后）。
    hs_sent: Arc<std::sync::atomic::AtomicBool>,
    /// 发出侧帧重组缓冲（写分片跨 poll_write 时拼帧）。
    out_obs: Vec<u8>,
    /// 读入侧帧重组缓冲。
    in_obs: Vec<u8>,
    /// 收录的 HEADERS 流 id（收发双向；RST 映射与精确注入用；conn 作用域）。
    wire_streams: Arc<std::sync::Mutex<Vec<u32>>>,
    /// 已见 END_STREAM 的流（读入侧；client：响应收齐后的 RST 不再报 aborted，
    /// 与 hyper 的 END 处理竞速时序——node 同形：END 后 RST 只是 close）。
    in_ended: std::collections::HashSet<u32>,
    /// 已见 END_STREAM 的流（发出侧；server：响应发完后的 RST 不再报 aborted）。
    out_ended: std::collections::HashSet<u32>,
    /// 事件上报（client: Some(session 级)；server: Some(server 级)；无则只记不报）。
    ev: Option<(tokio::sync::mpsc::UnboundedSender<NetEvent>, u64)>,
    /// 服务端侧（映射位序/事件取舍不同）。
    is_server: bool,
}

impl<T> Clone for SharedIo<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            hs_sent: Arc::clone(&self.hs_sent),
            out_obs: Vec::new(),
            in_obs: Vec::new(),
            wire_streams: Arc::clone(&self.wire_streams),
            in_ended: std::collections::HashSet::new(),
            out_ended: std::collections::HashSet::new(),
            ev: self.ev.clone(),
            is_server: self.is_server,
        }
    }
}

impl<T> SharedIo<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(inner)),
            hs_sent: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            out_obs: Vec::new(),
            in_obs: Vec::new(),
            wire_streams: Arc::new(std::sync::Mutex::new(Vec::new())),
            in_ended: std::collections::HashSet::new(),
            out_ended: std::collections::HashSet::new(),
            ev: None,
            is_server: true,
        }
    }

    /// 事件上报装配（serve_conn: server 级；drive_session: session 级）。
    pub(crate) fn set_ev(
        &mut self,
        ev_tx: tokio::sync::mpsc::UnboundedSender<NetEvent>,
        ev_id: u64,
        is_server: bool,
    ) {
        self.ev = Some((ev_tx, ev_id));
        self.is_server = is_server;
    }

    pub(crate) fn hs_sent(&self) -> bool {
        self.hs_sent.load(std::sync::atomic::Ordering::Acquire)
    }

    /// wire 序位 → JS sid（server 1 基逐个；client 奇数 1,3,5…）。
    fn js_sid(&self, wire: u32) -> Option<u64> {
        let list = match self.wire_streams.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        list.iter().position(|&w| w == wire).map(|i| {
            if self.is_server {
                (i + 1) as u64
            } else {
                (2 * i + 1) as u64
            }
        })
    }

    /// JS sid → wire 序位（精确 RST 注入用；client 奇数列，server 逐个）。
    fn wire_sid(&self, js_sid: u64) -> Option<u32> {
        let list = match self.wire_streams.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        let idx = if self.is_server {
            js_sid.checked_sub(1)? as usize
        } else {
            if js_sid % 2 != 1 {
                return None;
            }
            ((js_sid - 1) / 2) as usize
        };
        list.get(idx).cloned()
    }

    /// 记录 HEADERS 流 id（收发双向去重；位序即 JS sid 序）。
    fn note_headers(&self, sid: u32) {
        if sid == 0 {
            return;
        }
        let mut list = match self.wire_streams.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if !list.contains(&sid) {
            // 防御上限：流表 conn 作用域，超限即冻结（不再收录，映射退化为跳过）。
            if list.len() < 100000 {
                list.push(sid);
            }
        }
    }

    fn emit(&self, stream_id: u64, what: &str, payload: String) {
        if let Some((tx, id)) = &self.ev {
            let _ = tx.send(NetEvent {
                id: *id,
                kind: NetKind::H2Stream {
                    stream_id,
                    what: what.into(),
                    payload,
                },
            });
        }
    }

    /// 帧流扫描前处理：h2 客户端前导 24 字节 magic（`PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`）
    /// 不是帧——收发任一侧首现即剥离，否则长度域误读致解析器永久卡死
    ///（stream-destroy-event-order：client 注入映射/RST 窥探全灭即源于此）。
    fn strip_magic(obs: &mut Vec<u8>) {
        const MAGIC: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        if obs.len() >= MAGIC.len() && obs[..MAGIC.len()] == *MAGIC {
            obs.drain(..MAGIC.len());
        }
    }

    /// 读入帧扫描：HEADERS 收录；RST 翻 aborted 事件；client 另解 SETTINGS/GOAWAY。
    fn observe_in(&mut self, fresh: &[u8]) {
        if fresh.is_empty() {
            return;
        }
        self.in_obs.extend_from_slice(fresh);
        if self.in_obs.len() > 65536 {
            self.in_obs.clear();
            return;
        }
        Self::strip_magic(&mut self.in_obs);
        loop {
            if self.in_obs.len() < 9 {
                break;
            }
            let len = ((self.in_obs[0] as usize) << 16)
                | ((self.in_obs[1] as usize) << 8)
                | (self.in_obs[2] as usize);
            if self.in_obs.len() < 9 + len {
                break;
            }
            let ftype = self.in_obs[3];
            let flags = self.in_obs[4];
            let sid = u32::from_be_bytes([
                self.in_obs[5],
                self.in_obs[6],
                self.in_obs[7],
                self.in_obs[8],
            ]) & 0x7fff_ffff;
            // END_STREAM 记账（读入侧；RST 迟到即不再报 aborted，见 in_ended）。
            if flags & 0x1 != 0 && (ftype == 0x1 || ftype == 0x0) && sid != 0 {
                self.in_ended.insert(sid);
            }
            if ftype == 0x1 {
                // HEADERS（含 trailers/CONTINUATION 首帧同形）：新流收录。
                // （trailers 复用流 id，去重后无影响。）
                self.note_headers(sid);
            } else if ftype == 0x3 && len == 4 {
                // 对端 RST：仅服务端翻 aborted（service future 被 hyper 静默丢弃，
                // 无此帧即永无 server-stream error；stream-destroy-event-order）。
                // 客户端不翻——任务错误（send_request/body）是唯一信号源；
                // 双源会造 aborted+error 双终点竞速（write-after-destroy）。
                // 干净码（0/8）亦跳过——node 不为其发 aborted/error。
                let code = u32::from_be_bytes([
                    self.in_obs[9],
                    self.in_obs[10],
                    self.in_obs[11],
                    self.in_obs[12],
                ]);
                // 该方向已 END 即跳过（END 后 RST 只是 close；与 hyper END
                // 处理竞速——先报 aborted 会把干净收尾翻成 CANCEL 错误）。
                let ended = if self.is_server {
                    self.out_ended.contains(&sid)
                } else {
                    self.in_ended.contains(&sid)
                };
                if self.is_server && !ended && code != 0 && code != 8 {
                    if let Some(js) = self.js_sid(sid) {
                        self.emit(js, "aborted", code.to_string());
                    }
                }
            } else if !self.is_server && ftype == 0x4 && sid == 0 && flags & 0x1 == 0 {
                let mut pairs: Vec<(u32, u32)> = Vec::new();
                let mut i = 9;
                while i + 6 <= 9 + len {
                    let id = ((self.in_obs[i] as u32) << 8) | (self.in_obs[i + 1] as u32);
                    let val = u32::from_be_bytes([
                        self.in_obs[i + 2],
                        self.in_obs[i + 3],
                        self.in_obs[i + 4],
                        self.in_obs[i + 5],
                    ]);
                    pairs.push((id, val));
                    i += 6;
                }
                let payload = serde_json::to_string(&pairs).unwrap_or_else(|_| "[]".into());
                self.emit(0, "remoteSettings", payload);
            } else if !self.is_server && ftype == 0x7 && len >= 8 {
                let last_id = u32::from_be_bytes([
                    self.in_obs[9],
                    self.in_obs[10],
                    self.in_obs[11],
                    self.in_obs[12],
                ]) & 0x7fff_ffff;
                let code = u32::from_be_bytes([
                    self.in_obs[13],
                    self.in_obs[14],
                    self.in_obs[15],
                    self.in_obs[16],
                ]);
                if code != 0 {
                    let payload = serde_json::json!({ "code": code, "lastId": last_id }).to_string();
                    self.emit(0, "goawayError", payload);
                }
            }
            self.in_obs.drain(..9 + len);
        }
    }

    /// 发出字节流扫描：首个非 ACK 服务端 SETTINGS 置 hs_sent；
    /// 发出侧 HEADERS 流 id 同步收录（client 精确 RST 注入映射用）。
    fn observe_out(&mut self, fresh: &[u8]) {
        if fresh.is_empty() {
            return;
        }
        self.out_obs.extend_from_slice(fresh);
        if self.out_obs.len() > 65536 {
            self.out_obs.clear();
            return;
        }
        Self::strip_magic(&mut self.out_obs);
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
            ]) & 0x7fff_ffff;
            // END_STREAM 记账（发出侧；RST 迟到即不再报 aborted，见 out_ended）。
            if flags & 0x1 != 0 && (ftype == 0x1 || ftype == 0x0) && sid != 0 {
                self.out_ended.insert(sid);
            }
            if ftype == 0x4 && sid == 0 && flags & 0x1 == 0 {
                self.hs_sent.store(true, std::sync::atomic::Ordering::Release);
            } else if ftype == 0x1 {
                self.note_headers(sid);
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

    /// 精确 RST_STREAM 注入（JS sid → wire 序位；hyper 无 API，只能线写）。
    /// 返 false = 映射未知（调用方回落 abort/静默，禁伤及无辜流）。
    pub(crate) async fn inject_rst(&self, js_sid: u64, code: u32) -> bool
    where
        T: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt as _;
        let Some(wire) = self.wire_sid(js_sid) else {
            return false;
        };
        let mut frame = [0u8; 13];
        frame[2] = 4;
        frame[3] = 0x3;
        frame[5..9].copy_from_slice(&(wire & 0x7fff_ffff).to_be_bytes());
        frame[9..13].copy_from_slice(&code.to_be_bytes());
        let mut guard = self.inner.lock().await;
        guard.write_all(&frame).await.is_ok() && guard.flush().await.is_ok()
    }

    /// GOAWAY(code) 注入（propagate-session-destroy-code：session.destroy(code)
    /// 先 GOAWAY 后逐流 RST；hyper graceful_shutdown 只发 GOAWAY(0)）。
    /// last_id 取已见最大 wire sid（在途全拒；node 同形近似）。
    pub(crate) async fn inject_goaway(&self, code: u32) -> bool
    where
        T: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt as _;
        let last: u32 = match self.wire_streams.lock() {
            Ok(g) => g.iter().cloned().max().unwrap_or(0),
            Err(e) => e.into_inner().iter().cloned().max().unwrap_or(0),
        };
        let mut frame = [0u8; 17];
        frame[2] = 8;
        frame[3] = 0x7;
        frame[9..13].copy_from_slice(&(last & 0x7fff_ffff).to_be_bytes());
        frame[13..17].copy_from_slice(&code.to_be_bytes());
        let mut guard = self.inner.lock().await;
        guard.write_all(&frame).await.is_ok() && guard.flush().await.is_ok()
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
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        // SAFETY: ReadBufCursor 的未填区即合法写区（TokioIo 同款）。
        let (n, fresh): (usize, Option<Vec<u8>>) = {
            let mut guard = match lock_poll(&self.inner, cx) {
                Some(g) => g,
                None => return Poll::Pending,
            };
            let uninit = unsafe { buf.as_mut() };
            let mut tbuf = tokio::io::ReadBuf::uninit(uninit);
            futures::ready!(tokio::io::AsyncRead::poll_read(
                Pin::new(&mut *guard),
                cx,
                &mut tbuf
            ))?;
            let n = tbuf.filled().len();
            let fresh = if n > 0 {
                Some(tbuf.filled().to_vec())
            } else {
                None
            };
            (n, fresh)
        };
        unsafe { buf.advance(n) };
        if let Some(f) = fresh {
            self.as_mut().observe_in(&f);
        }
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

// ── 注入式连接（'connection' 手动 emit；socket-close 套件）───────────────────
// node：Http2Server 的 connectionListener 对注入 socket 直接建会话（secure
// 种先服务端 TLS 握手）。本仓 listen 路径自有 accept，此处只承接手动 emit：
// 通道桥 AdoptIo（读走 inbox channel、写走 adoptData 事件经既有 H2Stream
// 通道），复用 serve_conn 全套语义（connOpen/connClose/aborted/request）。
// JS 侧：socket data → h2_adopt_data；socket close → h2_adopt_close（inbox
// EOF → serve_conn 收尾 → connClose）；写出经 server __ev "adoptData" 落
// socket.write；connClose 时连带 destroy 注入 socket（见 h2_server_http2server.js）。

/// adopt inbox 表（conn_id → 入站字节发送端；task 退出即摘，不泄漏）。
static ADOPT_INBOX: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<u64, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
> = std::sync::OnceLock::new();
fn adopt_inbox() -> &'static std::sync::Mutex<
    std::collections::HashMap<u64, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
> {
    ADOPT_INBOX.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 通道桥 IO：读←inbox channel（sender 丢弃即 EOF）；写→H2Stream adoptData
/// 事件（payload = "id b64"，JS 侧落传输）。服务端注入（socket/流）与
/// 客户端 createConnection 共用（后者见 http2_client.rs）。
pub(crate) struct AdoptIo {
    inbox: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    pending: std::collections::VecDeque<u8>,
    eof: bool,
    ev_tx: tokio::sync::mpsc::UnboundedSender<NetEvent>,
    conn_id: u64,
}

impl AdoptIo {
    pub(crate) fn new(
        inbox: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        ev_tx: tokio::sync::mpsc::UnboundedSender<NetEvent>,
        conn_id: u64,
    ) -> Self {
        AdoptIo { inbox, pending: std::collections::VecDeque::new(), eof: false, ev_tx, conn_id }
    }
}

impl tokio::io::AsyncRead for AdoptIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        loop {
            while buf.remaining() > 0 {
                if let Some(b) = self.pending.pop_front() {
                    buf.put_slice(&[b]);
                } else {
                    break;
                }
            }
            if !buf.filled().is_empty() || self.eof {
                return Poll::Ready(Ok(()));
            }
            match self.inbox.poll_recv(cx) {
                Poll::Ready(Some(bytes)) => self.pending.extend(bytes),
                Poll::Ready(None) => {
                    self.eof = true;
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl tokio::io::AsyncWrite for AdoptIo {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        use base64::Engine as _;
        let payload = format!(
            "{} {}",
            self.conn_id,
            base64::engine::general_purpose::STANDARD.encode(buf)
        );
        self.ev_tx
            .send(NetEvent {
                id: self.conn_id,
                kind: NetKind::H2Stream {
                    stream_id: 0,
                    what: "adoptData".into(),
                    payload,
                },
            })
            .map(|()| buf.len())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            .into()
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        // 关闭语义由 serve_conn 尾部 connClose 承载（JS 侧连带 destroy socket）。
        Poll::Ready(Ok(()))
    }
}

/// `__wjs2_h2_adopt(optsJson{tls?,settings?}, peerStr, target)` → conn_id 数字串。
/// server_id 取 conn_id（adopt 连接独占 target 路由，见 dispatch）。
pub unsafe extern "C" fn h2_adopt(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 || !frame.arg(2).is_object() {
        report_error(&mut cx, "TypeError: h2 adopt internals missing target");
        return false;
    }
    let opts: ListenOpts =
        serde_json::from_str(&value_to_string(&mut cx, frame.arg(0))).unwrap_or_default();
    let peer_str = value_to_string(&mut cx, frame.arg(1));
    let srv = {
        let s = opts.settings.unwrap_or_default();
        super::http2::ServerConnSettings {
            enable_connect_protocol: s.enable_connect_protocol.unwrap_or(false),
            max_header_list_size: s.max_header_list_size.or(s.max_header_size),
        }
    };
    let target = frame.arg(2);
    ensure_provider();
    // TLS 配置预检（fail fast；h2c 无配置）。
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
    let Some(ev_tx) = state::net_ev_tx() else {
        report_error(&mut cx, "OperationError: net driver not installed");
        return false;
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        report_error(&mut cx, "OperationError: no async runtime for h2 adopt");
        return false;
    };
    let (conn_id, conn_cmd_rx) = state::net_conn_add();
    state::net_target_add(conn_id, target);
    let (in_tx, in_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    adopt_inbox().lock().expect("adopt inbox").insert(conn_id, in_tx);
    let peer: std::net::SocketAddr = peer_str
        .parse()
        .unwrap_or_else(|_| "127.0.0.1:0".parse().expect("literal addr"));
    let ev2 = ev_tx.clone();
    handle.spawn(async move {
        let io = AdoptIo::new(in_rx, ev2.clone(), conn_id);
        if let Some(acc) = tls_cfg.map(tokio_rustls::TlsAcceptor::from) {
            match acc.accept(io).await {
                Err(_) => {
                    state::net_purge(conn_id);
                }
                Ok(tls) => {
                    serve_conn(tls, srv, conn_id, conn_id, peer, ev2, conn_cmd_rx).await;
                }
            }
        } else {
            serve_conn(io, srv, conn_id, conn_id, peer, ev2, conn_cmd_rx).await;
        }
        adopt_inbox().lock().expect("adopt inbox").remove(&conn_id);
    });
    set_rval_str(&mut cx, &frame, &conn_id.to_string());
    true
}

/// `__wjs2_h2_adopt_data(connId, b64)`：入站字节喂 inbox（entry gone 即静默真）。
pub unsafe extern "C" fn h2_adopt_data(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    // arg(1) 越界即 debug 断言崩（Frame::arg 无 None 形），先门 argc。
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: h2 adopt data: bad args");
        return false;
    }
    let (Some(cid), b64) = (opt_num(&frame, 0), value_to_string(&mut cx, frame.arg(1))) else {
        report_error(&mut cx, "TypeError: h2 adopt data: bad args");
        return false;
    };
    if let Some(tx) = adopt_inbox().lock().expect("adopt inbox").get(&(cid as u64)) {
        use base64::Engine as _;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
            let _ = tx.send(bytes);
        }
    }
    true
}

/// `__wjs2_h2_adopt_close(connId)`：摘 inbox（读端见 EOF → serve_conn 收尾）。
pub unsafe extern "C" fn h2_adopt_close(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let Some(cid) = opt_num(&frame, 0) else {
        report_error(&mut cx, "TypeError: h2 adopt close: conn id must be a number");
        return false;
    };
    adopt_inbox().lock().expect("adopt inbox").remove(&(cid as u64));
    true
}

/// CONNECT 隧道态（proxy-over-h2；per-conn）：待取 upgrade 句柄 /
/// 隧道写出通道（None 消息即关写半部）/ 已升级旗。
pub(crate) type TunnelFuts = Arc<tokio::sync::Mutex<HashMap<u64, hyper::upgrade::OnUpgrade>>>;
pub(crate) type TunnelOuts =
    Arc<tokio::sync::Mutex<HashMap<u64, tokio::sync::mpsc::UnboundedSender<Option<Vec<u8>>>>>>;

/// CONNECT 隧道泵（proxy-over-h2）：hyper 在 2xx 应答后将流升级为隧道，
/// 后续 DATA 只进 upgrade 句柄（常规 Incoming 恒空）；且句柄必须持有——
/// 随 req 析构丢弃即触发隧道拆除（空 DATA-EOS + RST(0)，外层枉死）。
/// 本泵持有句柄（禁拆）并转隧道字节为体事件；非 2xx（405 等）时 future
/// 直接 Err → 空收尾（与常规空体流事件同形）。
pub(crate) async fn pump_upgrade_body(
    fut: hyper::upgrade::OnUpgrade,
    outbox: tokio::sync::mpsc::UnboundedReceiver<Option<Vec<u8>>>,
    ev_tx: tokio::sync::mpsc::UnboundedSender<NetEvent>,
    server_id: u64,
    stream_id: u64,
) {
    use hyper::rt::{Read as _, Write as _};
    let send = |what: &str, payload: String| {
        let _ = ev_tx.send(NetEvent {
            id: server_id,
            kind: NetKind::H2Stream { stream_id, what: what.into(), payload },
        });
    };
    let mut upgraded = match fut.await {
        Err(_) => {
            send("reqEnd", "[]".into());
            return;
        }
        Ok(u) => u,
    };
    // hyper::rt 经 poll_fn 双向驱动（Upgraded 无 tokio AsyncRead）。
    // 读→体事件；写←outbox（None 关写半部）；读侧 EOF 后仍服务写侧。
    let mut buf = vec![0u8; 32768];
    let mut outbox = outbox;
    let mut read_done = false;
    // 写侧关闭后仍服务读侧（2026-10-21，px11 递延 END 对侧）：
    // 隧道半关是常态（上行 END 先于下行数据到达），写关即退会吞下行尾包；
    // 双侧皆关才退。注意拆除传播仍靠 poll_shutdown/close_notify，见 4.321。
    let mut out_closed = false;
    loop {
        if read_done && out_closed {
            break;
        }
        enum Ev {
            Readed(Result<usize, std::io::Error>),
            Out(Option<Option<Vec<u8>>>),
        }
        let ev = if read_done {
            match outbox.recv().await {
                Some(m) => Ev::Out(Some(m)),
                None => Ev::Out(None),
            }
        } else {
            tokio::select! {
                r = async {
                    let mut readbuf = hyper::rt::ReadBuf::new(&mut buf);
                    std::future::poll_fn(|cx| {
                        let r = std::pin::Pin::new(&mut upgraded)
                            .poll_read(cx, readbuf.unfilled());
                        match r {
                            std::task::Poll::Ready(Ok(())) => {
                                std::task::Poll::Ready(Ok(readbuf.filled().len()))
                            }
                            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(e)),
                            std::task::Poll::Pending => std::task::Poll::Pending,
                        }
                    })
                    .await
                } => Ev::Readed(r),
                m = async {
                    if out_closed {
                        std::future::pending::<Option<Option<Vec<u8>>>>().await
                    } else {
                        outbox.recv().await
                    }
                } => Ev::Out(m),
            }
        };
        match ev {
            Ev::Readed(Ok(0)) | Ev::Readed(Err(_)) => {
                read_done = true;
                send("reqEnd", "[]".into());
            }
            Ev::Readed(Ok(n)) => {
                send("body", b64(&buf[..n]));
            }
            Ev::Out(None) => {
                out_closed = true;
            }
            Ev::Out(Some(None)) => {
                let _ = std::future::poll_fn(|cx| {
                    std::pin::Pin::new(&mut upgraded).poll_shutdown(cx)
                })
                .await;
            }
            Ev::Out(Some(Some(bytes))) => {
                let mut off = 0;
                while off < bytes.len() {
                    let w = std::future::poll_fn(|cx| {
                        std::pin::Pin::new(&mut upgraded)
                            .poll_write(cx, &bytes[off..])
                    })
                    .await;
                    match w {
                        Ok(0) | Err(_) => break,
                        Ok(n) => off += n,
                    }
                }
            }
        }
    }
}

