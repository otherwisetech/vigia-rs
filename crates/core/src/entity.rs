//! Who or what an item belongs to: a person, a vehicle, a company, a property, a pet or a
//! digital asset. Kinds are guessed from names (plates, legal suffixes, domains) and from type
//! templates; the user can always change them.
use crate::model::Kind;

pub type EntityId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum EntityKind {
    #[default]
    Person,
    Vehicle,
    Company,
    Property,
    Pet,
    Digital,
    Other,
}

impl EntityKind {
    pub const ALL: [EntityKind; 7] = [EntityKind::Person, EntityKind::Vehicle, EntityKind::Company, EntityKind::Property, EntityKind::Pet, EntityKind::Digital, EntityKind::Other];

    pub fn id(self) -> &'static str {
        match self {
            EntityKind::Person => "person",
            EntityKind::Vehicle => "vehicle",
            EntityKind::Company => "company",
            EntityKind::Property => "property",
            EntityKind::Pet => "pet",
            EntityKind::Digital => "digital",
            EntityKind::Other => "other",
        }
    }

    pub fn from_id(s: &str) -> Option<EntityKind> {
        let s = s.trim().to_ascii_lowercase();
        EntityKind::ALL.into_iter().find(|k| k.id() == s)
    }

    pub fn label(self) -> String {
        match self {
            EntityKind::Person => i18n::t!("core.entity.person"),
            EntityKind::Vehicle => i18n::t!("core.entity.vehicle"),
            EntityKind::Company => i18n::t!("core.entity.company"),
            EntityKind::Property => i18n::t!("core.entity.property"),
            EntityKind::Pet => i18n::t!("core.entity.pet"),
            EntityKind::Digital => i18n::t!("core.entity.digital"),
            EntityKind::Other => i18n::t!("core.entity.other"),
        }
    }

    /// What the `ident` field holds for this kind (plate, tax id, address, URL), if anything.
    pub fn ident_label(self) -> Option<String> {
        match self {
            EntityKind::Person => Some(i18n::t!("core.entity.ident.person")),
            EntityKind::Vehicle => Some(i18n::t!("core.entity.ident.vehicle")),
            EntityKind::Company => Some(i18n::t!("core.entity.ident.company")),
            EntityKind::Property => Some(i18n::t!("core.entity.ident.property")),
            EntityKind::Digital => Some(i18n::t!("core.entity.ident.digital")),
            EntityKind::Pet | EntityKind::Other => None,
        }
    }

    /// What the `detail` field holds (make and model, species, provider), if anything.
    pub fn detail_label(self) -> Option<String> {
        match self {
            EntityKind::Vehicle => Some(i18n::t!("core.entity.detail.vehicle")),
            EntityKind::Pet => Some(i18n::t!("core.entity.detail.pet")),
            EntityKind::Digital => Some(i18n::t!("core.entity.detail.digital")),
            EntityKind::Other => Some(i18n::t!("core.entity.detail.other")),
            _ => None,
        }
    }

    pub fn has_year(self) -> bool {
        matches!(self, EntityKind::Vehicle | EntityKind::Pet)
    }

    pub fn has_country(self) -> bool {
        matches!(self, EntityKind::Company | EntityKind::Person)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: String,
    /// Plate, tax id (NIT / EIN), address, URL, ID number.
    pub ident: String,
    /// Make and model, species, provider.
    pub detail: String,
    /// Model year, year of birth.
    pub year: Option<i32>,
    pub country: String,
    pub notes: String,
    pub created: i64,
    pub updated: i64,
}

impl Entity {
    pub fn new(kind: EntityKind, name: impl Into<String>) -> Entity {
        let name = name.into();
        let ident = if kind == EntityKind::Vehicle { plate_in(&name).unwrap_or_default() } else { String::new() };
        Entity { kind, name, ident, ..Default::default() }
    }
}

/// A link between two entities: Juan → Otherwise SAS (owner), Juan → Mazda (owner).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Relation {
    pub id: i64,
    pub from: EntityId,
    pub to: EntityId,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum Role {
    #[default]
    Owner,
    Member,
    Manager,
    Family,
    Uses,
    Other,
}

impl Role {
    pub const ALL: [Role; 6] = [Role::Owner, Role::Member, Role::Manager, Role::Family, Role::Uses, Role::Other];

