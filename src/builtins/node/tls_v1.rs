//! X.509 **v1** 证书的客户端校验兜底（P1 http2/tls，2026-09-25）。
//! webpki 只收 v3，node 测试 fixtures（agent*-cert）大量是 v1、OpenSSL 照收。
//! 策略：先走标准 WebPki 校验；仅当报 `UnsupportedCertVersion` 时改走手工路径——
//! ① 终端证书的 issuer 在用户给的 `ca` 里（Name DER 全等）且签名经该 CA 公钥验过
//! （复用 `node:crypto` X509 验签核 `x509_verify_impl`）；② 有效期覆盖当前时刻；
//! ③ 主机名对 subject CN 匹配（v1 无扩展、无 SAN；支持 `*.` 单层通配）。
//! 握手签名：TLS 1.3 经 SPKI 走 `verify_tls13_signature_with_raw_key`；TLS 1.2 无
//! 原始公钥变体（rustls 限制），v1 + 1.2 + 校验 组合如实失败（记档）。

use std::sync::Arc;

use der::{Decode as _, Encode as _};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, SubjectPublicKeyInfoDer, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, OtherError, SignatureScheme};

#[derive(Debug)]
pub(crate) struct V1FallbackVerifier {
    inner: Arc<rustls::client::WebPkiServerVerifier>,
    cas: Vec<CertificateDer<'static>>,
}

impl V1FallbackVerifier {
    pub(crate) fn new(inner: Arc<rustls::client::WebPkiServerVerifier>, cas: Vec<CertificateDer<'static>>) -> Self {
        Self { inner, cas }
    }
}

/// 是否为 webpki 的"证书版本不支持"错（v1 证书的唯一拒因）。
fn is_unsupported_version(e: &Error) -> bool {
    matches!(e, Error::InvalidCertificate(CertificateError::Other(OtherError(o)))
        if format!("{o:?}").contains("UnsupportedCertVersion"))
}

/// 是否为 webpki 的主机名失配错（SAN 有名对不上，或 v3 无 SAN 可对）。
fn is_name_error(e: &Error) -> bool {
    matches!(
        e,
        Error::InvalidCertificate(
            CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. }
        )
    )
}

/// 是否为 webpki 的"签发者不认识"错（自签/错 CA 同形，见 `SELF_SIGNED_SENTINEL`）。
fn is_unknown_issuer(e: &Error) -> bool {
    matches!(e, Error::InvalidCertificate(CertificateError::UnknownIssuer))
}

/// 自签哨兵（node DEPTH_ZERO_SELF_SIGNED_CERT 口径：链在 depth 0 终结于自签证书——
/// issuer 与 subject 同一 Name。经 `Error::General` 透出（Display 为
/// `"unexpected error: {哨兵}"`），捕获层/硬失败层原文存串，JS 侧子串匹配落码。
/// webpki 对自签一律报 UnknownIssuer（与"错 CA"不分），故在此分支先判自签。）
pub(crate) const SELF_SIGNED_SENTINEL: &str = "__wjs2_depth_zero_self_signed__";

/// 终端证书是否自签（issuer DER 与 subject DER 全等；OpenSSL `X509_check_issued`
/// 的 depth-0 判定同形。签名本身坏与否则由验签错另报，不归此判。）。
fn ee_is_self_signed(ee: &CertificateDer<'_>) -> bool {
    let Ok(cert) = x509_cert::Certificate::from_der(ee.as_ref()) else { return false };
    let tbs = cert.tbs_certificate();
    match (tbs.issuer().to_der(), tbs.subject().to_der()) {
        (Ok(i), Ok(s)) => i == s,
        _ => false,
    }
}

/// SAN 扩展 OID（2.5.29.17；有 SAN 即 CN 永不咨询——node `checkServerIdentity` 同口径）。
fn has_san_ext(cert: &x509_cert::Certificate) -> bool {
    cert.tbs_certificate()
        .extensions()
        .is_some_and(|exts| exts.iter().any(|e| e.extn_id.to_string() == "2.5.29.17"))
}

