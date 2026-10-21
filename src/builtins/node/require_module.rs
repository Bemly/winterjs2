//! `require()` Module/CJS 活表面（自 `require.rs` 按算法族纯搬移，调用方经原位重导出；F2 先例）：
//! `module._compile` 底座 + `builtinModules` + CJS 互操作垫片 + 主模块 +
//! `Module.wrapper`/`_extensions` 活读 + `require.cache` 四 natives。
//! 纯搬移，行为不变；跨文件依赖经 `super::require` 同源重用。

use mozjs::conversions::ToJSValConvertible as _;
use mozjs::jsapi::JSObject;
use mozjs::jsval::{JSVal, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::{CompileOptionsWrapper, evaluate_script};
use url::Url;

use std::collections::HashSet;


use crate::jsapi_glue::{call_one, get_prop_value, report_error, value_to_string, wrap_cx, Frame};
use crate::state;

use super::require::{NAMES_INFLIGHT, cjs_script_name, is_keep_pending, pending_message, report_require_err, require_value};
use super::require_cjs::cjs_static_names;

/// UNSAFE-BOUNDARY: `__wjs2_cjs_compile(module, code, filename)` → `module._compile`
/// 底座（vite loadConfigFromBundledFile：require.extensions 钩子把内存中的 CJS
/// 打包产物求值进给定 module 对象）。求值口径与 `require_cjs_file` 全同
/// （柯里化包装五连：exports/require/module/__filename/__dirname），差异：
/// module 由调用方传入、require 以 filename 为显式 base（prelude
/// `__wjs2_make_base_require`）、不进 cjs 注册表（缓存语义由调用方
/// require.cache 承载）。前置：module 对象、code 串、filename 为绝对路径或
/// file: URL 串；覆盖：`tests/node.rs::module_extensions_hook`。
pub unsafe extern "C" fn cjs_compile(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3
        || !frame.arg(0).is_object()
        || !frame.arg(1).is_string()
        || !frame.arg(2).is_string()
    {
        report_error(
            &mut cx,
            "TypeError: __wjs2_cjs_compile needs (module object, code string, filename string)",
        );
        return false;
    }
    let code = value_to_string(&mut cx, frame.arg(1));
    let filename_s = value_to_string(&mut cx, frame.arg(2));
    let global = state::global();
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    rooted!(&in(cx) let module_root: *mut JSObject = frame.arg(0).to_object());
    // filename → URL（file: 直 parse；路径串 canonicalize 对齐 §4.12，失败退原文）。
    let url = if let Ok(u) = Url::parse(&filename_s) {
        u
    } else {
        let p = std::path::PathBuf::from(&filename_s);
        let p = if p.is_absolute() {
            p
        } else {
            match std::env::current_dir() {
                Ok(d) => d.join(p),
                Err(e) => {
                    report_error(&mut cx, &format!("TypeError: cannot resolve '{filename_s}': {e}"));
                    return false;
                }
            }
        };
        let canon = p.canonicalize().unwrap_or(p);
        match Url::from_file_path(&canon) {
            Ok(u) => u,
            Err(_) => {
                report_error(&mut cx, &format!("TypeError: bad module filename '{filename_s}'"));
                return false;
            }
        }
    };
    // 柯里化包装（单参链，§4.9 合规；与 require_cjs_file 同形）。
    // 包装片取 `Module.wrapper` 活值（`module._compile` 口径统一）。
    let (whead, wtail) = cjs_wrapper_parts(&mut cx);
    let wrapped = format!("{whead}{code}{wtail}");
    // node 口径：CJS 栈帧是绝对路径（ESM 才是 file URL）——`stack.includes(__filename)` 类断言。
    let c_filename = std::ffi::CString::new(cjs_script_name(&url)).unwrap_or_else(|_| c"module.js".into());
    // 起始行 0（同 require_cjs_file：用户代码行号 = 物理行号）。
    let options = CompileOptionsWrapper::new(&mut cx, c_filename, 0);
    rooted!(&in(cx) let mut fn_v = UndefinedValue());
    let res = evaluate_script(&mut cx, global_root.handle(), wrapped.as_str(), fn_v.handle_mut(), options);
    if res.is_err() {
        // 原 SyntaxError 等留 pending 透传（同 require_cjs_file 口径）。
        return false;
    }
    if !fn_v.is_object() {
        report_error(&mut cx, &format!("cannot compile '{}': wrapper failed", url.as_str()));
        return false;
    }
    let Some(exports_v) = get_prop_value(&mut cx, module_root.get(), c"exports") else {
        report_error(&mut cx, "TypeError: module.exports missing");
        return false;
    };
    rooted!(&in(cx) let exports_root = exports_v);
    let Some(make_req) = get_prop_value(&mut cx, global_root.get(), c"__wjs2_make_base_require") else {
        report_error(&mut cx, "prelude helper __wjs2_make_base_require missing");
        return false;
    };
    rooted!(&in(cx) let mut url_v = UndefinedValue());
    url.as_str().to_jsval(&mut cx, url_v.handle_mut());
    let Some(require_v) = call_one(&mut cx, global_root.get(), make_req, url_v.get()) else {
        let msg = pending_message(&mut cx);
        report_error(&mut cx, &msg);
        return false;
    };
    rooted!(&in(cx) let require_root = require_v);
    // __filename/__dirname（file: URL；其余退原文）。
    let (filename_str, dirname_str) = match url.to_file_path() {
        Ok(p) => {
            let dir = p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
            (p.to_string_lossy().into_owned(), dir)
        }
        Err(_) => (url.as_str().to_owned(), String::new()),
    };
    rooted!(&in(cx) let mut s_v = UndefinedValue());
    filename_str.to_jsval(&mut cx, s_v.handle_mut());
    rooted!(&in(cx) let mut d_v = UndefinedValue());
    dirname_str.to_jsval(&mut cx, d_v.handle_mut());
    // 五连单参调用（§4.9）。§4.80：cur 全程 rooted、实参调用点现读（链内
    // call_one 可 GC，裸 JSVal 栈拷贝会悬垂）；用户代码异常留 pending 透传。
    rooted!(&in(cx) let mut cur = fn_v.get());
    for step in 0..5 {
        let (label, arg) = match step {
            0 => ("exports", exports_root.get()),
            1 => ("require", require_root.get()),
            2 => ("module", mozjs::jsval::ObjectValue(module_root.get())),
            3 => ("__filename", s_v.get()),
            _ => ("__dirname", d_v.get()),
        };
        if !cur.get().is_object() {
            report_error(&mut cx, &format!("cannot compile '{}': {label} step is not callable", url.as_str()));
            return false;
        }
        match call_one(&mut cx, global_root.get(), cur.get(), arg) {
            Some(v) => cur.set(v),
            None => return false,
        }
    }
    frame.set_rval(UndefinedValue());
    true
}

/// UNSAFE-BOUNDARY: `__wjs2_builtin_modules()` → JSON 数组（`node:module` 的
/// `builtinModules`/`isBuiltin` 用；裸名 + `node:` 双形，与 `available()` 同源，
/// 天然不漂移）；前置：无参；覆盖：`tests/node.rs::module_surface`。
pub unsafe extern "C" fn builtin_modules_json(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let mut names: Vec<&str> = Vec::new();
    for canonical in crate::builtins::node::available() {
        // R9：新风格仅前缀模块（真机 builtinModules 仅 `node:` 形，无裸名；
        // `getBuiltinModule('test')` 必须 undefined，而本仓 require 裸名可达）。
        // 跳过裸形：test / sqlite / quic（本仓三件；sea/ffi 无 canonical 不涉及）。
        match canonical {
            "node:test" | "node:test/reporters" | "node:sqlite" | "node:quic"
            | "node:sea" | "node:ffi" | "node:vfs" => {
                names.push(canonical);
            }
            _ => {
                names.push(canonical.strip_prefix("node:").unwrap_or(canonical));
                names.push(canonical);
            }
        }
    }
    // node 口径：废弃别名 `sys`（= util）在列（isBuiltin/builtinModules 套件点名；
    // 真机仅裸形，无 `node:sys`）。
    names.push("sys");
    let json = serde_json::to_string(&names).unwrap_or_else(|_| "[]".into());
    rooted!(&in(cx) let mut v = UndefinedValue());
    json.to_jsval(&mut cx, v.handle_mut());
    frame.set_rval(v.get());
    true
}
/// CJS 互操作命名导出发现（M5 vitest 牵引，供 `modules.rs` 垫片静态列名）。
/// 编译期同步 require 一次取运行时键（`Object.keys` 实测；`__exportStar` 合并键
/// 天然在内，静态词法分析做不到这点）；注册表缓存使求值期复用零重跑。
/// best-effort：失败（循环中体/抛错体）即回空 → 调用方退 default-only 垫片，
/// 求值期 require 走原语义（抛错/半成品照旧；抛错体在发现期跑过一次、求值期
/// 再跑一次，双跑记档）。
/// 前置：cx 在 realm 内；`url` 为 file: CJS（调用方已判 `cjs_interop`）。
pub(crate) fn cjs_export_names(
    cx: &mut mozjs::context::JSContext,
    global: *mut JSObject,
    url: &Url,
) -> Vec<String> {
    // 静态优先（cjs-module-lexer 口径，零求值——require(esm) 循环安全）。
    if let Ok(path) = url.to_file_path() {
        let mut seen = HashSet::new();
        let names = cjs_static_names(&path, 0, &mut seen);
        if !names.is_empty() {
            return names;
        }
    }
    // 运行时键快照回退（__exportStar 合并键等动态形；在飞护栏防 CJS↔ESM 环
    // 递归——重入即放弃，调用方退 default-only 垫片）。
    let reentered = NAMES_INFLIGHT
        .with(|s| !s.borrow_mut().insert(url.as_str().to_owned()));
    if reentered {
        return Vec::new();
    }
    let out = cjs_export_names_runtime(cx, global, url);
    NAMES_INFLIGHT.with(|s| {
        s.borrow_mut().remove(url.as_str());
    });
    out
}

fn cjs_export_names_runtime(
    cx: &mut mozjs::context::JSContext,
    global: *mut JSObject,
    url: &Url,
) -> Vec<String> {
    let exports = match require_value(cx, global, url.as_str(), None) {
        Ok(v) if v.is_object() => v,
        Err(e) if is_keep_pending(&e) => {
            // 吞错路径：原异常仍 pending，清掉再回空（不外泄到后续 JSAPI）。
            let _ = crate::jsapi_glue::take_pending_exception(cx);
            return Vec::new();
        }
        _ => return Vec::new(),
    };
    rooted!(&in(cx) let exports_root = exports);
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    // Object.keys(exports)（glue 直调，无新 JSAPI 面）。
    let keys_fn = match get_prop_value(cx, global_root.get(), c"Object")
        .filter(|o| o.is_object())
        .and_then(|o| get_prop_value(cx, o.to_object(), c"keys"))
    {
        Some(f) => f,
        _ => return Vec::new(),
    };
    let arr = match call_one(cx, global_root.get(), keys_fn, exports_root.get()) {
        Some(v) if v.is_object() => v,
        _ => return Vec::new(),
    };
    // JSON.stringify(keys) → Rust 侧解析（`parse_json` 另有落值用途，此处只要串）。
    let str_fn = match get_prop_value(cx, global_root.get(), c"JSON")
        .filter(|o| o.is_object())
        .and_then(|o| get_prop_value(cx, o.to_object(), c"stringify"))
    {
        Some(f) => f,
        _ => return Vec::new(),
    };
    let arr_rooted = arr;
    rooted!(&in(cx) let arr_root = arr_rooted);
    let json = match call_one(cx, global_root.get(), str_fn, arr_root.get()) {
        Some(v) => value_to_string(cx, v),
        _ => return Vec::new(),
    };
    serde_json::from_str::<Vec<String>>(&json).unwrap_or_default()
}
/// UNSAFE-BOUNDARY: `__wjs2_require_cjs_by_url(url)` → CJS 互操作垫片底座
/// （`import` 命中 CJS 文件时合成 `export default` + 命名导出；复用
/// `require_value` 全口径：注册表命中则同值、CJS 循环见半成品；垫片求值期
/// 同步执行 CJS 体——编译期发现已跑过则缓存复用）。
/// 前置：单参为 file: URL 串（非法即抛错，不回落）；
/// 覆盖：`tests/node.rs::cjs_interop_default`。
pub unsafe extern "C" fn require_cjs_by_url(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_string() {
        report_error(&mut cx, "TypeError: __wjs2_require_cjs_by_url needs a file URL string");
        return false;
    }
    let spec = value_to_string(&mut cx, frame.arg(0));
    let global = state::global();
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    // 绝对 file: URL 不依赖调用方 base（垫片求值点的调用方是垫片自身）。
    match require_value(&mut cx, global_root.get(), &spec, None) {
        Ok(v) => {
            rooted!(&in(cx) let v_root = v);
            frame.set_rval(v_root.get());
            true
        }
        Err(e) => {
            report_require_err(&mut cx, &e);
            false
        }
    }
}
/// `__wjs2_require_main_url()` → 主模块 URL 串｜undefined（prelude 包成对象）。
pub unsafe extern "C" fn require_main_url(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    match state::main_module() {
        Some(url) => {
            rooted!(&in(cx) let mut v = UndefinedValue());
            url.to_jsval(&mut cx, v.handle_mut());
            frame.set_rval(v.get());
            true
        }
        None => {
            frame.set_rval(UndefinedValue());
            true
        }
    }
}

/// CJS 包装 pieces（node `Module.wrapper` 活数组；native 编译期现读）。
/// 前置：cx 在 realm 内。失败/缺失即回默认壳（行号恒等）。
/// 单大括号：`'use strict'` 须在最内箭头体顶部成指令（双括号即内层块而丢失）。
pub(crate) fn cjs_wrapper_parts(cx: &mut mozjs::context::JSContext) -> (String, String) {
    const HEAD: &str = "((exports) => (require) => (module) => (__filename) => (__dirname) => {\n";
    const TAIL: &str = "\n})";
    let arr = state::with_rooted(|s| s.cjs_wrapper.as_ref().map(|h| h.get()));
    let Some(arr) = arr else {
        return (HEAD.to_owned(), TAIL.to_owned());
    };
    rooted!(&in(cx) let arr_root: *mut JSObject = arr);
    let read = |cx: &mut mozjs::context::JSContext, key: &std::ffi::CStr| -> Option<String> {
        let v = crate::jsapi_glue::get_prop_value(cx, arr_root.get(), key)?;
        if !v.is_string() {
            return None;
        }
        Some(value_to_string(cx, v))
    };
    match (read(cx, c"0"), read(cx, c"1")) {
        (Some(h), Some(t)) => (h, t),
        _ => (HEAD.to_owned(), TAIL.to_owned()),
    }
}

/// CJS 扩展名键（node `Module._extensions` 活对象；注册顺序）。
/// 前置：cx 在 realm 内。无即空。
pub(crate) fn cjs_extensions_keys(cx: &mut mozjs::context::JSContext) -> Vec<String> {
    let obj = state::with_rooted(|s| s.cjs_extensions.as_ref().map(|h| h.get()));
    let Some(obj) = obj else {
        return Vec::new();
    };
    rooted!(&in(cx) let obj_root: *mut JSObject = obj);
    let Some(json) = crate::jsapi_glue::own_keys_json(cx, obj_root.get()) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<String>>(&json).unwrap_or_default()
}

/// CJS 自定义扩展处理器（最长后缀匹配；node `findLongestRegisteredExtension`
/// 口径）。返回 handler（已 rooted 槽值需调用方重 root）。
/// 前置：cx 在 realm 内；filename 为绝对路径串。
pub(crate) fn cjs_custom_handler(
    cx: &mut mozjs::context::JSContext,
    filename: &str,
) -> Option<JSVal> {
    // node `findLongestRegisteredExtension` 口径：基名逐点（首点跳过），首个
    // 已注册即最长；未注册即 None（走 native 内建分派）。
    let obj = state::with_rooted(|s| s.cjs_extensions.as_ref().map(|h| h.get()))?;
    rooted!(&in(cx) let obj_root: *mut JSObject = obj);
    let Some(json) = crate::jsapi_glue::own_keys_json(cx, obj_root.get()) else {
        return None;
    };
    let keys: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
    let registered: std::collections::HashSet<&str> = keys.iter().map(|k| k.as_str()).collect();
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let mut start = 0usize;
    loop {
        let rel = &base[start..];
        let Some(dot) = rel.find('.') else {
            break;
        };
        let index = start + dot;
        start = index + 1;
        if index == 0 {
            continue;
        }
        let cand = &base[index..];
        if registered.contains(cand) {
            let ck = std::ffi::CString::new(cand).ok()?;
            let v = crate::jsapi_glue::get_prop_value(cx, obj_root.get(), &ck)?;
            if v.is_object() {
                return Some(v);
            }
            return None;
        }
    }
    None
}

/// UNSAFE-BOUNDARY: `__wjs2_cjs_live(wrapper, extensions)` → undefined。
/// `Module` 活对象进 state（编译/解析期现读；setter + 初始化调用）。
/// 前置：两参皆对象；覆盖：`tests/node/vm.rs::module_surface_faces`。
pub unsafe extern "C" fn cjs_live(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同 require_native
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 || !frame.arg(0).is_object() || !frame.arg(1).is_object() {
        report_error(&mut cx, "TypeError: __wjs2_cjs_live needs (wrapper array, extensions object)");
        return false;
    }
    state::cjs_live_set(frame.arg(0).to_object(), frame.arg(1).to_object());
    frame.set_rval(UndefinedValue());
    true
}

/// UNSAFE-BOUNDARY: `__wjs2_require_cache_delete(urlOrPath)` → boolean。
/// 清 native 注册表（`require.cache` Proxy `delete` 陷阱用；JS 侧背书另清）。
/// 前置：字符串参数；覆盖：同上。
pub unsafe extern "C" fn require_cache_delete(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_string() {
        report_error(&mut cx, "TypeError: __wjs2_require_cache_delete needs a url string");
        return false;
    }
    let key = value_to_string(&mut cx, frame.arg(0));
    // 路径串转 file: URL（`delete require.cache[path]` 口径）。
    let url = if key.starts_with("file:") {
        key.clone()
    } else {
        match Url::from_file_path(&key) {
            Ok(u) => u.as_str().to_owned(),
            Err(_) => key.clone(),
        }
    };
    let had = state::cjs_find(&mut cx, &url).is_some();
    state::cjs_remove(&url);
    rooted!(&in(cx) let mut v = UndefinedValue());
    (had).to_jsval(&mut cx, v.handle_mut());
    frame.set_rval(v.get());
    true
}

/// UNSAFE-BOUNDARY: `__wjs2_require_cache_has(url)` → boolean。
/// `require.cache` Proxy `has`/`in` 陷阱用。
/// 前置：字符串参数；覆盖：同上。
pub unsafe extern "C" fn require_cache_has(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_string() {
        report_error(&mut cx, "TypeError: __wjs2_require_cache_has needs a url string");
        return false;
    }
    let url = value_to_string(&mut cx, frame.arg(0));
    let had = state::cjs_find(&mut cx, &url).is_some();
    rooted!(&in(cx) let mut v = UndefinedValue());
    (had).to_jsval(&mut cx, v.handle_mut());
    frame.set_rval(v.get());
    true
}

/// UNSAFE-BOUNDARY: `__wjs2_require_cache_get(url)` → 模块对象｜undefined。
/// `require.cache` Proxy `get` 陷阱用（返回模块本体，`.exports` 照读）。
/// 前置：同上；覆盖：同上。
pub unsafe extern "C" fn require_cache_get(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_string() {
        report_error(&mut cx, "TypeError: __wjs2_require_cache_get needs a url string");
        return false;
    }
    let url = value_to_string(&mut cx, frame.arg(0));
    match state::cjs_find_module(&url) {
        Some(m) => {
            rooted!(&in(cx) let m_root = m);
            frame.set_rval(m_root.get());
        }
        None => frame.set_rval(UndefinedValue()),
    }
    true
}

/// UNSAFE-BOUNDARY: `__wjs2_require_cache_set(url, module)` → undefined。
/// `require.cache` Proxy `set` 陷阱用（存模块本体 + 其 `exports` 双份）。
/// 前置：url 串 + 对象；覆盖：同上。
pub unsafe extern "C" fn require_cache_set(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 || !frame.arg(0).is_string() || !frame.arg(1).is_object() {
        report_error(&mut cx, "TypeError: __wjs2_require_cache_set needs (url, module object)");
        return false;
    }
    let url = value_to_string(&mut cx, frame.arg(0));
    rooted!(&in(cx) let mod_root: *mut JSObject = frame.arg(1).to_object());
    let exports_v = match get_prop_value(&mut cx, mod_root.get(), c"exports") {
        Some(v) => v,
        None => {
            report_error(&mut cx, "TypeError: __wjs2_require_cache_set needs a module with exports");
            return false;
        }
    };
    state::cjs_remove(&url);
    state::cjs_register(url, exports_v, Some(mozjs::jsval::ObjectValue(mod_root.get())));
    frame.set_rval(UndefinedValue());
    true
}

