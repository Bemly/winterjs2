//! `node:vm` 动态导入投递（`vm_dyn.rs` 姊妹文件；纯搬移）。
//!
//! 内容：三路 referrer 匹配 + hook 投递 + 回调结果分类/结算 + 求值帮忙落定
//! natives + `initializeImportMeta` 投递。帧栈与注册 natives 见 `vm_dyn.rs`。

use std::cell::RefCell;

use mozjs::context::JSContext;
use mozjs::conversions::ToJSValConvertible as _;
use mozjs::jsapi::{JSFunction, JSObject, JSScript};
use mozjs::jsval::{JSVal, ObjectValue, UndefinedValue};
use mozjs::realm::AutoRealm;
use mozjs::rooted;
use mozjs::rust::wrappers2::{
    GetModuleNamespace, GetModuleRequestSpecifier, ModuleEvaluate, ModuleLink,
    ModuleRequestIsSourcePhase, RejectPromise, ResolvePromise,
};

use crate::jsapi_glue::{
    define_prop, get_prop_value, promise_settled_value, raw_handle, reflect_apply,
    reflect_construct, report_error, take_pending_exception, wrap_cx, Frame,
};
use crate::state;

use super::vm::{arg_string, lookup_global};
use super::vm_module::requests::dynamic_import_attrs;

// ── Script 帧栈 ──────────────────────────────────────────────────────────────

/// Script 单次 run 帧（文件名 + 注册 token）。
struct ScriptFrame {
    filename: String,
    token: u64,
}

thread_local! {
    static SCRIPT_FRAMES: RefCell<Vec<ScriptFrame>> = const { RefCell::new(Vec::new()) };
}

/// 帧守卫（RAII 弹帧 + 摘注册；求值成功/失败路径全覆盖）。
pub(super) struct ScriptFrameGuard {
    token: u64,
}

impl Drop for ScriptFrameGuard {
    fn drop(&mut self) {
        SCRIPT_FRAMES.with(|f| f.borrow_mut().retain(|fr| fr.token != self.token));
        state::vm_dyn_unregister_token(self.token);
    }
}

/// 推帧（Script 单次 run 用；返回 RAII 守卫）。
pub(super) fn script_frame_push(filename: String, token: u64) -> ScriptFrameGuard {
    SCRIPT_FRAMES.with(|f| f.borrow_mut().push(ScriptFrame { filename, token }));
    ScriptFrameGuard { token }
}

// ── 错误值工厂 ───────────────────────────────────────────────────────────────

/// 缺回调错（真机逐字）：TypeError + ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING +
/// "A dynamic import callback was not specified."。
fn missing_error(cx: &mut JSContext, global: *mut JSObject) -> Option<JSVal> {
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    let ctor = get_prop_value(cx, global_root.get(), c"TypeError")?;
    rooted!(&in(cx) let mut msg_v = UndefinedValue());
    "A dynamic import callback was not specified.".to_jsval(cx, msg_v.handle_mut());
    rooted!(&in(cx) let msg = msg_v.get());
    let err = reflect_construct(cx, ctor, &[msg.get()])?;
    rooted!(&in(cx) let err_root = err);
    rooted!(&in(cx) let mut code_v = UndefinedValue());
    "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING".to_jsval(cx, code_v.handle_mut());
    rooted!(&in(cx) let code = code_v.get());
    if !define_prop(cx, err_root.get().to_object(), c"code", code.get()) {
        return None;
    }
    Some(err_root.get())
}

/// 非法返回错（真机逐字）：Error + ERR_VM_MODULE_NOT_MODULE +
/// "Provided module is not an instance of Module"。
fn not_module_error(cx: &mut JSContext, global: *mut JSObject) -> Option<JSVal> {
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    let ctor = get_prop_value(cx, global_root.get(), c"Error")?;
    rooted!(&in(cx) let mut msg_v = UndefinedValue());
    "Provided module is not an instance of Module".to_jsval(cx, msg_v.handle_mut());
    rooted!(&in(cx) let msg = msg_v.get());
    let err = reflect_construct(cx, ctor, &[msg.get()])?;
    rooted!(&in(cx) let err_root = err);
    rooted!(&in(cx) let mut code_v = UndefinedValue());
    "ERR_VM_MODULE_NOT_MODULE".to_jsval(cx, code_v.handle_mut());
    rooted!(&in(cx) let code = code_v.get());
    if !define_prop(cx, err_root.get().to_object(), c"code", code.get()) {
        return None;
    }
    Some(err_root.get())
}

