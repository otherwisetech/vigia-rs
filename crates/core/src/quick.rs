//! Quick add: one line of text → a new item. `Pasaporte @Ana 12/05/2031`, `SOAT moto en 3
//! meses`, `otherwisetech.com 2027-08-13`, `Wyoming annual report`. The type is guessed from
//! keywords in the four languages, `@word` is the owner, the first date found (absolute or
//! relative) is the expiry.
use crate::date::{self, Date, Order};
use crate::model::{Item, Kind};

/// Keyword (folded, lowercase) → type. Longer phrases first.
const KEYWORDS: &[(&str, Kind)] = &[
    ("camara de comercio", Kind::CamaraComercio),
    ("registro mercantil", Kind::CamaraComercio),
    ("matricula mercantil", Kind::CamaraComercio),
    ("annual report", Kind::WyomingReport),
    ("wyoming", Kind::WyomingReport),
    ("revision tecnico", Kind::Tecnomecanica),
    ("tecnomecanica", Kind::Tecnomecanica),
    ("tecno", Kind::Tecnomecanica),
    ("soat", Kind::Soat),
    ("licencia de conduccion", Kind::DriverLicense),
    ("pase", Kind::DriverLicense),
    ("driver", Kind::DriverLicense),
    ("licence", Kind::DriverLicense),
    ("license", Kind::DriverLicense),
    ("permis", Kind::DriverLicense),
    ("cnh", Kind::DriverLicense),
    ("passport", Kind::Passport),
    ("pasaporte", Kind::Passport),
    ("passaporte", Kind::Passport),
    ("passeport", Kind::Passport),
    ("cedula", Kind::NationalId),
    ("dni", Kind::NationalId),
    ("identity card", Kind::NationalId),
    ("carte d'identite", Kind::NationalId),
    ("rg", Kind::NationalId),
    ("visa", Kind::Visa),
    ("visto", Kind::Visa),
    ("residencia", Kind::ResidencePermit),
    ("residence", Kind::ResidencePermit),
    ("titre de sejour", Kind::ResidencePermit),
    ("green card", Kind::ResidencePermit),
    ("tarjeta de propiedad", Kind::VehicleRegistration),
    ("registration", Kind::VehicleRegistration),
    ("impuesto vehicular", Kind::VehicleRegistration),
    ("seguro del carro", Kind::VehicleInsurance),
    ("seguro todo riesgo", Kind::VehicleInsurance),
    ("car insurance", Kind::VehicleInsurance),
    ("poliza", Kind::InsurancePolicy),
    ("policy", Kind::InsurancePolicy),
    ("seguro", Kind::InsurancePolicy),
    ("insurance", Kind::InsurancePolicy),
    ("assurance", Kind::InsurancePolicy),
    ("garantia", Kind::Warranty),
    ("warranty", Kind::Warranty),
    ("garantie", Kind::Warranty),
    ("ssl", Kind::Certificate),
    ("tls", Kind::Certificate),
    ("certificado", Kind::Certificate),
    ("certificate", Kind::Certificate),
    ("certificat", Kind::Certificate),
    ("dominio", Kind::Domain),
    ("domain", Kind::Domain),
    ("domaine", Kind::Domain),
    ("suscripcion", Kind::Subscription),
    ("subscription", Kind::Subscription),
    ("assinatura", Kind::Subscription),
    ("abonnement", Kind::Subscription),
    ("contrato", Kind::Contract),
    ("contract", Kind::Contract),
    ("contrat", Kind::Contract),
    ("arriendo", Kind::Contract),
    ("lease", Kind::Contract),
    ("rut", Kind::Rut),
    ("membresia", Kind::Membership),
    ("membership", Kind::Membership),
    ("adhesion", Kind::Membership),
    ("vacuna", Kind::Vaccine),
    ("vaccine", Kind::Vaccine),
    ("vacina", Kind::Vaccine),
    ("vaccin", Kind::Vaccine),
    ("chequeo", Kind::Checkup),
    ("check-up", Kind::Checkup),
    ("checkup", Kind::Checkup),
    ("bilan", Kind::Checkup),
    ("revision de gas", Kind::GasInspection),
    ("gas", Kind::GasInspection),
    ("extintor", Kind::Extinguisher),
    ("extinguisher", Kind::Extinguisher),
    ("extincteur", Kind::Extinguisher),
];

