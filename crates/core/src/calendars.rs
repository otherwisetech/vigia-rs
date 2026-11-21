//! Other calendars shown read-only next to the deadlines: any .ics file or URL (a Google public
//! holiday calendar, a team calendar). The text is kept in the database so it works offline;
//! URLs are fetched only when the user adds or refreshes them.
use crate::date::Date;
use crate::ics;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Calendar {
    pub id: i64,
    pub name: String,
    /// A file path or an http(s) URL.
    pub source: String,
    /// The .ics text last read.
    pub body: String,
    pub fetched: Option<i64>,
    pub enabled: bool,
}

impl Calendar {
    pub fn is_url(&self) -> bool {
        is_url(&self.source)
    }
}

pub fn is_url(s: &str) -> bool {
    let l = s.trim().to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://") || l.starts_with("webcal://")
}

/// `webcal://` is https in disguise.
pub fn fetch_url(s: &str) -> String {
    let t = s.trim();
    match t.get(..9) {
        Some(p) if p.eq_ignore_ascii_case("webcal://") => format!("https://{}", &t[9..]),
        _ => t.to_string(),
    }
}

/// The calendar's own name (`X-WR-CALNAME`), else `fallback`.
pub fn name_of(body: &str, fallback: &str) -> String {
    body.lines().find_map(|l| l.trim_end_matches('\r').strip_prefix("X-WR-CALNAME:").map(|v| v.trim().to_string())).filter(|n| !n.is_empty()).unwrap_or_else(|| fallback.to_string())
}

/// Whether the text is a calendar at all.
pub fn looks_like_ics(body: &str) -> bool {
    body.trim_start_matches('\u{feff}').trim_start().starts_with("BEGIN:VCALENDAR")
}

/// One day of an overlay event.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayDay {
    pub date: Date,
    pub summary: String,
    pub calendar: i64,
}

/// The enabled calendars' events between `from` and `to`, by date.
pub fn overlay(cals: &[Calendar], from: Date, to: Date) -> Vec<OverlayDay> {
    let mut v: Vec<OverlayDay> = cals
        .iter()
        .filter(|c| c.enabled)
        .flat_map(|c| ics::parse(&c.body).into_iter().flat_map(move |e| ics::occurrences(&e, from, to).into_iter().map(move |d| OverlayDay { date: d, summary: e.summary.clone(), calendar: c.id })))
        .collect();
    v.sort_by(|a, b| (a.date, a.calendar, &a.summary).cmp(&(b.date, b.calendar, &b.summary)));
    v.dedup();
    v
}

/// Downloads a calendar (the only network this module does; called on the user's request).
pub fn download(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).max_redirects(5).user_agent(format!("Vigia/{} (+https://otherwisetech.com)", env!("CARGO_PKG_VERSION"))).http_status_as_error(false).build().into();
    let mut resp = agent.get(&fetch_url(url)).header("Accept", "text/calendar, */*").call().map_err(|e| e.to_string())?;
    let code = resp.status().as_u16();
    let body = resp.body_mut().with_config().limit(16 << 20).read_to_string().map_err(|e| e.to_string())?;
    if !(200..300).contains(&code) {
        return Err(i18n::t!("core.check.http", code = code));
    }
    if !looks_like_ics(&body) {
        return Err(i18n::t!("core.cal.not_ics"));
    }
    Ok(body)
}

/// Reads a calendar file.
pub fn read_file(path: &std::path::Path) -> Result<String, String> {
    let body = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if !looks_like_ics(&body) {
        return Err(i18n::t!("core.cal.not_ics"));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    #[test]
    fn overlay_days() {
        let body = "BEGIN:VCALENDAR\nX-WR-CALNAME:Festivos de Colombia\nBEGIN:VEVENT\nSUMMARY:Navidad\nDTSTART;VALUE=DATE:20201225\nRRULE:FREQ=YEARLY\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:Día de la Raza\nDTSTART;VALUE=DATE:20261012\nEND:VEVENT\nEND:VCALENDAR\n";
        assert!(looks_like_ics(body) && !looks_like_ics("<html>"));
        assert_eq!(name_of(body, "x"), "Festivos de Colombia");
        assert_eq!(name_of("BEGIN:VCALENDAR", "file.ics"), "file.ics");
        let a = Calendar { id: 1, name: "F".into(), source: "/f.ics".into(), body: body.into(), fetched: None, enabled: true };
        let off = Calendar { id: 2, enabled: false, ..a.clone() };
        let v = overlay(&[a.clone(), off], ymd(2026, 10, 1).unwrap(), ymd(2026, 12, 31).unwrap());
        assert_eq!(v.iter().map(|o| (crate::date::iso(o.date), o.summary.as_str())).collect::<Vec<_>>(), vec![("2026-10-12".to_string(), "Día de la Raza"), ("2026-12-25".to_string(), "Navidad")]);
        assert!(!a.is_url() && is_url("webcal://example.org/x.ics") && is_url("HTTPS://x"));
        assert_eq!(fetch_url(" webcal://example.org/x.ics "), "https://example.org/x.ics");
        assert_eq!(fetch_url("https://example.org/x.ics"), "https://example.org/x.ics");
    }
}
