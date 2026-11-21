//! Reading the useful bits out of a document's text (from OCR or a PDF's text layer): the
//! expiry date, the issue date, the document number and a guess of the type. Passports and ID
//! cards with a machine-readable zone (MRZ) are read from it, check digits included.
use crate::date::{self, Date, Order};
use crate::model::Kind;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Extracted {
    pub kind: Option<Kind>,
    pub expires: Option<Date>,
    pub issued: Option<Date>,
    pub number: Option<String>,
    /// Holder's name from the MRZ (`SURNAME, Given Names`).
    pub holder: Option<String>,
}

/// Words that announce an expiry date, folded (no accents, lowercase).
const EXPIRY_WORDS: &[&str] = &[
    "expir", "vence", "vencimiento", "valid until", "valido hasta", "valida hasta", "valid thru", "valid to", "date of expiry", "fecha de vencimiento", "fecha de expiracion", "validade", "valido ate", "date d'expiration", "date d'exp", "fin de validite", "caduc", "hasta", "until", "expiry", "=exp", "=vto", "=venc",
];
const ISSUE_WORDS: &[&str] = &["issue", "expedicion", "emision", "emitido", "expedido", "expedition", "delivrance", "date de delivrance", "emissao", "fecha de expedicion", "purchase", "compra", "achat", "=iss", "factura", "invoice", "facture"];
const NUMBER_WORDS: &[&str] = &["passport no", "pasaporte no", "document no", "doc no", "numero", "number", "no.", "n°", "nº", "nro", "num", "policy", "poliza", "placa", "plate", "serial", "licencia"];

fn mrz_value(c: char) -> u32 {
    match c {
        '0'..='9' => c as u32 - '0' as u32,
        'A'..='Z' => c as u32 - 'A' as u32 + 10,
        _ => 0,
    }
}

/// ICAO 9303 check digit (weights 7, 3, 1).
pub fn mrz_check(s: &str) -> u32 {
    s.chars().zip([7, 3, 1].iter().cycle()).map(|(c, w)| mrz_value(c) * w).sum::<u32>() % 10
}

fn mrz_date(s: &str, future: bool) -> Option<Date> {
    let yy: i32 = s.get(..2)?.parse().ok()?;
    let m: u32 = s.get(2..4)?.parse().ok()?;
    let d: u32 = s.get(4..6)?.parse().ok()?;
    // expiry dates are in this century; birth dates may be in the last one
    let y = if future || yy < 40 { 2000 + yy } else { 1900 + yy };
    date::ymd(y, m, d)
}

fn clean_mrz(line: &str) -> String {
    line.trim().chars().filter(|c| !c.is_whitespace()).map(|c| match c {
        '«' | '‹' | '<' => '<',
        c => c.to_ascii_uppercase(),
    }).collect()
}

/// The MRZ of a passport (TD3: two lines of 44) or an ID card (TD1: three lines of 30).
pub fn parse_mrz(text: &str) -> Option<Extracted> {
    let lines: Vec<String> = text.lines().map(clean_mrz).filter(|l| l.len() >= 28 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '<') && l.contains('<')).collect();
    // TD3
    for w in lines.windows(2) {
        let (l1, l2) = (&w[0], &w[1]);
        if l1.len() >= 44 && l2.len() >= 44 && l1.starts_with('P') {
            let num = &l2[0..9];
            let exp = &l2[21..27];
            if mrz_check(num).to_string() == l2[9..10] && mrz_check(exp).to_string() == l2[27..28] {
                let names = &l1[5..44];
                let (sur, given) = names.split_once("<<").unwrap_or((names, ""));
                let holder = format!("{}, {}", sur.replace('<', " ").trim(), given.replace('<', " ").split_whitespace().collect::<Vec<_>>().join(" "));
                return Some(Extracted { kind: Some(Kind::Passport), expires: mrz_date(exp, true), issued: None, number: Some(num.trim_end_matches('<').to_string()), holder: Some(holder) });
            }
        }
    }
    // TD1
    for w in lines.windows(3) {
        let (l1, l2, l3) = (&w[0], &w[1], &w[2]);
        if l1.len() >= 30 && l2.len() >= 30 && l3.len() >= 30 && matches!(l1.chars().next(), Some('I' | 'A' | 'C')) {
            let num = &l1[5..14];
            let exp = &l2[8..14];
            if mrz_check(num).to_string() == l1[14..15] && mrz_check(exp).to_string() == l2[14..15] {
                let (sur, given) = l3.split_once("<<").unwrap_or((l3.as_str(), ""));
                let holder = format!("{}, {}", sur.replace('<', " ").trim(), given.replace('<', " ").split_whitespace().collect::<Vec<_>>().join(" "));
                return Some(Extracted { kind: Some(Kind::NationalId), expires: mrz_date(exp, true), issued: None, number: Some(num.trim_end_matches('<').to_string()), holder: Some(holder) });
            }
        }
    }
    None
}

