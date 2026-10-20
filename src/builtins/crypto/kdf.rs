//! crypto KDF 面（HKDF/PBKDF2；§0.9 拆分：symm 纯搬移，调用方 `pub use` 原位）。

use super::common::*;
use mozjs::jsval::JSVal;
use crate::jsapi_glue::{report_error, value_to_string, view_bytes, wrap_cx, Frame};

/// HKDF 块（RFC 5869；hkdf 0.13）：`__wjs2_hkdf_derive(hash, ikmU8, saltU8, infoU8, length)` → Uint8Array。
macro_rules! hkdf_with {
    ($hash:ty, $ikm:expr, $salt:expr, $info:expr, $len:expr) => {{
        use hkdf::Hkdf;
        let hk = Hkdf::<$hash>::new(Some($salt), $ikm);
        let mut okm = vec![0u8; $len];
        hk.expand($info, &mut okm)
            .map_err(|e| format!("OperationError: HKDF expand failed: {e}"))?;
        Ok::<Vec<u8>, String>(okm)
    }};
}

fn hkdf_hash_len(hash: &str) -> Result<usize, String> {
    match hash {
        "SHA-1" => Ok(20),
        "SHA-256" | "SHA3-256" => Ok(32),
        "SHA-384" | "SHA3-384" => Ok(48),
        "SHA-512" | "SHA3-512" => Ok(64),
        other => Err(format!("NotSupportedError: unsupported HKDF hash '{other}'")),
    }
}

/// 手工 HKDF-SHA3（RFC 5869；sha3 0.12 与 hkdf 的 digest 0.10 不互通）。
macro_rules! hkdf_sha3_with {
    ($hash:ty, $block:expr, $outlen:expr, $hashlen:expr, $ikm:expr, $salt:expr, $info:expr, $len:expr) => {{
        let salt_norm: Vec<u8> = if $salt.is_empty() {
            vec![0u8; $hashlen]
        } else {
            $salt.to_vec()
        };
        let prk = hmac_sha3_with!($hash, $block, $outlen, &salt_norm, $ikm);
        let mut okm = Vec::new();
        if $len > 0 {
            let n = $len.div_ceil($hashlen);
            if n > 255 {
                return Err(format!("OperationError: HKDF expand failed: length too large"));
            }
            let mut t = Vec::new();
            for i in 1..=n {
                let mut input = t.clone();
                input.extend_from_slice($info);
                input.push(i as u8);
                t = hmac_sha3_with!($hash, $block, $outlen, &prk, &input);
                okm.extend_from_slice(&t);
            }
            okm.truncate($len);
        }
        Ok::<Vec<u8>, String>(okm)
    }};
}

/// 手工 PBKDF2-SHA3（RFC 8018；sha3 0.12 与 pbkdf2 的 digest 0.10 不互通）。
macro_rules! pbkdf2_sha3_with {
    ($hash:ty, $block:expr, $outlen:expr, $hashlen:expr, $pass:expr, $salt:expr, $rounds:expr, $len:expr) => {{
        if $rounds == 0 {
            return Err(format!("OperationError: PBKDF2 needs at least 1 iteration"));
        }
        let mut out = Vec::new();
        if $len > 0 {
            let blocks = $len.div_ceil($hashlen);
            for b in 1..=blocks {
                let mut u_input = Vec::with_capacity($salt.len() + 4);
                u_input.extend_from_slice($salt);
                u_input.extend_from_slice(&(b as u32).to_be_bytes());
                let mut u = hmac_sha3_with!($hash, $block, $outlen, $pass, &u_input);
                let mut f = u.clone();
                for _ in 1..$rounds {
                    u = hmac_sha3_with!($hash, $block, $outlen, $pass, &u);
                    for (x, y) in f.iter_mut().zip(u.iter()) {
                        *x ^= *y;
                    }
                }
                out.extend_from_slice(&f);
            }
            out.truncate($len);
        }
        Ok::<Vec<u8>, String>(out)
    }};
}