// ── 投递判定 ─────────────────────────────────────────────────────────────────

/// 回调快照（投递用；调用方在 realm 内立即重 root）。
struct DynCall {
    callback: JSVal,
    referrer: JSVal,
    main_loader: bool,
    ctx: u64,
    source: String,
}

/// referrer 匹配（记录 → 函数 → 脚本帧 → incumbent 上下文；无命中即 None →
/// 主 loader 路）。
/// incumbent 兜底（referrer-realm 轮）：referrer 不可辨识时（eval 微任务等），
/// 按 hook 入口 realm 归属 vm 上下文即用其回调（真机 referrer null 即走上下文回调）。
/// 前置：cx 有效；referrer 为 hook 调用期内有效的脚本指针；
/// incumbent 为 hook 入口 realm global（可空 → 跳过兜底）。
fn dyn_match(cx: &mut JSContext, referrer: *mut JSScript, incumbent: *mut JSObject) -> Option<DynCall> {
    // 模块路：编译期定址脚本指针比对（求值后引擎清记录侧槽，此处自持根，
    // 指针恒稳；只比地址不解引用，恒安全，见 4.293）。
    for (id, script) in state::vm_mod_scripts() {
        if !script.is_null() && script == referrer {
            let (referrer, callback, _meta, entry_ctx, source) = state::vm_dyn_get_mod(id)?;
            return Some(DynCall {
                callback,
                referrer,
                main_loader: false,
                ctx: entry_ctx,
                source,
            });
        }
    }
    // 函数路：脚本 → compileFunction 结果。
    for func in state::vm_dyn_fns() {
        rooted!(&in(cx) let func_root: *mut JSObject = func);
        // SAFETY: func_root 为调用方 rooted 的函数对象；同一槽位转函数指针读脚本槽，
        // 纯读无 GC 点（槽位本身被 trace，见 §4.40）
        let script = unsafe {
            mozjs::jsapi::JS_GetFunctionScript(
                cx.raw_cx(),
                raw_handle(func_root.as_ptr() as *const *mut JSFunction),
            )
        };
        if script == referrer {
            let (referrer, callback, ctx, main_loader, source) = state::vm_dyn_get_fn(func)?;
            return Some(DynCall { callback, referrer, main_loader, ctx, source });
        }
    }
    // 脚本路：文件名自上而下首命中。
    let filename = unsafe {
        let c = mozjs::jsapi::JS_GetScriptFilename(referrer);
        if c.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr(c).to_string_lossy().into_owned()
    };
    SCRIPT_FRAMES.with(|f| {
        let frames = f.borrow();
        frames.iter().rev().find(|fr| fr.filename == filename).and_then(|fr| {
            state::vm_dyn_get_token(fr.token).map(|(referrer, callback, ctx, main_loader, source)| {
                DynCall { callback, referrer, main_loader, ctx, source }
            })
        })
    }).or_else(|| {
        // incumbent 兜底：referrer 未命中任一路（eval 微任务等），按 hook 入口
        // realm 归属 vm 上下文即用其回调；主域/未知即 None（主 loader）。
        if incumbent.is_null() {
            return None;
        }
        rooted!(&in(cx) let incumbent_root: *mut JSObject = incumbent);
        let found = state::vm_ctx_all()
            .into_iter()
            .find(|(_, g)| *g == incumbent_root.get())
            .map(|(id, _)| id);
        let Some(ctx) = found else {
            return None;
        };
        match state::vm_dyn_get_ctx(ctx) {
            // 未注册：上下文无回调 → 缺失错（主 loader 不回落）。
            None => Some(DynCall {
                callback: UndefinedValue(),
                referrer: UndefinedValue(),
                main_loader: false,
                ctx,
                source: String::new(),
            }),
            Some((referrer, callback, main_loader)) => Some(DynCall {
                callback,
                referrer,
                main_loader,
                ctx,
                source: String::new(),
            }),
        }
    })
}

// ── hook 投递 ────────────────────────────────────────────────────────────────