    pub fn id(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Member => "member",
            Role::Manager => "manager",
            Role::Family => "family",
            Role::Uses => "uses",
            Role::Other => "other",
        }
    }

    pub fn from_id(s: &str) -> Role {
        Role::ALL.into_iter().find(|r| r.id() == s.trim()).unwrap_or(Role::Other)
    }

    pub fn label(self) -> String {
        match self {
            Role::Owner => i18n::t!("core.role.owner"),
            Role::Member => i18n::t!("core.role.member"),
            Role::Manager => i18n::t!("core.role.manager"),
            Role::Family => i18n::t!("core.role.family"),
            Role::Uses => i18n::t!("core.role.uses"),
            Role::Other => i18n::t!("core.role.other"),
        }
    }
}

/// Uppercase letters and digits only: `abc-123` → `ABC123`.
pub fn normalize_ident(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_uppercase).collect()
}

/// A licence plate: Colombian cars `ABC123`, motorcycles `ABC12D`, and common US-like forms
/// (`ABC1234`, `7ABC123`, `ABC-1234`, `123 ABC` is not matched: too ambiguous).
pub fn is_plate(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric());
    if t.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '-')) || t.matches('-').count() > 1 {
        return false;
    }
    let s: Vec<char> = t.chars().filter(|c| *c != '-').map(|c| c.to_ascii_uppercase()).collect();
    let letters = s.iter().take_while(|c| c.is_ascii_alphabetic()).count();
    let rest = &s[letters..];
    let all_digits = |v: &[char]| !v.is_empty() && v.iter().all(|c| c.is_ascii_digit());
    match (letters, rest.len()) {
        // ABC123, ABC1234
        (3, 3) | (3, 4) if all_digits(rest) => true,
        // ABC12D (Colombian motorcycle)
        (3, 3) => rest[..2].iter().all(|c| c.is_ascii_digit()) && rest[2].is_ascii_alphabetic(),
        // 7ABC123 (California-style)
        (0, 7) => s[0].is_ascii_digit() && s[1..4].iter().all(|c| c.is_ascii_alphabetic()) && all_digits(&s[4..]),
        _ => false,
    }
}

/// The plate inside a name (`Mazda ABC123` → `ABC123`).
pub fn plate_in(name: &str) -> Option<String> {
    name.split_whitespace().find(|w| is_plate(w)).map(normalize_ident)
}

const LEGAL_SUFFIXES: &[&str] = &["LLC", "LLP", "PLLC", "INC", "CORP", "CORPORATION", "LTD", "LTDA", "SAS", "SA", "SL", "SRL", "SARL", "SPA", "GMBH", "AG", "BV", "PTY", "EIRL", "LDA", "SAC", "SCS", "PLC"];

/// `Otherwise Technologies LLC`, `Otherwise S.A.S.`, `Acme, Inc.`.
pub fn has_legal_suffix(name: &str) -> bool {
    let words: Vec<&str> = name.split_whitespace().collect();
    words.len() > 1 && words[1..].iter().any(|w| LEGAL_SUFFIXES.contains(&normalize_ident(w).as_str()))
}

fn is_domain(name: &str) -> bool {
    !name.contains(char::is_whitespace) && !name.contains('@') && name.contains('.') && crate::rdap::normalize(name).is_some()
}

/// What a name looks like, when it says so: a plate, a legal suffix or a domain.
pub fn infer(name: &str) -> Option<EntityKind> {
    let n = name.trim();
    if n.is_empty() {
        None
    } else if has_legal_suffix(n) {
        Some(EntityKind::Company)
    } else if plate_in(n).is_some() {
        Some(EntityKind::Vehicle)
    } else if is_domain(n) {
        Some(EntityKind::Digital)
    } else {
        None
    }
}

impl Kind {
    /// Who a document of this type usually belongs to.
    pub fn entity_kind(self) -> Option<EntityKind> {
        use Kind::*;
        match self {
            Passport | NationalId | DriverLicense | Visa | ResidencePermit | Checkup => Some(EntityKind::Person),
            Soat | Tecnomecanica | VehicleRegistration | VehicleInsurance => Some(EntityKind::Vehicle),
            WyomingReport | CamaraComercio | Rut => Some(EntityKind::Company),
            Domain | Certificate | Subscription => Some(EntityKind::Digital),
            GasInspection | Extinguisher => Some(EntityKind::Property),
            _ => None,
        }
    }
}