/// 连接主机名（DNS 名原串；IP 字面量不走 CN 回落——node 口径 IP 只认 SAN）。
fn dns_host(name: &ServerName<'_>) -> Option<String> {
    match name {
        ServerName::DnsName(d) => Some(d.as_ref().to_owned()),
        _ => None,
    }
}

/// SPKI → `x509_verify_impl` 的 (key, key_type)。
fn key_for_verify(spki: &x509_cert::spki::SubjectPublicKeyInfoOwned) -> Option<(Vec<u8>, &'static str)> {
    let der = spki.to_der().ok()?;
    match spki.algorithm.oid.to_string().as_str() {
        "1.2.840.113549.1.1.1" => Some((der, "rsa")),
        "1.2.840.10045.2.1" => Some((der, "ec")),
        "1.3.101.112" => Some((spki.subject_public_key.raw_bytes().to_vec(), "ed25519")),
        _ => None,
    }
}

/// subject CN（v1 证书的唯一主机名来源）。
fn subject_cn(cert: &x509_cert::Certificate) -> Option<String> {
    // 最后一个 CN（OpenSSL/node 取最具体者）；直接取值字节，兼容 UTF8/Printable/IA5 各串型。
    let mut out = None;
    for atv in cert.tbs_certificate().subject().iter() {
        if atv.oid.to_string() == "2.5.4.3" {
            out = std::str::from_utf8(atv.value.value()).ok().map(str::to_owned);
        }
    }
    out
}

/// 主机名匹配（大小写不敏感；`*.x.y` 只匹配单层标签）。
pub(crate) fn host_matches(pattern: &str, host: &str) -> bool {
    let (p, h) = (pattern.to_ascii_lowercase(), host.to_ascii_lowercase());
    if let Some(rest) = p.strip_prefix("*.") {
        return h.split_once('.').is_some_and(|(label, tail)| !label.is_empty() && tail == rest);
    }
    p == h
}

fn bad(msg: &str) -> Error {
    Error::General(format!("invalid peer certificate: {msg}"))
}

impl V1FallbackVerifier {
    fn verify_v1(&self, ee: &CertificateDer<'_>, name: &ServerName<'_>, now: UnixTime) -> Result<(), Error> {
        let cert = x509_cert::Certificate::from_der(ee.as_ref()).map_err(|_| bad("BadEncoding"))?;
        let issuer = cert.tbs_certificate().issuer().to_der().map_err(|_| bad("BadEncoding"))?;
        let mut signed = false;
        for ca in &self.cas {
            let Ok(c) = x509_cert::Certificate::from_der(ca.as_ref()) else { continue };
            if c.tbs_certificate().subject().to_der().ok().as_deref() != Some(issuer.as_slice()) {
                continue;
            }
            let Some((key, kt)) = key_for_verify(&c.tbs_certificate().subject_public_key_info()) else { continue };
            if crate::builtins::crypto::x509::x509_verify_impl(ee.as_ref(), &key, kt).unwrap_or(false) {
                signed = true;
                break;
            }
        }
        if !signed {
            // 自签（depth-0 终结）：node DEPTH_ZERO_SELF_SIGNED_CERT（真机 QA/QB：
            // 名对错皆此码）；须先判"已受信"（signed），受信自签即放行下文。
            if ee_is_self_signed(ee) {
                return Err(Error::General(SELF_SIGNED_SENTINEL.into()));
            }
            return Err(Error::InvalidCertificate(CertificateError::UnknownIssuer));
        }
        let v = &cert.tbs_certificate().validity();
        let t = now.as_secs();
        if t < v.not_before.to_unix_duration().as_secs() {
            return Err(Error::InvalidCertificate(CertificateError::NotValidYet));
        }
        if t > v.not_after.to_unix_duration().as_secs() {
            return Err(Error::InvalidCertificate(CertificateError::Expired));
        }
        let host = match name {
            ServerName::DnsName(d) => d.as_ref().to_owned(),
            ServerName::IpAddress(ip) => std::net::IpAddr::from(*ip).to_string(),
            _ => return Err(Error::InvalidCertificate(CertificateError::NotValidForName)),
        };
        match subject_cn(&cert) {
            Some(cn) if host_matches(&cn, &host) => Ok(()),
            _ => Err(Error::InvalidCertificate(CertificateError::NotValidForName)),
        }
    }

