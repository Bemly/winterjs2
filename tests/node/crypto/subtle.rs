//! tests/node/crypto/subtle.rs — WebCrypto subtle 面（HKDF/PBKDF2/对称/KDF 校验）。
//!
//! 对齐 `src/builtins/prelude/crypto*.rs` + `src/builtins/crypto/{symm,kdf}.rs`。

use crate::common::*;
use crate::helpers::*;
use assert_fs::prelude::*;

#[test]
fn subtle_kdf_faces() {
    // HKDF/PBKDF2：RFC 向量（正常）+ 长度/轮数校验（报错）+ 空盐空长（边界）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "k.mjs",
        r#"
const hex = (s) => new Uint8Array(Buffer.from(s, "hex"));
const toHex = (u8) => [...new Uint8Array(u8)].map((b) => b.toString(16).padStart(2, "0")).join("");
const t = async (f) => { try { await f(); return "ok"; } catch (e) { return (e.name ?? "noname") + "|" + (e.code ?? "nocode"); } };
// 正常：RFC5869 #1（336bit）+ PBKDF2-SHA256（256bit）。
const ikm = await crypto.subtle.importKey("raw", hex("0b".repeat(22)), "HKDF", false, ["deriveBits"]);
const bits = await crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", salt: hex("000102030405060708090a0b0c"), info: hex("f0f1f2f3f4f5f6f7f8f9") }, ikm, 336);
console.log("hkdf-vec", toHex(bits) === "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865");
const pw = await crypto.subtle.importKey("raw", new TextEncoder().encode("password"), "PBKDF2", false, ["deriveBits"]);
const pb = await crypto.subtle.deriveBits({ name: "PBKDF2", hash: "SHA-256", salt: new TextEncoder().encode("salt"), iterations: 1 }, pw, 256);
console.log("pbkdf2-vec", toHex(pb) === "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b");
// 报错：非 8 倍数/0 轮/超长/缺参。
console.log("badlen", await t(() => crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", salt: new Uint8Array(1), info: new Uint8Array(1) }, ikm, 7)));
console.log("nullen", await t(() => crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", salt: new Uint8Array(1), info: new Uint8Array(1) }, ikm, null)));
console.log("iter0", await t(() => crypto.subtle.deriveBits({ name: "PBKDF2", hash: "SHA-256", salt: new Uint8Array(1), iterations: 0 }, pw, 256)));
console.log("nosalt", await t(() => crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", info: new Uint8Array(1) }, ikm, 256)));
console.log("badhash", await t(() => crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA384", salt: new Uint8Array(1), info: new Uint8Array(1) }, ikm, 256)));
// 边界：空盐空 info 零长。
const ikm0 = await crypto.subtle.importKey("raw", new Uint8Array(0), "HKDF", false, ["deriveBits"]);
const b0 = await crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", salt: new Uint8Array(0), info: new Uint8Array(0) }, ikm0, 0);
console.log("empty", b0.byteLength === 0);
"#,
    );
    assert!(out.contains("hkdf-vec true"), "out: {out}");
    assert!(out.contains("pbkdf2-vec true"), "out: {out}");
    assert!(out.contains("badlen OperationError|"), "out: {out}");
    assert!(out.contains("nullen OperationError|"), "out: {out}");
    assert!(out.contains("iter0 OperationError|"), "out: {out}");
    assert!(out.contains("nosalt TypeError|"), "out: {out}");
    assert!(out.contains("badhash NotSupportedError|"), "out: {out}");
    assert!(out.contains("empty true"), "out: {out}");
    dir.close().unwrap();
}

#[test]
fn subtle_symmetric_faces() {
    // AES-CBC/CTR/KW + ChaCha：往返（正常）+ iv/长度校验（报错）+ 空数据（边界）。
    let dir = assert_fs::TempDir::new().unwrap();
    let out = run_fs_file(
        &dir,
        "s.mjs",
        r#"
const t = async (f) => { try { await f(); return "ok"; } catch (e) { return (e.name ?? "noname") + "|" + (e.code ?? "nocode"); } };
const te = new TextEncoder(), td = new TextDecoder();
// 正常：CBC/CTR/KW/ChaCha 往返。
const cbc = await crypto.subtle.importKey("raw", new Uint8Array(16).fill(7), "AES-CBC", true, ["encrypt", "decrypt"]);
const iv16 = new Uint8Array(16).fill(3);
const cct = await crypto.subtle.encrypt({ name: "AES-CBC", iv: iv16 }, cbc, te.encode("hello world test!"));
console.log("cbc", td.decode(await crypto.subtle.decrypt({ name: "AES-CBC", iv: iv16 }, cbc, cct)) === "hello world test!");
const ctr = await crypto.subtle.importKey("raw", new Uint8Array(32).fill(9), "AES-CTR", true, ["encrypt", "decrypt"]);
const ctr16 = new Uint8Array(16).fill(1);
const tct = await crypto.subtle.encrypt({ name: "AES-CTR", counter: ctr16, length: 64 }, ctr, te.encode("ctr data 123456"));
console.log("ctr", td.decode(await crypto.subtle.decrypt({ name: "AES-CTR", counter: ctr16, length: 64 }, ctr, tct)) === "ctr data 123456");
const ch = await crypto.subtle.generateKey({ name: "ChaCha20-Poly1305" }, true, ["encrypt", "decrypt"]);
const iv12 = new Uint8Array(12).fill(2);
const hct = await crypto.subtle.encrypt({ name: "ChaCha20-Poly1305", iv: iv12 }, ch, te.encode("chacha!"));
console.log("chacha", td.decode(await crypto.subtle.decrypt({ name: "ChaCha20-Poly1305", iv: iv12 }, ch, hct)) === "chacha!");
console.log("chacha-name", ch.algorithm.name === "ChaCha20-Poly1305");
// 报错：iv 长/标签长/密钥长。
console.log("cbiv", await t(() => crypto.subtle.encrypt({ name: "AES-CBC", iv: new Uint8Array(8) }, cbc, te.encode("x"))));
console.log("ctag", await t(() => crypto.subtle.encrypt({ name: "ChaCha20-Poly1305", iv: iv12, tagLength: 64 }, ch, te.encode("x"))));
console.log("chkey", await t(() => crypto.subtle.importKey("raw", new Uint8Array(16), "ChaCha20-Poly1305", true, ["encrypt"])));
// 边界：空明文（CBC 补一块）+ 错误 tag 解密失败。
const e0 = await crypto.subtle.encrypt({ name: "AES-CBC", iv: iv16 }, cbc, new Uint8Array(0));
console.log("cbc-empty", new Uint8Array(e0).length === 16);
const bad = new Uint8Array(hct); bad[0] ^= 1;
console.log("chacha-badtag", await t(() => crypto.subtle.decrypt({ name: "ChaCha20-Poly1305", iv: iv12 }, ch, bad)));
"#,
    );
    assert!(out.contains("cbc true"), "out: {out}");
    assert!(out.contains("ctr true"), "out: {out}");
    assert!(out.contains("chacha true"), "out: {out}");
    assert!(out.contains("chacha-name true"), "out: {out}");
    assert!(out.contains("cbiv OperationError|"), "out: {out}");
    assert!(out.contains("ctag OperationError|"), "out: {out}");
    assert!(out.contains("chkey DataError|"), "out: {out}");
    assert!(out.contains("cbc-empty true"), "out: {out}");
    assert!(out.contains("chacha-badtag OperationError|"), "out: {out}");
    dir.close().unwrap();
}
