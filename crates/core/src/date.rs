//! Calendar math and loose date parsing. Everything works on plain calendar dates
//! (`NaiveDate`): an expiry is a day, not an instant.
use chrono::{Datelike, Duration, NaiveDate};

pub type Date = NaiveDate;

/// Today in this computer's time zone.
pub fn today() -> Date {
    chrono::Local::now().date_naive()
}

pub fn ymd(y: i32, m: u32, d: u32) -> Option<Date> {
    NaiveDate::from_ymd_opt(y, m, d)
}

/// Adds months, clamping to the month's last day (Jan 31 + 1 month = Feb 28 / 29).
pub fn add_months(d: Date, months: i32) -> Date {
    let total = d.year() * 12 + d.month0() as i32 + months;
    let (y, m0) = (total.div_euclid(12), total.rem_euclid(12) as u32);
    let last = last_day(y, m0 + 1);
    NaiveDate::from_ymd_opt(y, m0 + 1, d.day().min(last)).unwrap_or(d)
}

pub fn add_years(d: Date, years: i32) -> Date {
    add_months(d, years * 12)
}

pub fn add_days(d: Date, days: i64) -> Date {
    d.checked_add_signed(Duration::days(days)).unwrap_or(d)
}

pub fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn last_day(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

/// Days from `a` to `b` (negative when `b` is earlier).
pub fn days_between(a: Date, b: Date) -> i64 {
    (b - a).num_days()
}

/// First day of `d`'s month.
pub fn month_start(d: Date) -> Date {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d)
}

/// ISO `2027-03-31`.
pub fn iso(d: Date) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn parse_iso(s: &str) -> Option<Date> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

/// Month names and abbreviations in the bundled languages (lowercase, without accents).
const MONTHS: &[(&str, u32)] = &[
    ("january", 1), ("enero", 1), ("janeiro", 1), ("janvier", 1), ("jan", 1), ("ene", 1), ("janv", 1),
    ("february", 2), ("febrero", 2), ("fevereiro", 2), ("fevrier", 2), ("feb", 2), ("fev", 2), ("fevr", 2),
    ("march", 3), ("marzo", 3), ("marco", 3), ("mars", 3), ("mar", 3),
    ("april", 4), ("abril", 4), ("avril", 4), ("apr", 4), ("abr", 4), ("avr", 4),
    ("may", 5), ("mayo", 5), ("maio", 5), ("mai", 5),
    ("june", 6), ("junio", 6), ("junho", 6), ("juin", 6), ("jun", 6),
    ("july", 7), ("julio", 7), ("julho", 7), ("juillet", 7), ("jul", 7), ("juil", 7),
    ("august", 8), ("agosto", 8), ("aout", 8), ("aug", 8), ("ago", 8),
    ("september", 9), ("septiembre", 9), ("setiembre", 9), ("setembro", 9), ("septembre", 9), ("sep", 9), ("sept", 9), ("set", 9),
    ("october", 10), ("octubre", 10), ("outubro", 10), ("octobre", 10), ("oct", 10), ("out", 10),
    ("november", 11), ("noviembre", 11), ("novembro", 11), ("novembre", 11), ("nov", 11),
    ("december", 12), ("diciembre", 12), ("dezembro", 12), ("decembre", 12), ("dec", 12), ("dic", 12), ("dez", 12),
];

pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

pub fn month_from_word(w: &str) -> Option<u32> {
    let w = fold(w.trim_end_matches('.'));
    MONTHS.iter().find(|(n, _)| *n == w).map(|(_, m)| *m)
}

/// Two-digit years: 00–69 → 20xx, 70–99 → 19xx.
fn full_year(y: i32) -> i32 {
    if y >= 100 {
        y
    } else if y < 70 {
        2000 + y
    } else {
        1900 + y
    }
}

/// Order of an all-numeric `a/b/yyyy` date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// 31/12/2027 (es, pt, fr and most of the world)
    DayFirst,
    /// 12/31/2027 (en-US)
    MonthFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok<'a> {
    Num(&'a str),
    Word(&'a str),
    Sep(char),
}

fn tokens(s: &str) -> Vec<(Tok<'_>, usize, usize)> {
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if c.is_ascii_digit() {
            let mut end = i + c.len_utf8();
            while let Some(&(j, d)) = it.peek() {
                if !d.is_ascii_digit() {
                    break;
                }
                end = j + d.len_utf8();
                it.next();
            }
            out.push((Tok::Num(&s[i..end]), i, end));
        } else if c.is_alphabetic() {
            let mut end = i + c.len_utf8();
            while let Some(&(j, d)) = it.peek() {
                if !d.is_alphabetic() {
                    break;
                }
                end = j + d.len_utf8();
                it.next();
            }
            out.push((Tok::Word(&s[i..end]), i, end));
        } else if !c.is_whitespace() {
            out.push((Tok::Sep(c), i, i + c.len_utf8()));
        }
    }
    out
}

