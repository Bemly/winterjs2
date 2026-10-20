//! `node:vm` 模块静态请求表（纯 Rust，无 JSAPI）。
//!
//! `vm_module.rs` 的子模块（`#[path]` 挂接，不碰 `mod.rs`）：oxc 二次解析
//! `loaded.js`（引擎编译文本），按语句序收集静态请求（spec/attrs/phase），
/// `with` 子句空格化剥离（V8 不透明口径，行列恒等）。

/// 有序去重后的单条静态请求（`moduleRequests`/`dependencySpecifiers` 同源；
/// 去重键 (spec, attrs, phase)，首现序——node 真机口径）。
pub struct VmRequest {
    pub spec: String,
    pub attrs: Vec<(String, String)>,
    pub phase: &'static str,
}

/// oxc 二次解析（`loaded.js` 即引擎编译文本，转译后与引擎一致；
///
/// 纯 Rust，无 JSAPI）：按语句序收集静态请求（import/export-from；
/// `import type` 整声明跳过；属性按 `with` 子句键排序）。
/// 返回（请求表，剥离后源码）：SM153 仅认 `type` 属性键（Modules.cpp
/// `We only support "type"`；其余键加载期即拒），而 V8 把属性当不透明
/// 透传 linker——为对齐真机，`with` 子句按 span 空格化（换行保留，
/// 行列恒等），引擎只见无属性请求；JS 面的属性（moduleRequests/linker
/// extra）仍取自原文解析。剥离物自校验（复解析：同 spec/phase、无残留
/// 子句），失手即回落原文。
/// UNSAFE-BOUNDARY 无（纯解析），覆盖见本文件 `#[cfg(test)]`。
pub fn extract_requests(js: &str, ext: &str) -> (Vec<VmRequest>, Option<String>) {
    use oxc::{allocator::Allocator, parser::Parser, span::SourceType};
    let allocator = Allocator::default();
    let path = std::path::PathBuf::from(format!("vm_mod.{ext}"));
    let source_type = SourceType::from_path(&path)
        .map(|t| t.with_module(true))
        .unwrap_or_else(|_| SourceType::mjs());
    let ret = Parser::new(&allocator, js, source_type).parse();
    if ret.fatal_error {
        return (Vec::new(), None);
    }
    let mut out: Vec<VmRequest> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut spans: Vec<(u32, u32)> = Vec::new();
    let mut push = |spec: &str, attrs: Vec<(String, String)>, phase: &'static str| {
        let mut attrs = attrs;
        attrs.sort();
        attrs.dedup();
        if seen.insert((spec.to_owned(), attrs.clone(), phase)) {
            out.push(VmRequest { spec: spec.to_owned(), attrs, phase });
        }
    };
    for stmt in ret.program.body.iter() {
        use oxc::ast::ast::{ImportOrExportKind, Statement};
        match stmt {
            Statement::ImportDeclaration(d) => {
                if matches!(d.import_kind, ImportOrExportKind::Type) {
                    continue;
                }
                let phase = match d.phase {
                    Some(oxc::ast::ast::ImportPhase::Source) => "source",
                    _ => "evaluation",
                };
                if let Some(wc) = d.with_clause.as_deref() {
                    // 回扩失手（理论不可达）即整单回落原文，不半剥离。
                    let Some(s) = extend_to_with_keyword(js, wc.span.start) else {
                        return (out, None);
                    };
                    spans.push((s, wc.span.end));
                }
                push(d.source.value.as_str(), with_attrs(d.with_clause.as_deref()), phase);
            }
            Statement::ExportAllDeclaration(d) => {
                if matches!(d.export_kind, ImportOrExportKind::Type) {
                    continue;
                }
                if let Some(wc) = d.with_clause.as_deref() {
                    let Some(s) = extend_to_with_keyword(js, wc.span.start) else {
                        return (out, None);
                    };
                    spans.push((s, wc.span.end));
                }
                push(d.source.value.as_str(), with_attrs(d.with_clause.as_deref()), "evaluation");
            }
            Statement::ExportNamedDeclaration(_) => {}
            Statement::ExportFromDeclaration(d) => {
                if matches!(d.export_kind, ImportOrExportKind::Type) {
                    continue;
                }
                if let Some(wc) = d.with_clause.as_deref() {
                    let Some(s) = extend_to_with_keyword(js, wc.span.start) else {
                        return (out, None);
                    };
                    spans.push((s, wc.span.end));
                }
                push(d.source.value.as_str(), with_attrs(d.with_clause.as_deref()), "evaluation");
            }
            _ => {}
        }
    }
    if spans.is_empty() {
        return (out, None);
    }
    spans.sort();
    spans.dedup();
    let stripped = blank_spans(js, &spans);
    let Some(stripped) = stripped else {
        return (out, None);
    };
    // 自校验：复解析剥离物——同 spec/phase 序列、无残留子句、无属性。
    let (re, re_stripped) = extract_requests(&stripped, ext);
    let same = re.len() == out.len()
        && re.iter().zip(out.iter()).all(|(a, b)| a.spec == b.spec && a.phase == b.phase)
        && re.iter().all(|r| r.attrs.is_empty())
        && re_stripped.is_none();
    if same { (out, Some(stripped)) } else { (out, None) }
}

