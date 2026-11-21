//! Public holidays computed from their rules (no data files, no network): Colombia (with the
//! Ley Emiliani Monday moves and the Easter-based days), US federal, Wyoming state, Spain
//! (national), Mexico (federal), Brazil (national) and Canada (federal). Plus business-day
//! shifting for deadlines that can't fall on a weekend or a holiday.
//!
//! Holiday names are proper names in the country's own language.
use crate::date::{self, Date};
use chrono::{Datelike, Weekday};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Country {
    Co,
    Us,
    UsWy,
    Es,
    Mx,
    Br,
    Ca,
}

impl Country {
    pub const ALL: [Country; 7] = [Country::Co, Country::Us, Country::UsWy, Country::Es, Country::Mx, Country::Br, Country::Ca];

    pub fn id(self) -> &'static str {
        match self {
            Country::Co => "co",
            Country::Us => "us",
            Country::UsWy => "us-wy",
            Country::Es => "es",
            Country::Mx => "mx",
            Country::Br => "br",
            Country::Ca => "ca",
        }
    }

    pub fn from_id(s: &str) -> Option<Country> {
        let s = s.trim().to_ascii_lowercase();
        Country::ALL.into_iter().find(|c| c.id() == s)
    }

    pub fn label(self) -> String {
        match self {
            Country::Co => i18n::t!("core.country.co"),
            Country::Us => i18n::t!("core.country.us"),
            Country::UsWy => i18n::t!("core.country.us_wy"),
            Country::Es => i18n::t!("core.country.es"),
            Country::Mx => i18n::t!("core.country.mx"),
            Country::Br => i18n::t!("core.country.br"),
            Country::Ca => i18n::t!("core.country.ca"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holiday {
    pub date: Date,
    pub name: &'static str,
    pub country: Country,
}

/// Easter Sunday (Gregorian; anonymous / Meeus algorithm).
pub fn easter(y: i32) -> Date {
    let a = y % 19;
    let b = y / 100;
    let c = y % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    date::ymd(y, month as u32, day as u32).unwrap_or_default()
}

fn d(y: i32, m: u32, day: u32) -> Date {
    date::ymd(y, m, day).unwrap_or_default()
}

/// The `n`th `wd` of the month (n = 1…5), or the last one with `n = -1`.
fn nth(y: i32, m: u32, wd: Weekday, n: i32) -> Date {
    if n < 0 {
        let mut x = d(y, m, date::last_day(y, m));
        while x.weekday() != wd {
            x = date::add_days(x, -1);
        }
        return x;
    }
    let mut x = d(y, m, 1);
    while x.weekday() != wd {
        x = date::add_days(x, 1);
    }
    date::add_days(x, 7 * (n as i64 - 1))
}

/// Ley Emiliani: the holiday moves to the next Monday unless it is one.
fn next_monday(x: Date) -> Date {
    let back = x.weekday().num_days_from_monday() as i64;
    if back == 0 { x } else { date::add_days(x, 7 - back) }
}

/// US federal rule: Saturday is observed on Friday, Sunday on Monday.
fn observed(x: Date) -> Date {
    match x.weekday() {
        Weekday::Sat => date::add_days(x, -1),
        Weekday::Sun => date::add_days(x, 1),
        _ => x,
    }
}

pub fn holidays(c: Country, y: i32) -> Vec<Holiday> {
    let e = easter(y);
    let ed = |n: i64| date::add_days(e, n);
    let v: Vec<(Date, &'static str)> = match c {
        Country::Co => vec![
            (d(y, 1, 1), "Año Nuevo"),                                         // i18n-ok: holiday name
            (next_monday(d(y, 1, 6)), "Día de los Reyes Magos"),               // i18n-ok: holiday name
            (next_monday(d(y, 3, 19)), "Día de San José"),                     // i18n-ok: holiday name
            (ed(-3), "Jueves Santo"),                                          // i18n-ok: holiday name
            (ed(-2), "Viernes Santo"),                                         // i18n-ok: holiday name
            (d(y, 5, 1), "Día del Trabajo"),                                   // i18n-ok: holiday name
            (ed(43), "Ascensión del Señor"),                                   // i18n-ok: holiday name
            (ed(64), "Corpus Christi"),                                        // i18n-ok: holiday name
            (ed(71), "Sagrado Corazón"),                                       // i18n-ok: holiday name
            (next_monday(d(y, 6, 29)), "San Pedro y San Pablo"),               // i18n-ok: holiday name
            (d(y, 7, 20), "Día de la Independencia"),                          // i18n-ok: holiday name
            (d(y, 8, 7), "Batalla de Boyacá"),                                 // i18n-ok: holiday name
            (next_monday(d(y, 8, 15)), "La Asunción de la Virgen"),            // i18n-ok: holiday name
            (next_monday(d(y, 10, 12)), "Día de la Raza"),                     // i18n-ok: holiday name
            (next_monday(d(y, 11, 1)), "Todos los Santos"),                    // i18n-ok: holiday name
            (next_monday(d(y, 11, 11)), "Independencia de Cartagena"),         // i18n-ok: holiday name
            (d(y, 12, 8), "Inmaculada Concepción"),                            // i18n-ok: holiday name
            (d(y, 12, 25), "Navidad"),                                         // i18n-ok: holiday name
        ],
        Country::Us => {
            let mut v = vec![
                (observed(d(y, 1, 1)), "New Year's Day"),                      // i18n-ok: holiday name
                (nth(y, 1, Weekday::Mon, 3), "Martin Luther King Jr. Day"),    // i18n-ok: holiday name
                (nth(y, 2, Weekday::Mon, 3), "Washington's Birthday"),         // i18n-ok: holiday name
                (nth(y, 5, Weekday::Mon, -1), "Memorial Day"),                 // i18n-ok: holiday name
                (observed(d(y, 7, 4)), "Independence Day"),                    // i18n-ok: holiday name
                (nth(y, 9, Weekday::Mon, 1), "Labor Day"),                     // i18n-ok: holiday name
                (nth(y, 10, Weekday::Mon, 2), "Columbus Day"),                 // i18n-ok: holiday name
                (observed(d(y, 11, 11)), "Veterans Day"),                      // i18n-ok: holiday name
                (nth(y, 11, Weekday::Thu, 4), "Thanksgiving Day"),             // i18n-ok: holiday name
                (observed(d(y, 12, 25)), "Christmas Day"),                     // i18n-ok: holiday name
            ];
            if y >= 2021 {
                v.insert(4, (observed(d(y, 6, 19)), "Juneteenth"));            // i18n-ok: holiday name
            }
            v
        }
        // Wyoming state offices: the federal days without Columbus Day and Juneteenth
        Country::UsWy => vec![
            (observed(d(y, 1, 1)), "New Year's Day"),                          // i18n-ok: holiday name
            (nth(y, 1, Weekday::Mon, 3), "Martin Luther King Jr./Wyoming Equality Day"), // i18n-ok: holiday name
            (nth(y, 2, Weekday::Mon, 3), "Presidents' Day"),                   // i18n-ok: holiday name
            (nth(y, 5, Weekday::Mon, -1), "Memorial Day"),                     // i18n-ok: holiday name
            (observed(d(y, 7, 4)), "Independence Day"),                        // i18n-ok: holiday name
            (nth(y, 9, Weekday::Mon, 1), "Labor Day"),                         // i18n-ok: holiday name
            (observed(d(y, 11, 11)), "Veterans Day"),                          // i18n-ok: holiday name
            (nth(y, 11, Weekday::Thu, 4), "Thanksgiving Day"),                 // i18n-ok: holiday name
            (observed(d(y, 12, 25)), "Christmas Day"),                         // i18n-ok: holiday name
        ],
        Country::Es => vec![
            (d(y, 1, 1), "Año Nuevo"),                                         // i18n-ok: holiday name
            (d(y, 1, 6), "Epifanía del Señor"),                                // i18n-ok: holiday name
            (ed(-2), "Viernes Santo"),                                         // i18n-ok: holiday name
            (d(y, 5, 1), "Fiesta del Trabajo"),                                // i18n-ok: holiday name
            (d(y, 8, 15), "Asunción de la Virgen"),                            // i18n-ok: holiday name
            (d(y, 10, 12), "Fiesta Nacional de España"),                       // i18n-ok: holiday name
            (d(y, 11, 1), "Todos los Santos"),                                 // i18n-ok: holiday name
            (d(y, 12, 6), "Día de la Constitución"),                           // i18n-ok: holiday name
            (d(y, 12, 8), "Inmaculada Concepción"),                            // i18n-ok: holiday name
            (d(y, 12, 25), "Navidad"),                                         // i18n-ok: holiday name
        ],
        Country::Mx => vec![
            (d(y, 1, 1), "Año Nuevo"),                                         // i18n-ok: holiday name
            (nth(y, 2, Weekday::Mon, 1), "Día de la Constitución"),            // i18n-ok: holiday name
            (nth(y, 3, Weekday::Mon, 3), "Natalicio de Benito Juárez"),        // i18n-ok: holiday name
            (d(y, 5, 1), "Día del Trabajo"),                                   // i18n-ok: holiday name
            (d(y, 9, 16), "Día de la Independencia"),                          // i18n-ok: holiday name
            (nth(y, 11, Weekday::Mon, 3), "Día de la Revolución"),             // i18n-ok: holiday name
            (d(y, 12, 25), "Navidad"),                                         // i18n-ok: holiday name
        ],
        Country::Br => {
            let mut v = vec![
                (d(y, 1, 1), "Confraternização Universal"),                    // i18n-ok: holiday name
                (ed(-2), "Sexta-feira Santa"),                                 // i18n-ok: holiday name
                (d(y, 4, 21), "Tiradentes"),                                   // i18n-ok: holiday name
                (d(y, 5, 1), "Dia do Trabalho"),                               // i18n-ok: holiday name
                (d(y, 9, 7), "Independência do Brasil"),                       // i18n-ok: holiday name
                (d(y, 10, 12), "Nossa Senhora Aparecida"),                     // i18n-ok: holiday name
                (d(y, 11, 2), "Finados"),                                      // i18n-ok: holiday name
                (d(y, 11, 15), "Proclamação da República"),                    // i18n-ok: holiday name
                (d(y, 12, 25), "Natal"),                                       // i18n-ok: holiday name
            ];
            if y >= 2024 {
                v.insert(8, (d(y, 11, 20), "Dia Nacional de Zumbi e da Consciência Negra")); // i18n-ok: holiday name
            }
            v
        }
        Country::Ca => {
            // Victoria Day: the Monday before May 25
            let victoria = next_monday(d(y, 5, 18));
            let mut v = vec![
                (d(y, 1, 1), "New Year's Day"),                                // i18n-ok: holiday name
                (ed(-2), "Good Friday"),                                       // i18n-ok: holiday name
                (victoria, "Victoria Day"),                                    // i18n-ok: holiday name
                (d(y, 7, 1), "Canada Day"),                                    // i18n-ok: holiday name
                (nth(y, 9, Weekday::Mon, 1), "Labour Day"),                    // i18n-ok: holiday name
                (nth(y, 10, Weekday::Mon, 2), "Thanksgiving"),                 // i18n-ok: holiday name
                (d(y, 11, 11), "Remembrance Day"),                             // i18n-ok: holiday name
                (d(y, 12, 25), "Christmas Day"),                               // i18n-ok: holiday name
                (d(y, 12, 26), "Boxing Day"),                                  // i18n-ok: holiday name
            ];
            if y >= 2021 {
                v.insert(5, (d(y, 9, 30), "National Day for Truth and Reconciliation")); // i18n-ok: holiday name
            }
            v
        }
    };
    let mut v: Vec<Holiday> = v.into_iter().map(|(date, name)| Holiday { date, name, country: c }).collect();
    v.sort_by_key(|h| h.date);
    v
}

/// The holidays of `countries` between `from` and `to` (inclusive), by date.
pub fn between(countries: &[Country], from: Date, to: Date) -> Vec<Holiday> {
    let mut v: Vec<Holiday> = countries.iter().flat_map(|c| (from.year()..=to.year() + 1).flat_map(move |y| holidays(*c, y))).filter(|h| h.date >= from && h.date <= to).collect();
    v.sort_by_key(|h| (h.date, h.country));
    v
}

pub fn is_holiday(c: Country, x: Date) -> bool {
    // a Saturday Jan 1 is observed on Dec 31 of the year before
    [x.year(), x.year() + 1].iter().any(|y| holidays(c, *y).iter().any(|h| h.date == x))
}

/// Not a Saturday, Sunday or (when a country is given) a public holiday there.
pub fn is_business_day(x: Date, c: Option<Country>) -> bool {
    !matches!(x.weekday(), Weekday::Sat | Weekday::Sun) && !c.is_some_and(|c| is_holiday(c, x))
}

/// Which way a deadline moves when it falls on a non-business day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shift {
    /// The business day before (filings due "by" a date).
    Previous,
    /// The business day after (most legal deadlines roll forward).
    Next,
}

/// The business-day rule of an item: direction and (optionally) whose holidays count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BusinessDays {
    pub shift: Shift,
    pub country: Option<Country>,
}

impl BusinessDays {
    /// `prev`, `next:co`, `prev:us-wy`; empty means none.
    pub fn id(self) -> String {
        let s = if self.shift == Shift::Previous { "prev" } else { "next" };
        match self.country {
            Some(c) => format!("{s}:{}", c.id()),
            None => s.to_string(),
        }
    }

    pub fn parse(s: &str) -> Option<BusinessDays> {
        let (dir, c) = s.trim().split_once(':').map_or((s.trim(), None), |(a, b)| (a, Some(b)));
        let shift = match dir {
            "prev" => Shift::Previous,
            "next" => Shift::Next,
            _ => return None,
        };
        Some(BusinessDays { shift, country: c.and_then(Country::from_id) })
    }

    pub fn apply(self, x: Date) -> Date {
        let step = if self.shift == Shift::Previous { -1 } else { 1 };
        let mut d = x;
        // a year of non-business days doesn't exist; the bound only guards bad data
        for _ in 0..30 {
            if is_business_day(d, self.country) {
                return d;
            }
            d = date::add_days(d, step);
        }
        x
    }

    pub fn label(self) -> String {
        let c = self.country.map(|c| c.label()).unwrap_or_else(|| i18n::t!("core.biz.weekends_only"));
        match self.shift {
            Shift::Previous => i18n::t!("core.biz.previous", holidays = c),
            Shift::Next => i18n::t!("core.biz.next", holidays = c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dates(c: Country, y: i32) -> Vec<String> {
        let mut v: Vec<String> = holidays(c, y).iter().map(|h| h.date.format("%m-%d").to_string()).collect();
        v.dedup();
        v
    }

    #[test]
    fn easter_dates() {
        let e: Vec<String> = [2024, 2025, 2026, 2027, 2028, 2038].iter().map(|y| date::iso(easter(*y))).collect();
        assert_eq!(e, ["2024-03-31", "2025-04-20", "2026-04-05", "2027-03-28", "2028-04-16", "2038-04-25"]);
    }

    #[test]
    fn colombia() {
        assert_eq!(dates(Country::Co, 2026), ["01-01", "01-12", "03-23", "04-02", "04-03", "05-01", "05-18", "06-08", "06-15", "06-29", "07-20", "08-07", "08-17", "10-12", "11-02", "11-16", "12-08", "12-25"]);
        // 2025: Sagrado Corazón and San Pedro both land on June 30
        assert_eq!(dates(Country::Co, 2025), ["01-01", "01-06", "03-24", "04-17", "04-18", "05-01", "06-02", "06-23", "06-30", "07-20", "08-07", "08-18", "10-13", "11-03", "11-17", "12-08", "12-25"]);
        assert_eq!(holidays(Country::Co, 2025).len(), 18);
        assert_eq!(dates(Country::Co, 2027), ["01-01", "01-11", "03-22", "03-25", "03-26", "05-01", "05-10", "05-31", "06-07", "07-05", "07-20", "08-07", "08-16", "10-18", "11-01", "11-15", "12-08", "12-25"]);
    }

    #[test]
    fn others() {
        assert_eq!(dates(Country::Us, 2026), ["01-01", "01-19", "02-16", "05-25", "06-19", "07-03", "09-07", "10-12", "11-11", "11-26", "12-25"]);
        assert!(is_holiday(Country::Us, d(2027, 12, 31)));
        assert_eq!(dates(Country::Us, 2027)[4..6], ["06-18".to_string(), "07-05".to_string()]);
        assert!(!dates(Country::UsWy, 2026).contains(&"10-12".to_string()));
        assert_eq!(dates(Country::UsWy, 2026).len(), 9);
        assert_eq!(dates(Country::Mx, 2026), ["01-01", "02-02", "03-16", "05-01", "09-16", "11-16", "12-25"]);
        assert_eq!(dates(Country::Ca, 2026)[2], "05-18");
        assert_eq!(dates(Country::Ca, 2025)[2], "05-19");
        assert!(dates(Country::Br, 2026).contains(&"11-20".to_string()));
        assert!(dates(Country::Es, 2026).contains(&"04-03".to_string()));
        assert!(Country::ALL.iter().all(|c| Country::from_id(c.id()) == Some(*c)));
        let span = between(&[Country::Co, Country::Us], d(2026, 12, 1), d(2027, 1, 31));
        assert_eq!(span.iter().map(|h| (date::iso(h.date), h.country)).collect::<Vec<_>>()[..3], [("2026-12-08".to_string(), Country::Co), ("2026-12-25".to_string(), Country::Co), ("2026-12-25".to_string(), Country::Us)]);
    }

    #[test]
    fn business_days() {
        let co = |s| BusinessDays { shift: s, country: Some(Country::Co) };
        // Sat Aug 15 2026 → (holiday Mon 17) → Tue 18, or back to Fri 14
        assert_eq!(co(Shift::Next).apply(d(2026, 8, 15)), d(2026, 8, 18));
        assert_eq!(co(Shift::Previous).apply(d(2026, 8, 15)), d(2026, 8, 14));
        // Holy Thursday and Good Friday 2026: back to Wednesday
        assert_eq!(co(Shift::Previous).apply(d(2026, 4, 3)), d(2026, 4, 1));
        assert_eq!(co(Shift::Next).apply(d(2026, 4, 2)), d(2026, 4, 6));
        let wk = BusinessDays { shift: Shift::Next, country: None };
        assert_eq!(wk.apply(d(2026, 8, 15)), d(2026, 8, 17));
        assert_eq!(wk.apply(d(2026, 8, 18)), d(2026, 8, 18));
        for b in [co(Shift::Next), co(Shift::Previous), wk] {
            assert_eq!(BusinessDays::parse(&b.id()), Some(b));
        }
        assert_eq!(BusinessDays::parse("sideways"), None);
        assert!(is_business_day(d(2026, 11, 20), Some(Country::Co)));
        assert!(!is_business_day(d(2026, 11, 16), Some(Country::Co)));
        assert!(is_business_day(d(2026, 11, 16), Some(Country::Us)));
    }
}
