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
    /// `FREQ=YEARLY;BYMONTH=11;BYDAY=4TH`, as written.
    pub rrule: String,
    /// Last day (inclusive) of a multi-day all-day event.
    pub last: Option<Date>,
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
            // DTEND of an all-day event is exclusive
            "DTEND" => e.last = chrono::NaiveDate::parse_from_str(value.get(..8).unwrap_or(value), "%Y%m%d").ok().map(|d| if value.len() == 8 { date::add_days(d, -1) } else { d }),
            "RRULE" => e.rrule = value.trim().to_string(),
            _ => {}
        }
    }
    out
}

fn rule_part<'a>(rule: &'a str, key: &str) -> Option<&'a str> {
    rule.split(';').filter_map(|p| p.split_once('=')).find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

/// `4TH` → (4, Thursday), `-1MO` → (-1, Monday), `MO` → (0, Monday).
fn by_day(s: &str) -> Option<(i32, chrono::Weekday)> {
    use chrono::Weekday::*;
    let s = s.trim().to_ascii_uppercase();
    let (n, wd) = s.split_at(s.len().checked_sub(2)?);
    let wd = match wd {
        "MO" => Mon,
        "TU" => Tue,
        "WE" => Wed,
        "TH" => Thu,
        "FR" => Fri,
        "SA" => Sat,
        "SU" => Sun,
        _ => return None,
    };
    Some((if n.is_empty() || n == "+" { 0 } else { n.trim_start_matches('+').parse().ok()? }, wd))
}

fn nth_weekday(y: i32, m: u32, n: i32, wd: chrono::Weekday) -> Option<Date> {
    use chrono::Datelike;
    let days: Vec<Date> = (1..=date::last_day(y, m)).filter_map(|d| date::ymd(y, m, d)).filter(|d| d.weekday() == wd).collect();
    if n > 0 { days.get(n as usize - 1).copied() } else { days.iter().rev().nth((-n) as usize - 1).copied() }
}

/// The days an event happens on between `from` and `to` (inclusive): its start (and the rest
/// of a multi-day event), repeated by a yearly, monthly or weekly RRULE (INTERVAL, COUNT,
/// UNTIL, and yearly BYMONTH + BYDAY like `4TH`).
pub fn occurrences(e: &Event, from: Date, to: Date) -> Vec<Date> {
    use chrono::Datelike;
    let Some(start) = e.start else { return Vec::new() };
    let span = e.last.map_or(0, |l| date::days_between(start, l).clamp(0, 60));
    let mut starts: Vec<Date> = Vec::new();
    let rule = e.rrule.as_str();
    let freq = rule_part(rule, "FREQ").map(|f| f.to_ascii_uppercase());
    let interval: i32 = rule_part(rule, "INTERVAL").and_then(|v| v.parse().ok()).filter(|n| *n > 0).unwrap_or(1);
    let count: Option<usize> = rule_part(rule, "COUNT").and_then(|v| v.parse().ok());
    let until = rule_part(rule, "UNTIL").and_then(|v| chrono::NaiveDate::parse_from_str(v.get(..8)?, "%Y%m%d").ok());
    let end = until.map_or(to, |u| u.min(to));
    match freq.as_deref() {
        Some("YEARLY") => {
            let month: u32 = rule_part(rule, "BYMONTH").and_then(|v| v.parse().ok()).unwrap_or(start.month());
            let byday = rule_part(rule, "BYDAY").and_then(by_day).filter(|(n, _)| *n != 0);
            let mut y = start.year();
            while y <= end.year() {
                let d = match byday {
                    Some((n, wd)) => nth_weekday(y, month, n, wd),
                    None => date::ymd(y, month, start.day().min(date::last_day(y, month))),
                };
                if let Some(d) = d.filter(|d| *d >= start) {
                    starts.push(d);
                }
                if count.is_some_and(|c| starts.len() >= c) {
                    break;
                }
                y += interval;
            }
        }
        Some("MONTHLY") => {
            let mut k = 0;
            loop {
                let d = date::add_months(start, k * interval);
                if d > end || count.is_some_and(|c| starts.len() >= c) {
                    break;
                }
                starts.push(d);
                k += 1;
            }
        }
        Some("WEEKLY") | Some("DAILY") => {
            let step = if freq.as_deref() == Some("WEEKLY") { 7 } else { 1 } * interval as i64;
            // jump close to `from` instead of walking from a start decades back
            let skip = if count.is_none() && from > start { (date::days_between(start, from) - span).max(0) / step } else { 0 };
            let mut d = date::add_days(start, skip * step);
            let mut n = skip as usize;
            while d <= end && count.is_none_or(|c| n < c) {
                starts.push(d);
                d = date::add_days(d, step);
                n += 1;
            }
        }
        _ => starts.push(start),
    }
    let mut out: Vec<Date> = starts.into_iter().filter(|d| *d <= end).flat_map(|s| (0..=span).map(move |k| date::add_days(s, k))).filter(|d| *d >= from && *d <= to).collect();
    out.sort();
    out.dedup();
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

    #[test]
    fn recurring_events() {
        let text = "BEGIN:VCALENDAR\r\nX-WR-CALNAME:Festivos\r\nBEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Navidad\r\nDTSTART;VALUE=DATE:20201225\r\nDTEND;VALUE=DATE:20201226\r\nRRULE:FREQ=YEARLY\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:b\r\nSUMMARY:Thanksgiving\r\nDTSTART;VALUE=DATE:20211125\r\nRRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=4TH\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:c\r\nSUMMARY:Feria\r\nDTSTART;VALUE=DATE:20260803\r\nDTEND;VALUE=DATE:20260806\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:d\r\nSUMMARY:Junta\r\nDTSTART:20260105T140000Z\r\nRRULE:FREQ=MONTHLY;INTERVAL=3;COUNT=3\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:e\r\nSUMMARY:Gym\r\nDTSTART;VALUE=DATE:20200106\r\nRRULE:FREQ=WEEKLY;UNTIL=20261130\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let ev = parse(text);
        assert_eq!(ev.len(), 5);
        let (from, to) = (ymd(2026, 1, 1).unwrap(), ymd(2027, 12, 31).unwrap());
        let iso = |v: Vec<Date>| v.into_iter().map(date::iso).collect::<Vec<_>>();
        assert_eq!(iso(occurrences(&ev[0], from, to)), ["2026-12-25", "2027-12-25"]);
        assert_eq!(iso(occurrences(&ev[1], from, to)), ["2026-11-26", "2027-11-25"]);
        assert_eq!(iso(occurrences(&ev[2], from, to)), ["2026-08-03", "2026-08-04", "2026-08-05"]);
        assert_eq!(iso(occurrences(&ev[3], from, to)), ["2026-01-05", "2026-04-05", "2026-07-05"]);
        let gym = occurrences(&ev[4], ymd(2026, 11, 1).unwrap(), to);
        assert_eq!(iso(gym), ["2026-11-02", "2026-11-09", "2026-11-16", "2026-11-23", "2026-11-30"]);
        assert_eq!(occurrences(&ev[0], ymd(2019, 1, 1).unwrap(), ymd(2019, 12, 31).unwrap()), Vec::<Date>::new());
        assert_eq!(super::by_day("-1MO"), Some((-1, chrono::Weekday::Mon)));
    }
}