/// 动态导入投递（`modules.rs::load_hook` 的动态分支调用；`None` = 非 vm
/// referrer → 主 loader 路；`Some(true)` = 已结算；`Some(false)` = 已报 pending）。
/// 前置：hook 参数在调用期内有效；payload 为 promise。
/// 覆盖：`tests/node/vm.rs::vm_module_dynamic_import_faces`。
pub fn vm_dyn_import(
    cx: &mut JSContext,
    referrer: *mut JSScript,
    request: *mut JSObject,
    payload: JSVal,
    _line: u32,
    _col: u32,
    incumbent: *mut JSObject,
) -> Option<bool> {
    rooted!(&in(cx) let payload_root = payload);
    let call = dyn_match(cx, referrer, incumbent)?;
    // 主 loader 直通（USE_MAIN_CONTEXT_DEFAULT_LOADER 符号）。
    if call.main_loader {
        return None;
    }
    // realm：referrer 所属 ctx（记录/函数/脚本三路统一）。
    let gptr = if call.ctx == state::VM_MAIN_CTX {
        state::global()
    } else {
        match super::vm::lookup_global(cx, call.ctx) {
            Some(p) => p,
            None => {
                report_error(cx, "ERR_VM_MODULE_NOT_FOUND: vm context has been released");
                return Some(false);
            }
        }
    };
    rooted!(&in(cx) let global = gptr);
    let mut realm = AutoRealm::new_from_handle(cx, global.handle());
    let cx: &mut JSContext = &mut realm;
    rooted!(&in(cx) let payload_obj: *mut JSObject = payload_root.get().to_object());
    // specifier + phase（引擎请求对象 root 后只读，无 GC 间隙外溢）。
    rooted!(&in(cx) let req_root: *mut JSObject = request);
    let (spec, source_phase) = unsafe {
        let jsstr = GetModuleRequestSpecifier(cx, req_root.handle());
        if jsstr.is_null() {
            report_error(cx, "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING: cannot read import specifier");
            return Some(false);
        }
        let s = mozjs::conversions::jsstr_to_string(cx, std::ptr::NonNull::new_unchecked(jsstr));
        (s, ModuleRequestIsSourcePhase(cx, req_root.handle()))
    };
    // 无回调 → CALLBACK_MISSING（真机逐字，见探针 vm-dyn1）。
    if call.callback.is_undefined() {
        let Some(err) = missing_error(cx, global.get()) else {
            return Some(false);
        };
        rooted!(&in(cx) let err_root = err);
        let ok = unsafe {
            let ph = mozjs::gc::Handle::from_marked_location(&*payload_obj.as_ptr());
            let eh = mozjs::gc::Handle::from_marked_location(&*err_root.as_ptr());
            RejectPromise(cx, ph, eh)
        };
        return Some(ok);
    }
    // attributes（null 原型；源码静态提取：同字面量 spec 首命中，否则单 import
    // 兜底；非字面量 options 即空对象。行列不可用——引擎 hook 行恒 0，见 4.294）。
    rooted!(&in(cx) let attrs_obj: *mut JSObject = unsafe {
        mozjs::jsapi::JS_NewObjectWithGivenProto(
            cx.raw_cx(),
            std::ptr::null(),
            raw_handle(&std::ptr::null_mut()),
        )
    });
    if attrs_obj.is_null() {
        return Some(false);
    }
    for (k, v) in dynamic_import_attrs(&call.source, "js", &spec) {
        let Ok(ck) = std::ffi::CString::new(k) else {
            continue;
        };
        rooted!(&in(cx) let mut vv_root = UndefinedValue());
        v.to_jsval(cx, vv_root.handle_mut());
        if !define_prop(cx, attrs_obj.get(), &ck, vv_root.get()) {
            return Some(false);
        }
    }
    // 实参（真机序：specifier, referrer, attributes, phase）。
    rooted!(&in(cx) let mut spec_v_root = UndefinedValue());
    spec.to_jsval(cx, spec_v_root.handle_mut());
    rooted!(&in(cx) let spec_v = spec_v_root.get());
    rooted!(&in(cx) let referrer_v = call.referrer);
    rooted!(&in(cx) let attrs_v = ObjectValue(attrs_obj.get()));
    rooted!(&in(cx) let mut phase_v_root = UndefinedValue());
    (if source_phase { "source" } else { "evaluation" }).to_jsval(cx, phase_v_root.handle_mut());
    rooted!(&in(cx) let phase_v = phase_v_root.get());
    rooted!(&in(cx) let cb = call.callback);
    let result = reflect_apply(
        cx,
        cb.get(),
        UndefinedValue(),
        &[spec_v.get(), referrer_v.get(), attrs_v.get(), phase_v.get()],
    );
    let Some(r) = result else {
        // 回调抛错 → pending 原样透传为 rejection（真机口径）。
        let Some(exc) = take_pending_exception(cx) else {
            report_error(cx, "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING: import callback failed");
            return Some(false);
        };
        rooted!(&in(cx) let exc_root = exc);
        let ok = unsafe {
            let ph = mozjs::gc::Handle::from_marked_location(&*payload_obj.as_ptr());
            let eh = mozjs::gc::Handle::from_marked_location(&*exc_root.as_ptr());
            RejectPromise(cx, ph, eh)
        };
        return Some(ok);
    };
    rooted!(&in(cx) let result_root = r);
    // async 回调 → 开票 + JS 帮忙挂落定（`__wjs2_vm_dyn_then`）。
    let result_is_promise = if result_root.is_object() {
        let obj = result_root.get().to_object();
        rooted!(&in(cx) let obj_root: *mut JSObject = obj);
        // SAFETY: obj_root 为调用方 rooted 的对象；谓词无 GC 点
        unsafe { mozjs::jsapi::IsPromiseObject(raw_handle(obj_root.as_ptr())) }
    } else {
        false
    };
    if result_is_promise {
        return dyn_commit_async(cx, global.get(), payload_obj.get(), call.ctx, source_phase, result_root.get().to_object());
    }
    // 同步结果就地分类结算。
    Some(dyn_commit_value(cx, global.get(), payload_obj.get(), source_phase, result_root.get()))
}

