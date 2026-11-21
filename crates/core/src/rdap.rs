//! Domain expiry over RDAP (the JSON successor of WHOIS). Looked up only for items the user
//! opted in: `https://rdap.org/domain/<name>` (which redirects to the registry), falling back
//! to the IANA bootstrap file to find the registry's server.
use crate::date::Date;
use serde_json::Value;
use std::time::Duration;

pub const RDAP_ORG: &str = "https://rdap.org/domain/";
pub const IANA_BOOTSTRAP: &str = "https://data.iana.org/rdap/dns.json";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DomainInfo {
    pub expires: Option<Date>,
    pub registered: Option<Date>,
    pub registrar: Option<String>,
    /// EPP status codes (`client transfer prohibited`, `auto renew period`…).
    pub status: Vec<String>,
}

/// Second-level labels under which country registries sell names (`example.com.co`).
const SECOND_LEVEL: &[&str] = &["co", "com", "net", "org", "gov", "edu", "ac", "mil", "nom", "gob", "or", "ne"];

/// `https://www.Example.com.co/path` → `example.com.co`; None when it isn't a domain name.
pub fn normalize(input: &str) -> Option<String> {
    let s = input.trim().to_lowercase();
    let s = s.split("://").last().unwrap_or(&s);
    let s = s.split(['/', '?', '#']).next().unwrap_or(s);
    let s = s.rsplit('@').next().unwrap_or(s);
    let s = s.split(':').next().unwrap_or(s).trim_end_matches('.');
    let labels: Vec<&str> = s.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() < 2 || !labels.iter().all(|l| l.chars().all(|c| c.is_alphanumeric() || c == '-') && !l.starts_with('-')) {
        return None;
    }
    if labels.last().is_some_and(|t| t.chars().all(|c| c.is_ascii_digit())) {
        return None; // an IP address
    }
    let n = labels.len();
    let keep = if n >= 3 && labels[n - 1].len() == 2 && SECOND_LEVEL.contains(&labels[n - 2]) { 3 } else { 2 };
    Some(labels[n.saturating_sub(keep)..].join("."))
}

fn event_date(v: &Value, action: &str) -> Option<Date> {
    v.get("events")?.as_array()?.iter().find(|e| e.get("eventAction").and_then(Value::as_str).is_some_and(|a| a.eq_ignore_ascii_case(action))).and_then(|e| e.get("eventDate")?.as_str()).and_then(parse_timestamp)
}

/// `2027-08-13T04:00:00Z` / `2027-08-13T04:00:00.000+00:00` / `2027-08-13` → the UTC day.
pub fn parse_timestamp(s: &str) -> Option<Date> {
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(t.with_timezone(&chrono::Utc).date_naive());
    }
    chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()
}

/// The registrar's name from its jCard (`vcardArray` → `fn`).
fn registrar(v: &Value) -> Option<String> {
    let ents = v.get("entities")?.as_array()?;
    let ent = ents.iter().find(|e| e.get("roles").and_then(Value::as_array).is_some_and(|r| r.iter().any(|x| x.as_str() == Some("registrar"))))?;
    let card = ent.get("vcardArray")?.as_array()?.get(1)?.as_array()?;
    card.iter().filter_map(Value::as_array).find(|p| p.first().and_then(Value::as_str) == Some("fn")).and_then(|p| p.get(3)?.as_str()).map(String::from).filter(|s| !s.is_empty())
}

/// Reads an RDAP domain response.
pub fn parse_domain(json: &str) -> Option<DomainInfo> {
    let v: Value = serde_json::from_str(json).ok()?;
    if v.get("objectClassName").and_then(Value::as_str).is_some_and(|c| c != "domain") {
        return None;
    }
    let status = v.get("status").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
    Some(DomainInfo { expires: event_date(&v, "expiration"), registered: event_date(&v, "registration"), registrar: registrar(&v), status })
}