    /// v3 无 SAN 证书的 CN 回落（见 verify_server_cert 名错分支）：SAN 在场/IP
    /// 字面量/CN 不对一律透原错；仅"无 SAN + DNS 名对上 CN"放行。
    fn verify_cn_fallback(
        &self,
        ee: &CertificateDer<'_>,
        name: &ServerName<'_>,
        orig: Error,
    ) -> Result<(), Error> {
        let Ok(cert) = x509_cert::Certificate::from_der(ee.as_ref()) else {
            return Err(orig);
        };
        if has_san_ext(&cert) {
            return Err(orig);
        }
        let Some(host) = dns_host(name) else {
            return Err(orig);
        };
        match subject_cn(&cert) {
            Some(cn) if host_matches(&cn, &host) => Ok(()),
            _ => Err(orig),
        }
    }
}

impl ServerCertVerifier for V1FallbackVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        match self.inner.verify_server_cert(end_entity, intermediates, server_name, ocsp, now) {
            Err(e) if is_unsupported_version(&e) => {
                self.verify_v1(end_entity, server_name, now)?;
                Ok(ServerCertVerified::assertion())
            }
            // v3 无 SAN 证书（node fixtures agent1/agent3 形；OpenSSL 照收、webpki
            // 严格拒 NotValidForName）：SAN 缺席即咨询 CN，对上放行、错位透原错
            // （JS 侧哨兵重跑 checkServerIdentity 落 ALTNAME）。SAN 在场即 CN
            // 永不咨询（node 口径），IP 字面量亦不回落。
            Err(e) if is_name_error(&e) => {
                self.verify_cn_fallback(end_entity, server_name, e)?;
                Ok(ServerCertVerified::assertion())
            }
            // 自签（depth-0 终结）：node DEPTH_ZERO_SELF_SIGNED_CERT（真机：
            // 名对错/有无 SAN 皆此码，见 strict-probe QA/QC/QD）。
            Err(e) if is_unknown_issuer(&e) && ee_is_self_signed(end_entity) => {
                Err(Error::General(SELF_SIGNED_SENTINEL.into()))
            }
            r => r,
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        match self.inner.verify_tls13_signature(message, cert, dss) {
            Err(e) if is_unsupported_version(&e) => {
                let c = x509_cert::Certificate::from_der(cert.as_ref()).map_err(|_| bad("BadEncoding"))?;
                let spki = c.tbs_certificate().subject_public_key_info().to_der().map_err(|_| bad("BadEncoding"))?;
                let algs = rustls::crypto::ring::default_provider().signature_verification_algorithms;
                rustls::crypto::verify_tls13_signature_with_raw_key(
                    message,
                    &SubjectPublicKeyInfoDer::from(spki.as_slice()),
                    dss,
                    &algs,
                )
            }
            r => r,
        }
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_matching() {
        assert!(host_matches("localhost", "LOCALHOST"));
        assert!(host_matches("*.example.com", "a.example.com"));
        // 报错：通配不跨层、不匹配裸域。
        assert!(!host_matches("*.example.com", "a.b.example.com"));
        assert!(!host_matches("*.example.com", "example.com"));
        // 边界：空标签。
        assert!(!host_matches("*.example.com", ".example.com"));
    }

