//! loader/模块系统黑盒测试(对齐 src/loader/ + src/modules.rs:resolve/转译/循环/http 导入/入口类型)。

mod common;

use common::*;

use assert_fs::prelude::*;

#[test]
fn relative_import() {
    let (_dir, entry) = mod_dir(
        &[
            ("lib.js", "export const x = 40 + 2;\n"),
            (
                "app.js",
                "import { x } from \"./lib.js\";\nconsole.log(x);\n",
            ),
        ],
        "app.js",
    );
    assert_eq!(stdout_of(&mut winterjs2().arg("--run").arg(&entry)), "42\n");
}

#[test]
fn typescript_transpile() {
    let (_dir, entry) = mod_dir(
        &[
            (
                "math.ts",
                "export function add(a: number, b: number): number { return a + b; }\n",
            ),
            (
                "app.ts",
                "import { add } from \"./math\";\nconsole.log(add(40, 2));\n",
            ),
        ],
        "app.ts",
    );
    assert_eq!(stdout_of(&mut winterjs2().arg("--run").arg(&entry)), "42\n");
}

#[test]
fn circular_import_no_deadlock() {
    let (_dir, entry) = mod_dir(
        &[
            ("a.js", "import \"./b.js\";\nconsole.log(\"a\");\n"),
            ("b.js", "import \"./a.js\";\nconsole.log(\"b\");\n"),
        ],
        "a.js",
    );
    // spec 求值序：b 先于 a，不死锁
    assert_eq!(
        stdout_of(&mut winterjs2().arg("--run").arg(&entry)),
        "b\na\n"
    );
}

#[test]
fn import_meta_url() {
    let (_dir, entry) = mod_dir(&[("meta.js", "console.log(import.meta.url);\n")], "meta.js");
    let out = stdout_of(&mut winterjs2().arg("--run").arg(&entry));
    assert!(
        out.starts_with("file://") && out.trim_end().ends_with("/meta.js"),
        "meta url: {out}"
    );
}

#[test]
fn bare_specifier_missing_friendly_error() {
    let (_dir, entry) = mod_dir(
        &[("bare.js", "import \"left-pad-xyz-absent\";\n")],
        "bare.js",
    );
    let out = winterjs2().arg("--run").arg(&entry).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot resolve 'left-pad-xyz-absent'"),
        "stderr: {stderr}"
    );
}

#[test]
fn bare_specifier_node_modules() {
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("node_modules/left-pad/package.json")
        .write_str("{\"name\":\"left-pad\",\"version\":\"1.0.0\",\"main\":\"index.js\"}")
        .unwrap();
    dir.child("node_modules/left-pad/index.js")
        .write_str("export default \"pad!\";\n")
        .unwrap();
    dir.child("nm.js")
        .write_str("import pad from \"left-pad\";\nconsole.log(pad);\n")
        .unwrap();
    let entry = dir.child("nm.js").path().to_path_buf();
    assert_eq!(
        stdout_of(&mut winterjs2().arg("--run").arg(&entry)),
        "pad!\n"
    );
    dir.close().unwrap();
}

#[test]
fn tsconfig_paths_alias() {
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("tsconfig.json")
        .write_str("{\"compilerOptions\":{\"baseUrl\":\".\",\"paths\":{\"@lib/*\":[\"src/*\"]}}}")
        .unwrap();
    dir.child("src/add.ts")
        .write_str("export const add = (a: number, b: number): number => a + b;\n")
        .unwrap();
    dir.child("app.ts")
        .write_str("import { add } from \"@lib/add\";\nconsole.log(add(1, 2));\n")
        .unwrap();
    let entry = dir.child("app.ts").path().to_path_buf();
    assert_eq!(stdout_of(&mut winterjs2().arg("--run").arg(&entry)), "3\n");
    dir.close().unwrap();
}

#[test]
fn ts_js_extension_alias() {
    // TS 约定：`./foo.js` 指向 `./foo.ts` 源码
    let (_dir, entry) = mod_dir(
        &[
            ("foo.ts", "export const v: number = 7;\n"),
            (
                "app.ts",
                "import { v } from \"./foo.js\";\nconsole.log(v);\n",
            ),
        ],
        "app.ts",
    );
    assert_eq!(stdout_of(&mut winterjs2().arg("--run").arg(&entry)), "7\n");
}

