//! iCalendar export: one all-day event per item on its due date, with an alarm per lead time.
//! The same text is written to a file calendar apps can subscribe to.
use crate::date::{self, Date};
use crate::model::{Item, State};

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(x) => out.push(x),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Lines longer than 75 octets continue on the next line after a space (RFC 5545 §3.1).
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut n = 0;
    for c in line.chars() {
        let w = c.len_utf8();
        if n + w > 75 {
            out.push_str("\r\n ");
            n = 1;
        }
        out.push(c);
        n += w;
    }
    out.push_str("\r\n");
    out
}

fn ics_date(d: Date) -> String {
    d.format("%Y%m%d").to_string()
}

fn stamp(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default().format("%Y%m%dT%H%M%SZ").to_string()
}

/// The calendar for `items` (archived ones and items without a date are left out).
pub fn export(items: &[Item], today: Date, now_secs: i64) -> String {
    let mut out = String::new();
    let mut line = |s: &str| out.push_str(&fold(s));
    line("BEGIN:VCALENDAR");
    line("VERSION:2.0");
    line("PRODID:-//Otherwise Technologies LLC//Vigia//EN");
    line("CALSCALE:GREGORIAN");
    line("METHOD:PUBLISH");
    line(&format!("X-WR-CALNAME:{}", escape(&i18n::t!("core.ics.calendar_name"))));
    line("REFRESH-INTERVAL;VALUE=DURATION:PT12H");
    for it in items.iter().filter(|i| i.state != State::Archived) {
        let Some(due) = it.due_date(today) else { continue };
        line("BEGIN:VEVENT");
        line(&format!("UID:vigia-item-{}@otherwisetech.com", it.id));
        line(&format!("DTSTAMP:{}", stamp(if it.updated > 0 { it.updated } else { now_secs })));
        line(&format!("DTSTART;VALUE=DATE:{}", ics_date(due)));
        line(&format!("DTEND;VALUE=DATE:{}", ics_date(date::add_days(due, 1))));
        line(&format!("SUMMARY:{}", escape(&summary(it))));
        let mut desc = vec![it.kind.label()];
        if !it.owner.is_empty() {
            desc.push(i18n::t!("core.ics.belongs_to", kind = it.owner_kind.unwrap_or_default().label(), owner = it.owner));
        }
        if !it.reference.is_empty() {
            desc.push(it.reference.clone());
        }
        if !it.provider.is_empty() {
            desc.push(it.provider.clone());
        }
        if !it.notes.is_empty() {
            desc.push(it.notes.clone());
        }
        line(&format!("DESCRIPTION:{}", escape(&desc.join("\n"))));
        let mut cats = vec![escape(&it.category.label())];
        if let Some(k) = it.owner_kind.filter(|_| !it.owner.is_empty()) {
            cats.push(escape(&k.label()));
        }
        line(&format!("CATEGORIES:{}", cats.join(",")));
        if !it.renewal_url.is_empty() {
            line(&format!("URL:{}", it.renewal_url));
        }
        line("TRANSP:TRANSPARENT");
        for lead in it.leads() {
            line("BEGIN:VALARM");
            line("ACTION:DISPLAY");
            line(&format!("DESCRIPTION:{}", escape(&summary(it))));
            line(&format!("TRIGGER:-P{lead}D"));
            line("END:VALARM");
        }
        line("END:VEVENT");
    }
    line("END:VCALENDAR");
    out
}

pub fn summary(it: &Item) -> String {
    if it.notice_days.is_some_and(|n| n > 0) { i18n::t!("core.ics.notice", title = it.title()) } else { i18n::t!("core.ics.expires", title = it.title()) }
}

/// An event read back from a calendar (enough to check exports and to import simple feeds).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Event {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub start: Option<Date>,
    /// Alarm offsets in days before the start.
    pub alarms: Vec<u32>,
}

pub fn parse(s: &str) -> Vec<Event> {
    // unfold continuation lines first
    let mut lines: Vec<String> = Vec::new();
    for raw in s.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = raw.strip_prefix(' ').or_else(|| raw.strip_prefix('\t')) {
            if let Some(last) = lines.last_mut() {
                last.push_str(rest);
            }
        } else {
            lines.push(raw.to_string());
        }
    }
    let mut out = Vec::new();
    let mut cur: Option<Event> = None;
    let mut in_alarm = false;
    for l in lines {
        let Some((name, value)) = l.split_once(':') else { continue };
        let key = name.split(';').next().unwrap_or(name).to_ascii_uppercase();
        match (key.as_str(), value) {
            ("BEGIN", "VEVENT") => cur = Some(Event::default()),
            ("END", "VEVENT") => out.extend(cur.take()),
            ("BEGIN", "VALARM") => in_alarm = true,
            ("END", "VALARM") => in_alarm = false,
            _ => {}
        }
        let Some(e) = cur.as_mut() else { continue };
        if in_alarm {
            if key == "TRIGGER"
                && let Some(d) = value.strip_prefix("-P").and_then(|v| v.strip_suffix('D')).and_then(|v| v.parse().ok())
            {
                e.alarms.push(d);
            }
            continue;
        }
        match key.as_str() {
            "UID" => e.uid = value.to_string(),
            "SUMMARY" => e.summary = unescape(value),
            "DESCRIPTION" => e.description = unescape(value),
            "DTSTART" => e.start = chrono::NaiveDate::parse_from_str(value.get(..8).unwrap_or(value), "%Y%m%d").ok(),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;
    use crate::model::Kind;

    #[test]
    fn export_parses_back() {
        let today = ymd(2026, 11, 20).unwrap();
        let mut a = Item::new(Kind::Passport, "Pasaporte");
        a.id = 7;
        a.owner = "Ana".into();
        a.owner_kind = Some(crate::model::Kind::Passport.entity_kind().unwrap());
        a.expires = ymd(2031, 5, 12);
        a.notes = "Consulado; cita, con \\ y\nsegunda línea muy larga que obliga a doblar la línea del archivo iCalendar en varias partes".into();
        let mut b = Item::new(Kind::Contract, "Arriendo");
        b.id = 8;
        b.expires = ymd(2027, 1, 31);
        b.notice_days = Some(60);
        let mut gone = Item::new(Kind::Custom, "old");
        gone.id = 9;
        gone.expires = ymd(2027, 1, 1);
        gone.state = State::Archived;
        let undated = Item::new(Kind::Custom, "no date");
        let text = export(&[a.clone(), b.clone(), gone, undated], today, 1_790_000_000);
        assert!(text.lines().all(|l| l.len() <= 76), "folded");
        assert!(text.ends_with("END:VCALENDAR\r\n"));
        let ev = parse(&text);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].uid, "vigia-item-7@otherwisetech.com");
        assert_eq!(ev[0].start, ymd(2031, 5, 12));
        assert_eq!(ev[0].alarms, vec![270, 180, 90, 30]);
        assert!(ev[0].description.ends_with(&a.notes));
        assert!(ev[0].description.contains(&i18n::t!("core.ics.belongs_to", kind = crate::entity::EntityKind::Person.label(), owner = "Ana")));
        assert!(text.contains(&format!("CATEGORIES:{},{}", a.category.label(), crate::entity::EntityKind::Person.label())));
        assert!(ev[0].summary.contains("Pasaporte") && ev[0].summary.contains("Ana"));
        // contracts: the event is on the notice deadline
        assert_eq!(ev[1].start, ymd(2026, 12, 2));
    }
}
