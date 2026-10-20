//! `node:vm` 动态导入注册与 Script run 帧（merge-note #2）。
//!
//! 落法见模块头注；投递/结算/落定实现在姊妹文件 `vm_dyn_dispatch.rs`
//! （纯搬移拆分，调用方路径经原位重导出不变）。

use mozjs::jsval::{JSVal, UndefinedValue};
use mozjs::rooted;

use crate::jsapi_glue::{report_error, wrap_cx, Frame};
use crate::state;

use super::vm::{arg_string, vm_run_core, vm_run_this_core};
use super::vm_dyn_dispatch::script_frame_push;

pub use super::vm_dyn_dispatch::{vm_dyn_done, vm_dyn_eval_done, vm_dyn_import, vm_meta_dispatch};

pub unsafe extern "C" fn vm_dyn_reg_mod(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 6 {
        report_error(&mut cx, "TypeError: vm dynamic import registration needs six arguments");
        return false;
    }
    rooted!(&in(cx) let module_obj = frame.arg(2));
    rooted!(&in(cx) let cb = frame.arg(3));
    rooted!(&in(cx) let meta = frame.arg(4));
    let mod_id_s = match arg_string(&mut cx, &frame, 0, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let ctx_s = match arg_string(&mut cx, &frame, 1, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let source = match arg_string(&mut cx, &frame, 5, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let mod_id = match mod_id_s.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            report_error(&mut cx, "TypeError: vm Module id must be a module id string");
            return false;
        }
    };
    let ctx = if ctx_s == "main" {
        state::VM_MAIN_CTX
    } else {
        match ctx_s.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                report_error(&mut cx, "TypeError: vm context id must be a context id string");
                return false;
            }
        }
    };
    state::vm_dyn_register_mod(mod_id, ctx, module_obj.get(), cb.get(), meta.get(), source);
    frame.set_rval(UndefinedValue());
    true
}

/// 登记函数路（`__wjs2_vm_dyn_reg_fn(fn, ctx, cb, mainLoaderFlag, source)`）。
/// UNSAFE-BOUNDARY: 同上；覆盖同上。
pub unsafe extern "C" fn vm_dyn_reg_fn(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_dyn_reg_mod
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 5 {
        report_error(&mut cx, "TypeError: vm dynamic import registration needs five arguments");
        return false;
    }
    rooted!(&in(cx) let fn_obj = frame.arg(0));
    rooted!(&in(cx) let cb = frame.arg(2));
    if !fn_obj.is_object() {
        report_error(&mut cx, "TypeError: vm dynamic import function must be an object");
        return false;
    }
    let ctx_s = match arg_string(&mut cx, &frame, 1, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let flag = match arg_string(&mut cx, &frame, 3, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let source = match arg_string(&mut cx, &frame, 4, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let ctx = if ctx_s == "main" {
        state::VM_MAIN_CTX
    } else {
        match ctx_s.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                report_error(&mut cx, "TypeError: vm context id must be a context id string");
                return false;
            }
        }
    };
    state::vm_dyn_register_fn(
        fn_obj.to_object(),
        ctx,
        fn_obj.get(),
        cb.get(),
        flag == "1",
        source,
    );
    frame.set_rval(UndefinedValue());
    true
}

/// 登记 synthetic 包装（`__wjs2_vm_dyn_reg_ns(wrapper, ns)`；回调返回包装/
 /// 命名空间 identity 比对用）。
/// UNSAFE-BOUNDARY: 实参 Frame rooted 后入 `Box<Heap>` 定址（§4.40/§4.80）；
/// 覆盖 `tests/node/vm.rs::vm_module_dynamic_import_faces`。
pub unsafe extern "C" fn vm_dyn_reg_ns(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_dyn_reg_mod
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: vm namespace registration needs two arguments");
        return false;
    }
    rooted!(&in(cx) let wrapper = frame.arg(0));
    rooted!(&in(cx) let ns = frame.arg(1));
    if !wrapper.is_object() || !ns.is_object() {
        report_error(&mut cx, "TypeError: vm namespace registration needs objects");
        return false;
    }
    state::vm_dyn_register_ns(wrapper.get(), ns.to_object());
    frame.set_rval(UndefinedValue());
    true
}

/// 登记上下文级回调（`__wjs2_vm_dyn_reg_ctx(ctxId, ctxObj, cb, mainLoaderFlag)`）。
/// referrer 不可辨识的动态 import 兜底（referrer-realm 轮）；cb 可为 Undefined。
/// UNSAFE-BOUNDARY: 同上；覆盖 `tests/node/vm.rs::vm_module_referrer_realm_faces`。
pub unsafe extern "C" fn vm_dyn_reg_ctx(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_dyn_reg_mod
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 {
        report_error(&mut cx, "TypeError: vm context dynamic import registration needs four arguments");
        return false;
    }
    let ctx_s = match arg_string(&mut cx, &frame, 0, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    rooted!(&in(cx) let referrer = frame.arg(1));
    rooted!(&in(cx) let cb = frame.arg(2));
    let flag = match arg_string(&mut cx, &frame, 3, "vm dynamic import") {
        Some(s) => s,
        None => return false,
    };
    let ctx = if ctx_s == "main" {
        state::VM_MAIN_CTX
    } else {
        match ctx_s.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                report_error(&mut cx, "TypeError: vm context id must be a context id string");
                return false;
            }
        }
    };
    state::vm_dyn_register_ctx(ctx, referrer.get(), cb.get(), flag == "1");
    frame.set_rval(UndefinedValue());
    true
}

