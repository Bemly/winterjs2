//! state/vm：vm 上下文/模块表/动态导入注册。

use super::*;
use mozjs::jsapi::{JSObject, JSScript};

// ── vm 上下文（同 Runtime 内多 global，各占新 compartment）────────────────
///
/// 登记新 global（`Box` 定址，`Heap::set` 后禁移动 §4.40），返回单调 id。
///
/// （VmCtx/VmMod 定义自 `state/mod.rs` 纯搬移归位，`pub use vm::*` 原位重导出。）

/// 一个 vm 上下文的独立 global（新 compartment；run 时重进其 realm；`Box` 定址）。
pub struct VmCtx {
    pub id: u64,
    pub global: Box<Heap<*mut JSObject>>,
}

// SAFETY: 只追踪 global（id 无 GC 指针）。
unsafe impl Traceable for VmCtx {
    unsafe fn trace(&self, trc: *mut JSTracer) { unsafe {
        self.global.trace(trc);
    }}
}

/// 一个 vm 模块记录（SourceTextModule 编译产物；新 compartment 归属其 ctx；
/// `Box` 定址，见 §4.40）。状态机（linked/evaluated 一次语义）由 JS 壳 نگه，
/// Rust 侧只记位防重复 link/evaluate；has_imports 为 true 者 v1 拒绝 link
///（带导入的 linker 切片后续做，见 vm.rs 模块头注）。
pub struct VmMod {
    pub id: u64,
    pub ctx: u64,
    pub identifier: String,
    pub record: Box<Heap<*mut JSObject>>,
    /// 编译期脚本（hook referrer 比对用；`Box` 定址保活——引擎在求值后清记录侧
    /// ScriptSlot（见 4.293），此处自持根，指针恒稳）。
    pub script: Box<Heap<*mut JSScript>>,
    pub has_imports: bool,
    /// 静态依赖 specifier 表（`dependencySpecifiers` 面；纯数据，无 GC 指针）。
    pub deps: Vec<String>,
    pub linked: bool,
    pub evaluated: bool,
}

// SAFETY: 追踪 record 与 script（其余无 GC 指针）。
unsafe impl Traceable for VmMod {
    unsafe fn trace(&self, trc: *mut JSTracer) { unsafe {
        self.record.trace(trc);
        self.script.trace(trc);
    }}
}

// ── vm 动态导入注册（importModuleDynamically/initializeImportMeta）──────────
///
/// 三路 referrer：
/// - 模块：`mod_id` 命中（SourceTextModule 构造期登记）；
/// - 函数：`func` 指针命中（compileFunction 结果，JS_GetFunctionScript 比对）；
/// - 脚本：文件名帧栈（Script 单次 run，见 `vm_dyn.rs` 帧栈——referrer 文件名
///   自上而下首命中；reentrant 主域 import 因文件名不同自动落主 loader）。

/// 单条动态导入注册（`Box` 定址，见 §4.40）。
pub struct VmDynEntry {
    pub mod_id: Option<u64>,
    pub func: Option<Box<Heap<*mut JSObject>>>,
    /// Script 单次 run 帧令牌（帧栈匹配用；模块/函数路为 None）。
    pub token: Option<u64>,
    /// 所属 realm 上下文（模块 ctx/函数 parsingContext/Script run ctx；主域哨兵见上）。
    pub ctx: u64,
    /// 回调侧 referrer 包装（JS Module/Script/Function 对象，真机恒等口径）。
    pub referrer: Box<Heap<JSVal>>,
    /// `importModuleDynamically` 回调（Undefined 即无回调 → CALLBACK_MISSING）。
    pub callback: Box<Heap<JSVal>>,
    /// `initializeImportMeta` 回调（仅模块路；Undefined 即无回调 → 空 meta）。
    pub meta_init: Box<Heap<JSVal>>,
    /// 主 loader 直通（USE_MAIN_CONTEXT_DEFAULT_LOADER 符号；回调忽略）。
    pub main_loader: bool,
    /// 命名空间（synthetic 注册；回调返回包装时直返用）。
    pub ns: Option<Box<Heap<*mut JSObject>>>,
    /// referrer 源码（动态 `with` 属性行列定位用）。
    pub source: String,
}