/// The RDAP base URL for a domain's TLD from the IANA bootstrap file.
pub fn bootstrap_url(json: &str, domain: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    let tld = domain.rsplit('.').next()?.to_lowercase();
    v.get("services")?.as_array()?.iter().find_map(|svc| {
        let pair = svc.as_array()?;
        let tlds = pair.first()?.as_array()?;
        if !tlds.iter().any(|t| t.as_str().is_some_and(|t| t.eq_ignore_ascii_case(&tld))) {
            return None;
        }
        let urls = pair.get(1)?.as_array()?;
        // prefer https
        urls.iter().filter_map(Value::as_str).find(|u| u.starts_with("https://")).or_else(|| urls.iter().filter_map(Value::as_str).next()).map(String::from)
    })
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).max_redirects(5).user_agent(format!("Vigia/{} (+https://otherwisetech.com)", env!("CARGO_PKG_VERSION"))).http_status_as_error(false).build().into()
}

fn get(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    let mut resp = agent.get(url).header("Accept", "application/rdap+json, application/json").call().map_err(|e| e.to_string())?;
    let code = resp.status().as_u16();
    let body = resp.body_mut().with_config().limit(4 << 20).read_to_string().map_err(|e| e.to_string())?;
    match code {
        200..300 => Ok(body),
        404 => Err(i18n::t!("core.check.rdap_not_found")),
        _ => Err(i18n::t!("core.check.http", code = code)),
    }
}

/// Blocking network lookup. Run off the UI thread.
pub fn lookup(domain: &str) -> Result<DomainInfo, String> {
    let name = normalize(domain).ok_or_else(|| i18n::t!("core.check.bad_domain", name = domain))?;
    let agent = agent();
    let first = get(&agent, &format!("{RDAP_ORG}{name}"));
    let body = match first {
        Ok(b) => b,
        Err(e) => {
            let boot = get(&agent, IANA_BOOTSTRAP).map_err(|_| e.clone())?;
            let base = bootstrap_url(&boot, &name).ok_or_else(|| i18n::t!("core.check.no_rdap", name = name))?;
            get(&agent, &format!("{}/domain/{name}", base.trim_end_matches('/')))?
        }
    };
    parse_domain(&body).ok_or_else(|| i18n::t!("core.check.bad_answer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    const VERISIGN: &str = include_str!("../tests/fixtures/rdap-verisign.json");
    const CO: &str = include_str!("../tests/fixtures/rdap-co.json");
    const BOOT: &str = include_str!("../tests/fixtures/iana-dns.json");

    #[test]
    fn parses_responses() {
        let a = parse_domain(VERISIGN).unwrap();
        assert_eq!(a.expires, ymd(2027, 8, 13));
        assert_eq!(a.registered, ymd(1997, 8, 13));
        assert_eq!(a.registrar.as_deref(), Some("Example Registrar, Inc."));
        assert!(a.status.iter().any(|s| s == "client transfer prohibited"));
        let b = parse_domain(CO).unwrap();
        assert_eq!(b.expires, ymd(2027, 3, 1));
        assert_eq!(b.registrar, None);
        assert_eq!(parse_domain(r#"{"objectClassName":"entity"}"#), None);
        assert_eq!(parse_domain("not json"), None);
        assert_eq!(parse_domain(r#"{"objectClassName":"domain","events":[]}"#).unwrap().expires, None);
    }

    #[test]
    fn bootstrap() {
        assert_eq!(bootstrap_url(BOOT, "example.com").as_deref(), Some("https://rdap.verisign.com/com/v1/"));
        assert_eq!(bootstrap_url(BOOT, "example.com.co").as_deref(), Some("https://rdap.nic.co/"));
        assert_eq!(bootstrap_url(BOOT, "example.zz"), None);
    }

    #[test]
    fn names() {
        assert_eq!(normalize("https://www.Example.com/path?q").as_deref(), Some("example.com"));
        assert_eq!(normalize("shop.example.com.co").as_deref(), Some("example.com.co"));
        assert_eq!(normalize("example.co").as_deref(), Some("example.co"));
        assert_eq!(normalize("a.b.example.io:443").as_deref(), Some("example.io"));
        assert_eq!(normalize("192.168.1.1"), None);
        assert_eq!(normalize("localhost"), None);
        assert_eq!(normalize("bad_name.com"), None);
        assert_eq!(parse_timestamp("2027-08-13T04:00:00.000+00:00"), ymd(2027, 8, 13));
        assert_eq!(parse_timestamp("2027-08-13T23:30:00-05:00"), ymd(2027, 8, 14));
    }
}