/// Script 单次 run（`__wjs2_vm_dyn_run(id, code, filename, timeout, cb, referrer, mode)`）。
/// mode: "custom"（调回调）/"main"（主 loader 直通）/"none"（无回调 → 缺失错）。
/// 三路一律推帧（文件名匹配用；main 帧携直通旗，供 referrer-realm 轮 incumbent
/// 兜底前优先判定——Script 自身指令优先于上下文回调）。
/// UNSAFE-BOUNDARY: 求值核心复用 `vm_run_core`（realm/超时/排空同源）；
/// 帧经 RAII 守卫收尾；覆盖同上。
pub unsafe extern "C" fn vm_dyn_run(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_dyn_reg_mod
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 7 {
        report_error(&mut cx, "TypeError: vm run needs seven arguments");
        return false;
    }
    rooted!(&in(cx) let cb = frame.arg(4));
    rooted!(&in(cx) let referrer = frame.arg(5));
    let id_s = match arg_string(&mut cx, &frame, 0, "vm run") {
        Some(s) => s,
        None => return false,
    };
    let code = match arg_string(&mut cx, &frame, 1, "vm run") {
        Some(s) => s,
        None => return false,
    };
    let filename = arg_string(&mut cx, &frame, 2, "vm run").unwrap_or_default();
    let timeout_ms = super::vm::vm_timeout_arg(&mut cx, &frame, 3);
    let mode = match arg_string(&mut cx, &frame, 6, "vm run") {
        Some(s) => s,
        None => return false,
    };
    let id = match id_s.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            report_error(&mut cx, "TypeError: vm context id must be a context id string");
            return false;
        }
    };
    if mode == "main" {
        // 主 loader 直通仍推帧（携直通旗；referrer-realm 轮 incumbent 兜底前
        // 优先命中，保 Script 自身指令优先于上下文回调）。
        // with 剥离同样适用（SM 解析器墙与 realm 无关）。
        let token = state::with_plain(|p| {
            p.vm_next_id += 1;
            p.vm_next_id
        });
        state::vm_dyn_register_token(token, id, referrer.get(), cb.get(), true, code.clone());
        let _guard = script_frame_push(filename.clone(), token);
        let compile_code = super::vm_module::requests::strip_dynamic_with(&code, "js")
            .unwrap_or(code);
        return vm_run_core(&mut cx, &frame, id, compile_code, filename, timeout_ms);
    }
    // 帧注册（token 单调；回调 Undefined 即 none，native 投递时判缺失）。
    let token = state::with_plain(|p| {
        p.vm_next_id += 1;
        p.vm_next_id
    });
    state::vm_dyn_register_token(token, id, referrer.get(), cb.get(), false, code.clone());
    let _guard = script_frame_push(filename.clone(), token);
    // 动态 with 剥离（SM153 仅认 type 键；剥离换行保留，attrs 行列仍对原文）；
    // 无子句即原文直跑。
    let compile_code = super::vm_module::requests::strip_dynamic_with(&code, "js")
        .unwrap_or(code);
    vm_run_core(&mut cx, &frame, id, compile_code, filename, timeout_ms)
}

/// 主域单次 run（`__wjs2_vm_dyn_run_this(code, filename, timeout, cb, referrer, mode)`）。
/// UNSAFE-BOUNDARY: 同 vm_dyn_run（核心复用 `vm_run_this_core`）；覆盖同上。
pub unsafe extern "C" fn vm_dyn_run_this(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 vm_dyn_reg_mod
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 6 {
        report_error(&mut cx, "TypeError: vm run needs six arguments");
        return false;
    }
    rooted!(&in(cx) let cb = frame.arg(3));
    rooted!(&in(cx) let referrer = frame.arg(4));
    let code = match arg_string(&mut cx, &frame, 0, "vm run") {
        Some(s) => s,
        None => return false,
    };
    let filename = arg_string(&mut cx, &frame, 1, "vm run").unwrap_or_default();
    let timeout_ms = super::vm::vm_timeout_arg(&mut cx, &frame, 2);
    let mode = match arg_string(&mut cx, &frame, 5, "vm run") {
        Some(s) => s,
        None => return false,
    };
    if mode == "main" {
        // 主 loader 直通仍推帧（携直通旗；Script 自身指令优先于上下文回调，
        // 与 vm_dyn_run 同源；with 剥离同样适用）。
        let token = state::with_plain(|p| {
            p.vm_next_id += 1;
            p.vm_next_id
        });
        state::vm_dyn_register_token(
            token,
            state::VM_MAIN_CTX,
            referrer.get(),
            cb.get(),
            true,
            code.clone(),
        );
        let _guard = script_frame_push(filename.clone(), token);
        let compile_code = super::vm_module::requests::strip_dynamic_with(&code, "js")
            .unwrap_or(code);
        return vm_run_this_core(&mut cx, &frame, compile_code, filename, timeout_ms);
    }
    let token = state::with_plain(|p| {
        p.vm_next_id += 1;
        p.vm_next_id
    });
    state::vm_dyn_register_token(
        token,
        state::VM_MAIN_CTX,
        referrer.get(),
        cb.get(),
        false,
        code.clone(),
    );
    let _guard = script_frame_push(filename.clone(), token);
    let compile_code = super::vm_module::requests::strip_dynamic_with(&code, "js")
        .unwrap_or(code);
    vm_run_this_core(&mut cx, &frame, compile_code, filename, timeout_ms)
}