// SAFETY: 追踪全部 JS 值槽位（mod_id/token/ctx/main_loader/source 无 GC 指针）。
unsafe impl Traceable for VmDynEntry {
    unsafe fn trace(&self, trc: *mut JSTracer) { unsafe {
        if let Some(f) = self.func.as_ref() {
            f.trace(trc);
        }
        self.referrer.trace(trc);
        self.callback.trace(trc);
        self.meta_init.trace(trc);
        if let Some(n) = self.ns.as_ref() {
            n.trace(trc);
        }
    }}
}

/// 未决动态导入（async 回调/异步求值跨轮；结果分类在落定时做；落定即摘，
/// `Box` 定址 §4.40）。纯 Resolve 方案：落定只用 payload + 结果记录/值。
pub struct VmDynTicket {
    pub id: u64,
    pub payload: Box<Heap<*mut JSObject>>,
    pub ctx: u64,
    /// 求值等待的记录（回调落定分类后填；None 即回调等待中）。
    pub record: Option<Box<Heap<*mut JSObject>>>,
    /// source-phase 请求（命名空间直返即 NOT_MODULE，真机口径）。
    pub source_phase: bool,
}

// SAFETY: 追踪全部 GC 槽位（id/ctx/phase 无 GC 指针）。
unsafe impl Traceable for VmDynTicket {
    unsafe fn trace(&self, trc: *mut JSTracer) { unsafe {
        self.payload.trace(trc);
        if let Some(r) = self.record.as_ref() {
            r.trace(trc);
        }
    }}
}

/// 主 realm 哨兵（无显式 context 的模块归属主域，不建新 global；真机口径）。
/// vm_next_id 单调递增永不触及此值（u64::MAX），与上下文 id 无交。
pub const VM_MAIN_CTX: u64 = u64::MAX;


// ── vm 上下文（同 Runtime 内多 global，各占新 compartment）────────────────
///
/// 登记新 global（`Box` 定址，`Heap::set` 后禁移动 §4.40），返回单调 id。

pub fn vm_add(global: *mut JSObject) -> u64 {
    let id = with_plain(|p| {
        p.vm_next_id += 1;
        p.vm_next_id
    });
    with_rooted(|s| {
        s.vm_contexts.push(VmCtx { id, global: Heap::boxed(global) });
    });
    id
}

/// 取上下文 global 裸指针（调用方必须立即重 root，中间无 GC 间隙，见 runtime 约定）。
pub fn vm_global(id: u64) -> Option<*mut JSObject> {
    with_rooted(|s| s.vm_contexts.iter().find(|c| c.id == id).map(|c| c.global.get()))
}

/// 摘除上下文（JS 侧 FinalizationRegistry/显式释放用；不在即 false）。
pub fn vm_release(id: u64) -> bool {
    let n0 = with_rooted(|s| {
        let n0 = s.vm_contexts.len();
        s.vm_contexts.retain(|c| c.id != id);
        n0
    });
    // 上下文摘除时顺带摘其名下未释放模块（record 随 global 走，无悬垂）
    // 与动态导入注册（条目均记归属 ctx，一并摘除；主域哨兵永不触及）。
    with_rooted(|s| {
        s.vm_mods.retain(|m| m.ctx != id);
        s.vm_dyn.retain(|e| e.ctx != id);
    });
    with_rooted(|s| s.vm_contexts.len() != n0)
}

// ── vm 模块（9i-1；SourceTextModule 编译产物，归属其 ctx 的 compartment）───

/// 登记模块记录，返回单调 id（`Box` 定址 §4.40）。
pub fn vm_mod_add(
    ctx: u64,
    identifier: String,
    record: *mut JSObject,
    script: *mut JSScript,
    has_imports: bool,
    deps: Vec<String>,
) -> u64 {
    let id = with_plain(|p| {
        p.vm_next_id += 1;
        p.vm_next_id
    });
    with_rooted(|s| {
        s.vm_mods.push(VmMod {
            id,
            ctx,
            identifier,
            record: Heap::boxed(record),
            script: Heap::boxed(script),
            has_imports,
            deps,
            linked: false,
            evaluated: false,
        });
    });
    id
}

/// 取模块（record 裸指针 + 状态快照；调用方立即重 root，中间无 GC 间隙）。
pub fn vm_mod_get(id: u64) -> Option<(*mut JSObject, u64, bool, bool, bool)> {
    with_rooted(|s| {
        s.vm_mods.iter().find(|m| m.id == id).map(|m| {
            (m.record.get(), m.ctx, m.has_imports, m.linked, m.evaluated)
        })
    })
}

/// 取模块标识（报错信息用）。
pub fn vm_mod_identifier(id: u64) -> Option<String> {
    with_rooted(|s| s.vm_mods.iter().find(|m| m.id == id).map(|m| m.identifier.clone()))
}

