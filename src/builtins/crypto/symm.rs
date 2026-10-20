//! crypto 对称面（AES-GCM/HMAC/随机数/Subtle digest）。

use super::common::*;
use mozjs::conversions::ToJSValConvertible as _;

use mozjs::jsapi::JSObject;
use mozjs::jsval::{JSVal, UndefinedValue};
use mozjs::rooted;
use mozjs::typedarray::{CreateWith, TypedArray, Uint8};
use crate::jsapi_glue::{report_error, value_to_string, view_bytes, wrap_cx, Frame};

pub unsafe extern "C" fn aesgcm_encrypt(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 {
        report_error(&mut cx, "TypeError: AES-GCM encrypt needs key, iv, aad and data");
        return false;
    }
    let (k, iv, aad, plain) = (frame.arg(0), frame.arg(1), frame.arg(2), frame.arg(3));
    let (Some(key), Some(iv), Some(aad), Some(plain)) = (
        view_bytes(&mut cx, k, "AES-GCM key"),
        view_bytes(&mut cx, iv, "AES-GCM iv"),
        opt_view_bytes(&mut cx, aad, "AES-GCM aad"),
        view_bytes(&mut cx, plain, "AES-GCM data"),
    ) else {
        return false;
    };
    if iv.len() != 12 {
        report_error(&mut cx, "OperationError: AES-GCM iv must be 12 bytes");
        return false;
    }
    let aad_ref = aad.as_deref().unwrap_or(&[]);
    match gcm_encrypt_raw(&key, &iv, aad_ref, &plain) {
        Ok(ct) => set_rval_bytes(&mut cx, &frame, &ct),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

fn encrypt_with<A>(key: &[u8], iv: &[u8], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, String>
where
    A: aes_gcm::aead::KeyInit + aes_gcm::aead::Aead,
{
    use aes_gcm::aead::Payload;
    let cipher = A::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce =
        aes_gcm::aead::Nonce::<A>::try_from(iv).map_err(|_| "bad nonce".to_string())?;
    cipher
        .encrypt(&nonce, Payload { msg: plain, aad })
        .map_err(|e| e.to_string())
}

/// `__wjs2_aesgcm_decrypt(keyU8, ivU8, aadU8?, dataU8)` → Uint8Array（认证失败抛 OperationError）。
pub unsafe extern "C" fn aesgcm_decrypt(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 {
        report_error(&mut cx, "TypeError: AES-GCM decrypt needs key, iv, aad and data");
        return false;
    }
    let (k, iv, aad, data) = (frame.arg(0), frame.arg(1), frame.arg(2), frame.arg(3));
    let (Some(key), Some(iv), Some(aad), Some(data)) = (
        view_bytes(&mut cx, k, "AES-GCM key"),
        view_bytes(&mut cx, iv, "AES-GCM iv"),
        opt_view_bytes(&mut cx, aad, "AES-GCM aad"),
        view_bytes(&mut cx, data, "AES-GCM data"),
    ) else {
        return false;
    };
    if iv.len() != 12 {
        report_error(&mut cx, "OperationError: AES-GCM iv must be 12 bytes");
        return false;
    }
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        report_error(&mut cx, "OperationError: AES-GCM key must be 16, 24 or 32 bytes");
        return false;
    }
    let aad_ref = aad.as_deref().unwrap_or(&[]);
    match gcm_decrypt_raw(&key, &iv, aad_ref, &data) {
        Ok(pt) => set_rval_bytes(&mut cx, &frame, &pt),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

fn decrypt_with<A>(key: &[u8], iv: &[u8], aad: &[u8], data: &[u8]) -> Result<Vec<u8>, String>
where
    A: aes_gcm::aead::KeyInit + aes_gcm::aead::Aead,
{
    use aes_gcm::aead::Payload;
    let cipher = A::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce =
        aes_gcm::aead::Nonce::<A>::try_from(iv).map_err(|_| "bad nonce".to_string())?;
    cipher
        .decrypt(&nonce, Payload { msg: data, aad })
        .map_err(|e| e.to_string())
}

/// AES-GCM 12B 裸加密（`__wjs2_aesgcm_encrypt` 与 node 侧 `gcm_anyiv` 共用；
/// 错误文案维持 `__wjs2_aesgcm_*` 口径，调用方按需包装）。
pub(crate) fn gcm_encrypt_raw(
    key: &[u8],
    iv: &[u8],
    aad: &[u8],
    plain: &[u8],
) -> Result<Vec<u8>, String> {
    type Aes192Gcm = aes_gcm::AesGcm<aes::Aes192, aes_gcm::aead::consts::U12>;
    if key.len() == 16 {
        encrypt_with::<aes_gcm::Aes128Gcm>(key, iv, aad, plain)
    } else if key.len() == 24 {
        encrypt_with::<Aes192Gcm>(key, iv, aad, plain)
    } else if key.len() == 32 {
        encrypt_with::<aes_gcm::Aes256Gcm>(key, iv, aad, plain)
    } else {
        Err("OperationError: AES-GCM key must be 16, 24 or 32 bytes".to_string())
    }
    .map_err(|e| {
        if e.starts_with("OperationError") {
            e
        } else {
            format!("OperationError: AES-GCM encrypt failed: {e}")
        }
    })
}

/// AES-GCM 12B 裸解密（共用；失败文案与 `__wjs2_aesgcm_decrypt` 一致）。
pub(crate) fn gcm_decrypt_raw(
    key: &[u8],
    iv: &[u8],
    aad: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, String> {
    type Aes192Gcm = aes_gcm::AesGcm<aes::Aes192, aes_gcm::aead::consts::U12>;
    if key.len() == 16 {
        decrypt_with::<aes_gcm::Aes128Gcm>(key, iv, aad, data)
    } else if key.len() == 24 {
        decrypt_with::<Aes192Gcm>(key, iv, aad, data)
    } else if key.len() == 32 {
        decrypt_with::<aes_gcm::Aes256Gcm>(key, iv, aad, data)
    } else {
        Err("bad key".to_string())
    }
    .map_err(|_| "OperationError: AES-GCM decrypt failed (bad key/iv/tag?)".to_string())
}

macro_rules! hmac_with {
    ($hash:ty, $key:expr, $data:expr) => {{
        use hmac::{Hmac, Mac as _, digest::KeyInit as _};
        let mut mac =
            Hmac::<$hash>::new_from_slice($key).map_err(|e| format!("OperationError: {e}"))?;
        mac.update($data);
        mac.finalize().into_bytes().to_vec()
    }};
}


fn hmac_bytes(hash: &str, key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    match hash {
        "SHA-1" => Ok(hmac_with!(sha1::Sha1, key, data)),
        "SHA-256" => Ok(hmac_with!(sha2::Sha256, key, data)),
        "SHA-384" => Ok(hmac_with!(sha2::Sha384, key, data)),
        "SHA-512" => Ok(hmac_with!(sha2::Sha512, key, data)),
        "SHA3-256" => Ok(hmac_sha3_with!(sha3::Sha3_256, 136, 32, key, data)),
        "SHA3-384" => Ok(hmac_sha3_with!(sha3::Sha3_384, 104, 48, key, data)),
        "SHA3-512" => Ok(hmac_sha3_with!(sha3::Sha3_512, 72, 64, key, data)),
        other => Err(format!("NotSupportedError: unsupported HMAC hash '{other}'")),
    }
}

/// `__wjs2_hmac_sign(hash, keyU8, dataU8)` → Uint8Array。
pub unsafe extern "C" fn hmac_sign(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: HMAC sign needs hash, key and data");
        return false;
    }
    let hash = value_to_string(&mut cx, frame.arg(0));
    let (Some(key), Some(data)) = (
        view_bytes(&mut cx, frame.arg(1), "HMAC key"),
        view_bytes(&mut cx, frame.arg(2), "HMAC data"),
    ) else {
        return false;
    };
    match hmac_bytes(&hash, &key, &data) {
        Ok(tag) => set_rval_bytes(&mut cx, &frame, &tag),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

/// `__wjs2_hmac_verify(hash, keyU8, sigU8, dataU8)` → boolean。
pub unsafe extern "C" fn hmac_verify(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 {
        report_error(&mut cx, "TypeError: HMAC verify needs hash, key, signature and data");
        return false;
    }
    let hash = value_to_string(&mut cx, frame.arg(0));
    let (Some(key), Some(sig), Some(data)) = (
        view_bytes(&mut cx, frame.arg(1), "HMAC key"),
        view_bytes(&mut cx, frame.arg(2), "HMAC signature"),
        view_bytes(&mut cx, frame.arg(3), "HMAC data"),
    ) else {
        return false;
    };
    match hmac_bytes(&hash, &key, &data) {
        Ok(tag) => {
            // 定长比较（长度先行，内容恒时由 Vec 比较短路程度可忽略的侧信道面）
            let ok = tag.len() == sig.len()
                && tag.iter().zip(sig.iter()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0;
            frame.set_rval(mozjs::jsval::BooleanValue(ok));
            true
        }
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

/// 上限 64KiB（规范 QuotaExceededError）。
const MAX_BYTES: usize = 65536;

macro_rules! try_fill {
    ($cx:expr, $obj:expr, $Marker:ty, $Elem:ty, $SIZE:expr) => {{
        if let Ok(mut arr) = TypedArray::<$Marker, *mut JSObject>::from($obj) {
            if arr.is_shared() {
                report_error($cx, "TypeError: getRandomValues does not accept SharedArrayBuffer views yet");
                return false;
            }
            let detached = arr.as_slice_safe($cx.no_gc()).is_none();
            if detached {
                report_error($cx, "TypeError: getRandomValues view is detached");
                return false;
            }
            let n: usize = arr.len();
            let bytes = n.saturating_mul($SIZE);
            if bytes > MAX_BYTES {
                report_error($cx, "QuotaExceededError: getRandomValues quota is 64KiB");
                return false;
            }
            let mut raw = vec![0u8; bytes];
            if let Err(e) = getrandom::fill(&mut raw) {
                report_error($cx, &format!("OperationError: cannot get random values: {e}"));
                return false;
            }
            if let Some(slot) = arr.as_mut_slice_safe($cx.no_gc_mut()) {
                for (i, cell) in slot.iter_mut().enumerate() {
                    let chunk: [u8; $SIZE] = raw[i * $SIZE..i * $SIZE + $SIZE]
                        .try_into()
                        .unwrap_or([0u8; $SIZE]);
                    *cell = <$Elem>::from_ne_bytes(chunk);
                }
            }
            tracing::trace!(target: "winterjs2::crypto", bytes, "getRandomValues filled");
            return true;
        }
    }};
}

/// `__wjs2_fill_random(view)`：就地填充（prelude 原样返回 view）。
pub unsafe extern "C" fn fill_random(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 引擎回调提供的 raw cx 有效；文档许可由此构造 wrapper
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 {
        report_error(&mut cx, "TypeError: getRandomValues requires an argument");
        return false;
    }
    let v = frame.arg(0);
    if !v.is_object() {
        report_error(&mut cx, "TypeError: getRandomValues requires a typed array view");
        return false;
    }
    // SAFETY: is_object 已判定（to_object 为 safe API，见 §6 审计）
    let obj = v.to_object();
    // 浮点视图先判（必须抛，不可按整数填充）
    if TypedArray::<mozjs::typedarray::Float32, *mut JSObject>::from(obj).is_ok()
        || TypedArray::<mozjs::typedarray::Float64, *mut JSObject>::from(obj).is_ok()
    {
        report_error(&mut cx, "TypeError: getRandomValues requires an integer typed array");
        return false;
    }
    try_fill!(&mut cx, obj, mozjs::typedarray::Uint8, u8, 1);
    try_fill!(&mut cx, obj, mozjs::typedarray::Int8, i8, 1);
    try_fill!(&mut cx, obj, mozjs::typedarray::ClampedU8, u8, 1);
    try_fill!(&mut cx, obj, mozjs::typedarray::Uint16, u16, 2);
    try_fill!(&mut cx, obj, mozjs::typedarray::Int16, i16, 2);
    try_fill!(&mut cx, obj, mozjs::typedarray::Uint32, u32, 4);
    try_fill!(&mut cx, obj, mozjs::typedarray::Int32, i32, 4);
    let got = value_to_string(&mut cx, v);
    report_error(
        &mut cx,
        &format!("TypeError: getRandomValues needs an integer TypedArray, got {got}"),
    );
    false
}

/// `__wjs2_random_uuid()` → v4 字符串。
pub unsafe extern "C" fn random_uuid(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    let s = uuid::Uuid::new_v4().hyphenated().to_string();
    rooted!(&in(cx) let mut v = UndefinedValue());
    s.to_jsval(&mut cx, v.handle_mut());
    frame.set_rval(v.get());
    true
}

/// `__wjs2_subtle_digest(alg, view)` → Uint8Array（SHA-1/256/384/512；`sha1`/`sha2` 轮子）。
/// prelude 包一层 async 即得规范的 Promise 返回（计算本身同步，无需事件循环改动）。
pub unsafe extern "C" fn subtle_digest(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: digest requires an algorithm and data");
        return false;
    }
    let alg = value_to_string(&mut cx, frame.arg(0));
    let data = frame.arg(1);
    let bytes = match view_bytes(&mut cx, data, "digest data") {
        Some(b) => b,
        None => return false,
    };
    let out: Vec<u8> = match alg.trim().to_ascii_lowercase().as_str() {
        "sha-1" => {
            use sha1::Digest as _;
            sha1::Sha1::digest(&bytes).to_vec()
        }
        "sha-256" => {
            use sha2::Digest as _;
            sha2::Sha256::digest(&bytes).to_vec()
        }
        "sha-384" => {
            use sha2::Digest as _;
            sha2::Sha384::digest(&bytes).to_vec()
        }
        "sha-512" => {
            use sha2::Digest as _;
            sha2::Sha512::digest(&bytes).to_vec()
        }
        "sha3-256" => {
            use sha3::Digest as _;
            sha3::Sha3_256::digest(&bytes).to_vec()
        }
        "sha3-384" => {
            use sha3::Digest as _;
            sha3::Sha3_384::digest(&bytes).to_vec()
        }
        "sha3-512" => {
            use sha3::Digest as _;
            sha3::Sha3_512::digest(&bytes).to_vec()
        }
        other => {
            report_error(&mut cx, &format!("NotSupportedError: unsupported digest algorithm '{other}'"));
            return false;
        }
    };
    rooted!(&in(cx) let mut obj: *mut JSObject = std::ptr::null_mut());
    // SAFETY: realm 内创建；obj 为 rooted 出参；out 存活到调用返回（§6 审计：边界调用）
    let ok = unsafe {
        TypedArray::<Uint8, *mut JSObject>::create(&mut cx, CreateWith::Slice(&out), obj.handle_mut())
    };
    if ok.is_err() || obj.is_null() {
        report_error(&mut cx, "RangeError: cannot allocate digest output");
        return false;
    }
    frame.set_rval(mozjs::jsval::ObjectValue(obj.get()));
    true
}


/// SHAKE 一次性（tiny-keccak；cSHAKE 无定制即 SHAKE）：`__wjs2_shake_digest(which, outLen, dataU8)` → Uint8Array。
/// which: 128/256；outLen 为字节数。
pub unsafe extern "C" fn shake_digest(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: SHAKE digest needs which, length and data");
        return false;
    }
    let which = value_to_string(&mut cx, frame.arg(0));
    let out_len = frame.arg(1).to_number() as usize;
    let Some(data) = view_bytes(&mut cx, frame.arg(2), "SHAKE data") else {
        return false;
    };
    use tiny_keccak::Hasher as _;
    let mut out = vec![0u8; out_len];
    match which.as_str() {
        "128" => {
            let mut h = tiny_keccak::Shake::v128();
            h.update(&data);
            h.finalize(&mut out);
        }
        "256" => {
            let mut h = tiny_keccak::Shake::v256();
            h.update(&data);
            h.finalize(&mut out);
        }
        other => {
            report_error(&mut cx, &format!("NotSupportedError: unsupported SHAKE '{other}'"));
            return false;
        }
    }
    set_rval_bytes(&mut cx, &frame, &out)
}

/// AES-CBC 块（PKCS#7；cbc 0.2 + aes 转出的 cipher traits，零新增依赖）：
/// `__wjs2_aescbc_encrypt(keyU8, ivU8, dataU8)` / `__wjs2_aescbc_decrypt(keyU8, ivU8, dataU8)` → Uint8Array。
macro_rules! cbc_with {
    ($aes:ty, $key:expr, $iv:expr, $data:expr, encrypt) => {{
        let enc = cbc::Encryptor::<$aes>::new_from_slices($key, $iv)
            .map_err(|e| format!("OperationError: bad AES-CBC key/iv: {e}"))?;
        let mut buf = vec![0u8; $data.len() + 16];
        let n = enc
            .encrypt_padded_b2b::<Pkcs7>($data, &mut buf)
            .map_err(|e| format!("OperationError: AES-CBC encrypt failed: {e}"))?
            .len();
        buf.truncate(n);
        Ok::<Vec<u8>, String>(buf)
    }};
    ($aes:ty, $key:expr, $iv:expr, $data:expr, decrypt) => {{
        let dec = cbc::Decryptor::<$aes>::new_from_slices($key, $iv)
            .map_err(|e| format!("OperationError: bad AES-CBC key/iv: {e}"))?;
        let mut buf = vec![0u8; $data.len()];
        let n = dec
            .decrypt_padded_b2b::<Pkcs7>($data, &mut buf)
            .map_err(|_| "OperationError: AES-CBC decrypt failed (bad key/iv/padding?)".to_string())?
            .len();
        buf.truncate(n);
        Ok::<Vec<u8>, String>(buf)
    }};
}

fn cbc_crypt(key: &[u8], iv: &[u8], data: &[u8], decrypt: bool) -> Result<Vec<u8>, String> {
    use aes::cipher::{block_padding::Pkcs7, BlockModeDecrypt as _, BlockModeEncrypt as _, KeyIvInit as _};
    if iv.len() != 16 {
        return Err("OperationError: AES-CBC iv must be 16 bytes".to_string());
    }
    match key.len() {
        16 => {
            if decrypt {
                cbc_with!(aes::Aes128, key, iv, data, decrypt)
            } else {
                cbc_with!(aes::Aes128, key, iv, data, encrypt)
            }
        }
        24 => {
            if decrypt {
                cbc_with!(aes::Aes192, key, iv, data, decrypt)
            } else {
                cbc_with!(aes::Aes192, key, iv, data, encrypt)
            }
        }
        32 => {
            if decrypt {
                cbc_with!(aes::Aes256, key, iv, data, decrypt)
            } else {
                cbc_with!(aes::Aes256, key, iv, data, encrypt)
            }
        }
        _ => Err("OperationError: AES-CBC key must be 16/24/32 bytes".to_string()),
    }
}

/// `__wjs2_aescbc_encrypt(keyU8, ivU8, dataU8)` → Uint8Array。
pub unsafe extern "C" fn aescbc_encrypt(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: AES-CBC encrypt needs key, iv and data");
        return false;
    }
    let (Some(key), Some(iv), Some(data)) = (
        view_bytes(&mut cx, frame.arg(0), "AES-CBC key"),
        view_bytes(&mut cx, frame.arg(1), "AES-CBC iv"),
        view_bytes(&mut cx, frame.arg(2), "AES-CBC data"),
    ) else {
        return false;
    };
    match cbc_crypt(&key, &iv, &data, false) {
        Ok(ct) => set_rval_bytes(&mut cx, &frame, &ct),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

/// `__wjs2_aescbc_decrypt(keyU8, ivU8, dataU8)` → Uint8Array（填充错抛 OperationError）。
pub unsafe extern "C" fn aescbc_decrypt(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: AES-CBC decrypt needs key, iv and data");
        return false;
    }
    let (Some(key), Some(iv), Some(data)) = (
        view_bytes(&mut cx, frame.arg(0), "AES-CBC key"),
        view_bytes(&mut cx, frame.arg(1), "AES-CBC iv"),
        view_bytes(&mut cx, frame.arg(2), "AES-CBC data"),
    ) else {
        return false;
    };
    match cbc_crypt(&key, &iv, &data, true) {
        Ok(pt) => set_rval_bytes(&mut cx, &frame, &pt),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

/// AES-CTR 手工流（counter 低 length 位递增回绕；length 须 8 整除，JS 侧校验）：
/// `__wjs2_aesctr_crypt(keyU8, counterU8, lengthBits, dataU8)` → Uint8Array（加解密同构）。
macro_rules! ctr_with {
    ($aes:ty, $key:expr, $counter:expr, $ctr_bytes:expr, $data:expr) => {{
        use aes::cipher::block::BlockCipherEncrypt as _;
        use aes::cipher::{Block, KeyInit as _};
        let cipher = <$aes>::new_from_slice($key)
            .map_err(|e| format!("OperationError: bad AES-CTR key: {e}"))?;
        let prefix = 16 - $ctr_bytes;
        let mut ctr_val: u128 = {
            let mut b = [0u8; 16];
            b[16 - $ctr_bytes..].copy_from_slice(&$counter[prefix..]);
            u128::from_be_bytes(b)
        };
        let mask: u128 = if $ctr_bytes >= 16 {
            u128::MAX
        } else {
            (1u128 << ($ctr_bytes * 8)) - 1
        };
        let mut out = Vec::with_capacity($data.len());
        for chunk in $data.chunks(16) {
            let mut raw = [0u8; 16];
            raw[..prefix].copy_from_slice(&$counter[..prefix]);
            raw[prefix..].copy_from_slice(&(ctr_val & mask).to_be_bytes()[16 - $ctr_bytes..]);
            let mut block = Block::<$aes>::from(raw);
            cipher.encrypt_block(&mut block);
            let ks = block.as_slice();
            for (i, &p) in chunk.iter().enumerate() {
                out.push(p ^ ks[i]);
            }
            ctr_val = ctr_val.wrapping_add(1) & mask;
        }
        Ok::<Vec<u8>, String>(out)
    }};
}

fn ctr_crypt(key: &[u8], counter: &[u8], len_bits: u32, data: &[u8]) -> Result<Vec<u8>, String> {
    if counter.len() != 16 {
        return Err("OperationError: AES-CTR counter must be 16 bytes".to_string());
    }
    if len_bits < 8 || len_bits > 128 || len_bits % 8 != 0 {
        return Err("OperationError: AES-CTR length must be a multiple of 8 between 8 and 128".to_string());
    }
    let ctr_bytes = (len_bits / 8) as usize;
    match key.len() {
        16 => ctr_with!(aes::Aes128, key, counter, ctr_bytes, data),
        24 => ctr_with!(aes::Aes192, key, counter, ctr_bytes, data),
        32 => ctr_with!(aes::Aes256, key, counter, ctr_bytes, data),
        _ => Err("OperationError: AES-CTR key must be 16/24/32 bytes".to_string()),
    }
}

/// `__wjs2_aesctr_crypt(keyU8, counterU8, lengthBits, dataU8)` → Uint8Array。
pub unsafe extern "C" fn aesctr_crypt(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 4 {
        report_error(&mut cx, "TypeError: AES-CTR crypt needs key, counter, length and data");
        return false;
    }
    let (Some(key), Some(counter), Some(data)) = (
        view_bytes(&mut cx, frame.arg(0), "AES-CTR key"),
        view_bytes(&mut cx, frame.arg(1), "AES-CTR counter"),
        view_bytes(&mut cx, frame.arg(3), "AES-CTR data"),
    ) else {
        return false;
    };
    let len_bits = frame.arg(2).to_number() as u32;
    match ctr_crypt(&key, &counter, len_bits, &data) {
        Ok(out) => set_rval_bytes(&mut cx, &frame, &out),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

/// AES-KW 手工（RFC 3394；ECB 块经 aes 转出 traits）：`__wjs2_aeskw_wrap(keyU8, dataU8)` /
/// `__wjs2_aeskw_unwrap(keyU8, dataU8)` → Uint8Array。
macro_rules! kw_block {
    ($aes:ty, $key:expr, $block:expr, encrypt) => {{
        use aes::cipher::block::BlockCipherEncrypt as _;
        use aes::cipher::{Block, KeyInit as _};
        let cipher = <$aes>::new_from_slice($key)
            .map_err(|e| format!("OperationError: bad AES-KW key: {e}"))?;
        let mut b = Block::<$aes>::from($block);
        cipher.encrypt_block(&mut b);
        let mut out = [0u8; 16];
        out.copy_from_slice(b.as_slice());
        out
    }};
    ($aes:ty, $key:expr, $block:expr, decrypt) => {{
        use aes::cipher::block::BlockCipherDecrypt as _;
        use aes::cipher::{Block, KeyInit as _};
        let cipher = <$aes>::new_from_slice($key)
            .map_err(|e| format!("OperationError: bad AES-KW key: {e}"))?;
        let mut b = Block::<$aes>::from($block);
        cipher.decrypt_block(&mut b);
        let mut out = [0u8; 16];
        out.copy_from_slice(b.as_slice());
        out
    }};
}

macro_rules! kw_with {
    ($aes:ty, $key:expr, $data:expr, $wrap:expr) => {{
        if $data.len() < 16 || $data.len() % 8 != 0 {
            return Err("OperationError: AES-KW data must be at least 16 bytes and a multiple of 8".to_string());
        }
        let n = $data.len() / 8;
        if $wrap {
            let mut a: [u8; 8] = [0xA6; 8];
            let mut r: Vec<[u8; 8]> = $data.chunks_exact(8).map(|c| {
                let mut b = [0u8; 8];
                b.copy_from_slice(c);
                b
            }).collect();
            for j in 0..6 {
                for i in 0..n {
                    let mut block = [0u8; 16];
                    block[..8].copy_from_slice(&a);
                    block[8..].copy_from_slice(&r[i]);
                    let out = kw_block!($aes, $key, block, encrypt);
                    let t = (n * j + i + 1) as u64;
                    for (k, byte) in t.to_be_bytes().iter().enumerate() {
                        a[k] = out[k] ^ byte;
                    }
                    r[i].copy_from_slice(&out[8..]);
                }
            }
            let mut res = Vec::with_capacity(8 + $data.len());
            res.extend_from_slice(&a);
            for b in &r {
                res.extend_from_slice(b);
            }
            Ok::<Vec<u8>, String>(res)
        } else {
            if $data.len() < 24 {
                return Err("OperationError: AES-KW wrapped data too short".to_string());
            }
            let mut a: [u8; 8] = $data[..8].try_into().unwrap();
            let mut r: Vec<[u8; 8]> = $data[8..].chunks_exact(8).map(|c| {
                let mut b = [0u8; 8];
                b.copy_from_slice(c);
                b
            }).collect();
            let n = r.len();
            for j in (0..6).rev() {
                for i in (0..n).rev() {
                    let t = (n * j + i + 1) as u64;
                    let mut aa = [0u8; 8];
                    for (k, byte) in t.to_be_bytes().iter().enumerate() {
                        aa[k] = a[k] ^ byte;
                    }
                    let mut block = [0u8; 16];
                    block[..8].copy_from_slice(&aa);
                    block[8..].copy_from_slice(&r[i]);
                    let out = kw_block!($aes, $key, block, decrypt);
                    a.copy_from_slice(&out[..8]);
                    r[i].copy_from_slice(&out[8..]);
                }
            }
            if a != [0xA6; 8] {
                return Err("OperationError: AES-KW unwrap failed (integrity check)".to_string());
            }
            let mut res = Vec::with_capacity(n * 8);
            for b in &r {
                res.extend_from_slice(b);
            }
            Ok::<Vec<u8>, String>(res)
        }
    }};
}

fn kw_crypt(key: &[u8], data: &[u8], wrap: bool) -> Result<Vec<u8>, String> {
    match key.len() {
        16 => kw_with!(aes::Aes128, key, data, wrap),
        24 => kw_with!(aes::Aes192, key, data, wrap),
        32 => kw_with!(aes::Aes256, key, data, wrap),
        _ => Err("OperationError: AES-KW key must be 16/24/32 bytes".to_string()),
    }
}

/// `__wjs2_aeskw_wrap(keyU8, dataU8)` → Uint8Array。
pub unsafe extern "C" fn aeskw_wrap(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: AES-KW wrap needs key and data");
        return false;
    }
    let (Some(key), Some(data)) = (
        view_bytes(&mut cx, frame.arg(0), "AES-KW key"),
        view_bytes(&mut cx, frame.arg(1), "AES-KW data"),
    ) else {
        return false;
    };
    match kw_crypt(&key, &data, true) {
        Ok(out) => set_rval_bytes(&mut cx, &frame, &out),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}

/// `__wjs2_aeskw_unwrap(keyU8, dataU8)` → Uint8Array（完整性错抛 OperationError）。
pub unsafe extern "C" fn aeskw_unwrap(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 2 {
        report_error(&mut cx, "TypeError: AES-KW unwrap needs key and data");
        return false;
    }
    let (Some(key), Some(data)) = (
        view_bytes(&mut cx, frame.arg(0), "AES-KW key"),
        view_bytes(&mut cx, frame.arg(1), "AES-KW data"),
    ) else {
        return false;
    };
    match kw_crypt(&key, &data, false) {
        Ok(out) => set_rval_bytes(&mut cx, &frame, &out),
        Err(e) => {
            report_error(&mut cx, &e);
            false
        }
    }
}