/// `with` 子句 span 起点回扩到关键字（含其前空白；oxc span 只覆 `{...}`）。
/// 找不到关键字即 None（调用方回落原文，不冒险半剥离）。
fn extend_to_with_keyword(js: &str, s: u32) -> Option<u32> {
    let bytes = js.as_bytes();
    let mut i = s as usize;
    while i > 0 && matches!(bytes[i - 1], b' ' | b'\t' | b'\r' | b'\n') {
        i -= 1;
    }
    for kw in ["with", "assert"] {
        if i >= kw.len() && &bytes[i - kw.len()..i] == kw.as_bytes() {
            let before = i - kw.len();
            if before == 0
                || !(bytes[before - 1].is_ascii_alphanumeric()
                    || bytes[before - 1] == b'_'
                    || bytes[before - 1] == b'$')
            {
                return Some(before as u32);
            }
        }
    }
    None
}

/// 动态 `import(spec, opts?)` 的 `with`/`assert` 子句空格化（SM153 仅认 `type`
/// 键、其余键加载期即拒；V8 视属性为不透明，真机回调照见原文属性）。
/// 含非 `type` 键的 options 对象整体空格化（换行保留，行列恒等）；纯 `type`
/// 顶层 await 探测（`SourceTextModule#hasTopLevelAwait` 面；真机 V8 口径）。
/// 模块顶层（函数体外；类静态块计入，方法/getter/setter/构造器不计）
/// 出现 `await` 即 true。`async function` 体的 await 不算。
/// 解析失败 → false。
pub fn has_top_level_await(source: &str, ext: &str) -> bool {
    use oxc::ast_visit::Visit;
    use oxc::{allocator::Allocator, parser::Parser, span::SourceType};
    let allocator = Allocator::default();
    let path = std::path::PathBuf::from(format!("vm_tla.{ext}"));
    let source_type = SourceType::from_path(&path)
        .map(|t| t.with_module(true))
        .unwrap_or_else(|_| SourceType::mjs());
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.fatal_error {
        return false;
    }
    struct Grab<'a> {
        fn_spans: Vec<oxc::span::Span>,
        await_spans: Vec<oxc::span::Span>,
        phantom: std::marker::PhantomData<&'a ()>,
    }
    impl<'a> Visit<'a> for Grab<'a> {
        fn visit_function(&mut self, it: &oxc::ast::ast::Function<'a>, flags: oxc::syntax::scope::ScopeFlags) {
            self.fn_spans.push(it.span);
            oxc::ast_visit::walk::walk_function(self, it, flags);
        }
        fn visit_arrow_function_expression(&mut self, it: &oxc::ast::ast::ArrowFunctionExpression<'a>) {
            self.fn_spans.push(it.span);
            oxc::ast_visit::walk::walk_arrow_function_expression(self, it);
        }
        fn visit_await_expression(&mut self, it: &oxc::ast::ast::AwaitExpression<'a>) {
            self.await_spans.push(it.span);
            oxc::ast_visit::walk::walk_await_expression(self, it);
        }
    }
    let mut grab = Grab { fn_spans: Vec::new(), await_spans: Vec::new(), phantom: std::marker::PhantomData };
    grab.visit_program(&ret.program);
    // 任一 await 落在全部函数体之外即顶层。
    grab.await_spans.iter().any(|a| {
        !grab.fn_spans.iter().any(|f| f.start <= a.start && a.end <= f.end)
    })
}