/// async 回调：开票 + `__wjs2_vm_dyn_then(resultPromise, ticket)` 帮忙。
/// 帮忙函数住主 global；进主 realm 查调（票据 realm 可能为 vm 上下文）。
fn dyn_commit_async(
    cx: &mut JSContext,
    _global: *mut JSObject,
    payload: *mut JSObject,
    ctx: u64,
    source_phase: bool,
    result_promise: *mut JSObject,
) -> Option<bool> {
    rooted!(&in(cx) let result_root: *mut JSObject = result_promise);
    let ticket = state::vm_dyn_ticket_open(payload, ctx, source_phase);
    rooted!(&in(cx) let mut ticket_v_root = UndefinedValue());
    ticket.to_string().to_jsval(cx, ticket_v_root.handle_mut());
    rooted!(&in(cx) let ticket_v = ticket_v_root.get());
    rooted!(&in(cx) let main_root: *mut JSObject = state::global());
    let mut main_realm = AutoRealm::new_from_handle(cx, main_root.handle());
    let cx: &mut JSContext = &mut main_realm;
    let helper = get_prop_value(cx, main_root.get(), c"__wjs2_vm_dyn_then")?;
    rooted!(&in(cx) let helper_root = helper);
    rooted!(&in(cx) let result_v = ObjectValue(result_root.get()));
    crate::jsapi_glue::call_two(cx, main_root.get(), helper_root.get(), result_v.get(), ticket_v.get())?;
    Some(true)
}

// ── 结果分类与结算 ───────────────────────────────────────────────────────────

/// 同步回调结果分类。
enum DynOutcome {
    /// vm/主模块记录（走 link + evaluate 后解命名空间）。
    Record(*mut JSObject),
    /// 命名空间直返（恒等解）。
    Namespace(JSVal),
    /// 非法返回（NOT_MODULE）。
    Invalid,
}

fn dyn_classify(cx: &mut JSContext, result: JSVal) -> DynOutcome {
    if !result.is_object() {
        return DynOutcome::Invalid;
    }
    // SAFETY: 纯读解包，无 GC 点（UnwrapObjectDynamic 只读）
    let raw = unsafe {
        mozjs::glue::UnwrapObjectDynamic(result.to_object(), cx.raw_cx(), false)
    };
    if raw.is_null() {
        return DynOutcome::Invalid;
    }
    // 包装 identity（回调返回模块/命名空间包装本体；CCW 双边解包比对）。
    // SAFETY: 同上纯读
    for (wrapper, mod_id, ns) in state::vm_dyn_wrappers() {
        if !wrapper.is_object() {
            continue;
        }
        let wraw = unsafe {
            mozjs::glue::UnwrapObjectDynamic(wrapper.to_object(), cx.raw_cx(), false)
        };
        if wraw == raw {
            // 模块包装 → 记录路；synthetic 包装 → 以其命名空间直解。
            if let Some(id) = mod_id {
                if let Some((rec, _, _, _)) = state::vm_mod_all().into_iter().find(|(_, _, i, _)| *i == id) {
                    return DynOutcome::Record(rec);
                }
                // 记录已释放（理论不可达）→ 非法。
                return DynOutcome::Invalid;
            }
            if let Some(ns) = ns {
                rooted!(&in(cx) let ns_root: *mut JSObject = ns);
                return DynOutcome::Namespace(ObjectValue(ns_root.get()));
            }
            // 函数/Script 包装自返（非模块）→ 非法。
            return DynOutcome::Invalid;
        }
    }
    // vm 记录（原生记录直返，linker 外自有记录）？
    for (rec, _ctx, _id, _linked) in state::vm_mod_all() {
        if rec == raw {
            return DynOutcome::Record(rec);
        }
    }
    // 命名空间（逐记录取名比对，各进其域；断言风险见 4.293，一律走安全路）。
    if dyn_is_ns(cx, raw) {
        return DynOutcome::Namespace(result);
    }
    DynOutcome::Invalid
}

