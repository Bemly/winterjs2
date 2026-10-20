//! state/net：网络驱动端点/socket 表/半关状态机 + 占位 listener。

use super::*;
use mozjs::jsval::JSVal;


// ── 网络驱动（node:net；task → channel → 事件循环，同 child 模型）───────────

/// 分配网络 id + 事件端点。
pub fn net_alloc() -> Option<(
    u64,
    tokio::sync::mpsc::UnboundedSender<crate::builtins::node::net::NetEvent>,
)> {
    with_plain(|p| {
        let tx = p.net_tx.clone()?;
        p.net_next_id += 1;
        Some((p.net_next_id, tx))
    })
}

/// 登记客户端/服务端 socket（native 侧；返回写端命令接收端交泵 task）。
pub fn net_socket_add(
    id: u64,
    target: JSVal,
) -> tokio::sync::mpsc::UnboundedReceiver<crate::builtins::node::net::NetCmd> {
    with_rooted(|s| s.net_targets.push(NetTarget { id, target: Heap::boxed(target) }));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    with_plain(|p| {
        p.net_sockets.insert(id, NetEntry { cmd_tx: tx, half_read: false, half_write: false, close_sent: false, writer_alive: true, reader_done: false, refed: true, holding: true });
        p.net_open += 1;
    });
    rx
}

/// server accept 出的连接：无 target 入表（JS 侧 attach 后补），返回命令接收端。
pub fn net_conn_add() -> (u64, tokio::sync::mpsc::UnboundedReceiver<crate::builtins::node::net::NetCmd>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let id = with_plain(|p| {
        p.net_next_id += 1;
        let id = p.net_next_id;
        p.net_sockets.insert(id, NetEntry { cmd_tx: tx, half_read: false, half_write: false, close_sent: false, writer_alive: true, reader_done: false, refed: true, holding: true });
        p.net_open += 1;
        id
    });
    (id, rx)
}

/// 事后补登记 target（server 连接 attach）。
pub fn net_target_add(id: u64, target: JSVal) {
    with_rooted(|s| s.net_targets.push(NetTarget { id, target: Heap::boxed(target) }));
}

pub fn net_target(id: u64) -> Option<JSVal> {
    with_rooted(|s| s.net_targets.iter().find(|t| t.id == id).map(|t| t.target.get()))
}

/// 发命令（socket 已收尾即 false）。
pub fn net_cmd(id: u64, cmd: crate::builtins::node::net::NetCmd) -> bool {
    with_plain(|p| p.net_sockets.get(&id).is_some_and(|e| e.cmd_tx.send(cmd).is_ok()))
}

/// 驱动事件发送端（adopt 等无 task 归属的发送者用；无驱动即 None）。
pub fn net_ev_tx() -> Option<tokio::sync::mpsc::UnboundedSender<crate::builtins::node::net::NetEvent>> {
    with_plain(|p| p.net_tx.clone())
}

/// 置半关旗（读端）；返回是否两侧都已半关（旗已随 purge 清空不重发）。
pub fn net_half_read(id: u64) -> bool {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            e.half_read = true;
            e.half_read && e.half_write
        } else {
            false
        }
    })
}

/// 置半关旗（写端）。
pub fn net_half_write(id: u64) -> bool {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            e.half_write = true;
            e.half_read && e.half_write
        } else {
            false
        }
    })
}

/// 标记写端 task 退出（返回此前是否存活）。
pub fn net_writer_exit(id: u64) -> bool {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            std::mem::replace(&mut e.writer_alive, false)
        } else {
            false
        }
    })
}

/// 标记读端 task 退出（EOF/错后 break；写端已死则此处不补——写端退出侧补）。
pub fn net_reader_done(id: u64) {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            e.reader_done = true;
        }
    });
}

/// 读端是否已退出（写端退出时若读端已走则补发 Close，close_once 防双发）。
pub fn net_reader_gone(id: u64) -> bool {
    with_plain(|p| p.net_sockets.get(&id).is_some_and(|e| e.reader_done))
}

// ── RST 面（resetAndDestroy 发 RST；ports.rs/worker.rs 同款 OnceLock 静态，
// entry 外记账，purge 即清；无 GC 指针，线程安全）─────────────────────────
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// TCP 原始 fd 表（connect/accept 成功点记；take 即摘，防 fd 号复用误伤）。
fn net_rst_fds() -> &'static Mutex<HashMap<u64, i32>> {
    static MAP: OnceLock<Mutex<HashMap<u64, i32>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// RST 静默位表（reset 后读端异常不发 Error，只走 Close 收尾）。
fn net_rst_quiet() -> &'static Mutex<HashMap<u64, bool>> {
    static MAP: OnceLock<Mutex<HashMap<u64, bool>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 记录 TCP 原始 fd（resetAndDestroy 发 RST 用；connect/accept 成功点）。
