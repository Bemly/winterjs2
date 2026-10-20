//! http2 客户端域（连接选项 + h2_connect；对齐 http2.rs；纯搬移）。

use mozjs::jsval::JSVal;
use crate::jsapi_glue::{report_error, value_to_string, wrap_cx, Frame};
use crate::state;
use super::http2::{BodyFeeds, BodyMsg, ChanBody, Exec, b64, b64dec, ensure_provider, h2_err_msg_rst, headers_json, report_error_static, opt_num, set_rval_str};
use super::http2_server::{AdoptIo, SharedIo};
use super::net::{NetCmd, NetEvent, NetKind};
use std::collections::HashMap;
use std::sync::Arc;

// ── 客户端 ──────────────────────────────────────────────────────────────────

/// 连接选项 JSON：`{tls?: {ca?, rejectUnauthorized?, servername?}}`。
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct ConnectOpts {
    tls: Option<TlsClientOpts>,
}

#[derive(Debug, Default, serde::Deserialize, Clone)]
#[serde(default)]
struct TlsClientOpts {
    #[serde(rename = "ca")]
    ca_pem: Option<String>,
    #[serde(rename = "rejectUnauthorized")]
    reject_unauthorized: Option<bool>,
    servername: Option<String>,
}

/// 开流请求 JSON：`{method, path, scheme, authority, waitTrailers, endStream, protocol, headers: [[k,v]]}`。
#[derive(Debug, serde::Deserialize)]
struct OpenReq {
    method: String,
    #[serde(default)]
    path: String,
    #[serde(default = "default_scheme")]
    scheme: String,
    #[serde(default)]
    authority: String,
    #[serde(rename = "waitTrailers", default)]
    wait_trailers: bool,
    // JS 缺省意图（GET/HEAD/DELETE 无 body 即随头 fin；POST 分开；
    // endafterheaders 套件）。native 不再按体空猜。
    #[serde(rename = "endStream", default)]
    end_stream: bool,
    // 流式上传（cancel-while-client-reading：首 write 即开流，后续块经
    // H2RespondData、收尾经 H2RespondEnd；无新 native，复用既有命令）。
    // 非流式保持单发语义（整包 body + End/EndPending 一次到位）。
    #[serde(default)]
    streaming: bool,
    // 扩展 CONNECT 的 `:protocol`（RFC 8441；hyper 经 extensions 发送）。
    // 空串 = 非扩展请求（常规 CONNECT/他法无此键）。
    #[serde(default)]
    protocol: String,
    // 值可为 string 或 array（set-cookie 等多值头；数组展开为多条线，
    // node 客户端同口径）。此前 String 收不了 sequence 即
    // "bad open params: invalid type: sequence"（cookies/multiheaders 套件）。
    #[serde(default)]
    headers: Vec<Vec<serde_json::Value>>,
}

/// OpenReq 头展开：string 值单条，array 值逐元素展开。
fn openreq_headers(req: &OpenReq) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for pair in &req.headers {
        let Some(name) = pair.first().and_then(|v| v.as_str()) else { continue };
        let values: Vec<String> = match pair.get(1) {
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(serde_json::Value::Number(n)) => vec![n.to_string()],
            Some(serde_json::Value::Bool(b)) => vec![b.to_string()],
            // 客户端 cookie 数组已在 JS 侧 "; " 并串，其余数组展开为多条线
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    other => other.to_string(),
                })
                .collect(),
            _ => continue,
        };
        for v in values {
            out.push((name.to_ascii_lowercase(), v));
        }
    }
    out
}

fn default_scheme() -> String {
    "http".into()
}

