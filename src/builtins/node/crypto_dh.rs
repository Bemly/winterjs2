//! crypto dh 域（DH/素性；对齐 crypto.rs；纯搬移）。

use mozjs::jsval::JSVal;

use crate::jsapi_glue::{report_error, view_bytes, wrap_cx, Frame};
use super::crypto::{pad_be, set_rval_bytes, set_rval_str};

// ── DH / 素性（`rsa::BigUint` 直用，`crypto.rs` 同款零新增口径）──────────────

fn dh_range(p: &rsa::BigUint, x: &rsa::BigUint) -> bool {
    let one = rsa::BigUint::from(1u32);
    x > &one && x < &(p - &one)
}

/// `__wjs2_dh_genkey(primeU8, generatorNum, privLenNum)` → JSON `{priv,pub}`（b64）。
pub unsafe extern "C" fn dh_genkey(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: DH genkey needs prime, generator and length");
        return false;
    }
    let prime = match view_bytes(&mut cx, frame.arg(0), "DH prime") {
        Some(b) => b,
        None => return false,
    };
    let generator = if frame.arg(1).is_number() { frame.arg(1).to_number() as u64 } else { 2 };
    let plen = if frame.arg(2).is_number() && frame.arg(2).to_number() > 0.0 {
        frame.arg(2).to_number() as usize
    } else {
        prime.len()
    };
    if prime.len() < 64 || generator < 2 {
        report_error(&mut cx, "OperationError: bad DH parameters");
        return false;
    }
    let p = rsa::BigUint::from_bytes_be(&prime);
    let g = rsa::BigUint::from(generator);
    // 私钥 ∈ [2, p-2]（高位掩码 + 重试，恒终止）
    let mut priv_b = vec![0u8; plen];
    let x = loop {
        if getrandom::fill(&mut priv_b).is_err() {
            report_error(&mut cx, "OperationError: cannot get random values");
            return false;
        }
        let x = rsa::BigUint::from_bytes_be(&priv_b);
        if dh_range(&p, &x) {
            break x;
        }
    };
    let y = g.modpow(&x, &p);
    use base64::Engine as _;
    let json = serde_json::json!({
        "priv": base64::engine::general_purpose::STANDARD.encode(pad_be(&x.to_bytes_be(), plen)),
        "pub": base64::engine::general_purpose::STANDARD.encode(pad_be(&y.to_bytes_be(), prime.len())),
    })
    .to_string();
    set_rval_str(&mut cx, &frame, &json);
    true
}

/// `__wjs2_dh_secret(primeU8, privU8, pubU8)` → 定长密钥（prime 长左补零，Node 同款）。
pub unsafe extern "C" fn dh_secret(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 3 {
        report_error(&mut cx, "TypeError: DH secret needs prime, private and public values");
        return false;
    }
    let (Some(prime), Some(priv_b), Some(pub_b)) = (
        view_bytes(&mut cx, frame.arg(0), "DH prime"),
        view_bytes(&mut cx, frame.arg(1), "DH private key"),
        view_bytes(&mut cx, frame.arg(2), "DH public key"),
    ) else {
        return false;
    };
    let p = rsa::BigUint::from_bytes_be(&prime);
    let x = rsa::BigUint::from_bytes_be(&priv_b);
    let y = rsa::BigUint::from_bytes_be(&pub_b);
    if !dh_range(&p, &y) {
        report_error(&mut cx, "OperationError: invalid DH public key");
        return false;
    }
    let s = y.modpow(&x, &p);
    set_rval_bytes(&mut cx, &frame, &pad_be(&s.to_bytes_be(), prime.len()))
}