/// 取静态依赖表 JSON（`dependencySpecifiers` 面）。
pub fn vm_mod_deps_json(id: u64) -> Option<String> {
    with_rooted(|s| {
        s.vm_mods.iter().find(|m| m.id == id).map(|m| {
            serde_json::Value::Array(m.deps.iter().map(|d| serde_json::Value::String(d.clone())).collect())
                .to_string()
        })
    })
}

/// 置 link 位（重复 link 由 JS 壳按 status 机拦截，此处幂等）。
pub fn vm_mod_set_linked(id: u64) {
    with_rooted(|s| {
        if let Some(m) = s.vm_mods.iter_mut().find(|m| m.id == id) {
            m.linked = true;
        }
    });
}

/// 置 evaluate 位（幂等）。
pub fn vm_mod_set_evaluated(id: u64) {
    with_rooted(|s| {
        if let Some(m) = s.vm_mods.iter_mut().find(|m| m.id == id) {
            m.evaluated = true;
        }
    });
}

/// 摘除模块（重复释放 false；顺带摘其动态导入注册）。
pub fn vm_mod_release(id: u64) -> bool {
    let n0 = with_rooted(|s| {
        let n0 = s.vm_mods.len();
        s.vm_mods.retain(|m| m.id != id);
        n0
    });
    vm_dyn_unregister_mod(id);
    with_rooted(|s| s.vm_mods.len() != n0)
}

/// 列模块脚本（id, script）：动态导入 referrer 比对用（编译期定址，见上；
/// 调用方立即重 root，中间无 GC 间隙）。
pub fn vm_mod_scripts() -> Vec<(u64, *mut JSScript)> {
    with_rooted(|s| s.vm_mods.iter().map(|m| (m.id, m.script.get())).collect())
}

/// 列全部上下文 global（主 global + 各 vm global；跨域恒等判定用——
/// global 属身份基础设施，永不包装，见 4.296）。
/// 调用方立即重 root，中间无 GC 间隙。
pub fn vm_global_all() -> Vec<*mut JSObject> {
    let mut out = vec![super::global()];
    with_rooted(|s| {
        out.extend(s.vm_contexts.iter().map(|c| c.global.get()));
    });
    out
}

/// 列全部模块（record, ctx, id, linked）：动态导入 referrer 匹配用
///（调用方立即重 root，中间无 GC 间隙）。
pub fn vm_mod_all() -> Vec<(*mut JSObject, u64, u64, bool)> {
    with_rooted(|s| {
        s.vm_mods
            .iter()
            .map(|m| (m.record.get(), m.ctx, m.id, m.linked))
            .collect()
    })
}

// ── 动态导入注册表 ─────────────────────────────────────────────────────────

/// 登记模块路动态导入（SourceTextModule 构造期；cb/meta 可为 Undefined）。
pub fn vm_dyn_register_mod(
    mod_id: u64,
    ctx: u64,
    referrer: mozjs::jsval::JSVal,
    callback: mozjs::jsval::JSVal,
    meta_init: mozjs::jsval::JSVal,
    source: String,
) {
    with_rooted(|s| {
        s.vm_dyn.retain(|e| e.mod_id != Some(mod_id));
        s.vm_dyn.push(VmDynEntry {
            mod_id: Some(mod_id),
            func: None,
            token: None,
            ctx,
            referrer: Heap::boxed(referrer),
            callback: Heap::boxed(callback),
            meta_init: Heap::boxed(meta_init),
            main_loader: false,
            ns: None,
            source,
        });
    });
}

/// 登记函数路动态导入（compileFunction 结果；cb 可为 Undefined；mode 主 loader 时直通）。
pub fn vm_dyn_register_fn(
    func: *mut JSObject,
    ctx: u64,
    referrer: mozjs::jsval::JSVal,
    callback: mozjs::jsval::JSVal,
    main_loader: bool,
    source: String,
) {
    with_rooted(|s| {
        s.vm_dyn.retain(|e| e.func.as_ref().map(|f| f.get() != func).unwrap_or(true));
        s.vm_dyn.push(VmDynEntry {
            mod_id: None,
            func: Some(Heap::boxed(func)),
            token: None,
            ctx,
            referrer: Heap::boxed(referrer),
            callback: Heap::boxed(callback),
            meta_init: Heap::boxed(mozjs::jsval::UndefinedValue()),
            main_loader,
            ns: None,
            source,
        });
    });
}