    /// rcgen CA 签发的终端证书 DER（`sans` 为空即 v3 无 SAN 形，node agent1 形）。
    #[cfg(test)]
    fn ee_der(cn: &str, sans: Vec<String>) -> (Vec<u8>, Vec<u8>) {
        let mut ca_params = rcgen::CertificateParams::new(vec![]).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "test-ca");
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let ca = rcgen::CertifiedIssuer::self_signed(ca_params, ca_key).unwrap();
        let mut srv_params = rcgen::CertificateParams::new(sans).unwrap();
        srv_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, cn);
        let srv_key = rcgen::KeyPair::generate().unwrap();
        let srv = rcgen::CertifiedIssuer::signed_by(srv_params, srv_key, &ca).unwrap();
        (srv.der().to_vec(), ca.der().to_vec())
    }

    #[cfg(test)]
    fn dns(name: &str) -> ServerName<'static> {
        ServerName::try_from(name.to_owned()).unwrap()
    }

    #[test]
    fn cn_fallback_accepts_matching_cn_without_san() {
        // 正常：无 SAN + CN 对上 → 放行（node agent1/agent3 形）。
        let (ee, ca) = ee_der("leaf.test", vec![]);
        let v = mk_verifier(&ca);
        assert!(v.verify_cn_fallback(&ee_der_wrap(&ee), &dns("leaf.test"), bad("orig")).is_ok());
        assert!(v.verify_cn_fallback(&ee_der_wrap(&ee), &dns("LEAF.TEST"), bad("orig")).is_ok());
        // 报错：CN 不对 → 透原错。
        let e = v
            .verify_cn_fallback(&ee_der_wrap(&ee), &dns("other.test"), bad("orig"))
            .unwrap_err();
        assert!(format!("{e}").contains("orig"));
    }

    #[test]
    fn cn_fallback_keeps_error_when_san_present_or_ip() {
        // 报错：SAN 在场即 CN 永不咨询（node 口径 SAN 赢）。
        let (ee, ca) = ee_der("leaf.test", vec!["leaf.test".to_string()]);
        let v = mk_verifier(&ca);
        assert!(has_san_ext(&x509_cert::Certificate::from_der(&ee).unwrap()));
        let e = v
            .verify_cn_fallback(&ee_der_wrap(&ee), &dns("leaf.test"), bad("orig"))
            .unwrap_err();
        assert!(format!("{e}").contains("orig"));
        // 边界：IP 字面量不回落 CN。
        let (ee_nosan, _) = ee_der("leaf.test", vec![]);
        assert!(!has_san_ext(&x509_cert::Certificate::from_der(&ee_nosan).unwrap()));
        let ip = ServerName::try_from("127.0.0.1".to_owned()).unwrap();
        let e = v
            .verify_cn_fallback(&ee_der_wrap(&ee_nosan), &ip, bad("orig"))
            .unwrap_err();
        assert!(format!("{e}").contains("orig"));
    }

    #[test]
    fn v1_path_self_signed_reports_depth_zero() {
        // v1 自签（node agent2 形）走 UnsupportedCertVersion→verify_v1：未受信即哨兵。
        let ss = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
        let der = ss.cert.der().to_vec();
        let (_, ca) = ee_der("leaf.test", vec![]);
        let v = mk_verifier(&ca);
        let now = rustls::pki_types::UnixTime::since_unix_epoch(std::time::Duration::from_secs(1_800_000_000));
        let e = v
            .verify_v1(&ee_der_wrap(&der), &dns("leaf.test"), now)
            .unwrap_err();
        assert!(format!("{e}").contains(SELF_SIGNED_SENTINEL), "got {e}");
    }

    #[test]
    fn self_signed_detection() {        // 正常：自签（rcgen 自签，SAN 有无皆是）→ true。
        let ss = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
        assert!(ee_is_self_signed(&ee_der_wrap(&ss.cert.der().to_vec())));
        // 报错：CA 签发 → false；坏 DER → false。
        let (ee, _) = ee_der("leaf.test", vec![]);
        assert!(!ee_is_self_signed(&ee_der_wrap(&ee)));
        assert!(!ee_is_self_signed(&ee_der_wrap(b"nope")));
    }

    #[cfg(test)]
    fn ee_der_wrap(der: &[u8]) -> CertificateDer<'_> {
        CertificateDer::from(der)
    }

    #[cfg(test)]
    fn mk_verifier(ca_der: &[u8]) -> V1FallbackVerifier {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(CertificateDer::from(ca_der)).unwrap();
        let inner = rustls::client::WebPkiServerVerifier::builder(std::sync::Arc::new(roots))
            .build()
            .unwrap();
        V1FallbackVerifier { inner, cas: vec![CertificateDer::from(ca_der).into_owned()] }
    }
}