/// 子句保留（JSON 模块引擎面用）；非字面量 options 不动（attrs 侧回空）。
/// 无需剥离/解析失败 → None（调用方用原文）。
pub fn strip_dynamic_with(source: &str, ext: &str) -> Option<String> {
    use oxc::span::GetSpan;
    use oxc::ast_visit::Visit;
    use oxc::{allocator::Allocator, parser::Parser, span::SourceType};
    let allocator = Allocator::default();
    let path = std::path::PathBuf::from(format!("vm_dyn.{ext}"));
    let source_type = SourceType::from_path(&path)
        .map(|t| t.with_module(true))
        .unwrap_or_else(|_| SourceType::mjs());
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.fatal_error {
        return None;
    }
    struct Grab<'a> {
        spans: Vec<(u32, u32)>,
        phantom: std::marker::PhantomData<&'a ()>,
    }
    impl<'a> Visit<'a> for Grab<'a> {
        fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'a>) {
            if let Some(opts) = it.options.as_ref() {
                if options_has_non_type(opts) {
                    self.spans.push((opts.span().start, opts.span().end));
                }
            }
            oxc::ast_visit::walk::walk_import_expression(self, it);
        }
    }
    let mut grab = Grab { spans: Vec::new(), phantom: std::marker::PhantomData };
    grab.visit_program(&ret.program);
    if grab.spans.is_empty() {
        return None;
    }
    grab.spans.sort();
    grab.spans.dedup();
    blank_spans(source, &grab.spans)
}

/// options 对象是否含非 `type` 属性键（`with`/`assert` 下钻一层；非字面量即保守保留）。
fn options_has_non_type(opts: &oxc::ast::ast::Expression<'_>) -> bool {
    use oxc::ast::ast::{Expression, ObjectPropertyKind, PropertyKey};
    let Expression::ObjectExpression(obj) = opts else {
        return false;
    };
    for prop in obj.properties.iter() {
        let ObjectPropertyKind::ObjectProperty(p) = prop else {
            continue;
        };
        let key = match &p.key {
            PropertyKey::StaticIdentifier(id) => id.name.as_str(),
            PropertyKey::StringLiteral(s) => s.value.as_str(),
            _ => continue,
        };
        if key != "with" && key != "assert" {
            continue;
        }
        let Expression::ObjectExpression(inner) = &p.value else {
            continue;
        };
        for ip in inner.properties.iter() {
            let ObjectPropertyKind::ObjectProperty(q) = ip else {
                // 展开/方法等非常规条目 → 保守剥离（引擎同样拒收）。
                return true;
            };
            let k = match &q.key {
                PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                PropertyKey::StringLiteral(s) => s.value.as_str(),
                _ => return true,
            };
            if k != "type" {
                return true;
            }
        }
    }
    false
}

/// span 区间空格化（`\n` 保留，行列恒等；区间须升序不交叠且越界即 None）。
fn blank_spans(js: &str, spans: &[(u32, u32)]) -> Option<String> {
    let bytes = js.as_bytes();
    let mut last = 0u32;
    for &(s, e) in spans {
        if s < last || e < s || e as usize > bytes.len() {
            return None;
        }
        last = e;
    }
    let mut out = js.as_bytes().to_vec();
    for &(s, e) in spans {
        for b in out[s as usize..e as usize].iter_mut() {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    }
    String::from_utf8(out).ok()
}

/// `with` 子句 → 有序键值对（值恒字符串字面量）。
fn with_attrs(clause: Option<&oxc::ast::ast::WithClause<'_>>) -> Vec<(String, String)> {
    let Some(c) = clause else { return Vec::new() };
    c.with_entries
        .iter()
        .map(|e| {
            let k = match &e.key {
                oxc::ast::ast::ImportAttributeKey::Identifier(id) => id.name.as_str().to_owned(),
                oxc::ast::ast::ImportAttributeKey::StringLiteral(s) => s.value.as_str().to_owned(),
            };
            (k, e.value.value.as_str().to_owned())
        })
        .collect()
}

/// 请求表 JSON（`__wjs2_vm_mod_deps` 面）：`[{specifier,attributes,phase}]`。
pub fn requests_json(reqs: &[VmRequest]) -> String {
    let mut s = String::from("[");
    for (i, r) in reqs.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"specifier\":");
        s.push_str(&serde_json::Value::String(r.spec.clone()).to_string());
        s.push_str(",\"attributes\":{");
        for (j, (k, v)) in r.attrs.iter().enumerate() {
            if j > 0 {
                s.push(',');
            }
            s.push_str(&serde_json::Value::String(k.clone()).to_string());
            s.push(':');
            s.push_str(&serde_json::Value::String(v.clone()).to_string());
        }
        s.push_str("},\"phase\":\"");
        s.push_str(r.phase);
        s.push_str("\"}");
    }
    s.push(']');
    s
}