/// The kind for a new entity named `name` on an item of type `item`: the name's own pattern,
/// else the template's, else a person.
pub fn guess(name: &str, item: Option<Kind>) -> EntityKind {
    infer(name).or(item.and_then(Kind::entity_kind)).unwrap_or(EntityKind::Person)
}

/// The existing entity a typed reference means: the same name, the same identifier (plate,
/// tax id), else the only entity with that word in its name (`ABC123` → `Mazda ABC123`).
pub fn find<'a>(entities: &'a [Entity], query: &str) -> Option<&'a Entity> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    if let Some(e) = entities.iter().find(|e| e.name.trim().to_lowercase() == q.to_lowercase()) {
        return Some(e);
    }
    let nq = normalize_ident(q);
    if nq.is_empty() {
        return None;
    }
    if let Some(e) = entities.iter().find(|e| !e.ident.is_empty() && normalize_ident(&e.ident) == nq) {
        return Some(e);
    }
    let mut by_word = entities.iter().filter(|e| e.name.split_whitespace().any(|w| normalize_ident(w) == nq));
    match (by_word.next(), by_word.next()) {
        (Some(e), None) => Some(e),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plates() {
        for p in ["ABC123", "abc123", "ABC12D", "ABC-1234", "7ABC123", "Mazda KLM456"] {
            assert!(p.split_whitespace().any(is_plate), "{p}");
        }
        for p in ["Ana", "COVID19", "Mazda3", "AB123", "ABCD123", "2027", "ABC", "12345", "A-B-C123"] {
            assert!(!is_plate(p), "{p}");
        }
        assert_eq!(plate_in("Mazda ABC123"), Some("ABC123".into()));
        assert_eq!(plate_in("moto xyz-12a"), Some("XYZ12A".into()));
    }

    #[test]
    fn inference() {
        assert_eq!(infer("Mazda ABC123"), Some(EntityKind::Vehicle));
        assert_eq!(infer("Moto ABC12D"), Some(EntityKind::Vehicle));
        assert_eq!(infer("Otherwise Technologies LLC"), Some(EntityKind::Company));
        assert_eq!(infer("Otherwise S.A.S."), Some(EntityKind::Company));
        assert_eq!(infer("Otherwise SAS"), Some(EntityKind::Company));
        assert_eq!(infer("Acme, Inc."), Some(EntityKind::Company));
        assert_eq!(infer("otherwisetech.com"), Some(EntityKind::Digital));
        assert_eq!(infer("Ana"), None);
        assert_eq!(infer("Ana María"), None);
        // a single word is a name, not a company called "SAS"
        assert_eq!(infer("Sas"), None);
        assert_eq!(guess("Ana", Some(Kind::Passport)), EntityKind::Person);
        assert_eq!(guess("Mazda", Some(Kind::Soat)), EntityKind::Vehicle);
        assert_eq!(guess("Otherwise LLC", Some(Kind::Domain)), EntityKind::Company);
        assert_eq!(guess("Casa", None), EntityKind::Person);
        assert_eq!(Kind::Tecnomecanica.entity_kind(), Some(EntityKind::Vehicle));
        assert_eq!(Kind::Rut.entity_kind(), Some(EntityKind::Company));
        assert!(EntityKind::ALL.iter().all(|k| EntityKind::from_id(k.id()) == Some(*k)));
        assert!(Role::ALL.iter().all(|r| Role::from_id(r.id()) == *r));
    }

    #[test]
    fn finding() {
        let mk = |id, kind, name: &str, ident: &str| Entity { id, kind, name: name.into(), ident: ident.into(), ..Default::default() };
        let es = vec![mk(1, EntityKind::Person, "Ana", ""), mk(2, EntityKind::Vehicle, "Mazda", "ABC123"), mk(3, EntityKind::Vehicle, "Moto XYZ12A", ""), mk(4, EntityKind::Person, "Ana María", ""), mk(5, EntityKind::Company, "Otherwise SAS", "901.234.567-8")];
        assert_eq!(find(&es, "ana").map(|e| e.id), Some(1));
        assert_eq!(find(&es, "abc-123").map(|e| e.id), Some(2));
        assert_eq!(find(&es, "XYZ12A").map(|e| e.id), Some(3));
        assert_eq!(find(&es, "9012345678").map(|e| e.id), Some(5));
        assert_eq!(find(&es, "María").map(|e| e.id), Some(4));
        assert_eq!(find(&es, "Luis"), None);
        assert_eq!(find(&es, "  "), None);
    }
}