/// 命名空间比对（vm 记录 linked 才可读 + 主记录 + synthetic 表）。
fn dyn_is_ns(cx: &mut JSContext, raw: *mut JSObject) -> bool {
    for (rec, ctx, _id, linked) in state::vm_mod_all() {
        if !linked {
            continue;
        }
        let gptr = if ctx == state::VM_MAIN_CTX {
            state::global()
        } else {
            match lookup_global(&mut *cx, ctx) {
                Some(p) => p,
                None => continue,
            }
        };
        rooted!(&in(cx) let global = gptr);
        let mut realm = AutoRealm::new_from_handle(&mut *cx, global.handle());
        rooted!(&in(&mut realm) let rec_root: *mut JSObject = rec);
        // SAFETY: rec_root 为真模块记录（表内）；linked 可读（既有 vm_mod_ns 同款）
        let ns = unsafe { GetModuleNamespace(&mut realm, rec_root.handle()) };
        if !ns.is_null() && ns == raw {
            return true;
        }
    }
    // 主模块记录（闭包：回调返回主侧命名空间，如 import('fs') 中转）。
    {
        rooted!(&in(cx) let global = state::global());
        let mut realm = AutoRealm::new_from_handle(&mut *cx, global.handle());
        for rec in state::main_mod_all() {
            rooted!(&in(&mut realm) let rec_root: *mut JSObject = rec);
            // SAFETY: 主表记录皆真（registry 登记）；可读性同上
            let ns = unsafe { GetModuleNamespace(&mut realm, rec_root.handle()) };
            if !ns.is_null() && ns == raw {
                return true;
            }
        }
    }
    // synthetic 命名空间表。
    state::vm_dyn_ns_all().iter().any(|ns| *ns == raw)
}

/// 记录落定（link + evaluate；TLA 挂帮忙；与 dyn_done/dyn_eval_done 共用）。
/// realm 纪律：一切记录操作进记录所属 compartment（跨域回调返回异域模块时，
/// referrer realm 与记录 realm 可能不一致，就地操作即断言崩，见 4.312）。
fn dyn_commit_record(
    cx: &mut JSContext,
    payload: *mut JSObject,
    record: *mut JSObject,
) -> bool {
    rooted!(&in(cx) let payload_root: *mut JSObject = payload);
    rooted!(&in(cx) let record_root: *mut JSObject = record);
    let rec_ctx = record_ctx_of(record);
    let gptr = if rec_ctx == state::VM_MAIN_CTX {
        state::global()
    } else {
        match super::vm::lookup_global(cx, rec_ctx) {
            Some(p) => p,
            None => {
                report_error(cx, "ERR_VM_MODULE_NOT_FOUND: vm context has been released");
                return false;
            }
        }
    };
    rooted!(&in(cx) let global_root: *mut JSObject = gptr);
    let mut realm = AutoRealm::new_from_handle(cx, global_root.handle());
    let cx: &mut JSContext = &mut realm;
    // SAFETY: record 有效 rooted；realm 内同步 link（幂等，已验证）
    if !unsafe { ModuleLink(cx, record_root.handle()) } {
        let exc = take_pending_exception(cx).unwrap_or_else(|| {
            rooted!(&in(cx) let mut fallback_v = UndefinedValue());
            "vm module failed to link".to_jsval(cx, fallback_v.handle_mut());
            fallback_v.get()
        });
        rooted!(&in(cx) let exc_root = exc);
        return unsafe {
            let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
            let eh = mozjs::gc::Handle::from_marked_location(&*exc_root.as_ptr());
            RejectPromise(cx, ph, eh)
        };
    }
    rooted!(&in(cx) let mut rval = UndefinedValue());
    // SAFETY: record 有效 rooted；realm 内同步求值
    if !unsafe { ModuleEvaluate(cx, record_root.handle(), rval.handle_mut()) } {
        let exc = take_pending_exception(cx).unwrap_or_else(|| {
            rooted!(&in(cx) let mut fallback_v = UndefinedValue());
            "vm module failed to evaluate".to_jsval(cx, fallback_v.handle_mut());
            fallback_v.get()
        });
        rooted!(&in(cx) let exc_root = exc);
        return unsafe {
            let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
            let eh = mozjs::gc::Handle::from_marked_location(&*exc_root.as_ptr());
            RejectPromise(cx, ph, eh)
        };
    }
    // 落定判定（跨 compartment 安全：GetPromiseState 内解包）。
    if rval.is_object() {
        rooted!(&in(cx) let rval_obj: *mut JSObject = rval.to_object());
        let is_promise =
            unsafe { mozjs::jsapi::IsPromiseObject(raw_handle(rval_obj.as_ptr())) };
        if is_promise {
            return match promise_settled_value(cx, rval_obj.get()) {
                Some(Some((true, _))) => dyn_commit_ns(cx, payload_root.get(), record_root.get()),
                Some(Some((false, reason))) => {
                    rooted!(&in(cx) let reason_root = reason);
                    unsafe {
                        let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
                        let eh = mozjs::gc::Handle::from_marked_location(&*reason_root.as_ptr());
                        RejectPromise(cx, ph, eh)
                    }
                }
                // TLA 挂起：开求值票 + 帮忙（payload 随票跨轮）。
                _ => {
                    let ticket = state::vm_dyn_ticket_open_eval(
                        payload_root.get(),
                        record_ctx_of(record_root.get()),
                        record_root.get(),
                        false,
                    );
                    dyn_then_helper(cx, rval_obj.get(), ticket, true)
                }
            };
        }
    }
    dyn_commit_ns(cx, payload_root.get(), record_root.get())
}