/// A date found in free text: the date and its byte span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub date: Date,
    pub start: usize,
    pub end: usize,
}

/// Every absolute date in `s`: `2027-03-31`, `31/03/2027`, `31.03.27`, `03/2027` (end of that
/// month, as on cards), `31 mar 2027`, `31 de marzo de 2027`, `March 31, 2027`, `mar 2027`.
pub fn find_dates(s: &str, order: Order) -> Vec<Found> {
    let t = tokens(s);
    let mut out = Vec::new();
    let mut i = 0;
    let num = |k: usize| match t.get(k) {
        Some((Tok::Num(n), _, _)) => n.parse::<i32>().ok().map(|v| (v, n.len())),
        _ => None,
    };
    let sep = |k: usize| match t.get(k) {
        Some((Tok::Sep(c), _, _)) if matches!(c, '/' | '-' | '.') => Some(*c),
        _ => None,
    };
    let word = |k: usize| match t.get(k) {
        Some((Tok::Word(w), _, _)) => Some(*w),
        _ => None,
    };
    let is_filler = |w: &str| matches!(fold(w).as_str(), "de" | "del" | "of" | "the");
    let is_ordinal = |w: &str| matches!(fold(w).as_str(), "er" | "st" | "nd" | "rd" | "th" | "o");
    while i < t.len() {
        let start = t[i].1;
        // yyyy-mm-dd / yyyy/mm/dd
        if let (Some((y, 4)), Some(s1), Some((m, _)), Some(s2), Some((d, _))) = (num(i), sep(i + 1), num(i + 2), sep(i + 3), num(i + 4))
            && s1 == s2
            && let Some(date) = ymd(y, m as u32, d as u32)
        {
            out.push(Found { date, start, end: t[i + 4].2 });
            i += 5;
            continue;
        }
        // a/b/yyyy or a.b.yy
        if let (Some((a, la)), Some(s1), Some((b, lb)), Some(s2), Some((y, ly))) = (num(i), sep(i + 1), num(i + 2), sep(i + 3), num(i + 4))
            && s1 == s2
            && la <= 2
            && lb <= 2
            && (ly == 4 || ly == 2)
        {
            let y = full_year(y);
            let (d, m) = match order {
                Order::DayFirst if a <= 31 && b <= 12 => (a, b),
                Order::DayFirst => (b, a),
                Order::MonthFirst if a <= 12 && b <= 31 => (b, a),
                Order::MonthFirst => (a, b),
            };
            if let Some(date) = ymd(y, m as u32, d as u32) {
                out.push(Found { date, start, end: t[i + 4].2 });
                i += 5;
                continue;
            }
        }
        // mm/yyyy → last day of that month
        if let (Some((m, lm)), Some('/' | '-'), Some((y, 4))) = (num(i), sep(i + 1), num(i + 2))
            && lm <= 2
            && (1..=12).contains(&m)
            && sep(i + 3).is_none()
            && (i == 0 || sep(i - 1).is_none())
            && let Some(date) = ymd(y, m as u32, last_day(y, m as u32))
        {
            out.push(Found { date, start, end: t[i + 2].2 });
            i += 3;
            continue;
        }
        // 31 [de] marzo [de] 2027
        if let Some((d, ld)) = num(i)
            && ld <= 2
        {
            let mut k = i + 1;
            if t.get(k).is_some_and(|x| x.1 == t[i].2) && word(k).is_some_and(is_ordinal) {
                k += 1;
            }
            if word(k).is_some_and(is_filler) {
                k += 1;
            }
            if sep(k) == Some('-') || sep(k) == Some('.') {
                k += 1;
            }
            if let Some(m) = word(k).and_then(month_from_word) {
                let mut j = k + 1;
                if sep(j).is_some() {
                    j += 1;
                }
                if word(j).is_some_and(is_filler) {
                    j += 1;
                }
                if let Some((y, ly)) = num(j)
                    && (ly == 4 || ly == 2)
                    && let Some(date) = ymd(full_year(y), m, d as u32)
                {
                    out.push(Found { date, start, end: t[j].2 });
                    i = j + 1;
                    continue;
                }
            }
        }
        // March 31, 2027 / mar 2027
        if let Some(m) = word(i).and_then(month_from_word) {
            let mut k = i + 1;
            if word(k).is_some_and(is_filler) {
                k += 1;
            }
            if let Some((d, ld)) = num(k)
                && ld <= 2
            {
                let mut j = k + 1;
                if t.get(j).is_some_and(|x| x.1 == t[k].2) && word(j).is_some_and(is_ordinal) {
                    j += 1;
                }
                if matches!(t.get(j), Some((Tok::Sep(','), _, _))) {
                    j += 1;
                }
                if let Some((y, 4)) = num(j)
                    && let Some(date) = ymd(y, m, d as u32)
                {
                    out.push(Found { date, start, end: t[j].2 });
                    i = j + 1;
                    continue;
                }
            }
            if let Some((y, 4)) = num(k)
                && let Some(date) = ymd(y, m, last_day(y, m))
            {
                out.push(Found { date, start, end: t[k].2 });
                i = k + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Relative phrases: `in 3 months`, `en 2 semanas`, `em 10 dias`, `dans 1 an`, `+6m`,
/// `tomorrow` / `mañana` / `amanhã` / `demain`, `next year` / `el próximo año`.
pub fn find_relative(s: &str, today: Date) -> Option<Found> {
    let t = tokens(s);
    let unit = |w: &str| -> Option<char> {
        match fold(w).as_str() {
            "day" | "days" | "dia" | "dias" | "jour" | "jours" | "d" => Some('d'),
            "week" | "weeks" | "semana" | "semanas" | "semaine" | "semaines" | "w" => Some('w'),
            "month" | "months" | "mes" | "meses" | "mois" | "m" => Some('m'),
            "year" | "years" | "ano" | "anos" | "an" | "ans" | "annee" | "annees" | "y" => Some('y'),
            _ => None,
        }
    };
    let apply = |n: i64, u: char| match u {
        'd' => add_days(today, n),
        'w' => add_days(today, n * 7),
        'm' => add_months(today, n as i32),
        _ => add_years(today, n as i32),
    };
    for (k, (tok, start, end)) in t.iter().enumerate() {
        match tok {
            Tok::Word(w) => {
                let f = fold(w);
                if matches!(f.as_str(), "tomorrow" | "manana" | "amanha" | "demain") {
                    return Some(Found { date: add_days(today, 1), start: *start, end: *end });
                }
                if matches!(f.as_str(), "today" | "hoy" | "hoje" | "aujourd") {
                    return Some(Found { date: today, start: *start, end: *end });
                }
                if matches!(f.as_str(), "in" | "en" | "em" | "dans" | "within")
                    && let Some((Tok::Num(n), _, _)) = t.get(k + 1)
                    && let Some((Tok::Word(u), _, uend)) = t.get(k + 2)
                    && let (Ok(n), Some(u)) = (n.parse::<i64>(), unit(u))
                {
                    return Some(Found { date: apply(n, u), start: *start, end: *uend });
                }
                if matches!(f.as_str(), "next" | "proximo" | "proxima" | "prochain" | "prochaine")
                    && let Some((Tok::Word(u), _, uend)) = t.get(k + 1)
                    && let Some(u) = unit(u)
                {
                    let s0 = if k > 0 && matches!(t[k - 1].0, Tok::Word(w) if matches!(fold(w).as_str(), "el" | "la" | "o" | "a" | "le")) { t[k - 1].1 } else { *start };
                    return Some(Found { date: apply(1, u), start: s0, end: *uend });
                }
            }
            Tok::Sep('+') => {
                if let Some((Tok::Num(n), _, nend)) = t.get(k + 1)
                    && let Ok(n) = n.parse::<i64>()
                    && let Some((Tok::Word(u), _, uend)) = t.get(k + 2)
                    && *nend == t[k + 2].1
                    && let Some(u) = unit(u)
                {
                    return Some(Found { date: apply(n, u), start: *start, end: *uend });
                }
            }
            _ => {}
        }
    }
    None
}

/// One date typed by the user: absolute (any format above) or relative.
pub fn parse_loose(s: &str, today: Date, order: Order) -> Option<Date> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(d) = parse_iso(s) {
        return Some(d);
    }
    find_dates(s, order).first().map(|f| f.date).or_else(|| find_relative(s, today).map(|f| f.date))
}

/// A date as typed in a form in that order: `12/05/2031` (day first) or `05/12/2031`.
pub fn short(d: Date, order: Order) -> String {
    match order {
        Order::DayFirst => format!("{:02}/{:02}/{}", d.day(), d.month(), d.year()), // i18n-ok: date format
        Order::MonthFirst => format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()), // i18n-ok: date format
    }
}

/// A day and month without a year (`24/12`, `12-24` in month-first order): `(month, day)`.
pub fn parse_day_month(s: &str, order: Order) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.trim().split(['/', '-', '.', ' ']).filter(|p| !p.is_empty()).collect();
    let [a, b] = parts.as_slice() else { return None };
    let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
    let (month, day) = match order {
        Order::DayFirst => (b, a),
        Order::MonthFirst => (a, b),
    };
    ((1..=12).contains(&month) && (1..=last_day(2028, month)).contains(&day)).then_some((month, day))
}

/// The day and month typed in `order`: `24/12` → `24/12`, `12/24` in month-first.
pub fn short_day_month(month: u32, day: u32, order: Order) -> String {
    match order {
        Order::DayFirst => format!("{day:02}/{month:02}"), // i18n-ok: date format
        Order::MonthFirst => format!("{month:02}/{day:02}"), // i18n-ok: date format
    }
}

/// The numeric order people use in a UI language.
pub fn order_for(lang: &str) -> Order {
    if lang == "en" { Order::MonthFirst } else { Order::DayFirst }
}

/// The deadline of a Wyoming annual report: the first day of the formation (anniversary) month,
/// the first one on or after `from`.
pub fn wyoming_next(formation_month: u32, from: Date) -> Date {
    let m = formation_month.clamp(1, 12);
    let this_year = ymd(from.year(), m, 1).unwrap_or(from);
    if this_year >= from { this_year } else { ymd(from.year() + 1, m, 1).unwrap_or(from) }
}

/// The next `month`/`day` on or after `from` (Feb 29 falls back to Feb 28 in common years).
pub fn yearly_next(month: u32, day: u32, from: Date) -> Date {
    let on = |y: i32| ymd(y, month, day.min(last_day(y, month))).unwrap_or(from);
    let d = on(from.year());
    if d >= from { d } else { on(from.year() + 1) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_dates_follow_the_order() {
        let d = ymd(2031, 5, 12).unwrap();
        assert_eq!(short(d, Order::DayFirst), "12/05/2031");
        assert_eq!(short(d, Order::MonthFirst), "05/12/2031");
        for o in [Order::DayFirst, Order::MonthFirst] {
            assert_eq!(parse_loose(&short(d, o), d, o), Some(d), "round trip {o:?}");
        }
        assert_eq!(parse_day_month("24/12", Order::DayFirst), Some((12, 24)));
        assert_eq!(parse_day_month("12-24", Order::MonthFirst), Some((12, 24)));
        assert_eq!(parse_day_month("29/02", Order::DayFirst), Some((2, 29)));
        assert_eq!(parse_day_month("31/02", Order::DayFirst), None);
        assert_eq!(parse_day_month("24/12/1990", Order::DayFirst), None);
        assert_eq!(short_day_month(12, 24, Order::DayFirst), "24/12");
    }

    fn d(y: i32, m: u32, dd: u32) -> Date {
        ymd(y, m, dd).unwrap()
    }

    #[test]
    fn month_ends_and_leap_years() {
        assert_eq!(add_months(d(2027, 1, 31), 1), d(2027, 2, 28));
        assert_eq!(add_months(d(2028, 1, 31), 1), d(2028, 2, 29));
        assert_eq!(add_months(d(2028, 2, 29), 12), d(2029, 2, 28));
        assert_eq!(add_years(d(2028, 2, 29), 4), d(2032, 2, 29));
        assert_eq!(add_months(d(2027, 3, 31), -1), d(2027, 2, 28));
        assert_eq!(add_months(d(2027, 12, 15), 1), d(2028, 1, 15));
        assert_eq!(add_months(d(2027, 1, 15), -13), d(2025, 12, 15));
        assert!(is_leap(2000) && !is_leap(1900) && is_leap(2028) && !is_leap(2027));
        assert_eq!(last_day(2100, 2), 28);
        assert_eq!(days_between(d(2027, 2, 28), d(2027, 3, 1)), 1);
        assert_eq!(days_between(d(2028, 2, 28), d(2028, 3, 1)), 2);
    }

    #[test]
    fn wyoming_and_yearly_rules() {
        // formed in March: due March 1st every year
        assert_eq!(wyoming_next(3, d(2026, 11, 20)), d(2027, 3, 1));
        assert_eq!(wyoming_next(3, d(2027, 3, 1)), d(2027, 3, 1));
        assert_eq!(wyoming_next(3, d(2027, 3, 2)), d(2028, 3, 1));
        assert_eq!(wyoming_next(12, d(2026, 11, 20)), d(2026, 12, 1));
        // Cámara de Comercio: by March 31
        assert_eq!(yearly_next(3, 31, d(2026, 11, 20)), d(2027, 3, 31));
        assert_eq!(yearly_next(3, 31, d(2027, 1, 5)), d(2027, 3, 31));
        assert_eq!(yearly_next(2, 29, d(2026, 3, 1)), d(2027, 2, 28));
        assert_eq!(yearly_next(2, 29, d(2027, 3, 1)), d(2028, 2, 29));
    }

    #[test]
    fn absolute_formats() {
        let one = |s: &str| find_dates(s, Order::DayFirst).first().map(|f| f.date);
        assert_eq!(one("vence 2027-03-31"), Some(d(2027, 3, 31)));
        assert_eq!(one("31/03/2027"), Some(d(2027, 3, 31)));
        assert_eq!(one("31.03.27"), Some(d(2027, 3, 31)));
        assert_eq!(one("05/2027"), Some(d(2027, 5, 31)));
        assert_eq!(one("02/2028"), Some(d(2028, 2, 29)));
        assert_eq!(one("12 mar 2027"), Some(d(2027, 3, 12)));
        assert_eq!(one("12 de marzo de 2027"), Some(d(2027, 3, 12)));
        assert_eq!(one("1er août 2030"), Some(d(2030, 8, 1)));
        assert_eq!(one("March 3rd, 2027"), Some(d(2027, 3, 3)));
        assert_eq!(one("1 août 2030"), Some(d(2030, 8, 1)));
        assert_eq!(one("15 DEZ 2029"), Some(d(2029, 12, 15)));
        assert_eq!(one("March 31, 2027"), Some(d(2027, 3, 31)));
        assert_eq!(one("12-JUN-2031"), Some(d(2031, 6, 12)));
        assert_eq!(one("sept 2027"), Some(d(2027, 9, 30)));
        assert_eq!(one("31/02/2027"), None);
        assert_eq!(one("no date here"), None);
        // the day can't be 13+ as a month: swapped automatically
        assert_eq!(one("12/31/2027"), Some(d(2027, 12, 31)));
        assert_eq!(find_dates("03/04/2027", Order::MonthFirst)[0].date, d(2027, 3, 4));
        let all = find_dates("Issued 01/02/2020 Expires 01/02/2030", Order::DayFirst);
        assert_eq!(all.iter().map(|f| f.date).collect::<Vec<_>>(), vec![d(2020, 2, 1), d(2030, 2, 1)]);
        let s = "Pasaporte Ana 12/05/2031";
        let f = find_dates(s, Order::DayFirst)[0];
        assert_eq!(&s[f.start..f.end], "12/05/2031");
    }

    #[test]
    fn relative_phrases() {
        let today = d(2026, 11, 20);
        let rel = |s: &str| find_relative(s, today).map(|f| f.date);
        assert_eq!(rel("in 3 months"), Some(d(2027, 2, 20)));
        assert_eq!(rel("en 2 semanas"), Some(d(2026, 12, 4)));
        assert_eq!(rel("em 10 dias"), Some(d(2026, 11, 30)));
        assert_eq!(rel("dans 1 an"), Some(d(2027, 11, 20)));
        assert_eq!(rel("mañana"), Some(d(2026, 11, 21)));
        assert_eq!(rel("el próximo año"), Some(d(2027, 11, 20)));
        assert_eq!(rel("next month"), Some(d(2026, 12, 20)));
        assert_eq!(rel("+6m"), Some(d(2027, 5, 20)));
        assert_eq!(rel("nothing"), None);
        assert_eq!(parse_loose("2031-05-12", today, Order::DayFirst), Some(d(2031, 5, 12)));
        assert_eq!(parse_loose("  ", today, Order::DayFirst), None);
    }
}