/// Byte position (in the folded line) right after the first label of `words`. An entry
/// starting with `=` is an abbreviation that must stand alone (`=exp` matches "EXP 03/30",
/// not "expedición").
fn label_end(folded: &str, words: &[&str]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for w in words {
        let (word, whole) = match w.strip_prefix('=') {
            Some(x) => (x, true),
            None => (*w, false),
        };
        let mut from = 0;
        while let Some(i) = folded[from..].find(word) {
            let s = from + i;
            let e = s + word.len();
            let left = folded[..s].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
            let right = !whole || folded[e..].chars().next().is_none_or(|c| !c.is_alphanumeric());
            if left && right {
                best = Some(best.map_or(e, |b: usize| b.min(e)));
                break;
            }
            from = s + word.len();
        }
    }
    best
}

/// A document number after a label on the same line, else the longest code-looking token.
fn number(text: &str) -> Option<String> {
    let is_code = |w: &str| {
        let digits = w.chars().filter(|c| c.is_ascii_digit()).count();
        w.len() >= 5 && digits >= 4 && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    };
    for line in text.lines() {
        let f = date::fold(line);
        if let Some(pos) = NUMBER_WORDS.iter().filter_map(|w| f.find(w).map(|p| p + w.len())).min() {
            let after = &f[pos.min(f.len())..];
            if let Some(tok) = after.split(|c: char| c.is_whitespace() || c == ':' || c == '#').map(|t| t.trim_matches(|c: char| c == '.' || c == ',')).find(|t| is_code(t)) {
                // take it from the original line to keep its case
                let orig = line.split_whitespace().map(|t| t.trim_matches(|c: char| c == '.' || c == ',' || c == ':' || c == '#')).find(|t| t.eq_ignore_ascii_case(tok)).unwrap_or(tok);
                return Some(orig.to_string());
            }
        }
    }
    text.split_whitespace().map(|t| t.trim_matches(|c: char| !c.is_ascii_alphanumeric())).filter(|t| is_code(t) && date::find_dates(t, Order::DayFirst).is_empty()).max_by_key(|t| t.len()).map(String::from)
}

pub fn extract(text: &str, today: Date, order: Order) -> Extracted {
    if let Some(m) = parse_mrz(text) {
        // printed dates on the page can still give the issue date
        let issued = labelled_date(text, ISSUE_WORDS, order);
        return Extracted { issued, ..m };
    }
    let expires = labelled_date(text, EXPIRY_WORDS, order);
    let issued = labelled_date(text, ISSUE_WORDS, order);
    let all: Vec<Date> = date::find_dates(text, order).into_iter().map(|f| f.date).collect();
    // unlabelled: the latest future date is the expiry, the earliest past one the issue date
    let expires = expires.or_else(|| all.iter().copied().filter(|d| *d > today).max());
    let issued = issued.or_else(|| all.iter().copied().filter(|d| *d <= today && Some(*d) != expires).min());
    Extracted { kind: crate::quick::guess_kind(text), expires, issued, number: number(text), holder: None }
}