/// 记录所属 ctx（跨域返回时进记录 realm；未知即主域）。
fn record_ctx_of(record: *mut JSObject) -> u64 {
    state::vm_mod_all()
        .into_iter()
        .find(|(rec, _, _, _)| *rec == record)
        .map(|(_, ctx, _, _)| ctx)
        .unwrap_or(state::VM_MAIN_CTX)
}

/// 求值帮忙（`__wjs2_vm_dyn_evalThen(evalPromise, ticket)`；缺席即报失败）。
/// 帮忙函数住主 global（vm_modules.js 接线处），一律按主域查找。
fn dyn_then_helper(
    cx: &mut JSContext,
    eval_promise: *mut JSObject,
    ticket: u64,
    is_eval: bool,
) -> bool {
    rooted!(&in(cx) let eval_root: *mut JSObject = eval_promise);
    rooted!(&in(cx) let mut ticket_v_root = UndefinedValue());
    ticket.to_string().to_jsval(cx, ticket_v_root.handle_mut());
    rooted!(&in(cx) let ticket_v = ticket_v_root.get());
    let name = if is_eval { c"__wjs2_vm_dyn_evalThen" } else { c"__wjs2_vm_dyn_then" };
    // 帮忙函数住主 global；进主 realm 查（票据 realm 可能为 vm 上下文）。
    rooted!(&in(cx) let main_root: *mut JSObject = state::global());
    let mut main_realm = AutoRealm::new_from_handle(cx, main_root.handle());
    let cx: &mut JSContext = &mut main_realm;
    let helper = match get_prop_value(cx, main_root.get(), name) {
        Some(h) => h,
        None => {
            report_error(cx, "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING: vm dynamic import helper missing");
            return false;
        }
    };
    rooted!(&in(cx) let helper_root = helper);
    rooted!(&in(cx) let eval_v = ObjectValue(eval_root.get()));
    crate::jsapi_glue::call_two(
        cx,
        main_root.get(),
        helper_root.get(),
        eval_v.get(),
        ticket_v.get(),
    )
    .is_some()
}

/// 命名空间直解（source-phase 即 NOT_MODULE，真机口径）。
/// realm 纪律同记录落定（进记录所属 compartment；调用方 realm 可能为 referrer 域）。
fn dyn_commit_ns(cx: &mut JSContext, payload: *mut JSObject, record: *mut JSObject) -> bool {
    rooted!(&in(cx) let payload_root: *mut JSObject = payload);
    rooted!(&in(cx) let record_root: *mut JSObject = record);
    let rec_ctx = record_ctx_of(record);
    let gptr = if rec_ctx == state::VM_MAIN_CTX {
        state::global()
    } else {
        match super::vm::lookup_global(cx, rec_ctx) {
            Some(p) => p,
            None => {
                report_error(cx, "ERR_VM_MODULE_NOT_FOUND: vm context has been released");
                return false;
            }
        }
    };
    rooted!(&in(cx) let global_root: *mut JSObject = gptr);
    let mut realm = AutoRealm::new_from_handle(cx, global_root.handle());
    let cx: &mut JSContext = &mut realm;
    // SAFETY: record 有效 rooted；返回命名空间由记录保活
    let ns = unsafe { GetModuleNamespace(cx, record_root.handle()) };
    if ns.is_null() {
        report_error(cx, "ERR_VM_MODULE_LINK_FAILURE: vm could not read module namespace");
        return false;
    }
    rooted!(&in(cx) let ns_root: *mut JSObject = ns);
    rooted!(&in(cx) let ns_v = ObjectValue(ns_root.get()));
    unsafe {
        let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
        let val_h = mozjs::gc::Handle::from_marked_location(&*ns_v.as_ptr());
        ResolvePromise(cx, ph, val_h)
    }
}