pub fn net_fd_record(id: u64, fd: i32) {
    if let Ok(mut m) = net_rst_fds().lock() {
        m.insert(id, fd);
    }
}

/// 取出并摘除 TCP fd（reset 一次性；兼防 fd 号复用误伤）。
pub fn net_fd_take(id: u64) -> Option<i32> {
    net_rst_fds().lock().ok().and_then(|mut m| m.remove(&id))
}

/// 置 RST 静默位（读端 EBADF 不发 Error，只走 Close 收尾）。
pub fn net_reset_quiet(id: u64) {
    if let Ok(mut m) = net_rst_quiet().lock() {
        m.insert(id, true);
    }
}

/// 取 RST 静默位。
pub fn net_reset_quiet_check(id: u64) -> bool {
    net_rst_quiet().lock().map(|m| m.get(&id).copied().unwrap_or(false)).unwrap_or(false)
}

/// RST 面收尾（purge 即清，防跨连接残留）。
pub(crate) fn net_rst_purge(id: u64) {
    if let Ok(mut m) = net_rst_fds().lock() {
        m.remove(&id);
    }
    if let Ok(mut m) = net_rst_quiet().lock() {
        m.remove(&id);
    }
}

// ── server 监听 fd 记账（cluster-1 传柄 dup 用；RST 面同款静态）──────────────
/// (会话序号, server id) → 监听 fd（TCP listen-bind 成功点记；take/dup 后摘除
/// 或 close 即清。会话键：各会话 id 计数器各自从零起，裸 id 跨会话必撞——
/// cleanup-handles 套件 worker/主会话各持表，必须会话隔离）。
fn net_srv_fds() -> &'static Mutex<HashMap<(u64, u64), i32>> {
    static MAP: OnceLock<Mutex<HashMap<(u64, u64), i32>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 会话键（调用方会话的序号 + server id；记/取/清三方同口径）。
fn net_srv_key(id: u64) -> (u64, u64) {
    (session_seq(), id)
}

/// 记录 server 监听 fd。
pub fn net_srv_fd_record(id: u64, fd: i32) {
    if let Ok(mut m) = net_srv_fds().lock() {
        m.insert(net_srv_key(id), fd);
    }
}

/// 取 server 监听 fd（只读；dup 不摘，close 清）。
pub fn net_srv_fd_get(id: u64) -> Option<i32> {
    net_srv_fds().lock().ok().and_then(|m| m.get(&net_srv_key(id)).copied())
}

/// 清 server 监听 fd（close/purge）。
pub fn net_srv_fd_clear(id: u64) {
    if let Ok(mut m) = net_srv_fds().lock() {
        m.remove(&net_srv_key(id));
    }
}

/// 写端是否已死（死则读端 EOF 需代行收尾，End 命令无人消费）。
pub fn net_writer_dead(id: u64) -> bool {
    with_plain(|p| p.net_sockets.get(&id).is_some_and(|e| !e.writer_alive))
}

/// Close 单次发送旗（task 侧防双发；purge 由 dispatch 完成后统一做）。
pub fn net_close_once(id: u64) -> bool {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            if e.close_sent {
                false
            } else {
                e.close_sent = true;
                true
            }
        } else {
            false
        }
    })
}

/// ref/unref 真计数（10a）：切换 refed 位并增减 net_open；entry 不在即 false。
/// unref 后 Close 派发不再减（purge 按位），ref 装回后恢复。
/// holding 摘除后只翻位不碰数（数已在 halfhold 落账）。
pub fn net_set_ref(id: u64, refed: bool) -> bool {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            if e.refed != refed {
                e.refed = refed;
                if e.holding {
                    if refed {
                        p.net_open += 1;
                    } else {
                        p.net_open = p.net_open.saturating_sub(1);
                    }
                }
            }
            true
        } else {
            false
        }
    })
}

/// 半开摘续命（G11）：对端 FIN 后 JS 侧半开持有即不再续命（真机同款）。
/// 只落账一次；purge 按 refed && holding 结算，不双减。
pub fn net_halfhold(id: u64) -> bool {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            if e.holding {
                e.holding = false;
                if e.refed {
                    p.net_open = p.net_open.saturating_sub(1);
                }
                return true;
            }
        }
        false
    })
}

/// 续命装回（pause-unhold 的逆操作；只在 holding 为 false 时落账一次）。
pub fn net_rehold(id: u64) -> bool {
    with_plain(|p| {
        if let Some(e) = p.net_sockets.get_mut(&id) {
            if !e.holding {
                e.holding = true;
                if e.refed {
                    p.net_open += 1;
                }
                return true;
            }
        }
        false
    })
}

/// 收尾清除（entry + target；Close 派发后调用；返回首次 true）。
pub fn net_purge(id: u64) -> bool {
    let counted = net_purge_entry(id);
    with_rooted(|s| s.net_targets.retain(|t| t.id != id));
    counted
}

