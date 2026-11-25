//! Automatic checks for items that opted in: a domain's expiry over RDAP, a certificate's
//! `notAfter` over TLS. The only network Vigía uses for your items, and only for these.
use crate::date::Date;
use crate::model::{Item, Kind};

pub const EVERY_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Domain(crate::rdap::DomainInfo),
    Cert(crate::cert::CertInfo),
    Failed(String),
}

/// The network side, behind a trait so tests use fixtures.
pub trait Checker {
    fn domain(&self, name: &str) -> Result<crate::rdap::DomainInfo, String>;
    fn cert(&self, host: &str) -> Result<crate::cert::CertInfo, String>;
}

pub struct Net;

impl Checker for Net {
    fn domain(&self, name: &str) -> Result<crate::rdap::DomainInfo, String> {
        crate::rdap::lookup(name)
    }
    fn cert(&self, host: &str) -> Result<crate::cert::CertInfo, String> {
        crate::cert::fetch(host)
    }
}

/// What to look up for `item`: its host, else its name.
pub fn target(item: &Item) -> String {
    if item.host.trim().is_empty() { item.name.trim().to_string() } else { item.host.trim().to_string() }
}

pub fn run(item: &Item, net: &dyn Checker) -> Outcome {
    let t = target(item);
    match item.kind {
        Kind::Domain => net.domain(&t).map(Outcome::Domain).unwrap_or_else(Outcome::Failed),
        Kind::Certificate => net.cert(&t).map(Outcome::Cert).unwrap_or_else(Outcome::Failed),
        _ => Outcome::Failed(i18n::t!("core.check.not_checkable")),
    }
}

/// Items whose daily check is due.
pub fn due(items: &[Item], now: i64) -> Vec<i64> {
    items.iter().filter(|i| i.auto_check && i.kind.checkable() && i.state != crate::model::State::Archived && i.last_checked.is_none_or(|t| now - t >= EVERY_SECS || t > now)).map(|i| i.id).collect()
}

/// Writes an outcome into the item. Returns whether the expiry changed.
pub fn apply(item: &mut Item, outcome: &Outcome, now: i64) -> bool {
    item.last_checked = Some(now);
    let before = item.expires;
    match outcome {
        Outcome::Domain(d) => {
            if let Some(e) = d.expires {
                item.expires = Some(e);
            }
            if let Some(r) = &d.registrar
                && item.provider.is_empty()
            {
                item.provider = r.clone();
            }
            if d.status.iter().any(|s| s.contains("auto renew")) {
                item.auto_renews = true;
            }
            item.check_note = match d.expires {
                Some(e) => i18n::t!("core.check.domain_ok", date = crate::date::iso(e)),
                // some registries answer without publishing the expiry
                None => i18n::t!("core.check.unsupported"),
            };
        }
        Outcome::Cert(c) => {
            if let Some(e) = c.not_after {
                item.expires = Some(e);
            }
            if let Some(b) = c.not_before {
                item.issued = Some(b);
            }
            if let Some(i) = &c.issuer
                && item.provider.is_empty()
            {
                item.provider = i.clone();
            }
            item.check_note = match c.not_after {
                Some(e) => i18n::t!("core.check.cert_ok", date = crate::date::iso(e), issuer = c.issuer.clone().unwrap_or_default()),
                None => i18n::t!("core.check.no_expiry"),
            };
        }
        Outcome::Failed(e) if is_in_any("core.check.no_rdap", e) => item.check_note = i18n::t!("core.check.unsupported"),
        Outcome::Failed(e) => item.check_note = i18n::t!("core.check.failed", detail = e),
    }
    item.expires != before
}

/// Whether `text` starts like catalog entry `key` in any bundled language.
fn is_in_any(key: &str, text: &str) -> bool {
    i18n::available().iter().any(|(lang, _)| {
        let t = i18n::tr_in(lang, key);
        let prefix = t.split('{').next().unwrap_or_default().trim();
        !prefix.is_empty() && text.trim_start().starts_with(prefix)
    })
}

/// The registry can't be asked (no RDAP for the TLD, or it doesn't publish the expiry): the
/// user enters the date by hand.
pub fn note_is_unsupported(note: &str) -> bool {
    is_in_any("core.check.unsupported", note)
}

/// The last check left the item without a date: say so and ask for it.
pub fn needs_manual(item: &Item) -> bool {
    item.kind.checkable() && item.last_checked.is_some() && item.expires.is_none() && (note_is_unsupported(&item.check_note) || note_is_failure(&item.check_note))
}

/// Where the date comes from: `RDAP` for domains, `TLS` for certificates.
pub fn source(item: &Item) -> &'static str {
    if item.kind == Kind::Domain { "RDAP" } else { "TLS" } // i18n-ok: protocol names
}

/// Domains that have a name to look up but no date and were never checked: looked up once
/// (0.7.0 backfill), whether or not their daily check is on.
pub fn backfill(items: &[Item]) -> Vec<i64> {
    items.iter().filter(|i| i.kind == Kind::Domain && i.state != crate::model::State::Archived && i.expires.is_none() && i.last_checked.is_none() && crate::rdap::normalize(&target(i)).is_some()).map(|i| i.id).collect()
}