/// 动态 `import(spec, opts?)` 调用点属性（`importModuleDynamically` 第四参面）。
/// 引擎 hook 不给可用行列（恒 0，见 4.294），按运行时 specifier 匹配源码：
/// 同字面量 spec 首命中，否则源码仅一处动态 import 即兜底，否则空。
/// options 取静态对象中 `with`（回落 `assert`）的串字面量键值（源码序）；
/// 非字面量 options/值（运行时变量）→ 空（真机传求值后对象，引擎墙，见 4.291）；
/// 解析失败 → 空。
pub fn dynamic_import_attrs(source: &str, ext: &str, spec: &str) -> Vec<(String, String)> {
    use oxc::ast_visit::Visit;
    use oxc::{allocator::Allocator, parser::Parser, span::SourceType};
    let allocator = Allocator::default();
    let path = std::path::PathBuf::from(format!("vm_dyn.{ext}"));
    let source_type = SourceType::from_path(&path)
        .map(|t| t.with_module(true))
        .unwrap_or_else(|_| SourceType::mjs());
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.fatal_error {
        return Vec::new();
    }
    struct Grab<'a> {
        specs: Vec<Option<String>>,
        phantom: std::marker::PhantomData<&'a ()>,
    }
    impl<'a> Visit<'a> for Grab<'a> {
        fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'a>) {
            let lit = match &it.source {
                oxc::ast::ast::Expression::StringLiteral(s) => Some(s.value.as_str().to_owned()),
                _ => None,
            };
            self.specs.push(lit);
            oxc::ast_visit::walk::walk_import_expression(self, it);
        }
    }
    let mut grab = Grab { specs: Vec::new(), phantom: std::marker::PhantomData };
    grab.visit_program(&ret.program);
    // 同字面量 spec 首命中；否则源码仅一处动态 import 即兜底；否则空。
    let single = grab.specs.len() == 1;
    if !single && !grab.specs.iter().any(|lit| lit.as_deref() == Some(spec)) {
        return Vec::new();
    }
    struct AttrGrab<'a> {
        spec: &'a str,
        single: bool,
        out: Vec<(String, String)>,
        done: bool,
    }
    impl<'a> Visit<'a> for AttrGrab<'a> {
        fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'a>) {
            if self.done {
                return;
            }
            let lit = match &it.source {
                oxc::ast::ast::Expression::StringLiteral(s) => Some(s.value.as_str().to_owned()),
                _ => None,
            };
            if lit.as_deref() == Some(self.spec) || self.single {
                self.done = true;
                if let Some(opts) = it.options.as_ref() {
                    self.out = options_attrs(opts);
                }
            }
            oxc::ast_visit::walk::walk_import_expression(self, it);
        }
    }
    let mut attr = AttrGrab { spec, single, out: Vec::new(), done: false };
    attr.visit_program(&ret.program);
    attr.out
}