/// The first date after a label on its line, else on the next line (labels often sit above).
fn labelled_date(text: &str, words: &[&str], order: Order) -> Option<Date> {
    let lines: Vec<String> = text.lines().map(date::fold).collect();
    for (i, l) in lines.iter().enumerate() {
        let Some(pos) = label_end(l, words) else { continue };
        if let Some(d) = date::find_dates(&l[pos..], order).first() {
            return Some(d.date);
        }
        if let Some(next) = lines.get(i + 1)
            && label_end(next, EXPIRY_WORDS).is_none()
            && label_end(next, ISSUE_WORDS).is_none()
            && let Some(d) = date::find_dates(next, order).first()
        {
            return Some(d.date);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    #[test]
    fn icao_specimen_passport() {
        // ICAO 9303 part 4 specimen
        let text = "PASSPORT\nP<UTOERIKSSON<<ANNA<MARIA<<<<<<<<<<<<<<<<<<<\nL898902C36UTO7408122F1204159ZE184226B<<<<<10\n";
        let e = extract(text, ymd(2010, 1, 1).unwrap(), Order::DayFirst);
        assert_eq!(e.kind, Some(Kind::Passport));
        assert_eq!(e.number.as_deref(), Some("L898902C3"));
        assert_eq!(e.expires, ymd(2012, 4, 15));
        assert_eq!(e.holder.as_deref(), Some("ERIKSSON, ANNA MARIA"));
        assert_eq!(mrz_check("L898902C3"), 6);
        // a misread digit fails the check and is not trusted
        assert_eq!(parse_mrz("P<UTOERIKSSON<<ANNA<MARIA<<<<<<<<<<<<<<<<<<<\nL898902C36UTO7408122F1204169ZE184226B<<<<<10"), None);
    }

    #[test]
    fn icao_specimen_id_card() {
        let text = "I<UTOD231458907<<<<<<<<<<<<<<<\n7408122F1204159UTO<<<<<<<<<<<6\nERIKSSON<<ANNA<MARIA<<<<<<<<<<\n";
        let e = parse_mrz(text).unwrap();
        assert_eq!((e.kind, e.number.as_deref(), e.expires), (Some(Kind::NationalId), Some("D23145890"), ymd(2012, 4, 15)));
    }

    #[test]
    fn printed_documents() {
        let today = ymd(2026, 11, 20).unwrap();
        let soat = "SEGURO OBLIGATORIO SOAT\nPóliza No. 1234567890\nPlaca ABC123\nVigencia desde 01/03/2026\nVence: 28/02/2027\n";
        let e = extract(soat, today, Order::DayFirst);
        assert_eq!(e.kind, Some(Kind::Soat));
        assert_eq!(e.expires, ymd(2027, 2, 28));
        assert_eq!(e.number.as_deref(), Some("1234567890"));
        let lic = "DRIVER LICENSE\nDL 99887766\nISS 03/15/2022\nEXP 03/15/2030";
        let e = extract(lic, today, Order::MonthFirst);
        assert_eq!(e.kind, Some(Kind::DriverLicense));
        assert_eq!((e.issued, e.expires), (ymd(2022, 3, 15), ymd(2030, 3, 15)));
        assert_eq!(e.number.as_deref(), Some("99887766"));
        let receipt = "Factura 2026-10-02\nTV Samsung 55\" Garantía 12 meses";
        let e = extract(receipt, today, Order::DayFirst);
        assert_eq!(e.kind, Some(Kind::Warranty));
        assert_eq!((e.issued, e.expires), (ymd(2026, 10, 2), None));
        let label_above = "Fecha de vencimiento\n12 de marzo de 2028";
        assert_eq!(extract(label_above, today, Order::DayFirst).expires, ymd(2028, 3, 12));
        assert_eq!(extract("", today, Order::DayFirst), Extracted::default());
    }
}