/// Whether `note` is a failed check's note, in whichever bundled language it was written.
pub fn note_is_failure(note: &str) -> bool {
    i18n::available().iter().any(|(lang, _)| {
        let t = i18n::tr_in(lang, "core.check.failed");
        let prefix = t.split("{detail}").next().unwrap_or_default().trim();
        !prefix.is_empty() && note.trim_start().starts_with(prefix)
    })
}

/// Whether the last check is old enough to call stale in the UI (two missed days).
pub fn stale(item: &Item, now: i64) -> bool {
    item.auto_check && item.last_checked.is_none_or(|t| now - t > 2 * EVERY_SECS)
}

pub fn last_checked_day(item: &Item) -> Option<Date> {
    item.last_checked.and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|t| t.with_timezone(&chrono::Local).date_naive())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::CertInfo;
    use crate::date::ymd;
    use crate::rdap::DomainInfo;

    struct Fake;
    impl Checker for Fake {
        fn domain(&self, name: &str) -> Result<DomainInfo, String> {
            if name == "example.com" { Ok(crate::rdap::parse_domain(include_str!("../tests/fixtures/rdap-verisign.json")).unwrap()) } else { Err("boom".into()) }
        }
        fn cert(&self, host: &str) -> Result<CertInfo, String> {
            assert_eq!(host, "test.example.com:8443");
            Ok(crate::cert::parse_der(include_bytes!("../tests/fixtures/cert.der")).unwrap())
        }
    }

    #[test]
    fn checks_update_items() {
        let now = 1_790_000_000;
        let mut d = Item { auto_check: true, ..Item::new(Kind::Domain, "example.com") };
        let o = run(&d, &Fake);
        assert!(apply(&mut d, &o, now));
        assert_eq!(d.expires, ymd(2027, 8, 13));
        assert_eq!(d.provider, "Example Registrar, Inc.");
        assert_eq!(d.last_checked, Some(now));
        assert!(!apply(&mut d, &o, now), "same expiry: no change");
        let mut c = Item { host: "test.example.com:8443".into(), auto_check: true, ..Item::new(Kind::Certificate, "API cert") };
        let o = run(&c, &Fake);
        assert!(apply(&mut c, &o, now));
        assert_eq!((c.issued, c.expires), (ymd(2026, 1, 1), ymd(2027, 3, 15)));
        assert_eq!(c.provider, "Vigia Test");
        let mut bad = Item { auto_check: true, expires: ymd(2027, 1, 1), ..Item::new(Kind::Domain, "nope.com") };
        let o = run(&bad, &Fake);
        assert!(matches!(o, Outcome::Failed(_)));
        assert!(!apply(&mut bad, &o, now));
        assert_eq!(bad.expires, ymd(2027, 1, 1), "a failed check keeps the date");
        let items = vec![Item { id: 1, last_checked: Some(now - 10), ..d.clone() }, Item { id: 2, last_checked: Some(now - EVERY_SECS), ..d.clone() }, Item { id: 3, last_checked: None, ..d.clone() }, Item { id: 4, auto_check: false, ..d.clone() }, Item { id: 5, auto_check: true, ..Item::new(Kind::Passport, "p") }];
        assert_eq!(due(&items, now), vec![2, 3]);
        assert!(stale(&Item { last_checked: Some(now - 3 * EVERY_SECS), ..d.clone() }, now));
    }

    #[test]
    fn unsupported_registries_ask_for_the_date() {
        let mut io = Item { auto_check: true, host: "vcity.io".into(), ..Item::new(Kind::Domain, "vcity.io") };
        assert_eq!(backfill(std::slice::from_ref(&io)), vec![0]);
        apply(&mut io, &Outcome::Failed(i18n::t!("core.check.no_rdap", name = "vcity.io")), 1);
        assert!(note_is_unsupported(&io.check_note) && needs_manual(&io));
        assert!(backfill(std::slice::from_ref(&io)).is_empty(), "looked up once");
        let mut silent = Item::new(Kind::Domain, "example.org");
        apply(&mut silent, &Outcome::Domain(crate::rdap::DomainInfo::default()), 1);
        assert!(needs_manual(&silent));
        let mut ok = Item::new(Kind::Domain, "example.com");
        apply(&mut ok, &Outcome::Domain(crate::rdap::DomainInfo { expires: ymd(2027, 8, 13), ..Default::default() }), 1);
        assert!(!needs_manual(&ok) && !note_is_unsupported(&ok.check_note));
        assert_eq!(source(&ok), "RDAP");
        assert!(backfill(&[Item::new(Kind::Domain, "Main site"), Item { expires: ymd(2027, 1, 1), ..Item::new(Kind::Domain, "a.com") }]).is_empty());
    }

    #[test]
    fn failed_notes_are_recognized_in_any_language() {
        let mut it = Item::new(crate::model::Kind::Domain, "example.org");
        apply(&mut it, &Outcome::Failed("timeout".into()), 1);
        assert!(note_is_failure(&it.check_note));
        assert!(note_is_failure("Check failed: dns") && note_is_failure("Falló la revisión: x"));
        assert!(!note_is_failure("") && !note_is_failure(&i18n::t!("core.check.domain_ok", date = "2027-01-01")));
    }
}