/// 登记上下文级动态导入（createContext；referrer 不可辨识时兜底用）。
/// cb 可为 Undefined（无回调 → CALLBACK_MISSING）；main_loader 直通主 loader。
pub fn vm_dyn_register_ctx(
    ctx: u64,
    referrer: mozjs::jsval::JSVal,
    callback: mozjs::jsval::JSVal,
    main_loader: bool,
) {
    with_rooted(|s| {
        s.vm_dyn.retain(|e| !(e.mod_id.is_none() && e.func.is_none() && e.token.is_none() && e.ctx == ctx));
        s.vm_dyn.push(VmDynEntry {
            mod_id: None,
            func: None,
            token: None,
            ctx,
            referrer: Heap::boxed(referrer),
            callback: Heap::boxed(callback),
            meta_init: Heap::boxed(mozjs::jsval::UndefinedValue()),
            main_loader,
            ns: None,
            source: String::new(),
        });
    });
}

/// 取上下文级注册（快照；调用方立即重 root）。
/// 返回（referrer, callback, main_loader）。
pub fn vm_dyn_get_ctx(ctx: u64) -> Option<(mozjs::jsval::JSVal, mozjs::jsval::JSVal, bool)> {
    with_rooted(|s| {
        s.vm_dyn
            .iter()
            .find(|e| e.mod_id.is_none() && e.func.is_none() && e.token.is_none() && e.ctx == ctx)
            .map(|e| (e.referrer.get(), e.callback.get(), e.main_loader))
    })
}

/// 列上下文（id, global）：hook incumbent 匹配用（调用方立即重 root）。
pub fn vm_ctx_all() -> Vec<(u64, *mut JSObject)> {
    with_rooted(|s| s.vm_contexts.iter().map(|c| (c.id, c.global.get())).collect())
}

/// 登记 Script 单次 run 帧（token 匹配；mode 主 loader 时直通）。
pub fn vm_dyn_register_token(
    token: u64,
    ctx: u64,
    referrer: mozjs::jsval::JSVal,
    callback: mozjs::jsval::JSVal,
    main_loader: bool,
    source: String,
) {
    with_rooted(|s| {
        s.vm_dyn.push(VmDynEntry {
            mod_id: None,
            func: None,
            token: Some(token),
            ctx,
            referrer: Heap::boxed(referrer),
            callback: Heap::boxed(callback),
            meta_init: Heap::boxed(mozjs::jsval::UndefinedValue()),
            main_loader,
            ns: None,
            source,
        });
    });
}

/// 摘除帧注册（run 结束；幂等）。
pub fn vm_dyn_unregister_token(token: u64) {
    with_rooted(|s| {
        s.vm_dyn.retain(|e| e.token != Some(token));
    });
}

/// 取模块路注册（快照；调用方立即重 root）。
/// 返回（referrer, callback, meta_init, ctx, source）。
pub fn vm_dyn_get_mod(
    mod_id: u64,
) -> Option<(mozjs::jsval::JSVal, mozjs::jsval::JSVal, mozjs::jsval::JSVal, u64, String)> {
    with_rooted(|s| {
        s.vm_dyn.iter().find(|e| e.mod_id == Some(mod_id)).map(|e| {
            (e.referrer.get(), e.callback.get(), e.meta_init.get(), e.ctx, e.source.clone())
        })
    })
}

/// 取帧注册（快照；调用方立即重 root）。
/// 返回（referrer, callback, ctx, main_loader, source）。
pub fn vm_dyn_get_token(
    token: u64,
) -> Option<(mozjs::jsval::JSVal, mozjs::jsval::JSVal, u64, bool, String)> {
    with_rooted(|s| {
        s.vm_dyn.iter().find(|e| e.token == Some(token)).map(|e| {
            (e.referrer.get(), e.callback.get(), e.ctx, e.main_loader, e.source.clone())
        })
    })
}

/// 列函数路注册（函数对象裸指针快照；调用方立即重 root）。
pub fn vm_dyn_fns() -> Vec<*mut JSObject> {
    with_rooted(|s| s.vm_dyn.iter().filter_map(|e| e.func.as_ref().map(|f| f.get())).collect())
}