/// options 对象 → `with`（回落 `assert`）键值（串字面量键值对，源码序）。
fn options_attrs(opts: &oxc::ast::ast::Expression<'_>) -> Vec<(String, String)> {
    use oxc::ast::ast::{Expression, ObjectPropertyKind, PropertyKey};
    let Expression::ObjectExpression(obj) = opts else {
        return Vec::new();
    };
    let mut with_obj = None;
    for prop in obj.properties.iter() {
        let ObjectPropertyKind::ObjectProperty(p) = prop else {
            continue;
        };
        let key = match &p.key {
            PropertyKey::StaticIdentifier(id) => id.name.as_str().to_owned(),
            PropertyKey::StringLiteral(s) => s.value.as_str().to_owned(),
            _ => continue,
        };
        if key == "with" || (key == "assert" && with_obj.is_none()) {
            if matches!(&p.value, Expression::ObjectExpression(_)) {
                if key == "with" {
                    with_obj = Some(&p.value);
                    break;
                }
                with_obj = Some(&p.value);
            }
        }
    }
    let Some(Expression::ObjectExpression(attrs)) = with_obj else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for prop in attrs.properties.iter() {
        let ObjectPropertyKind::ObjectProperty(p) = prop else {
            return Vec::new();
        };
        let key = match &p.key {
            PropertyKey::StaticIdentifier(id) => id.name.as_str().to_owned(),
            PropertyKey::StringLiteral(s) => s.value.as_str().to_owned(),
            _ => return Vec::new(),
        };
        let Expression::StringLiteral(v) = &p.value else {
            return Vec::new();
        };
        out.push((key, v.value.as_str().to_owned()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reqs_of(src: &str) -> Vec<(String, Vec<(String, String)>, &'static str)> {
        extract_requests(src, "js").0.into_iter().map(|r| (r.spec, r.attrs, r.phase)).collect()
    }

    #[test]
    fn vm_requests_order_dedup_attrs_phase() {
        // 首现序 + (spec, attrs, phase) 去重（modulerequests 夹具口径）。
        let src = "import { foo } from './foo.js';\n\
            import * as F from './foo.js';\n\
            import { bar } from './bar.json' with { type: 'json' };\n\
            import { q } from './q.js' with { a: '1' };\n\
            import { q2 } from './q.js' with { b: '2', a: '1' };\n\
            import source S from './s.js';\n\
            import * as NS from './s.js';\n\
            export * from './e.js';\n\
            export { z } from './z.js' with { k: 'v' };\n";
        let got = reqs_of(src);
        assert_eq!(got.len(), 8);
        assert_eq!(got[0], ("./foo.js".to_owned(), vec![], "evaluation"));
        assert_eq!(got[1].1, vec![("type".to_owned(), "json".to_owned())]);
        assert_eq!(got[2], ("./q.js".to_owned(), vec![("a".to_owned(), "1".to_owned())], "evaluation"));
        assert_eq!(got[3].1, vec![("a".to_owned(), "1".to_owned()), ("b".to_owned(), "2".to_owned())]);
        assert_eq!(got[4], ("./s.js".to_owned(), vec![], "source"));
        assert_eq!(got[5], ("./s.js".to_owned(), vec![], "evaluation"));
        assert_eq!(got[6], ("./e.js".to_owned(), vec![], "evaluation"));
        assert_eq!(got[7], ("./z.js".to_owned(), vec![("k".to_owned(), "v".to_owned())], "evaluation"));
    }

    #[test]
    fn vm_requests_skips_type_only() {
        // `import type` 仅 TS 合法（.ts 转译后已擦除；此处按 ts 解析验证跳过）。
        let src = "import type { T } from './t.js';\nimport { v } from './v.js';\n";
        let (got, stripped): (Vec<(String, Vec<(String, String)>, &'static str)>, Option<String>) = {
            let (reqs, stripped) = extract_requests(src, "ts");
            (reqs.into_iter().map(|r| (r.spec, r.attrs, r.phase)).collect(), stripped)
        };
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "./v.js");
        assert!(stripped.is_none());
    }

    #[test]
    fn vm_requests_json_shape() {
        let reqs = vec![VmRequest {
            spec: "./a.js".to_owned(),
            attrs: vec![("k".to_owned(), "v".to_owned())],
            phase: "evaluation",
        }];
        assert_eq!(
            requests_json(&reqs),
            r#"[{"specifier":"./a.js","attributes":{"k":"v"},"phase":"evaluation"}]"#
        );
    }

    #[test]
    fn vm_with_clause_stripped_for_engine() {
        // `with` 子句空格化（换行保留）：引擎见无属性请求，JS 面属性保留。
        let src = "import \"foo\" with { n1: 'v1',\n n2: 'v2' };\nimport { a } from './a.js';\n";
        let (reqs, stripped) = extract_requests(src, "js");
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].attrs.len(), 2);
        let stripped = stripped.expect("with-clause must be stripped");
        assert_eq!(stripped.chars().filter(|c| *c == '\n').count(), src.chars().filter(|c| *c == '\n').count());
        assert!(!stripped.contains("n1"));
        let (re, re_stripped) = extract_requests(&stripped, "js");
        assert_eq!(re.len(), 2);
        assert_eq!(re[0].spec, "foo");
        assert!(re.iter().all(|r| r.attrs.is_empty()));
        assert!(re_stripped.is_none());
    }

    #[test]
    fn vm_blank_spans_rejects_overlap() {
        assert!(blank_spans("abcdef", &[(2, 4), (3, 5)]).is_none());
        assert!(blank_spans("abcdef", &[(0, 7)]).is_none());
        assert_eq!(blank_spans("ab\ncd", &[(0, 2)]).as_deref(), Some("  \ncd"));
    }

    #[test]
    fn vm_dynamic_import_attrs_faces() {
        // 正常：同字面量 spec 首命中（源码序）。
        let src = "const p = import(\"foo\", { with: { k: \"v\" } });\n\
            const q = import(\"b\", { with: { \"a\": \"1\", b: \"2\" } });\n";
        assert_eq!(
            dynamic_import_attrs(src, "js", "foo"),
            vec![("k".to_owned(), "v".to_owned())]
        );
        assert_eq!(
            dynamic_import_attrs(src, "js", "b"),
            vec![("a".to_owned(), "1".to_owned()), ("b".to_owned(), "2".to_owned())]
        );
        // 正常：单调用兜底（spec 动态值）。
        assert_eq!(
            dynamic_import_attrs("const p = import(x, { with: { k: \"v\" } });", "js", "whatever"),
            vec![("k".to_owned(), "v".to_owned())]
        );
        // 正常：无 options/非对象 options 即空。
        assert!(dynamic_import_attrs("import(\"x\")", "js", "x").is_empty());
        assert!(dynamic_import_attrs("const o = {}; import(\"x\", o)", "js", "x").is_empty());
        // 报错沿：多调用无字面量命中即空；解析失败即空。
        assert!(dynamic_import_attrs("import(a); import(b);", "js", "c").is_empty());
        assert!(dynamic_import_attrs("import(", "js", "x").is_empty());
        // 边界：非字面量值（运行时变量）即空（引擎墙，见函数头注）。
        assert!(dynamic_import_attrs("const v = \"1\"; import(\"x\", { with: { k: v } })", "js", "x").is_empty());
        // 边界：legacy assert 键回落。
        assert_eq!(
            dynamic_import_attrs("import(\"x\", { assert: { k: \"v\" } })", "js", "x"),
            vec![("k".to_owned(), "v".to_owned())]
        );
    }

    #[test]
    fn vm_strip_dynamic_with_faces() {
        // 正常：非 type 键整体空格化（行列恒等：换行保留、等长）。
        let src = "const p = import(\"foo\", { with: { k: \"v\" } });\nimport(\"b\");";
        let stripped = strip_dynamic_with(src, "js").expect("stripped");
        assert_eq!(stripped.len(), src.len());
        assert_eq!(stripped.chars().filter(|c| *c == '\n').count(), 1);
        assert!(!stripped.contains('k'));
        assert!(stripped.contains("import(\"b\")"));
        // 正常：纯 type 键保留（JSON 引擎面）。
        let keep = "import(\"d.json\", { with: { type: \"json\" } })";
        assert!(strip_dynamic_with(keep, "js").is_none());
        // 正常：无子句/解析失败即 None。
        assert!(strip_dynamic_with("import(\"x\")", "js").is_none());
        assert!(strip_dynamic_with("import(", "js").is_none());
    }

    #[test]
    fn vm_top_level_await_faces() {
        // 正常：顶层 await 即 true。
        assert!(has_top_level_await("export const a = 1;\nawait 0;", "js"));
        // 正常：纯声明/异步函数体即 false。
        assert!(!has_top_level_await("export const a = 1;", "js"));
        assert!(!has_top_level_await("export async function f() { await 0; }", "js"));
        assert!(!has_top_level_await("const f = async () => await 1;", "js"));
        // 边界：类静态块计入（真机 TLA 口径）。
        assert!(has_top_level_await("class C { static { await 1; } }", "js"));
        // 报错：解析失败即 false。
        assert!(!has_top_level_await("export const =", "js"));
    }
}