/// 同步结果结算（Invalid → NOT_MODULE；Namespace → 直解；Record → link/evaluate）。
fn dyn_commit_value(
    cx: &mut JSContext,
    global: *mut JSObject,
    payload: *mut JSObject,
    source_phase: bool,
    result: JSVal,
) -> bool {
    match dyn_classify(cx, result) {
        DynOutcome::Namespace(v) => {
            if source_phase {
                return dyn_fail_not_module(cx, global, payload);
            }
            rooted!(&in(cx) let payload_root: *mut JSObject = payload);
            rooted!(&in(cx) let v_root = v);
            unsafe {
                let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
                let vh = mozjs::gc::Handle::from_marked_location(&*v_root.as_ptr());
                ResolvePromise(cx, ph, vh)
            }
        }
        DynOutcome::Invalid => dyn_fail_not_module(cx, global, payload),
        DynOutcome::Record(rec) => dyn_commit_record(cx, payload, rec),
    }
}

/// NOT_MODULE 失败（真机逐字，见探针 vm-dyn1）。
fn dyn_fail_not_module(cx: &mut JSContext, global: *mut JSObject, payload: *mut JSObject) -> bool {
    let Some(err) = not_module_error(cx, global) else {
        return false;
    };
    rooted!(&in(cx) let err_root = err);
    rooted!(&in(cx) let payload_root: *mut JSObject = payload);
    unsafe {
        let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
        let eh = mozjs::gc::Handle::from_marked_location(&*err_root.as_ptr());
        RejectPromise(cx, ph, eh)
    }
}

// ── 落定 natives ─────────────────────────────────────────────────────────────

/// 回调落定（`__wjs2_vm_dyn_done(ticket, value, isErr)`；`__wjs2_vm_dyn_then`
/// 帮忙调用）。isErr 真值即 rejection。
/// UNSAFE-BOUNDARY: 票据 take 后重 root 再结算（take 与结之间无引擎调用，
/// 见票据注记）；覆盖 `tests/node/vm.rs::vm_module_dynamic_import_faces`。
pub unsafe extern "C" fn vm_dyn_done(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: vm dynamic import settlement needs three arguments");
        return false;
    }
    rooted!(&in(cx) let value = frame.arg(1));
    rooted!(&in(cx) let is_err_v = frame.arg(2));
    let ticket_s = match arg_string(&mut cx, &frame, 0, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let ticket = match ticket_s.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            report_error(&mut cx, "TypeError: vm dynamic import ticket must be a ticket id string");
            return false;
        }
    };
    let is_err = crate::jsapi_glue::value_to_string(&mut cx, is_err_v.get()) == "true";
    let Some((payload, ctx_id, _record, source_phase)) = state::vm_dyn_ticket_take(ticket) else {
        report_error(&mut cx, "TypeError: vm dynamic import ticket has been settled");
        return false;
    };
    // realm：票据 ctx。
    let gptr = if ctx_id == state::VM_MAIN_CTX {
        state::global()
    } else {
        match lookup_global(&mut cx, ctx_id) {
            Some(p) => p,
            None => {
                report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm context has been released");
                return false;
            }
        }
    };
    rooted!(&in(cx) let global = gptr);
    rooted!(&in(cx) let payload_root: *mut JSObject = payload);
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let cx: &mut JSContext = &mut realm;
    rooted!(&in(cx) let value_root = value.get());
    if is_err {
        let ok = unsafe {
            let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
            let eh = mozjs::gc::Handle::from_marked_location(&*value_root.as_ptr());
            RejectPromise(cx, ph, eh)
        };
        frame.set_rval(UndefinedValue());
        return ok;
    }
    let ok = dyn_commit_value(cx, global.get(), payload_root.get(), source_phase, value_root.get());
    frame.set_rval(UndefinedValue());
    ok
}