/// CONNECT 隧道泵·客户端（proxy-over-h2）：200 后 hyper 将流升级，后续
/// DATA 只进 upgrade 句柄；且句柄必须持有（随 resp 析构丢弃即隧道拆除）。
/// 读→outer `data` 事件；写←outbox（None 关写半部）；EOF/错即 `end` + 记完结。
/// 调用方已决议隧道（upgrade Ok 才起泵），此处只做中继。
async fn pump_client_upgrade(
    mut upgraded: hyper::upgrade::Upgraded,
    outbox: tokio::sync::mpsc::UnboundedReceiver<Option<Vec<u8>>>,
    ev_tx: tokio::sync::mpsc::UnboundedSender<NetEvent>,
    id: u64,
    stream_id: u64,
    open: Arc<std::sync::Mutex<HashMap<u64, bool>>>,
) {
    use hyper::rt::{Read as _, Write as _};
    let send = |what: &str, payload: String| {
        let _ = ev_tx.send(NetEvent {
            id,
            kind: NetKind::H2Stream { stream_id, what: what.into(), payload },
        });
    };
    let mut buf = vec![0u8; 32768];
    let mut outbox = outbox;
    let mut read_done = false;
    loop {
        enum Ev {
            Readed(Result<usize, std::io::Error>),
            Out(Option<Option<Vec<u8>>>),
        }
        let ev = if read_done {
            match outbox.recv().await {
                Some(m) => Ev::Out(Some(m)),
                None => break,
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
                m = outbox.recv() => Ev::Out(m),
            }
        };
        match ev {
            Ev::Readed(Ok(0)) | Ev::Readed(Err(_)) => {
                read_done = true;
                open.lock().unwrap().insert(stream_id, true);
                send("end", String::new());
            }
            Ev::Readed(Ok(n)) => {
                send("data", super::http2::b64(&buf[..n]));
            }
            Ev::Out(None) => break,
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

/// 客户端 session 驱动（conn 与命令同 task `select!`；终结即 Close 事件）。
/// 10f：response 事件带真 flags（END_STREAM→5）、trailers 事件、session 终结时
/// 未完结流补 aborted（先于 close，§4.52 顺序）。
async fn drive_session<S>(
    raw: S,
    id: u64,
    ev_tx: tokio::sync::mpsc::UnboundedSender<NetEvent>,
    mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<NetCmd>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    // 双向 wire 窥探 + 精确 RST 注入（hyper client 无 SETTINGS 回调/RST 码 API；
    // remoteSettings/goawayError/对端 RST-aborted 事件源；close(code) 原码上线）。
    let mut io = SharedIo::new(raw);
    io.set_ev(ev_tx.clone(), id, false);
    let injector = io.clone();
    // 精确 RST 开关：任一 send_request 出错即 wire 序位不可信，此后只 abort
    // 不注码（禁伤及无辜流；stream-destroy 的单流场景恒精确）。
    let exact_rst = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let hs = hyper::client::conn::http2::Builder::new(Exec).handshake::<_, ChanBody>(io);
    let (sender, conn) = match hs.await {
        Ok(t) => t,
        Err(e) => {
            let _ = ev_tx.send(NetEvent {
                id,
                kind: NetKind::Error {
                    code: "ERR_HTTP2_CONNECT".into(),
                    msg: format!("ERR_HTTP2_CONNECT: {e}"),
                },
            });
            if state::net_close_once(id) {
                let _ = ev_tx.send(NetEvent { id, kind: NetKind::H2SessionClose });
            }
            return;
        }
    };
    let _ = ev_tx.send(NetEvent { id, kind: NetKind::Connect { local: None } });
    // 开流登记：stream_id → 响应体是否已完结（session 死时未完结者报 aborted）。
    let open: Arc<std::sync::Mutex<HashMap<u64, bool>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    // 上传体通道（waitTrailers 悬置流；trailer 命令回注）。
    let uploads: BodyFeeds = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    // CONNECT 隧道态（proxy-over-h2；per-session）：升级写出通道
    // （None 即关写半部；存在即已升级，写改道凭此）。
    let ctunnel_out: Arc<
        tokio::sync::Mutex<HashMap<u64, tokio::sync::mpsc::UnboundedSender<Option<Vec<u8>>>>>,
    > = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    // 在途请求任务句柄（H2RespondReset → abort → hyper RST(CANCEL)）
    let stream_tasks: Arc<std::sync::Mutex<HashMap<u64, tokio::task::JoinHandle<()>>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    tokio::pin!(conn);
    loop {
        tokio::select! {
            r = &mut conn => {
                let _ = r;
                // conn 收尾与流任务唤醒竞态（socket-destroy-delayed 根因）：
                // 流任务（send_request 已决议、事件待发）若排在驱动退出之后，
                // 其 response/data/end 将因 H2SessionClose 的 purge 而被丢弃。
                // 此处先等流任务落定（2s 上限防真挂死），再发 aborted/close，
                // 保证"应答先于终结"的 node 序。
                let pending: Vec<tokio::task::JoinHandle<()>> =
                    stream_tasks.lock().unwrap().drain().map(|(_, h)| h).collect();
                if !pending.is_empty() {
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                        for h in pending {
                            let _ = h.await;
                        }
                    })
                    .await;
                }
                if state::net_close_once(id) {
                    for (stream_id, done) in open.lock().unwrap().iter() {
                        if !*done {
                            let _ = ev_tx.send(NetEvent {
                                id,
                                kind: NetKind::H2Stream {
                                    stream_id: *stream_id,
                                    what: "aborted".into(),
                                    payload: String::new(),
                                },
                            });
                        }
                    }
                    let _ = ev_tx.send(NetEvent { id, kind: NetKind::H2SessionClose });
                }
                return;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(NetCmd::H2Open { stream_id, headers, body_b64 }) => {
                        let req: OpenReq = match serde_json::from_str(&headers) {
                            Ok(r) => r,
                            Err(e) => {
                                let _ = ev_tx.send(NetEvent {
                                    id,
                                    kind: NetKind::H2Stream {
                                        stream_id,
                                        what: "error".into(),
                                        payload: serde_json::json!({
                                            "code": "ERR_HTTP2_STREAM_ERROR",
                                            "msg": format!("bad open params: {e}"),
                                        })
                                        .to_string(),
                                    },
                                });
                                continue;
                            }
                        };
                        let body = b64dec(&body_b64).unwrap_or_default();
                        let parsed_headers = openreq_headers(&req);
                        // :authority 经完整 URI 带（hyper h2 客户端不合成伪头）；
                        // authority 为空回落 host 头（host-only 请求，host 回落套件）。
                        let authority = if req.authority.is_empty() {
                            parsed_headers
                                .iter()
                                .find(|(k, _)| k == "host")
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default()
                        } else {
                            req.authority.clone()
                        };
                        let uri = format!("{}://{}{}", req.scheme, authority, req.path);
                        let mut builder = http::Request::builder()
                            .method(req.method.as_str())
                            .uri(uri.as_str());
                        // 扩展 CONNECT：`:protocol` 经 extensions 上线（hyper/h2
                        // 按 RFC 8441 编成伪头；对端未开 ECP 即流错误，client 侧报）。
                        if !req.protocol.is_empty() {
                            builder = builder.extension(hyper::ext::Protocol::from(
                                req.protocol.as_str(),
                            ));
                        }
                        for (k, v) in parsed_headers {
                            builder = builder.header(k.as_str(), v.as_str());
                        }
                        // 请求体经 ChanBody：END_STREAM 随头与否听 JS 意图
                        //（endafterheaders 套件：GET 缺省随头 fin，POST 分开），
                        // 不再按体空猜——同 tick 的 req.end() 不得抢跑。
                        // 有体即先 Data 后 End；waitTrailers 悬置 EndPending。
                        let (utx, urx) = tokio::sync::mpsc::unbounded_channel::<BodyMsg>();
                        let mut done = req.end_stream && body.is_empty() && !req.wait_trailers;
                        // CONNECT 隧道（proxy-over-h2）：体通道常开待隧道字节，
                        // 永不 eager-End（node 在 end() 前不半关隧道；旧分支对空体
                        // CONNECT 即发 End，隧道写向先死，嵌套请求出不去）。
                        // 流式上传：首块随头出线但流保持开放（done=false 且不排 End），
                        // 后续块走 H2RespondData、收尾走 H2RespondEnd；utx 必登记。
                        let streaming = req.streaming && !req.end_stream;
                        let tunnel = req.method == "CONNECT" || req.method == "connect";
                        if streaming {
                            let _ = utx.send(BodyMsg::Data(bytes::Bytes::from(body)));
                            done = false;
                        } else if !body.is_empty() {
                            let _ = utx.send(BodyMsg::Data(bytes::Bytes::from(body)));
                            done = false;
                            if req.wait_trailers {
                                let _ = utx.send(BodyMsg::EndPending);
                            } else {
                                let _ = utx.send(BodyMsg::End(None));
                            }
                        } else if req.wait_trailers {
                            let _ = utx.send(BodyMsg::EndPending);
                            done = false;
                        } else if !req.end_stream && !tunnel {
                            // 空体但意不在随头 fin：显式 End 消息走 Data 后，
                            // 迫 hyper 分开发（END_STREAM 不随头）。
                            let _ = utx.send(BodyMsg::End(None));
                        }
                        if req.wait_trailers || streaming || tunnel {
                            uploads.lock().await.insert(stream_id, utx);
                        }
                        let request = builder.body(ChanBody { rx: urx, done });
                        let request = match request {
                            Ok(r) => r,
                            Err(e) => {
                                let _ = ev_tx.send(NetEvent {
                                    id,
                                    kind: NetKind::H2Stream {
                                        stream_id,
                                        what: "error".into(),
                                        payload: serde_json::json!({
                                            "code": "ERR_HTTP2_STREAM_ERROR",
                                            "msg": format!("bad request: {e}"),
                                        })
                                        .to_string(),
                                    },
                                });
                                continue;
                            }
                        };
                        let ev2 = ev_tx.clone();
                        let open2 = open.clone();
                        let exact2 = exact_rst.clone();
                        let req_method = req.method.clone();
                        let ctunnel_out = ctunnel_out.clone();
                        open.lock().unwrap().insert(stream_id, false);
                        let mut sender = sender.clone();
                        let task = tokio::spawn(async move {
                            match sender.send_request(request).await {
                                Err(e) => {
                                    // 发送失败 helm：wire 序位此后不可信，精确 RST 熄火。
                                    exact2.store(false, std::sync::atomic::Ordering::Release);
                                    let (msg, rst) = h2_err_msg_rst(&e);
                                    let _ = ev2.send(NetEvent {
                                        id,
                                        kind: NetKind::H2Stream {
                                            stream_id,
                                            what: "error".into(),
                                            payload: serde_json::json!({
                                                "code": "ERR_HTTP2_STREAM_ERROR",
                                                "msg": msg,
                                                "rst": rst,
                                            })
                                            .to_string(),
                                        },
                                    });
                                }
                                Ok(resp) => {
                                    use http_body::Body as _;
                                    use std::future::poll_fn;
                                    let status = resp.status().as_u16();
                                    let head = headers_json(resp.headers());
                                    // c2：response 头到即发（node 头到即 'response'；
                                    // 修前整收体后才发，flushheaders 套件服务端等
                                    // response 才 end 即双边互等挂死）。flags 取
                                    // 真 END_STREAM（hyper Incoming::is_end_stream），
                                    // 不再事后推断。
                                    let flags =
                                        if resp.body().is_end_stream() { 5 } else { 4 };
                                    let _ = ev2.send(NetEvent {
                                        id,
                                        kind: NetKind::H2Stream {
                                            stream_id,
                                            what: "response".into(),
                                            payload: serde_json::json!({ "status": status, "flags": flags, "headers": head }).to_string(),
                                        },
                                    });
                                    // CONNECT 隧道（proxy-over-h2）：200 后 hyper 将流升级，
                                    // 后续 DATA 只进 upgrade 句柄。取句柄（&mut resp，必须
                                    // 在 into_body 之前）并决议：Ok 即隧道（起泵+跳过常规
                                    // 体循环——读之即空还会送虚假 end）；Err/他法走常规。
                                    // upgrade 在 2xx/non-2xx 处理时即决议，此处 await 不挂起。
                                    let mut resp = resp;
                                    let is_tunnel_req =
                                        req_method == "CONNECT" || req_method == "connect";
                                    if is_tunnel_req {
                                        let cup = hyper::upgrade::on(&mut resp);
                                        match cup.await {
                                            Ok(u) => {
                                                let (otx, orx) = tokio::sync::mpsc::unbounded_channel::<Option<Vec<u8>>>();
                                                ctunnel_out.lock().await.insert(stream_id, otx);
                                                let ev3 = ev2.clone();
                                                let open3 = open2.clone();
                                                tokio::spawn(pump_client_upgrade(
                                                    u, orx, ev3, id, stream_id, open3,
                                                ));
                                                return;
                                            }
                                            Err(_) => {}
                                        }
                                    }
                                    // 增量读体：DATA 到即发（close/aborted 套件：
                                    // 服务端只写不终结时客户端仍见 data）。
                                    let mut body = resp.into_body();
                                    let mut trailers: Vec<(String, String)> = Vec::new();
                                    let mut body_err: Option<(String, Option<u32>)> = None;
                                    loop {
                                        match poll_fn(|cx| std::pin::Pin::new(&mut body).poll_frame(cx)).await {
                                            None => break,
                                            Some(Ok(f)) => {
                                                if let Some(d) = f.data_ref() {
                                                    if !d.is_empty() {
                                                        let _ = ev2.send(NetEvent {
                                                            id,
                                                            kind: NetKind::H2Stream {
                                                                stream_id,
                                                                what: "data".into(),
                                                                payload: b64(d),
                                                            },
                                                        });
                                                    }
                                                }
                                                if let Some(t) = f.trailers_ref() {
                                                    for (k, v) in t.iter() {
                                                        trailers.push((
                                                            k.as_str().to_owned(),
                                                            String::from_utf8_lossy(v.as_bytes())
                                                                .into_owned(),
                                                        ));
                                                    }
                                                }
                                            }
                                            Some(Err(e)) => {
                                                let (msg, rst) = h2_err_msg_rst(&e);
                                                body_err = Some((msg, rst));
                                                break;
                                            }
                                        }
                                    }
                                    match body_err {
                                        None => {
                                            if !trailers.is_empty() {
                                                let _ = ev2.send(NetEvent {
                                                    id,
                                                    kind: NetKind::H2Stream {
                                                        stream_id,
                                                        what: "trailers".into(),
                                                        payload: serde_json::to_string(&trailers)
                                                            .unwrap_or_else(|_| "[]".into()),
                                                    },
                                                });
                                            }
                                            let _ = ev2.send(NetEvent {
                                                id,
                                                kind: NetKind::H2Stream {
                                                    stream_id,
                                                    what: "end".into(),
                                                    payload: String::new(),
                                                },
                                            });
                                            open2.lock().unwrap().insert(stream_id, true);
                                        }
                                        Some((e, rst)) => {
                                            // NO_ERROR 干净收尾按子串判；h2 原因码按流错误上报；
                                            // 无原因码的传输层死亡（server socket destroy 等）→
                                            // node 口径 ECONNRESET（server-socket-destroy
                                            // 套件：ECONNRESET 或无错，二者皆允）。
                                            if e.contains("NO_ERROR") {
                                                let _ = ev2.send(NetEvent {
                                                    id,
                                                    kind: NetKind::H2Stream {
                                                        stream_id,
                                                        what: "end".into(),
                                                        payload: String::new(),
                                                    },
                                                });
                                                open2.lock().unwrap().insert(stream_id, true);
                                            } else if rst.is_some() {
                                                let _ = ev2.send(NetEvent {
                                                    id,
                                                    kind: NetKind::H2Stream {
                                                        stream_id,
                                                        what: "error".into(),
                                                        payload: serde_json::json!({
                                                            "code": "ERR_HTTP2_STREAM_ERROR",
                                                            "msg": e,
                                                            "rst": rst,
                                                        })
                                                        .to_string(),
                                                    },
                                                });
                                                open2.lock().unwrap().insert(stream_id, true);
                                            } else {
                                                let _ = ev2.send(NetEvent {
                                                    id,
                                                    kind: NetKind::H2Stream {
                                                        stream_id,
                                                        what: "error".into(),
                                                        payload: serde_json::json!({
                                                            "code": "ECONNRESET",
                                                            "msg": "connection reset by peer",
                                                        })
                                                        .to_string(),
                                                    },
                                                });
                                                open2.lock().unwrap().insert(stream_id, true);
                                            }
                                        }
                                    }
                                }
                            }
                        });
                        stream_tasks.lock().unwrap().insert(stream_id, task);
                    }
                    Some(NetCmd::H2RespondReset { stream_id, code }) => {
                        // 客户端流取消：abort 在途请求任务 + 精确 RST 码线注
                        //（close(code) 原码上线；hyper abort 只给 CANCEL）。
                        if let Some(h) = stream_tasks.lock().unwrap().remove(&stream_id) {
                            h.abort();
                        }
                        if exact_rst.load(std::sync::atomic::Ordering::Acquire) {
                            let _ = injector.inject_rst(stream_id, code).await;
                        }
                        uploads.lock().await.remove(&stream_id);
                        open.lock().unwrap().insert(stream_id, true);
                    }
                    Some(NetCmd::H2RespondData { stream_id, data_b64 }) => {
                        // 流式上传后续块（cancel-while-client-reading；H2RespondData
                        // 复用：client 会话上即上传块，server 会话由 serve_conn 消费）。
                        // 已升级隧道：改走 upgrade 写出（hyper 上传通道已死，见 4.302）。
                        if let Some(tx) = ctunnel_out.lock().await.get(&stream_id) {
                            if let Ok(data) = b64dec(&data_b64) {
                                if !data.is_empty() {
                                    let _ = tx.send(Some(data));
                                }
                            }
                            continue;
                        }
                        if let Some(tx) = uploads.lock().await.get(&stream_id) {
                            if let Ok(data) = b64dec(&data_b64) {
                                if !data.is_empty() {
                                    let _ = tx.send(BodyMsg::Data(bytes::Bytes::from(data)));
                                }
                            }
                        }
                    }
                    Some(NetCmd::H2RespondEnd { stream_id, trailers_json }) => {
                        // 流式上传收尾（end()）：trailers 有即带、无即干净 END。
                        // 已升级隧道：关写半部（trailers 隧道无载，丢弃）。
                        if let Some(tx) = ctunnel_out.lock().await.remove(&stream_id) {
                            let _ = tx.send(None);
                            continue;
                        }
                        let trailers: Vec<(String, String)> =
                            serde_json::from_str(&trailers_json).unwrap_or_default();
                        if let Some(tx) = uploads.lock().await.remove(&stream_id) {
                            let _ = tx.send(BodyMsg::End(Some(trailers)));
                        }
                    }
                    Some(NetCmd::H2OpenTrailers { stream_id, trailers_json }) => {
                        let trailers: Vec<(String, String)> =
                            serde_json::from_str(&trailers_json).unwrap_or_default();
                        let tx = uploads.lock().await.remove(&stream_id);
                        if let Some(tx) = tx {
                            let _ = tx.send(BodyMsg::End(Some(trailers)));
                        }
                    }
                    Some(NetCmd::Close) | None => {
                        // c2：在途请求任务持有 sender 克隆——hyper Transport 疑似被
                        // sender 续命，须先 abort 任务再退，否则 socket 永不关
                        //（aborted/close 套件：destroy 后服务端永不见 FIN）。
                        for (_, h) in stream_tasks.lock().unwrap().iter() {
                            h.abort();
                        }
                        if state::net_close_once(id) {
                            for (stream_id, done) in open.lock().unwrap().iter() {
                                if !*done {
                                    let _ = ev_tx.send(NetEvent {
                                        id,
                                        kind: NetKind::H2Stream {
                                            stream_id: *stream_id,
                                            what: "aborted".into(),
                                            payload: String::new(),
                                        },
                                    });
                                }
                            }
                            let _ = ev_tx.send(NetEvent { id, kind: NetKind::H2SessionClose });
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// `__wjs2_h2_connect(host, port, optsJson, target)` → session id。
pub unsafe extern "C" fn h2_connect(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 || !frame.arg(3).is_object() {
        report_error(&mut cx, "TypeError: h2 connect internals missing target");
        return false;
    }
    let host = value_to_string(&mut cx, frame.arg(0));
    let Some(port) = opt_num(&frame, 1) else {
        report_error(&mut cx, "TypeError: h2 connect: port must be a number");
        return false;
    };
    let opts: ConnectOpts =
        serde_json::from_str(&value_to_string(&mut cx, frame.arg(2))).unwrap_or_default();
    let target = frame.arg(3);
    ensure_provider();
    let Some((id, ev_tx)) = state::net_alloc() else {
        report_error(&mut cx, "OperationError: net driver not installed");
        return false;
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        report_error(&mut cx, "OperationError: no async runtime for h2 connect");
        return false;
    };
    let cmd_rx = state::net_socket_add(id, target);
    set_rval_str(&mut cx, &frame, &id.to_string());
    handle.spawn(async move {
        let tcp = match tokio::net::TcpStream::connect((host.as_str(), port as u16)).await {
            Ok(s) => s,
            Err(e) => {
                let code = crate::builtins::node::fs::io_code(&e);
                let _ = ev_tx.send(NetEvent {
                    id,
                    kind: NetKind::Error { code: code.into(), msg: format!("{code}: {e}") },
                });
                if state::net_close_once(id) {
                    let _ = ev_tx.send(NetEvent { id, kind: NetKind::H2SessionClose });
                }
                return;
            }
        };
        if let Some(t) = opts.tls {
            let reject = t.reject_unauthorized.unwrap_or(true);
            let cfg = match crate::builtins::node::tls::client_config_h2(t.ca_pem.as_deref(), reject) {
                Ok(c) => c,
                Err(e) => {
                    report_error_static(&ev_tx, id, &e);
                    return;
                }
            };
            let servername = t.servername.unwrap_or_else(|| host.clone());
            let name = match rustls::pki_types::ServerName::try_from(servername.clone()) {
                Ok(n) => n,
                Err(e) => {
                    report_error_static(
                        &ev_tx,
                        id,
                        &format!("ERR_TLS_HANDSHAKE: bad servername '{servername}': {e}"),
                    );
                    return;
                }
            };
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(cfg));
            match connector.connect(name, tcp).await {
                Err(e) => {
                    report_error_static(&ev_tx, id, &format!("ERR_TLS_HANDSHAKE: {e}"));
                }
                Ok(tls) => drive_session(tls, id, ev_tx, cmd_rx).await,
            }
        } else {
            drive_session(tcp, id, ev_tx, cmd_rx).await;
        }
    });
    true
}


// ── 客户端 createConnection 桥（proxy-over-h2 套件）──────────────────────────
// node：connect(url, { createConnection }) 以用户 Duplex 为传输建会话
// （CONNECT 隧道等）。本仓 dial 路径外增通道桥：JS 泵 Duplex 字节进出，
// Rust 侧 drive_session 复用（AdoptIo 与服务端注入共用，见 http2_server.rs）。

/// client adopt inbox 表（session_id → 入站发送端；task 退出即摘）。
static CLIENT_ADOPT_INBOX: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<u64, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
> = std::sync::OnceLock::new();
fn client_adopt_inbox() -> &'static std::sync::Mutex<
    std::collections::HashMap<u64, tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
> {
    CLIENT_ADOPT_INBOX.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// `__wjs2_h2_client_adopt(target)` → session id 数字串（h2c，隧道内无 TLS）。
pub unsafe extern "C" fn h2_client_adopt(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_object() {
        report_error(&mut cx, "TypeError: h2 client adopt internals missing target");
        return false;
    }
    let target = frame.arg(0);
    ensure_provider();
    let Some(ev_tx) = state::net_ev_tx() else {
        report_error(&mut cx, "OperationError: net driver not installed");
        return false;
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        report_error(&mut cx, "OperationError: no async runtime for h2 client adopt");
        return false;
    };
    let (id, cmd_rx) = state::net_conn_add();
    state::net_target_add(id, target);
    let (in_tx, in_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    client_adopt_inbox().lock().expect("client adopt inbox").insert(id, in_tx);
    let ev2 = ev_tx.clone();
    handle.spawn(async move {
        let io = AdoptIo::new(in_rx, ev2.clone(), id);
        drive_session(io, id, ev2, cmd_rx).await;
        client_adopt_inbox().lock().expect("client adopt inbox").remove(&id);
    });
    set_rval_str(&mut cx, &frame, &id.to_string());
    true
}

/// `__wjs2_h2_client_adopt_data(sessionId, b64)`：入站字节喂 inbox。
pub unsafe extern "C" fn h2_client_adopt_data(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: h2 client adopt data: bad args");
        return false;
    }
    let (Some(sid), b64) = (opt_num(&frame, 0), value_to_string(&mut cx, frame.arg(1))) else {
        report_error(&mut cx, "TypeError: h2 client adopt data: bad args");
        return false;
    };
    if let Some(tx) = client_adopt_inbox().lock().expect("client adopt inbox").get(&(sid as u64)) {
        use base64::Engine as _;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
            let _ = tx.send(bytes);
        }
    }
    true
}

/// `__wjs2_h2_client_adopt_close(sessionId)`：摘 inbox（读端见 EOF）。
pub unsafe extern "C" fn h2_client_adopt_close(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let Some(sid) = opt_num(&frame, 0) else {
        report_error(&mut cx, "TypeError: h2 client adopt close: id must be a number");
        return false;
    };
    client_adopt_inbox().lock().expect("client adopt inbox").remove(&(sid as u64));
    true
}
