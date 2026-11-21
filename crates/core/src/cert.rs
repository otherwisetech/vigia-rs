//! TLS certificate expiry: a handshake with `host:443` (or the port given), then the leaf
//! certificate's `notAfter`. The certificate is never trusted or used for anything else, so
//! expired and self-signed ones are read too — that's the point.
use crate::date::Date;
use std::io::{Read as _, Write as _};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CertInfo {
    pub not_before: Option<Date>,
    pub not_after: Option<Date>,
    pub subject_cn: Option<String>,
    pub issuer: Option<String>,
}

// ── a minimal DER reader: enough to walk to Validity, Subject and Issuer ─────────────────────

struct Tlv<'a> {
    tag: u8,
    body: &'a [u8],
    rest: &'a [u8],
}

fn tlv(b: &[u8]) -> Option<Tlv<'_>> {
    let tag = *b.first()?;
    let first = *b.get(1)? as usize;
    let (len, hdr) = if first < 0x80 {
        (first, 2)
    } else {
        let n = first & 0x7f;
        if n == 0 || n > 4 {
            return None;
        }
        let mut len = 0usize;
        for i in 0..n {
            len = (len << 8) | *b.get(2 + i)? as usize;
        }
        (len, 2 + n)
    };
    let body = b.get(hdr..hdr.checked_add(len)?)?;
    Some(Tlv { tag, body, rest: &b[hdr + len..] })
}

/// `UTCTime` (YYMMDDHHMMSSZ) or `GeneralizedTime` (YYYYMMDDHHMMSSZ) → the UTC day.
fn der_time(t: &Tlv) -> Option<Date> {
    let s = std::str::from_utf8(t.body).ok()?;
    let (y, rest) = match t.tag {
        0x17 => {
            let yy: i32 = s.get(..2)?.parse().ok()?;
            (if yy >= 50 { 1900 + yy } else { 2000 + yy }, s.get(2..)?)
        }
        0x18 => (s.get(..4)?.parse().ok()?, s.get(4..)?),
        _ => return None,
    };
    crate::date::ymd(y, rest.get(..2)?.parse().ok()?, rest.get(2..4)?.parse().ok()?)
}

/// The value of attribute `oid` (e.g. CN = 2.5.4.3) in a Name.
fn name_attr(name: &[u8], oid: &[u8]) -> Option<String> {
    let mut sets = name;
    while let Some(set) = tlv(sets) {
        sets = set.rest;
        let mut atvs = set.body;
        while let Some(atv) = tlv(atvs) {
            atvs = atv.rest;
            let o = tlv(atv.body)?;
            if o.tag == 0x06 && o.body == oid {
                let v = tlv(o.rest)?;
                return String::from_utf8(v.body.to_vec()).ok();
            }
        }
    }
    None
}

const OID_CN: &[u8] = &[0x55, 0x04, 0x03];
const OID_O: &[u8] = &[0x55, 0x04, 0x0a];

/// Reads validity, subject CN and issuer (O or CN) of a DER X.509 certificate.
pub fn parse_der(der: &[u8]) -> Option<CertInfo> {
    let cert = tlv(der).filter(|t| t.tag == 0x30)?;
    let tbs = tlv(cert.body).filter(|t| t.tag == 0x30)?;
    let mut f = tbs.body;
    let mut next = || -> Option<Tlv> {
        let t = tlv(f)?;
        f = t.rest;
        Some(t)
    };
    if next()?.tag == 0xa0 {
        next()?; // explicit version, then the serial
    }
    // signature algorithm, issuer, validity, subject
    let _sig = next()?;
    let issuer = next()?;
    let validity = next().filter(|v| v.tag == 0x30)?;
    let subject = next()?;
    let nb = tlv(validity.body)?;
    let na = tlv(nb.rest)?;
    Some(CertInfo { not_before: der_time(&nb), not_after: der_time(&na), subject_cn: name_attr(subject.body, OID_CN), issuer: name_attr(issuer.body, OID_O).or_else(|| name_attr(issuer.body, OID_CN)) })
}