fn hkdf_bytes(hash: &str, ikm: &[u8], salt: &[u8], info: &[u8], len: usize) -> Result<Vec<u8>, String> {
    match hash {
        "SHA-1" => hkdf_with!(sha1::Sha1, ikm, salt, info, len),
        "SHA-256" => hkdf_with!(sha2::Sha256, ikm, salt, info, len),
        "SHA-384" => hkdf_with!(sha2::Sha384, ikm, salt, info, len),
        "SHA-512" => hkdf_with!(sha2::Sha512, ikm, salt, info, len),
        "SHA3-256" => hkdf_sha3_with!(sha3::Sha3_256, 136, 32, 32, ikm, salt, info, len),
        "SHA3-384" => hkdf_sha3_with!(sha3::Sha3_384, 104, 48, 48, ikm, salt, info, len),
        "SHA3-512" => hkdf_sha3_with!(sha3::Sha3_512, 72, 64, 64, ikm, salt, info, len),
        other => Err(format!("NotSupportedError: unsupported HKDF hash '{other}'")),
    }
}

/// `__wjs2_hkdf_derive(hash, ikmU8, saltU8, infoU8, length)` → Uint8Array。
pub unsafe extern "C" fn hkdf_derive(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 5 {
        report_error(&mut cx, "TypeError: HKDF derive needs hash, ikm, salt, info and length");
        return false;
    }
    let hash = value_to_string(&mut cx, frame.arg(0));
    let (Some(ikm), Some(salt), Some(info)) = (
        view_bytes(&mut cx, frame.arg(1), "HKDF ikm"),
        view_bytes(&mut cx, frame.arg(2), "HKDF salt"),
        view_bytes(&mut cx, frame.arg(3), "HKDF info"),
    ) else {
        return false;
    };
    let len = frame.arg(4).to_number() as usize;
    // RFC 5869：空 salt 按 HashLen 个 0 处理。
    let salt_norm = match hkdf_hash_len(&hash) {
        Ok(hash_len) if salt.is_empty() => vec![0u8; hash_len],
        Ok(_) => salt,
        Err(e) => {
            report_error(&mut cx, &e);
            return false;
        }
    };
    match hkdf_bytes(&hash, &ikm, &salt_norm, &info, len) {
        Ok(out) => set_rval_bytes(&mut cx, &frame, &out),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

/// PBKDF2 块（RFC 8018；pbkdf2 0.13）：`__wjs2_pbkdf2_derive(hash, passU8, saltU8, iterations, length)` → Uint8Array。
macro_rules! pbkdf2_with {
    ($hash:ty, $pass:expr, $salt:expr, $rounds:expr, $len:expr) => {{
        let mut out = vec![0u8; $len];
        pbkdf2::pbkdf2_hmac::<$hash>($pass, $salt, $rounds, &mut out);
        Ok::<Vec<u8>, String>(out)
    }};
}

fn pbkdf2_bytes(hash: &str, pass: &[u8], salt: &[u8], rounds: u32, len: usize) -> Result<Vec<u8>, String> {
    match hash {
        "SHA-1" => pbkdf2_with!(sha1::Sha1, pass, salt, rounds, len),
        "SHA-256" => pbkdf2_with!(sha2::Sha256, pass, salt, rounds, len),
        "SHA-384" => pbkdf2_with!(sha2::Sha384, pass, salt, rounds, len),
        "SHA-512" => pbkdf2_with!(sha2::Sha512, pass, salt, rounds, len),
        "SHA3-256" => pbkdf2_sha3_with!(sha3::Sha3_256, 136, 32, 32, pass, salt, rounds, len),
        "SHA3-384" => pbkdf2_sha3_with!(sha3::Sha3_384, 104, 48, 48, pass, salt, rounds, len),
        "SHA3-512" => pbkdf2_sha3_with!(sha3::Sha3_512, 72, 64, 64, pass, salt, rounds, len),
        other => Err(format!("NotSupportedError: unsupported PBKDF2 hash '{other}'")),
    }
}

/// `__wjs2_pbkdf2_derive(hash, passU8, saltU8, iterations, length)` → Uint8Array。
pub unsafe extern "C" fn pbkdf2_derive(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 5 {
        report_error(&mut cx, "TypeError: PBKDF2 derive needs hash, password, salt, iterations and length");
        return false;
    }
    let hash = value_to_string(&mut cx, frame.arg(0));
    let (Some(pass), Some(salt)) = (
        view_bytes(&mut cx, frame.arg(1), "PBKDF2 password"),
        view_bytes(&mut cx, frame.arg(2), "PBKDF2 salt"),
    ) else {
        return false;
    };
    let rounds = frame.arg(3).to_number() as u32;
    let len = frame.arg(4).to_number() as usize;
    match pbkdf2_bytes(&hash, &pass, &salt, rounds, len) {
        Ok(out) => set_rval_bytes(&mut cx, &frame, &out),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}