/// entry 侧清除（含计数结算；`net_purge` 的可单测半——target 表需 rooted 会话，
/// 单测起不来引擎；双减防护的凭据在此）。
/// 返回该 entry 是否曾计入 `net_open`（refed && holding）。
pub(crate) fn net_purge_entry(id: u64) -> bool {
    let counted = with_plain(|p| {
        p.net_sockets
            .remove(&id)
            .is_some_and(|e| e.refed && e.holding)
    });
    net_rst_purge(id);
    net_srv_fd_clear(id);
    if counted {
        with_plain(|p| p.net_open = p.net_open.saturating_sub(1));
    }
    counted
}

/// 存活 socket/server 数（事件循环退出条件用）。
pub fn net_open() -> usize {
    with_plain(|p| p.net_open)
}

/// BoundSocket 占位保活：存入 listener 回 token；取出消费；丢弃释放。
pub fn net_hold_add() -> u64 {
    with_plain(|p| {
        p.net_hold_next += 1;
        p.net_hold_next
    })
}
pub fn net_hold_put(token: u64, l: std::net::TcpListener) {
    with_plain(|p| {
        p.net_held.insert(token, l);
    });
}
pub fn net_hold_take(token: u64) -> bool {
    with_plain(|p| p.net_held.remove(&token).is_some())
}

/// 取出预留 listener（cluster-2 裸柄 take-direct 认领；取走即摘，无关绑竞速）。
pub fn net_hold_take_listener(token: u64) -> Option<std::net::TcpListener> {
    with_plain(|p| p.net_held.remove(&token))
}

/// 占位 listener 的 fd（dup 出独立 fd；未知 token 回 -1）。
/// win 无 as_fd：回 -1（套件 win 侧不断 fd>=0，只断类型）。
pub fn net_hold_fd(token: u64) -> i32 {
    with_plain(|p| {
        let Some(l) = p.net_held.get(&token) else {
            return -1;
        };
        #[cfg(unix)]
        {
            use std::os::fd::AsFd as _;
            match l.as_fd().try_clone_to_owned() {
                Ok(owned) => {
                    use std::os::fd::IntoRawFd as _;
                    owned.into_raw_fd()
                }
                Err(_) => -1,
            }
        }
        #[cfg(not(unix))]
        {
            let _ = l;
            -1
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// 半开续命状态机：conn_add +1；unref/ref 成对；halfhold 落账一次且幂等；
    /// halfhold 后 unref/ref 只翻位不碰数；purge_entry 按 refed && holding 结算。
    #[test]
    #[serial]
    fn net_halfhold_balance() {
        let base = net_open();
        let (id, _rx) = net_conn_add();
        assert_eq!(net_open(), base + 1);
        // 正常路径：unref/ref 成对，数来回。
        assert!(net_set_ref(id, false));
        assert_eq!(net_open(), base);
        assert!(net_set_ref(id, true));
        assert_eq!(net_open(), base + 1);
        // 半开摘续命：落账一次，二次幂等 false。
        assert!(net_halfhold(id));
        assert_eq!(net_open(), base);
        assert!(!net_halfhold(id));
        assert_eq!(net_open(), base);
        // 摘除后 unref/ref 只翻位。
        assert!(net_set_ref(id, false));
        assert_eq!(net_open(), base);
        assert!(net_set_ref(id, true));
        assert_eq!(net_open(), base);
        // purge_entry：holding 已摘，不结算不双减。
        assert!(!net_purge_entry(id));
        assert_eq!(net_open(), base);
        // 正常 entry：purge 结算一次。
        let (id2, _rx2) = net_conn_add();
        assert_eq!(net_open(), base + 1);
        assert!(net_purge_entry(id2));
        assert_eq!(net_open(), base);
        // 未知 id 全静默 false。
        assert!(!net_halfhold(u64::MAX));
        assert!(!net_set_ref(u64::MAX, false));
        assert!(!net_purge_entry(u64::MAX));
        assert_eq!(net_open(), base);
    }

    /// RST 面记账：fd 记/take（一次性）+ 静默位置位/读取 + purge 即清。
    #[test]
    #[serial]
    fn net_rst_ledger() {
        let id = 424242u64;
        assert_eq!(net_fd_take(id), None);
        assert!(!net_reset_quiet_check(id));
        net_fd_record(id, 99);
        net_reset_quiet(id);
        assert!(net_reset_quiet_check(id));
        assert_eq!(net_fd_take(id), Some(99));
        assert_eq!(net_fd_take(id), None);
        assert!(net_reset_quiet_check(id));
        net_rst_purge(id);
        assert!(!net_reset_quiet_check(id));
        assert_eq!(net_fd_take(id), None);
    }
}