/// Whole-word match of a (folded) phrase.
fn has_phrase(hay: &str, phrase: &str) -> bool {
    let mut from = 0;
    while let Some(i) = hay[from..].find(phrase) {
        let s = from + i;
        let e = s + phrase.len();
        let before = hay[..s].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = hay[e..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        if before && after {
            return true;
        }
        from = s + 1;
        while !hay.is_char_boundary(from) {
            from += 1;
        }
    }
    false
}

pub fn guess_kind(text: &str) -> Option<Kind> {
    let f = date::fold(text);
    KEYWORDS.iter().find(|(k, _)| has_phrase(&f, k)).map(|(_, kind)| *kind)
}

/// A token that looks like a domain (`example.com`, `https://shop.example.co/x`).
fn domain_token(text: &str) -> Option<String> {
    text.split_whitespace().find(|w| w.contains('.') && !w.chars().next().is_some_and(|c| c.is_ascii_digit()) && crate::rdap::normalize(w).is_some()).map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
}

pub fn parse(line: &str, today: Date, order: Order) -> Item {
    let mut rest = line.trim().to_string();
    // date first, so its words aren't mistaken for the name
    let found = date::find_dates(&rest, order).into_iter().next().or_else(|| date::find_relative(&rest, today));
    let mut expires = None;
    if let Some(f) = found {
        expires = Some(f.date);
        rest.replace_range(f.start..f.end, " ");
    }
    let mut owner = String::new();
    let words: Vec<&str> = rest.split_whitespace().collect();
    let mut kept = Vec::new();
    for w in words {
        if let Some(o) = w.strip_prefix('@').filter(|o| !o.is_empty() && !o.contains('.')) {
            owner = o.replace('_', " ");
        } else {
            kept.push(w);
        }
    }
    // connectors left dangling by the date: "vence el", "expires on"
    while kept.last().is_some_and(|w| matches!(date::fold(w).as_str(), "el" | "on" | "le" | "em" | "en" | "vence" | "expires" | "expira" | "hasta" | "until" | "jusqu'au" | "ate" | "-" | ":")) {
        kept.pop();
    }
    let name = kept.join(" ");
    let domain = domain_token(&name);
    let kind = guess_kind(&name).unwrap_or(if domain.is_some() { Kind::Domain } else { Kind::Custom });
    let mut it = Item::new(kind, if name.is_empty() { kind.label() } else { name });
    it.owner = owner;
    it.expires = expires;
    if matches!(kind, Kind::Domain | Kind::Certificate)
        && let Some(d) = domain
    {
        it.host = if kind == Kind::Domain { crate::rdap::normalize(&d).unwrap_or(d) } else { d };
    }
    it
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    #[test]
    fn lines() {
        let today = ymd(2026, 11, 20).unwrap();
        let p = |s: &str| parse(s, today, Order::DayFirst);
        let a = p("Pasaporte @Ana 12/05/2031");
        assert_eq!((a.kind, a.name.as_str(), a.owner.as_str(), a.expires), (Kind::Passport, "Pasaporte", "Ana", ymd(2031, 5, 12)));
        assert_eq!(a.lead_days, vec![270, 180, 90, 30]);
        let b = p("SOAT moto en 3 meses");
        assert_eq!((b.kind, b.name.as_str(), b.expires), (Kind::Soat, "SOAT moto", ymd(2027, 2, 20)));
        let c = p("otherwisetech.com 2027-08-13");
        assert_eq!((c.kind, c.host.as_str(), c.expires), (Kind::Domain, "otherwisetech.com", ymd(2027, 8, 13)));
        let d = p("SSL api.example.com vence el 15 mar 2027");
        assert_eq!((d.kind, d.host.as_str(), d.name.as_str(), d.expires), (Kind::Certificate, "api.example.com", "SSL api.example.com", ymd(2027, 3, 15)));
        let e = p("Wyoming annual report Otherwise Technologies");
        assert_eq!(e.kind, Kind::WyomingReport);
        assert_eq!(e.expires, None);
        assert_eq!(p("Cámara de Comercio @Otherwise_Tech").owner, "Otherwise Tech");
        assert_eq!(p("Garantía nevera").kind, Kind::Warranty);
        assert_eq!(p("Passeport Luc dans 2 ans").kind, Kind::Passport);
        assert_eq!(p("Arriendo apto").kind, Kind::Contract);
        assert_eq!(p("Netflix").kind, Kind::Custom);
        // "gas" as a word, not inside "Vegas"
        assert_eq!(p("Las Vegas trip").kind, Kind::Custom);
        assert_eq!(p("").name, Kind::Custom.label());
    }
}