#[test]
fn dynamic_import() {
    let (_dir, entry) = mod_dir(
        &[
            ("lib.js", "export const x = 40 + 2;\n"),
            (
                "dyn.js",
                "const m = await import(\"./lib.js\");\nconsole.log(m.x);\n",
            ),
        ],
        "dyn.js",
    );
    assert_eq!(stdout_of(&mut winterjs2().arg("--run").arg(&entry)), "42\n");
}

#[test]
fn top_level_await_entry() {
    let (_dir, entry) = mod_dir(
        &[(
            "tla.js",
            "await new Promise(r=>setTimeout(()=>r(7),5)).then(v=>console.log(\"tla\",v));\n",
        )],
        "tla.js",
    );
    assert_eq!(
        stdout_of(&mut winterjs2().arg("--run").arg(&entry)),
        "tla 7\n"
    );
}

#[test]
fn data_url_import() {
    let (_dir, entry) = mod_dir(
        &[(
            "data.js",
            "import x from \"data:text/javascript,export default 99\";\nconsole.log(x);\n",
        )],
        "data.js",
    );
    assert_eq!(stdout_of(&mut winterjs2().arg("--run").arg(&entry)), "99\n");
}

#[test]
fn ts_runtime_error_location() {
    // TS 报错行号经 sourcemap 回映射到原文（转译行会漂移，断言原文行）
    let (_dir, entry) = mod_dir(
        &[(
            "e.ts",
            "interface Big {\n  a: string;\n}\nconst o: Big = { a: \"x\" };\nconsole.log(o.a);\nboom_ts();\n",
        )],
        "e.ts",
    );
    let out = winterjs2().arg("--run").arg(&entry).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("e.ts:6:1"), "stderr: {stderr}");
    assert!(
        stderr.contains("boom_ts is not defined"),
        "stderr: {stderr}"
    );
}

// ── Phase 3a：URL / 编码 / crypto ──────────────────────────────────────────

#[test]
fn loader_http_import_end_to_end() {
    // 正常：绝对 http 导入 + 远端相对导入（URL join）；TF：同 URL 去重（模块单例）。
    let port = serve_http(3, move |head, _body| {
        let line = head.lines().next().unwrap_or("").to_owned();
        let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
        let body = if path == "/main.mjs" {
            "import { answer } from \"./lib.mjs\";\nexport const double = answer * 2;\n"
        } else if path == "/lib.mjs" {
            "export const answer = 42;\n"
        } else {
            return (404, vec![], b"nope".to_vec());
        };
        (
            200,
            vec![("content-type", "text/javascript".into())],
            body.as_bytes().to_vec(),
        )
    });
    let code = format!(
        "const m = await import(\"http://127.0.0.1:{port}/main.mjs\"); \
         const m2 = await import(\"http://127.0.0.1:{port}/main.mjs\"); \
         console.log(m.double, m === m2);"
    );
    assert_eq!(
        stdout_of(&mut winterjs2().args(["--eval", &code])),
        "84 true\n"
    );
}

#[test]
fn loader_http_errors() {
    // 报错：404 可读错（exit=1）；边界：超大/非 UTF-8 由单元口径覆盖，此处只钉 404。
    let port = serve_http(1, move |_head, _body| (404, vec![], b"nope".to_vec()));
    let code = format!("await import(\"http://127.0.0.1:{port}/missing.mjs\")");
    let out = winterjs2().args(["--eval", &code]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("404"), "stderr:\n{err}");
}