/// 取函数路注册（快照；调用方立即重 root）。
/// 返回（referrer, callback, ctx, main_loader, source）。
pub fn vm_dyn_get_fn(
    func: *mut JSObject,
) -> Option<(mozjs::jsval::JSVal, mozjs::jsval::JSVal, u64, bool, String)> {
    with_rooted(|s| {
        s.vm_dyn
            .iter()
            .find(|e| e.func.as_ref().is_some_and(|f| f.get() == func))
            .map(|e| (e.referrer.get(), e.callback.get(), e.ctx, e.main_loader, e.source.clone()))
    })
}

/// 摘除模块路注册（模块释放/上下文摘除时；幂等）。
pub fn vm_dyn_unregister_mod(mod_id: u64) {
    with_rooted(|s| {
        s.vm_dyn.retain(|e| e.mod_id != Some(mod_id));
    });
}

/// 登记 synthetic 包装（回调返回包装时直返命名空间用；重复登记去重）。
/// 包装与命名空间同时入表（直接返回命名空间的 identity 比对走 `vm_dyn_ns`）。
pub fn vm_dyn_register_ns(wrapper: mozjs::jsval::JSVal, ns: *mut JSObject) {
    with_rooted(|s| {
        if !s.vm_dyn_ns.iter().any(|h| h.get() == ns) {
            s.vm_dyn_ns.push(Heap::boxed(ns));
        }
        s.vm_dyn.retain(|e| !(e.mod_id.is_none() && e.ns.is_some() && {
            e.referrer.get().to_object() == wrapper.to_object()
        }));
        s.vm_dyn.push(VmDynEntry {
            mod_id: None,
            func: None,
            token: None,
            ctx: VM_MAIN_CTX,
            referrer: Heap::boxed(wrapper),
            callback: Heap::boxed(mozjs::jsval::UndefinedValue()),
            meta_init: Heap::boxed(mozjs::jsval::UndefinedValue()),
            main_loader: false,
            ns: Some(Heap::boxed(ns)),
            source: String::new(),
        });
    });
}

/// 列 synthetic 命名空间（裸指针快照；调用方立即重 root）。
pub fn vm_dyn_ns_all() -> Vec<*mut JSObject> {
    with_rooted(|s| s.vm_dyn_ns.iter().map(|h| h.get()).collect())
}

/// 列包装注册（(包装值, mod_id, ns) 快照；调用方立即重 root 并解包比对）。
pub fn vm_dyn_wrappers() -> Vec<(mozjs::jsval::JSVal, Option<u64>, Option<*mut JSObject>)> {
    with_rooted(|s| {
        s.vm_dyn
            .iter()
            .filter(|e| e.mod_id.is_some() || e.ns.is_some())
            .map(|e| (e.referrer.get(), e.mod_id, e.ns.as_ref().map(|n| n.get())))
            .collect()
    })
}

/// 列主模块记录（裸指针快照；调用方立即重 root；动态导入命名空间比对用）。
pub fn main_mod_all() -> Vec<*mut JSObject> {
    with_rooted(|s| s.modules.iter().map(|m| m.record.get()).collect())
}

/// 开未决票（async 回调跨轮；返回票号）。
pub fn vm_dyn_ticket_open(payload: *mut JSObject, ctx: u64, source_phase: bool) -> u64 {
    let id = with_plain(|p| {
        p.vm_next_id += 1;
        p.vm_next_id
    });
    with_rooted(|s| {
        s.vm_dyn_tickets.push(VmDynTicket {
            id,
            payload: Heap::boxed(payload),
            ctx,
            record: None,
            source_phase,
        });
    });
    id
}

/// 结票取出（落定/释放用；无票即 None）。
pub fn vm_dyn_ticket_take(id: u64) -> Option<(*mut JSObject, u64, Option<*mut JSObject>, bool)> {
    with_rooted(|s| {
        s.vm_dyn_tickets
            .iter()
            .position(|t| t.id == id)
            .map(|i| {
                let t = s.vm_dyn_tickets.remove(i);
                (t.payload.get(), t.ctx, t.record.map(|r| r.get()), t.source_phase)
            })
    })
}

/// 求值等待票（回调已分类出记录，异步求值落定用；返回票号）。
pub fn vm_dyn_ticket_open_eval(
    payload: *mut JSObject,
    ctx: u64,
    record: *mut JSObject,
    source_phase: bool,
) -> u64 {
    let id = with_plain(|p| {
        p.vm_next_id += 1;
        p.vm_next_id
    });
    with_rooted(|s| {
        s.vm_dyn_tickets.push(VmDynTicket {
            id,
            payload: Heap::boxed(payload),
            ctx,
            record: Some(Heap::boxed(record)),
            source_phase,
        });
    });
    id
}