/// Miller-Rabin（`checks` 轮随机基；小素数先试除。纯函数，单元测试覆盖）。
pub(crate) fn is_prime(n: &rsa::BigUint, checks: u32) -> bool {
    let zero = rsa::BigUint::from(0u32);
    let two = rsa::BigUint::from(2u32);
    if *n < two {
        return false;
    }
    for p in [2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let pp = rsa::BigUint::from(p);
        if *n == pp {
            return true;
        }
        if n % &pp == zero {
            return false;
        }
    }
    // n-1 = d·2^s（尾零字节数 + 末字节尾零位）
    let one = rsa::BigUint::from(1u32);
    let nm1 = n - &one;
    let nm1_bytes = nm1.to_bytes_be();
    let mut s = 0u32;
    for &b in nm1_bytes.iter().rev() {
        if b != 0 {
            s += b.trailing_zeros();
            break;
        }
        s += 8;
    }
    let mut d = nm1.clone();
    d >>= s as usize;
    let mut base = vec![0u8; n.to_bytes_be().len()];
    let two_b = rsa::BigUint::from(2u32);
    for _ in 0..checks.max(1) {
        if getrandom::fill(&mut base).is_err() {
            return false;
        }
        // 基 ∈ [2, n-2]
        let range = &nm1 - &two_b - &one;
        let a = rsa::BigUint::from_bytes_be(&base) % &range + &two_b;
        let mut x = a.modpow(&d, n);
        if x == one || x == nm1 {
            continue;
        }
        let mut composite = true;
        for _ in 1..s {
            x = x.modpow(&two_b, n);
            if x == nm1 {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

/// `__wjs2_prime_check(bytesU8, checksNum)` → boolean。
pub unsafe extern "C" fn prime_check(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 {
        report_error(&mut cx, "TypeError: prime check needs a candidate");
        return false;
    }
    let bytes = match view_bytes(&mut cx, frame.arg(0), "prime candidate") {
        Some(b) => b,
        None => return false,
    };
    let checks = if frame.argc() > 1 && frame.arg(1).is_number() {
        frame.arg(1).to_number() as u32
    } else {
        64
    };
    let n = rsa::BigUint::from_bytes_be(&bytes);
    frame.set_rval(mozjs::jsval::BooleanValue(is_prime(&n, checks)));
    true
}

/// `__wjs2_prime_gen(bitsNum, checksNum, safeNum[, addU8[, remU8]])` →
/// 素数 Uint8Array（定长）。add/rem 约束（node random.js 口径）：
/// 候选 ≡ rem (mod add)，类内拒绝采样（均匀）；单候选类确定性命中
///（(4,15,13)→13）；空类收敛失败（真机此处 hang，本仓报错，套件未覆盖）。
pub unsafe extern "C" fn prime_gen(
    cx_raw: *mut mozjs::jsapi::JSContext,
    argc: u32,
    vp: *mut JSVal,
) -> bool {
    // SAFETY: 同上
    let mut cx = unsafe { wrap_cx(cx_raw) };
    let frame = unsafe { Frame::from_raw(vp, argc) };
    if frame.argc() < 1 || !frame.arg(0).is_number() {
        report_error(&mut cx, "TypeError: prime generation needs a bit length");
        return false;
    }
    let bits = frame.arg(0).to_number() as usize;
    if bits < 2 || bits > 4096 {
        report_error(&mut cx, "ERR_OUT_OF_RANGE: prime bit length out of range (2..4096)");
        return false;
    }
    let checks = if frame.argc() > 1 && frame.arg(1).is_number() {
        frame.arg(1).to_number() as u32
    } else {
        64
    };
    let safe = frame.argc() > 2 && frame.arg(2).is_number() && frame.arg(2).to_number() != 0.0;
    // add/rem 约束（JS 侧已做 add<2^bits、rem<add 预检；此处只做解析 + 零保护）。
    let mut add: Option<rsa::BigUint> = None;
    let mut rem: Option<rsa::BigUint> = None;
    if frame.argc() > 3 && !frame.arg(3).is_undefined() && !frame.arg(3).is_null() {
        let Some(b) = view_bytes(&mut cx, frame.arg(3), "prime add") else {
            return false;
        };
        let a = rsa::BigUint::from_bytes_be(&b);
        if a != rsa::BigUint::from(0u32) {
            add = Some(a);
        }
    }
    if frame.argc() > 4 && !frame.arg(4).is_undefined() && !frame.arg(4).is_null() {
        let Some(b) = view_bytes(&mut cx, frame.arg(4), "prime rem") else {
            return false;
        };
        rem = Some(rsa::BigUint::from_bytes_be(&b));
    }
    let len = bits.div_ceil(8);
    let try_prime = |n: &rsa::BigUint| -> bool {
        if !is_prime(n, checks) {
            return false;
        }
        if safe {
            let half = (n - rsa::BigUint::from(1u32)) >> 1usize;
            if !is_prime(&half, checks) {
                return false;
            }
        }
        true
    };
    // DH-1024（2026-10-21）：无约束路径走 OpenSSL C 底座（真机同源
    // `BN_generate_prime_ex`；naive BigInt 路径 1024-bit safe 素数 debug 约 12min，
    // C 路径约 158ms）。add/rem 约束路径维持 naive（小尺寸为主，且确定性单候选
    // `(4,15,13)→13` 口径钉死）；孤 rem（无 add）两边一致忽略（node 同款）。
    // checks 轮数 fast 路径交由 OpenSSL 内置（node 同款不透传）。
    if add.is_none() {
        let mut big = match openssl::bn::BigNum::new() {
            Ok(b) => b,
            Err(_) => {
                report_error(&mut cx, "OperationError: prime generation failed to converge");
                return false;
            }
        };
        if big.generate_prime(bits as i32, safe, None, None).is_err() {
            report_error(&mut cx, "OperationError: prime generation failed to converge");
            return false;
        }
        return set_rval_bytes(&mut cx, &frame, &pad_be(&big.to_vec(), len));
    }
    let add = add.expect("prime_gen: add.is_none() 已早返");
    {
        let rem = rem.unwrap_or(if safe {
            rsa::BigUint::from(3u32)
        } else {
            rsa::BigUint::from(1u32)
        });
        let one = rsa::BigUint::from(1u32);
        let base = &one << (bits - 1);
        let top = (&one << bits) - &one;
        // lo = [2^(bits-1), 2^bits) 内最小 ≡rem(mod add) 元（全加式，无下溢）。
        let bmod = &base % &add;
        let rmod = &rem % &add;
        let lo = &base + ((&rmod + &add - &bmod) % &add);
        if lo > top {
            report_error(&mut cx, "OperationError: prime generation failed to converge");
            return false;
        }
        let count = (&top - &lo) / &add + &one;
        let count_bytes = count.to_bytes_be();
        let klen = count_bytes.len().max(1);
        for _ in 0..(bits as u64 * 1000 + 1000) {
            let mut kb = vec![0u8; klen];
            if getrandom::fill(&mut kb).is_err() {
                report_error(&mut cx, "OperationError: cannot get random values");
                return false;
            }
            let k = rsa::BigUint::from_bytes_be(&kb) % &count;
            let n = &lo + &k * &add;
            if try_prime(&n) {
                return set_rval_bytes(&mut cx, &frame, &pad_be(&n.to_bytes_be(), len));
            }
        }
        report_error(&mut cx, "OperationError: prime generation failed to converge");
        return false;
    }
}