/// 求值落定（`__wjs2_vm_dyn_evalDone(ticket, value, isErr)`；`__wjs2_vm_dyn_evalThen`
/// 帮忙调用）。成功即解记录命名空间，失败即透传。
/// UNSAFE-BOUNDARY: 同 vm_dyn_done；覆盖同上。
pub unsafe extern "C" fn vm_dyn_eval_done(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_dyn_done
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: vm dynamic import settlement needs three arguments");
        return false;
    }
    rooted!(&in(cx) let value = frame.arg(1));
    rooted!(&in(cx) let is_err_v = frame.arg(2));
    let ticket_s = match arg_string(&mut cx, &frame, 0, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let ticket = match ticket_s.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            report_error(&mut cx, "TypeError: vm dynamic import ticket must be a ticket id string");
            return false;
        }
    };
    let is_err = crate::jsapi_glue::value_to_string(&mut cx, is_err_v.get()) == "true";
    let Some((payload, ctx_id, record, _phase)) = state::vm_dyn_ticket_take(ticket) else {
        report_error(&mut cx, "TypeError: vm dynamic import ticket has been settled");
        return false;
    };
    let Some(record) = record else {
        report_error(&mut cx, "ERR_VM_MODULE_LINK_FAILURE: vm dynamic import lost its record");
        return false;
    };
    let gptr = if ctx_id == state::VM_MAIN_CTX {
        state::global()
    } else {
        match lookup_global(&mut cx, ctx_id) {
            Some(p) => p,
            None => {
                report_error(&mut cx, "ERR_VM_MODULE_NOT_FOUND: vm context has been released");
                return false;
            }
        }
    };
    rooted!(&in(cx) let global = gptr);
    rooted!(&in(cx) let payload_root: *mut JSObject = payload);
    let mut realm = AutoRealm::new_from_handle(&mut cx, global.handle());
    let cx: &mut JSContext = &mut realm;
    rooted!(&in(cx) let value_root = value.get());
    if is_err {
        let ok = unsafe {
            let ph = mozjs::gc::Handle::from_marked_location(&*payload_root.as_ptr());
            let eh = mozjs::gc::Handle::from_marked_location(&*value_root.as_ptr());
            RejectPromise(cx, ph, eh)
        };
        frame.set_rval(UndefinedValue());
        return ok;
    }
    let ok = dyn_commit_ns(cx, payload_root.get(), record);
    frame.set_rval(UndefinedValue());
    ok
}

// ── import.meta 投递 ─────────────────────────────────────────────────────────

/// import.meta 投递（`modules.rs::metadata_hook` 的 vm 记录分支调用；`true` =
/// 继续，`false` = pending 失败）。
/// 无回调即空 meta（真机口径，见探针 #9）；有回调经 Reflect.apply 调用
/// `(meta, module)`（this undefined，返回值忽略）；回调抛错即失败。
/// 前置：record 为有效 vm 模块记录；meta 为引擎创建的元对象。
/// 覆盖：`tests/node/vm.rs::vm_module_import_meta_faces`。
pub fn vm_meta_dispatch(
    cx: &mut JSContext,
    record: *mut JSObject,
    meta: *mut JSObject,
) -> bool {
    let mut found = None;
    for (rec, _ctx, id, _linked) in state::vm_mod_all() {
        if rec == record {
            found = Some(id);
            break;
        }
    }
    let Some(mod_id) = found else {
        return true;
    };
    let Some((referrer, _cb, meta_init, ctx_id, _source)) = state::vm_dyn_get_mod(mod_id) else {
        return true;
    };
    if meta_init.is_undefined() {
        return true;
    }
    let gptr = if ctx_id == state::VM_MAIN_CTX {
        state::global()
    } else {
        match lookup_global(cx, ctx_id) {
            Some(p) => p,
            None => {
                report_error(cx, "ERR_VM_MODULE_NOT_FOUND: vm context has been released");
                return false;
            }
        }
    };
    rooted!(&in(cx) let global = gptr);
    let mut realm = AutoRealm::new_from_handle(cx, global.handle());
    let cx: &mut JSContext = &mut realm;
    rooted!(&in(cx) let meta_root: *mut JSObject = meta);
    rooted!(&in(cx) let referrer_root = referrer);
    rooted!(&in(cx) let init_root = meta_init);
    rooted!(&in(cx) let meta_v = ObjectValue(meta_root.get()));
    reflect_apply(cx, init_root.get(), UndefinedValue(), &[meta_v.get(), referrer_root.get()])
        .is_some()
}