/// `example.com`, `example.com:8443`, `https://example.com/x` → (host, port).
pub fn split_host(input: &str) -> Option<(String, u16)> {
    let s = input.trim();
    let s = s.split("://").last().unwrap_or(s);
    let s = s.split(['/', '?', '#']).next().unwrap_or(s);
    if s.is_empty() {
        return None;
    }
    if let Some(v6) = s.strip_prefix('[') {
        let (h, rest) = v6.split_once(']')?;
        let port = rest.strip_prefix(':').map(|p| p.parse().ok()).unwrap_or(Some(443))?;
        return Some((h.to_string(), port));
    }
    match s.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => Some((h.to_lowercase(), p.parse().ok()?)),
        _ => Some((s.to_lowercase(), 443)),
    }
}

#[derive(Debug)]
struct ReadAnything(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for ReadAnything {
    fn verify_server_cert(&self, _: &rustls::pki_types::CertificateDer<'_>, _: &[rustls::pki_types::CertificateDer<'_>], _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Blocking: connect, handshake, read the leaf certificate, close. Run off the UI thread.
pub fn fetch(input: &str) -> Result<CertInfo, String> {
    let (host, port) = split_host(input).ok_or_else(|| i18n::t!("core.check.bad_host", host = input))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions().map_err(|e| e.to_string())?.dangerous().with_custom_certificate_verifier(Arc::new(ReadAnything(provider))).with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| i18n::t!("core.check.bad_host", host = host))?;
    let addr = (host.as_str(), port).to_socket_addrs().map_err(|e| i18n::t!("core.check.dns", host = host, detail = e))?.next().ok_or_else(|| i18n::t!("core.check.bad_host", host = host))?;
    let mut sock = TcpStream::connect_timeout(&addr, Duration::from_secs(10)).map_err(|e| i18n::t!("core.check.connect", host = host, detail = e))?;
    sock.set_read_timeout(Some(Duration::from_secs(10))).ok();
    sock.set_write_timeout(Some(Duration::from_secs(10))).ok();
    let mut conn = rustls::ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
    while conn.is_handshaking() {
        conn.complete_io(&mut sock).map_err(|e| i18n::t!("core.check.tls", detail = e))?;
    }
    let der = conn.peer_certificates().and_then(|c| c.first()).map(|c| c.as_ref().to_vec()).ok_or_else(|| i18n::t!("core.check.no_cert"))?;
    conn.send_close_notify();
    let _ = conn.complete_io(&mut sock);
    let _ = sock.flush();
    let mut sink = [0u8; 1];
    let _ = sock.read(&mut sink);
    parse_der(&der).ok_or_else(|| i18n::t!("core.check.bad_answer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    #[test]
    fn reads_validity_and_names() {
        let c = parse_der(include_bytes!("../tests/fixtures/cert.der")).unwrap();
        assert_eq!(c.not_before, ymd(2026, 1, 1));
        assert_eq!(c.not_after, ymd(2027, 3, 15));
        assert_eq!(c.subject_cn.as_deref(), Some("test.example.com"));
        assert_eq!(c.issuer.as_deref(), Some("Vigia Test"));
        // after 2049 the time is a GeneralizedTime
        let g = parse_der(include_bytes!("../tests/fixtures/cert-2055.der")).unwrap();
        assert_eq!(g.not_after, ymd(2055, 6, 1));
        assert_eq!(g.issuer.as_deref(), Some("long.example.org"));
        assert_eq!(parse_der(b"\x30\x03\x02\x01\x01"), None);
        assert_eq!(parse_der(&[]), None);
        let der = include_bytes!("../tests/fixtures/cert.der");
        assert_eq!(parse_der(&der[..der.len() / 2]), None);
    }

    #[test]
    fn hosts() {
        assert_eq!(split_host("Example.com"), Some(("example.com".into(), 443)));
        assert_eq!(split_host("https://example.com:8443/path"), Some(("example.com".into(), 8443)));
        assert_eq!(split_host("[::1]:9443"), Some(("::1".into(), 9443)));
        assert_eq!(split_host("example.com:x"), None);
        assert_eq!(split_host(""), None);
    }
}
