//! CSV export / import of items (one row per item, ISO dates, `;`-separated tags and lead
//! days inside their cells).
use crate::date;
use crate::model::*;

pub const COLUMNS: &[&str] = &[
    "id", "kind", "name", "owner", "category", "issued", "expires", "recurrence", "term_months", "notice_days", "cost", "currency", "provider", "reference", "renewal_url", "auto_renews", "tags", "lead_days", "state", "host", "auto_check", "notes",
];

fn quote(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

pub fn export(items: &[Item]) -> String {
    let mut out = COLUMNS.join(",");
    out.push('\n');
    for i in items {
        let cells = [
            i.id.to_string(),
            i.kind.id().into(),
            i.name.clone(),
            i.owner.clone(),
            i.category.id().into(),
            opt(i.issued.map(date::iso)),
            opt(i.expires.map(date::iso)),
            i.recurrence.id(),
            opt(i.term_months),
            opt(i.notice_days),
            opt(i.cost),
            i.currency.clone(),
            i.provider.clone(),
            i.reference.clone(),
            i.renewal_url.clone(),
            (i.auto_renews as u8).to_string(),
            i.tags.join(";"),
            i.lead_days.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(";"),
            i.state.id().into(),
            i.host.clone(),
            (i.auto_check as u8).to_string(),
            i.notes.clone(),
        ];
        out.push_str(&cells.iter().map(|c| quote(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

/// RFC 4180-ish: quoted cells may hold commas, quotes ("") and newlines.
pub fn parse_rows(s: &str) -> Vec<Vec<String>> {
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = s.chars().peekable();
    // `;` is the separator Excel uses in comma-decimal locales
    let sep = s.lines().next().map(|h| if h.matches(';').count() > h.matches(',').count() { ';' } else { ',' }).unwrap_or(',');
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' && cell.is_empty() {
            quoted = true;
        } else if c == sep {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            if !(row.len() == 1 && row[0].is_empty()) {
                rows.push(std::mem::take(&mut row));
            }
            row.clear();
        } else {
            cell.push(c);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    pub items: Vec<Item>,
    /// 1-based line numbers of rows that were skipped (no name).
    pub skipped: Vec<usize>,
}

/// Rows → items. Columns are matched by header name (any order, unknown ones ignored);
/// `kind` and `name` are enough. Dates may be ISO or `dd/mm/yyyy`. Ids are not kept.
pub fn import(s: &str, today: date::Date) -> Imported {
    let rows = parse_rows(s);
    let Some((header, body)) = rows.split_first() else { return Imported::default() };
    let col = |name: &str| header.iter().position(|h| h.trim().eq_ignore_ascii_case(name));
    let idx: Vec<Option<usize>> = COLUMNS.iter().map(|c| col(c)).collect();
    let mut out = Imported::default();
    for (n, r) in body.iter().enumerate() {
        let get = |name: &str| -> String {
            let k = COLUMNS.iter().position(|c| *c == name).and_then(|k| idx[k]);
            k.and_then(|k| r.get(k)).map(|v| v.trim().to_string()).unwrap_or_default()
        };
        let name = get("name");
        if name.is_empty() {
            out.skipped.push(n + 2);
            continue;
        }
        let kind = Kind::from_id(&get("kind")).unwrap_or(Kind::Custom);
        let mut it = Item::new(kind, name);
        let dt = |v: String| date::parse_loose(&v, today, date::Order::DayFirst);
        it.owner = get("owner");
        if let Some(c) = Category::from_id(&get("category")) {
            it.category = c;
        }
        it.issued = dt(get("issued"));
        it.expires = dt(get("expires"));
        if let Some(r) = Recurrence::parse(&get("recurrence")).filter(|_| idx[7].is_some()) {
            it.recurrence = r;
        }
        if idx[8].is_some() {
            it.term_months = get("term_months").parse().ok();
        }
        it.notice_days = get("notice_days").parse().ok();
        it.cost = get("cost").replace(',', ".").parse().ok();
        it.currency = get("currency");
        it.provider = get("provider");
        it.reference = get("reference");
        it.renewal_url = get("renewal_url");
        it.auto_renews = matches!(get("auto_renews").to_lowercase().as_str(), "1" | "true" | "yes" | "si" | "sí");
        it.tags = get("tags").split(';').map(str::trim).filter(|t| !t.is_empty()).map(String::from).collect();
        if idx[17].is_some() {
            it.lead_days = parse_leads(&get("lead_days"));
            if it.lead_days.is_empty() {
                it.lead_days = kind.lead_days();
            }
        }
        it.state = State::from_id(&get("state"));
        it.host = get("host");
        it.auto_check = matches!(get("auto_check").to_lowercase().as_str(), "1" | "true" | "yes");
        it.notes = get("notes");
        out.items.push(it);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    #[test]
    fn roundtrip() {
        let today = ymd(2026, 11, 20).unwrap();
        let mut a = Item::new(Kind::Domain, "otherwisetech.com");
        a.expires = ymd(2027, 8, 13);
        a.host = "otherwisetech.com".into();
        a.auto_check = true;
        a.auto_renews = true;
        a.cost = Some(12.5);
        a.currency = "USD".into();
        a.tags = vec!["work".into(), "web".into()];
        a.notes = "line one, with \"quotes\"\nline two".into();
        let mut b = Item::new(Kind::Contract, "Arriendo, apto 301");
        b.owner = " Ana ".trim().into();
        b.expires = ymd(2027, 1, 31);
        b.notice_days = Some(60);
        b.state = State::InProgress;
        b.recurrence = Recurrence::Yearly { month: 1, day: 31 };
        let csv = export(&[a.clone(), b.clone()]);
        let back = import(&csv, today);
        assert!(back.skipped.is_empty());
        assert_eq!(back.items, vec![a, b]);
    }

    #[test]
    fn foreign_files() {
        let today = ymd(2026, 11, 20).unwrap();
        let s = "\u{feff}Name;Expires;Kind;Cost\r\nSOAT moto;31/03/2027;soat;350000,5\r\n;2027-01-01;custom;\r\n\"Seguro; hogar\";2027-06-30;insurance;\r\n";
        let r = import(s, today);
        assert_eq!(r.skipped, vec![3]);
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.items[0].kind, Kind::Soat);
        assert_eq!(r.items[0].expires, ymd(2027, 3, 31));
        assert_eq!(r.items[0].cost, Some(350000.5));
        assert_eq!(r.items[0].lead_days, Kind::Soat.lead_days());
        assert_eq!(r.items[1].name, "Seguro; hogar");
        assert_eq!(import("", today), Imported::default());
    }
}
