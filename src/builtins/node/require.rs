//! `require()` CJS（plan Phase 4d-1/c-4x）：
//! - `require("node:X")`：ESM 子图求值（幂等标记）+ `default` 导出（缺省回 namespace）。
//! - `require("./rel")`：调用方定位（`describe_scripted_caller`）→ `.json` 解析 /
//!   `.cjs` 包装执行（`exports` 预注册，循环可见半成品；失败清场）/ ESM 报
//!   `ERR_REQUIRE_ESM` 用 `import`。
//! - `.js` 读最近 `package.json` 的 `type`（Node 口径）：`module` → 一律
//!   `ERR_REQUIRE_ESM`；其余（`commonjs`/缺省/无清单）强制 CJS（ESM 语法自然
//!   报 SyntaxError，不再走嗅探误判）。
//! - 调用传参用柯里化 `call_one` 链（§4.9：native 内禁 `Rooted<ValueArray>`）。
//! - 入口 `.cjs` 经 prelude `__wjs2_require_main` 起（`run` 不打印其 exports）。

use mozjs::conversions::ToJSValConvertible as _;
use mozjs::jsapi::JSObject;
use mozjs::jsval::{JSVal, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::{evaluate_script, CompileOptionsWrapper};
use url::Url;

use std::cell::RefCell;
use std::collections::HashSet;

use crate::error::Error;
use crate::jsapi_glue::{
    call_method, call_one, get_prop_value, parse_json, report_error, set_prop_value,
    value_to_string, wrap_cx, Frame,
};
use crate::loader::{load_cjs_js, load_js};
use crate::loader::resolve::resolve_require;
use crate::state;

// CJS 静态发现（具名表/包类型）见 `require_cjs.rs`（F2 按算法族拆分），
// 调用路径经此处重导出，不变。
pub(crate) use super::require_cjs::nearest_pkg_type;

// 发现期在飞护栏（递归环上重入即放弃，调用方退 default-only 垫片）。
thread_local! {
    pub(crate) static NAMES_INFLIGHT: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// 调用方 base（脚本文件名 → URL；eval/prelude 等非 URL 返回 None）。
/// 10f：入口经典脚本的 filename 是裸文件路径（非 file:// URL，见 runtime 入口
/// 求值），此处回落文件路径 → file URL，否则 CJS 入口的相对 require 全挂。
fn caller_base(cx: &mozjs::context::JSContext) -> Option<Url> {
    let caller = mozjs::rust::describe_scripted_caller(cx).ok()?;
    tracing::debug!(target: "winterjs2::require", caller = caller.filename, "scripted caller");
    if let Ok(url) = Url::parse(&caller.filename) {
        return match url.scheme() {
            "file" | "node" => Some(url),
            _ => None,
        };
    }
    let path = std::path::Path::new(&caller.filename);
    if path.is_absolute() {
        return Url::from_file_path(path).ok();
    }
    None
}

/// 用户代码异常原样透传的哨兵（2026-09-25）：CJS 包装/求值期的 JS 异常**不消费**，
/// 留在 pending 位由 native 入口直接 `return false`——require 调用方拿到原异常对象
/// （身份/类/code/stack 与 node 同），入口报错位置取真实抛点（修前一律
/// `__wjs2_require_main` 的 prelude 424:53，NodeError message 为空）。
const KEEP_PENDING: &str = "\u{0}wjs-keep-pending";

/// 是否为"异常仍 pending、原样透传"哨兵。
pub(crate) fn is_keep_pending(e: &Error) -> bool {
    matches!(e, Error::Other(s) if s == KEEP_PENDING)
}

/// 用户代码失败：有 pending 异常即保留并回哨兵，否则兜底文案。
fn keep_pending(cx: &mut mozjs::context::JSContext) -> String {
    if crate::jsapi_glue::exception_pending(cx) {
        KEEP_PENDING.to_string()
    } else {
        "uncaught exception".to_string()
    }
}

/// native 入口的 require 失败出口：哨兵即异常已 pending（直接 false），
/// 其余按文案抛 Error。
/// `MODULE_NOT_FOUND` 真码：文案以 `Cannot find module` 开头即挂 code
///（真机口径；cache/main-fail 等套件点名）。
pub(crate) fn report_require_err(cx: &mut mozjs::context::JSContext, e: &Error) {
    if !is_keep_pending(e) {
        let msg = e.to_string();
        report_error(cx, &msg);
        if msg.starts_with("Cannot find module") {
            if let Some(exc) = crate::jsapi_glue::take_pending_exception(cx) {
                rooted!(&in(cx) let exc_root = exc);
                rooted!(&in(cx) let mut code_v = UndefinedValue());
                "MODULE_NOT_FOUND".to_jsval(cx, code_v.handle_mut());
                if exc_root.is_object() {
                    let obj = exc_root.get().to_object();
                    rooted!(&in(cx) let obj_root: *mut JSObject = obj);
                    let _ = crate::jsapi_glue::define_prop(
                        cx,
                        obj_root.get(),
                        c"code",
                        code_v.get(),
                    );
                }
                crate::jsapi_glue::set_pending_exception(cx, exc_root.get());
            }
        }
    }
}

/// pending 异常 → 消息串（消费异常；无则兜底）。
pub(crate) fn pending_message(cx: &mut mozjs::context::JSContext) -> String {
    rooted!(&in(cx) let mut exc = UndefinedValue());
    match mozjs::rust::error_info_from_exception_stack(cx, exc.handle_mut()) {
        Some(info) => info.message,
        None => "uncaught exception".to_string(),
    }
}

/// `node:` ESM 求值 + `default` 导出（无 default 回 namespace 本体）。
fn require_esm_default(
    cx: &mut mozjs::context::JSContext,
    global: *mut JSObject,
    url: &Url,
) -> Result<JSVal, String> {
    use mozjs::rust::wrappers2::{GetModuleNamespace, ModuleEvaluate};
    let record = crate::modules::ensure_subgraph(cx, url).map_err(|e| e.to_string())?;
    if !state::module_evaluated(url.as_str()) {
        rooted!(&in(cx) let record_root: *mut JSObject = record);
        rooted!(&in(cx) let mut rval = UndefinedValue());
        // SAFETY: record 为有效 rooted 记录；realm 内同步求值
        if !unsafe { ModuleEvaluate(cx, record_root.handle(), rval.handle_mut()) } {
            let msg = crate::modules::module_error(cx, url.as_str()).to_string();
            return Err(msg);
        }
        state::set_module_evaluated(url.as_str().to_owned());
    }
    rooted!(&in(cx) let record_root: *mut JSObject = record);
    // SAFETY: record 有效；返回的 namespace 由记录保活（引擎内边）
    let ns = unsafe { GetModuleNamespace(cx, record_root.handle()) };
    if ns.is_null() {
        return Err(format!("cannot read namespace of '{}'", url.as_str()));
    }
    rooted!(&in(cx) let ns_root: *mut JSObject = ns);
    let _ = global;
    match get_prop_value(cx, ns_root.get(), c"default") {
        Some(v) if !v.is_undefined() => Ok(v),
        _ => Ok(mozjs::jsval::ObjectValue(ns_root.get())),
    }
}

/// CJS 编译文件名：file URL → 绝对路径（非 file 方案原样）。
pub(crate) fn cjs_script_name(url: &Url) -> String {
    url.to_file_path().ok().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| url.as_str().to_owned())
}

/// CJS 包装执行（`load_js` 已转译；`module` 经 prelude 建；返回终态 `module.exports`）。
#[allow(clippy::too_many_lines)]
fn require_cjs_file(
    cx: &mut mozjs::context::JSContext,
    global: *mut JSObject,
    url: &Url,
    js: &str,
) -> Result<JSVal, String> {
    // 柯里化包装（单参链，§4.9 合规；`module`/`exports` 同一对象起）。
    // 包装片取 `Module.wrapper` 活值（改写即对后续编译生效；默认与旧壳字节
    // 恒等，行号恒等）。
    let (whead, wtail) = cjs_wrapper_parts(cx);
    let wrapped = format!("{whead}{js}{wtail}");
    // TEMP-DIAG-7171: wrapped 内容取证（bind-store strict 案）。
    if std::env::var("WJS_DUMP_WRAP").is_ok() {
        eprintln!("WRAP-HEAD{:?} JS-HEAD{:?}", &whead, js.chars().take(120).collect::<String>());
    }
    // node 口径：CJS 栈帧是绝对路径（ESM 才是 file URL）——`stack.includes(__filename)` 类断言。
    let c_filename = std::ffi::CString::new(cjs_script_name(url)).unwrap_or_else(|_| c"module.js".into());
    // 起始行 0：包装头独占第 0 行，用户代码第 1 行即物理第 1 行（栈/报错行号
    // 与源文件一致；修前恒 +1，mapper 需折算）。
    let options = CompileOptionsWrapper::new(cx, c_filename, 0);
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    rooted!(&in(cx) let mut fn_v = UndefinedValue());
    let res = evaluate_script(cx, global_root.handle(), wrapped.as_str(), fn_v.handle_mut(), options);
    if res.is_err() {
        return Err(keep_pending(cx));
    }
    if !fn_v.is_object() {
        return Err(format!("cannot load '{}': wrapper failed", url.as_str()));
    }
    // module 对象（prelude 建；`{exports: {}, id, filename, paths}`）。
    let Some(make_fn) = get_prop_value(cx, global_root.get(), c"__wjs2_make_module") else {
        return Err("prelude helper __wjs2_make_module missing".into());
    };
    // §4.80 同族（async-dispose 138 实锤）：make_fn 裸值禁跨 to_jsval 分配——
    // 字符串具现可触发 GC 搬移，栈拷贝即悬垂（call_one 的入口 rooting 盖不住
    // 调用间窗口）；先入槽再分配。
    rooted!(&in(cx) let make_fn_root = make_fn);
    rooted!(&in(cx) let mut url_v = UndefinedValue());
    url.as_str().to_jsval(cx, url_v.handle_mut());
    let Some(module_v) = call_one(cx, global_root.get(), make_fn_root.get(), url_v.get()) else {
        return Err(pending_message(cx));
    };
    if !module_v.is_object() {
        return Err(format!("cannot load '{}': bad module object", url.as_str()));
    }
    rooted!(&in(cx) let module_root: *mut JSObject = module_v.to_object());
    // CJS 求值栈（parent/children 归属；RAII 出栈，异常路径同出）。
    struct CjsStackGuard;
    impl Drop for CjsStackGuard {
        fn drop(&mut self) {
            state::cjs_stack_pop();
        }
    }
    state::cjs_stack_push(url.as_str().to_owned(), module_root.get());
    let _stack_guard = CjsStackGuard;
    // parent 回填（栈次顶；入口即 null，prelude 默认）。
    if let Some(parent) = state::with_rooted(|s| {
        let n = s.cjs_require_stack.len();
        (n >= 2).then(|| s.cjs_require_stack[n - 2].get())
    }) {
        rooted!(&in(cx) let parent_root = parent);
        rooted!(&in(cx) let parent_v = mozjs::jsval::ObjectValue(parent_root.get()));
        let _ = set_prop_value(cx, module_root.get(), c"__wjs2_parent", parent_v.get());
    }
    let Some(exports_v) = get_prop_value(cx, module_root.get(), c"exports") else {
        return Err(pending_message(cx));
    };
    let Some(require_v) = get_prop_value(cx, global_root.get(), c"require") else {
        return Err("global require missing".into());
    };
    // 预注册（循环可见半成品；失败清场）。
    rooted!(&in(cx) let module_val_root2 = module_v);
    state::cjs_register(url.as_str().to_owned(), exports_v, Some(module_val_root2.get()));
    // __filename/__dirname（file: URL；其余退原文）。
    let (filename_s, dirname_s) = match url.to_file_path() {
        Ok(p) => {
            let dir = p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
            (p.to_string_lossy().into_owned(), dir)
        }
        Err(_) => (url.as_str().to_owned(), String::new()),
    };
    // 五连单参调用（§4.9：不用 ValueArray；串先 rooted，避闭包借用冲突）。
    // §4.40/§4.68 教训的 require 版：链内每次 call_one 都可能 GC（curried 闭包
    // 分配 + wrapper 体嵌套 require），cur 与各实参一律全程 rooted、调用点现读
    // 现传——裸 JSVal 栈拷贝在 GC 搬移后即悬垂（css-tree 深图 worker 线程
    // SIGBUS 实锤：cur/arg 位型 0xFFF8/0x5800 垃圾，require 条件修复后 CJS
    // 图暴增才把它养出来）。
    rooted!(&in(cx) let mut cur = fn_v.get());
    rooted!(&in(cx) let exports_root = exports_v);
    rooted!(&in(cx) let require_root = require_v);
    rooted!(&in(cx) let module_val_root = module_v);
    rooted!(&in(cx) let mut s_v = UndefinedValue());
    filename_s.to_jsval(cx, s_v.handle_mut());
    rooted!(&in(cx) let mut d_v = UndefinedValue());
    dirname_s.to_jsval(cx, d_v.handle_mut());
    let mut call_arg = |label: &str, arg: JSVal| -> Result<JSVal, String> {
        if !cur.get().is_object() {
            state::cjs_remove(url.as_str());
            return Err(format!("cannot load '{}': {label} is not callable", url.as_str()));
        }
        match call_one(cx, global_root.get(), cur.get(), arg) {
            Some(v) => {
                cur.set(v);
                Ok(v)
            }
            None => {
                state::cjs_remove(url.as_str());
                Err(keep_pending(cx))
            }
        }
    };
    call_arg("exports", exports_root.get())?;
    call_arg("require", require_root.get())?;
    call_arg("module", module_val_root.get())?;
    call_arg("__filename", s_v.get())?;
    call_arg("__dirname", d_v.get())?;
    // 终态 exports（允许执行期重赋值 `module.exports = …`；先移除预注册再记终态）。
    match get_prop_value(cx, module_root.get(), c"exports") {
        Some(final_v) => {
            // 循环警告原型摘除（node `updateChildren` 恢复口径；仅警告期挂载才动）。
            let final_v = match get_prop_value(cx, global_root.get(), c"__wjs2_cjs_circular_done") {
                Some(done_fn) => {
                    rooted!(&in(cx) let done_root = done_fn);
                    rooted!(&in(cx) let final_root = final_v);
                    call_one(cx, global_root.get(), done_root.get(), final_root.get())
                        .unwrap_or(final_v)
                }
                None => final_v,
            };
            state::cjs_remove(url.as_str());
            state::cjs_register(url.as_str().to_owned(), final_v, Some(module_val_root.get()));
            // children 回填（node 口径：去重 + 内建不进；父为栈次顶）。
            let parent = state::with_rooted(|s| {
                let n = s.cjs_require_stack.len();
                (n >= 2).then(|| s.cjs_require_stack[n - 2].get())
            });
            if let Some(parent) = parent {
                rooted!(&in(cx) let parent_root: *mut JSObject = parent);
                if let Some(children_v) = get_prop_value(cx, parent_root.get(), c"children") {
                    if children_v.is_object() {
                        rooted!(&in(cx) let children_root = children_v);
                        rooted!(&in(cx) let module_v2 =
                            mozjs::jsval::ObjectValue(module_root.get()));
                        rooted!(&in(cx) let children_obj =
                            mozjs::jsval::ObjectValue(children_root.get().to_object()));
                        // 去重（SameValueZero 口径经 includes）。
                        let has = call_method(
                            cx,
                            children_obj.get(),
                            c"includes",
                            &[module_v2.get()],
                        )
                        .map(|v| v.is_boolean() && v.to_boolean())
                        .unwrap_or(true);
                        if !has {
                            let _ = call_method(
                                cx,
                                children_obj.get(),
                                c"push",
                                &[module_v2.get()],
                            );
                        }
                    }
                }
            }
            let _ = set_prop_value(
                cx,
                module_root.get(),
                c"loaded",
                mozjs::jsval::BooleanValue(true),
            );
            Ok(final_v)
        }
        None => {
            state::cjs_remove(url.as_str());
            Err(keep_pending(cx))
        }
    }
}

/// spec → 值（node:/file: 分流；调用方 base 定位相对路径）。
pub(crate) fn require_value(
    cx: &mut mozjs::context::JSContext,
    global: *mut JSObject,
    spec: &str,
    base: Option<Url>,
) -> Result<JSVal, Error> {
    // 内建（含 `fs` 裸名）优先，与 ESM 一致。
    if let Some(canonical) = crate::builtins::node::normalize_spec(spec) {
        let url = Url::parse(canonical)
            .map_err(|e| Error::Other(format!("bad builtin URL: {e}")))?;
        return require_esm_default(cx, global, &url).map_err(Error::Other);
    }
    // R2-iter：`node:` 前缀非内建即真机文案（CJS/ESM 同文；外层再包
    // "Cannot find module" 前缀，正则子串仍命中，见 disabled 套件）。
    if spec.starts_with("node:") {
        return Err(Error::Other(format!("No such built-in module: {spec}")));
    }
    let url = match resolve_require(spec, base.as_ref()) {
        Ok(u) => u,
        Err(e) => {
            // 自定义扩展重试（node tryExtensions 口径：注册键顺序追加试探；
            // 仅 CJS require 路径，不碰共享解析器）。
            let mut retried = None;
            for ext in cjs_extensions_keys(cx) {
                if ext.starts_with('.') {
                    let cand = format!("{spec}{ext}");
                    if let Ok(u) = resolve_require(&cand, base.as_ref()) {
                        retried = Some(u);
                        break;
                    }
                }
            }
            match retried {
                Some(u) => u,
                None => {
                    return Err(Error::Other(format!("Cannot find module '{spec}' ({e})")));
                }
            }
        }
    };
    // DEP0128（`main` 指向缺失文件但 `index` 兜底成功；真机文案逐字；每包一次）。
    if let Ok(path) = url.to_file_path() {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == "index.js" || name == "index.json" || name == "index.node" {
                if let Some(dir) = path.parent() {
                    let pkg_path = dir.join("package.json");
                    if let Ok(text) = std::fs::read_to_string(&pkg_path) {
                        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&text) {
                            let main_field = pkg.get("main").and_then(|m| m.as_str());
                            let main_missing = main_field
                                .map(|m| !dir.join(m).exists())
                                .unwrap_or(false);
                            if main_missing {
                                let warned = state::with_rooted(|s| {
                                    if s.cjs_dep0128_warned.contains(pkg_path.as_path()) {
                                        true
                                    } else {
                                        s.cjs_dep0128_warned.insert(pkg_path.clone());
                                        false
                                    }
                                });
                                if !warned {
                                    if let Some(process_v) = get_prop_value(cx, global, c"process") {
                                        if process_v.is_object() {
                                            rooted!(&in(cx) let process_root = process_v);
                                            let msg = format!(
                                                "Invalid 'main' field in '{}' of '{}'. Please either fix that or report it to the module author",
                                                pkg_path.to_string_lossy(),
                                                main_field.unwrap_or("")
                                            );
                                            rooted!(&in(cx) let mut msg_v = UndefinedValue());
                                            msg.to_jsval(cx, msg_v.handle_mut());
                                            rooted!(&in(cx) let msg_root = msg_v.get());
                                            rooted!(&in(cx) let mut type_v = UndefinedValue());
                                            "DeprecationWarning".to_jsval(cx, type_v.handle_mut());
                                            rooted!(&in(cx) let type_root = type_v.get());
                                            rooted!(&in(cx) let mut code_v = UndefinedValue());
                                            "DEP0128".to_jsval(cx, code_v.handle_mut());
                                            rooted!(&in(cx) let code_root = code_v.get());
                                            let _ = call_method(
                                                cx,
                                                process_root.get(),
                                                c"emitWarning",
                                                &[msg_root.get(), type_root.get(), code_root.get()],
                                            );
                                            let _ = crate::jsapi_glue::take_pending_exception(cx);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    match url.scheme() {
        "node" => require_esm_default(cx, global, &url).map_err(Error::Other),
        "file" => {
            if let Some(hit) = state::cjs_find(cx, url.as_str()) {
                // 缓存命中同样建链（node `updateChildren` 口径：去重；父为栈顶）。
                if let Some(child_mod) = state::cjs_find_module(url.as_str()) {
                    if let Some(parent) = state::with_rooted(|s| {
                        s.cjs_require_stack.last().map(|h| h.get())
                    }) {
                        rooted!(&in(cx) let parent_root: *mut JSObject = parent);
                        if let Some(children_v) = get_prop_value(cx, parent_root.get(), c"children") {
                            if children_v.is_object() {
                                rooted!(&in(cx) let children_root = children_v);
                                rooted!(&in(cx) let child_mod_root = child_mod);
                                rooted!(&in(cx) let children_obj = mozjs::jsval::ObjectValue(
                                    children_root.get().to_object(),
                                ));
                                rooted!(&in(cx) let child_v = mozjs::jsval::ObjectValue(
                                    child_mod_root.get().to_object(),
                                ));
                                let has = call_method(
                                    cx,
                                    children_obj.get(),
                                    c"includes",
                                    &[child_v.get()],
                                )
                                .map(|v| v.is_boolean() && v.to_boolean())
                                .unwrap_or(true);
                                if !has {
                                    let _ = call_method(
                                        cx,
                                        children_obj.get(),
                                        c"push",
                                        &[child_v.get()],
                                    );
                                }
                            }
                        }
                    }
                }
                // 循环中模块走警告代理（node `getExportsForCircularRequire` 口径；
                // 求值中（栈上）即挂警告原型，已落定直返）。
                if state::cjs_inflight(url.as_str()) {
                    if let Some(cw) = get_prop_value(cx, global, c"__wjs2_cjs_circular") {
                        rooted!(&in(cx) let cw_root = cw);
                        rooted!(&in(cx) let mut hit_root = hit);
                        if let Some(w) = call_one(cx, global, cw_root.get(), hit_root.get()) {
                            return Ok(w);
                        }
                        let _ = crate::jsapi_glue::take_pending_exception(cx);
                    }
                }
                return Ok(hit);
            }
            let path = url
                .to_file_path()
                .map_err(|_| Error::Other(format!("Cannot find module '{spec}'")))?;
            let filename_str = path.to_string_lossy().into_owned();
            // 自定义扩展处理器（`Module._extensions` 活表；最长后缀匹配优先于
            // 内建分派；node 口径 handler(module, filename)；进 native 缓存，
            // `delete` 清之）。
            if let Some(handler_v) = cjs_custom_handler(cx, &filename_str) {
                rooted!(&in(cx) let global_root: *mut JSObject = global);
                rooted!(&in(cx) let handler_root = handler_v);
                rooted!(&in(cx) let mut fname_v = UndefinedValue());
                filename_str.to_jsval(cx, fname_v.handle_mut());
                let Some(make_fn) = get_prop_value(cx, global_root.get(), c"__wjs2_make_module") else {
                    return Err(Error::Other("prelude helper __wjs2_make_module missing".into()));
                };
                rooted!(&in(cx) let make_fn_root = make_fn);
                rooted!(&in(cx) let mut url_v = UndefinedValue());
                url.as_str().to_jsval(cx, url_v.handle_mut());
                let Some(module_v) = call_one(cx, global_root.get(), make_fn_root.get(), url_v.get())
                else {
                    return Err(Error::Other(pending_message(cx)));
                };
                if !module_v.is_object() {
                    return Err(Error::Other(format!("cannot load '{spec}': bad module object")));
                }
                rooted!(&in(cx) let module_root: *mut JSObject = module_v.to_object());
                struct CjsStackGuard;
                impl Drop for CjsStackGuard {
                    fn drop(&mut self) {
                        state::cjs_stack_pop();
                    }
                }
                state::cjs_stack_push(url.as_str().to_owned(), module_root.get());
                let _stack_guard = CjsStackGuard;
                if let Some(parent) = state::with_rooted(|s| {
                    let n = s.cjs_require_stack.len();
                    (n >= 2).then(|| s.cjs_require_stack[n - 2].get())
                }) {
                    rooted!(&in(cx) let parent_root = parent);
                    rooted!(&in(cx) let parent_v = mozjs::jsval::ObjectValue(parent_root.get()));
                    let _ = set_prop_value(cx, module_root.get(), c"__wjs2_parent", parent_v.get());
                }
                rooted!(&in(cx) let module_v2 = mozjs::jsval::ObjectValue(module_root.get()));
                let ok = crate::jsapi_glue::call_two(
                    cx,
                    global_root.get(),
                    handler_root.get(),
                    module_v2.get(),
                    fname_v.get(),
                );
                if ok.is_none() {
                    return Err(Error::Other(keep_pending(cx)));
                }
                let _ = set_prop_value(
                    cx,
                    module_root.get(),
                    c"loaded",
                    mozjs::jsval::BooleanValue(true),
                );
                return match get_prop_value(cx, module_root.get(), c"exports") {
                    Some(v) => {
                        state::cjs_register(
                            url.as_str().to_owned(),
                            v,
                            Some(mozjs::jsval::ObjectValue(module_root.get())),
                        );
                        // children 建链 + 循环警告摘除（与 plain 路径同口径）。
                        if let Some(parent) = state::with_rooted(|s| {
                            let n = s.cjs_require_stack.len();
                            (n >= 2).then(|| s.cjs_require_stack[n - 2].get())
                        }) {
                            rooted!(&in(cx) let p_root: *mut JSObject = parent);
                            if let Some(children_v) = get_prop_value(cx, p_root.get(), c"children") {
                                if children_v.is_object() {
                                    rooted!(&in(cx) let children_root = children_v);
                                    rooted!(&in(cx) let children_obj = mozjs::jsval::ObjectValue(
                                        children_root.get().to_object(),
                                    ));
                                    rooted!(&in(cx) let child_v = mozjs::jsval::ObjectValue(module_root.get()));
                                    let has = call_method(
                                        cx,
                                        children_obj.get(),
                                        c"includes",
                                        &[child_v.get()],
                                    )
                                    .map(|v| v.is_boolean() && v.to_boolean())
                                    .unwrap_or(true);
                                    if !has {
                                        let _ = call_method(
                                            cx,
                                            children_obj.get(),
                                            c"push",
                                            &[child_v.get()],
                                        );
                                    }
                                }
                            }
                        }
                        if let Some(done_fn) = get_prop_value(cx, global_root.get(), c"__wjs2_cjs_circular_done") {
                            rooted!(&in(cx) let done_root = done_fn);
                            rooted!(&in(cx) let v_root = v);
                            let _ = call_one(cx, global_root.get(), done_root.get(), v_root.get());
                        }
                        Ok(v)
                    }
                    None => Err(Error::Other(keep_pending(cx))),
                };
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            // napi（`.node` 二进制，plan-napi M0）：权限门控（复用 --allow-ffi）
            // → dlopen + register → exports（9j 的可读拒错退役）。
            // 非法二进制预检（真机口径：平台相关文案；libloading 0.8 在残缺
            // 文件上只给 `dlopen failed` 无细节，此处按魔数直判）。
            if ext == "node" {
                let bad_magic = match std::fs::read(&path) {
                    Ok(b) => {
                        b.len() < 4
                            || !matches!(
                                (b[0], b[1], b[2], b[3]),
                                (0xFE, 0xED, 0xFA, 0xCE)
                                    | (0xFE, 0xED, 0xFA, 0xCF)
                                    | (0xCE, 0xFA, 0xED, 0xFE)
                                    | (0xCF, 0xFA, 0xED, 0xFE)
                                    | (0xCA, 0xFE, 0xBA, 0xBE)
                                    | (0xCA, 0xFE, 0xBA, 0xBF)
                            )
                    }
                    Err(_) => false,
                };
                if bad_magic {
                    #[cfg(target_os = "macos")]
                    return Err(Error::Other(format!(
                        "cannot load native module '{spec}': not a mach-o file"
                    )));
                    #[cfg(target_os = "linux")]
                    return Err(Error::Other(format!(
                        "cannot load native module '{spec}': Exec format error"
                    )));
                    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                    return Err(Error::Other(format!(
                        "cannot load native module '{spec}': bad native format"
                    )));
                }
                return crate::napi::load(cx, global, spec, &path).map_err(Error::Other);
            }
            if ext == "json" {
                let text = std::fs::read_to_string(&path).map_err(|e| {
                    Error::Other(format!("Cannot find module '{spec}' ({})", e))
                })?;
                match parse_json(cx, global, &text) {
                    Some(v) => {
                        state::cjs_register(url.as_str().to_owned(), v, None);
                        Ok(v)
                    }
                    None => Err(Error::Other(format!("'{spec}' is not valid JSON"))),
                }
            } else {
                let text = std::fs::read_to_string(&path).map_err(|e| {
                    Error::Other(format!("Cannot find module '{spec}' ({e})"))
                })?;
                // require(esm)（Node ≥22.12，26 无条件）：type==module 的 .js
                // 或带模块语法的文件走同步求值返回 namespace；其余强制 CJS
                // （ESM 语法在包装执行期自然报 SyntaxError）。
                // CJS 分类经 `cjs_interop`（含顶层 return 的函数体 goal 重试，
                // 同 runtime 入口口径）——命中即走 CJS goal 转译，module goal
                // 对此类文件必报 "return not in function"。
                let type_module = ext == "js" && nearest_pkg_type(&path).as_deref() == Some("module");
                if !type_module && crate::modules::cjs_interop(&url, false, &text) {
                    let loaded = load_cjs_js(&text, url.as_str(), &path).map_err(|e| {
                        Error::Other(format!("Cannot load '{spec}' ({e})"))
                    })?;
                    return require_cjs_file(cx, global, &url, &loaded.js).map_err(Error::Other);
                }
                let loaded = load_js(&text, url.as_str(), &path).map_err(|e| {
                    Error::Other(format!("Cannot load '{spec}' ({e})"))
                })?;
                if type_module || loaded.is_module {
                    return crate::modules::require_esm(cx, &url);
                }
                require_cjs_file(cx, global, &url, &loaded.js).map_err(Error::Other)
            }
        }
        s => Err(Error::Other(format!("Cannot require '{spec}' (scheme '{s}:')"))),
    }
}

/// `require(id)` 全局函数（裸 native：调用方定位要求直调，禁 prelude 包装）。
pub unsafe extern "C" fn require_native(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_string() {
        // node 口径：`TypeError [ERR_INVALID_ARG_TYPE]` + 值按类格式化
        //（真机逐字：数字/布尔带类型前缀，对象为 an instance of，null/undefined 直给）。
        let msg = if frame.argc() < 1 {
            "The \"id\" argument must be of type string. Received undefined".to_owned()
        } else {
            let a = frame.arg(0);
            let detail = if a.is_number() {
                format!("type number ({})", crate::jsapi_glue::value_to_string(&mut cx, a))
            } else if a.is_boolean() {
                format!("type boolean ({})", crate::jsapi_glue::value_to_string(&mut cx, a))
            } else if a.is_object() {
                // 真机为 `an instance of Object/Array`；此处统一 Object（套件只验前缀）。
                "an instance of Object".to_owned()
            } else {
                crate::jsapi_glue::value_to_string(&mut cx, a)
            };
            format!("The \"id\" argument must be of type string. Received {detail}")
        };
        crate::jsapi_glue::throw_coded_error(&mut cx, c"TypeError", Some("ERR_INVALID_ARG_TYPE"), &msg);
        return false;
    }
    if crate::jsapi_glue::value_to_string(&mut cx, frame.arg(0)).is_empty() {
        // 空串 id 即非法值（真机逐字）。
        crate::jsapi_glue::throw_coded_error(
            &mut cx,
            c"TypeError",
            Some("ERR_INVALID_ARG_VALUE"),
            "The argument 'id' must be a non-empty string. Received ''",
        );
        return false;
    }
    let spec = value_to_string(&mut cx, frame.arg(0));
    let global = state::global();
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    let base = caller_base(&cx);
    match require_value(&mut cx, global_root.get(), &spec, base) {
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

/// `__wjs2_require_resolve(id)` → 解析后 URL 串（`require.resolve` 用；同调用方规则）。
pub unsafe extern "C" fn require_resolve(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_string() {
        crate::jsapi_glue::throw_named_error(&mut cx, c"TypeError", "require.resolve needs a module id string");
        return false;
    }
    let spec = value_to_string(&mut cx, frame.arg(0));
    let base = caller_base(&cx);
    let url = if let Some(canonical) = crate::builtins::node::normalize_spec(&spec) {
        Url::parse(canonical).map_err(|e| Error::Other(format!("bad builtin URL: {e}")))
    } else {
        resolve_require(&spec, base.as_ref())
    };
    match url {
        Ok(u) => {
            rooted!(&in(cx) let mut v = UndefinedValue());
            u.as_str().to_jsval(&mut cx, v.handle_mut());
            frame.set_rval(v.get());
            true
        }
        Err(e) => {
            report_error(&mut cx, &format!("Cannot find module '{spec}' ({e})"));
            false
        }
    }
}

pub use super::require_prelude::REQUIRE_PRELUDE;


/// 显式 base 解析（`createRequire(filename)` 用；`file:` URL 或路径，
/// 相对路径按 cwd 拼；非法 base 即错，不回落调用方）。
fn explicit_base(base_s: &str) -> Result<Url, String> {
    if let Ok(u) = Url::parse(base_s)
        && u.scheme() == "file"
    {
        return Ok(u);
    }
    let path = std::path::PathBuf::from(base_s);
    let abs = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cannot get cwd: {e}"))?
            .join(path)
    };
    Url::from_file_path(&abs).map_err(|_| format!("bad require base '{base_s}'"))
}

/// UNSAFE-BOUNDARY: `__wjs2_require_from(base, id)` → `createRequire` 底座，
/// 显式 base 复用 `require_value`（调用方定位/JSON/CJS/ESM 口径与全局 `require`
/// 完全一致）；前置：两参皆字符串（非串即 TypeError，不读值）；
/// 覆盖：`tests/node.rs::module_create_require`。
pub unsafe extern "C" fn require_from(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 || !frame.arg(0).is_string() || !frame.arg(1).is_string() {
        report_error(&mut cx, "TypeError: __wjs2_require_from needs (base, id) strings");
        return false;
    }
    let base_s = value_to_string(&mut cx, frame.arg(0));
    let spec = value_to_string(&mut cx, frame.arg(1));
    let global = state::global();
    rooted!(&in(cx) let global_root: *mut JSObject = global);
    let base = match explicit_base(&base_s) {
        Ok(u) => u,
        Err(e) => {
            report_error(&mut cx, &format!("TypeError: {e}"));
            return false;
        }
    };
    match require_value(&mut cx, global_root.get(), &spec, Some(base)) {
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

/// UNSAFE-BOUNDARY: `__wjs2_require_resolve_from(base, id)` → 解析后 URL 串
/// （`createRequire().resolve` 用；同显式 base 规则）；
/// 前置同上；覆盖：`tests/node.rs::module_create_require`。
pub unsafe extern "C" fn require_resolve_from(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 || !frame.arg(0).is_string() || !frame.arg(1).is_string() {
        report_error(&mut cx, "TypeError: __wjs2_require_resolve_from needs (base, id) strings");
        return false;
    }
    let base_s = value_to_string(&mut cx, frame.arg(0));
    let spec = value_to_string(&mut cx, frame.arg(1));
    let base = match explicit_base(&base_s) {
        Ok(u) => u,
        Err(e) => {
            report_error(&mut cx, &format!("TypeError: {e}"));
            return false;
        }
    };
    let url = if let Some(canonical) = crate::builtins::node::normalize_spec(&spec) {
        Url::parse(canonical).map_err(|e| Error::Other(format!("bad builtin URL: {e}")))
    } else {
        resolve_require(&spec, Some(&base))
    };
    match url {
        Ok(u) => {
            rooted!(&in(cx) let mut v = UndefinedValue());
            u.as_str().to_jsval(&mut cx, v.handle_mut());
            frame.set_rval(v.get());
            true
        }
        Err(e) => {
            report_error(&mut cx, &format!("Cannot find module '{spec}' ({e})"));
            false
        }
    }
}


// Module/CJS 活表面（`require_module.rs`，F2 按算法族拆分），调用方经此处重导出，不变。
pub use super::require_module::{
    builtin_modules_json, cjs_compile, cjs_live, require_cache_delete, require_cache_get,
    require_cache_has, require_cache_set, require_cjs_by_url, require_main_url,
};
pub(crate) use super::require_module::{cjs_custom_handler, cjs_export_names, cjs_extensions_keys, cjs_wrapper_parts};