#[test]
fn run_entry_respects_package_json_type() {
    // 入口与 require() 同口径：`type: module` 包的 extensionless bin 走模块；
    // 无 type 即经典（Node 口径）；`type: commonjs` 的 ESM 语法自然报 SyntaxError。
    let dir = assert_fs::TempDir::new().unwrap();
    dir.child("package.json")
        .write_str(r#"{"type":"module"}"#)
        .unwrap();
    dir.child("lib.mjs")
        .write_str("export const x = 1;\n")
        .unwrap();
    dir.child("bin-noext")
        .write_str("import \"./lib.mjs\";\nconsole.log(\"esm-entry-ok\");\n")
        .unwrap();
    let out = winterjs2()
        .arg("--run")
        .arg(dir.path().join("bin-noext"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "esm-entry-ok\n");
    // 无 type：同内容走经典，import 即 SyntaxError（exit=1，可读）。
    let dir2 = assert_fs::TempDir::new().unwrap();
    dir2.child("bin-noext")
        .write_str("import \"./lib.mjs\";\n")
        .unwrap();
    let out = winterjs2()
        .arg("--run")
        .arg(dir2.path().join("bin-noext"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    dir.close().unwrap();
    dir2.close().unwrap();
}

#[test]
fn json_module_default_export() {
    // `import d from './d.json' with { type: 'json' }`：默认导出即 JSON 值
    //（对象 + 数组；whatwg encoding-singlebyte 面）。
    let (_dir, entry) = mod_dir(
        &[
            ("d.json", "{\"a\":1,\"b\":[true,null]}\n"),
            (
                "app.mjs",
                "import d from \"./d.json\" with { type: \"json\" };\nconsole.log(JSON.stringify(d));\n",
            ),
        ],
        "app.mjs",
    );
    assert_eq!(
        stdout_of(&mut winterjs2().arg("--run").arg(&entry)),
        "{\"a\":1,\"b\":[true,null]}\n"
    );
}

#[test]
fn json_module_invalid_rejected() {
    // 非法 JSON：入口失败 exit=1，文案点名 is not valid JSON。
    let (_dir, entry) = mod_dir(
        &[
            ("bad.json", "{\"a\":}\n"),
            (
                "app.mjs",
                "import d from \"./bad.json\" with { type: \"json\" };\nconsole.log(d);\n",
            ),
        ],
        "app.mjs",
    );
    let out = winterjs2().arg("--run").arg(&entry).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("is not valid JSON"),
        "stderr: {stderr}"
    );
}

#[test]
fn json_module_scalar_roundtrip() {
    // 边界：顶层标量 JSON（数字/字符串）同样走默认导出。
    let (_dir, entry) = mod_dir(
        &[
            ("n.json", "42\n"),
            (
                "app.mjs",
                "import n from \"./n.json\" with { type: \"json\" };\nconsole.log(typeof n, n);\n",
            ),
        ],
        "app.mjs",
    );
    assert_eq!(
        stdout_of(&mut winterjs2().arg("--run").arg(&entry)),
        "number 42\n"
    );
}

/// 搭一个临时模块目录：files 为 (name, content)，返回 dir（调用方持有）+ 入口路径。
fn mod_dir(files: &[(&str, &str)], entry: &str) -> (assert_fs::TempDir, std::path::PathBuf) {
    let dir = assert_fs::TempDir::new().unwrap();
    for (name, content) in files {
        dir.child(name).write_str(content).unwrap();
    }
    let path = dir.child(entry).path().to_path_buf();
    (dir, path)
}

#[test]
fn import_meta_filename_dirname() {
    // 正常：file: 模块 import.meta.filename 为解码路径、dirname 为其父目录；
    // 边界：data: 模块无此二键（undefined）；file: 嵌套子目录路径正确。
    let (_dir, entry) = mod_dir(
        &[
            (
                "app.mjs",
                "console.log(JSON.stringify([import.meta.filename, import.meta.dirname]));\n\
                 console.log(import.meta.filename.endsWith(\"/app.mjs\"));\n\
                 console.log(import.meta.dirname === import.meta.filename.slice(0, -\"/app.mjs\".length));\n\
                 import(\"data:text/javascript,console.log(JSON.stringify([import.meta.filename,import.meta.dirname]))\");\n",
            ),
        ],
        "app.mjs",
    );
    let out = stdout_of(&mut winterjs2().arg("--run").arg(&entry));
    let mut lines = out.lines();
    let first: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert!(first[0].as_str().unwrap().ends_with("/app.mjs"), "out: {out}");
    assert_eq!(first[1].as_str().unwrap().len() + "/app.mjs".len(), first[0].as_str().unwrap().len(), "out: {out}");
    assert!(lines.any(|l| l == "true"), "out: {out}");
    assert!(out.contains("[null,null]"), "out: {out}");
}
