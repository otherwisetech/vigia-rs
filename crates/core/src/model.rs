//! What Vigía keeps track of: items that expire, their type templates, recurrence rules and
//! status.
use crate::date::{self, Date};
use serde::{Deserialize, Serialize};

pub type ItemId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum Category {
    Identity,
    Vehicle,
    Insurance,
    Warranty,
    Digital,
    Contract,
    Company,
    Membership,
    Health,
    Home,
    #[default]
    Other,
}

impl Category {
    pub const ALL: [Category; 11] = [Category::Identity, Category::Vehicle, Category::Insurance, Category::Warranty, Category::Digital, Category::Contract, Category::Company, Category::Membership, Category::Health, Category::Home, Category::Other];

    pub fn id(self) -> &'static str {
        match self {
            Category::Identity => "identity",
            Category::Vehicle => "vehicle",
            Category::Insurance => "insurance",
            Category::Warranty => "warranty",
            Category::Digital => "digital",
            Category::Contract => "contract",
            Category::Company => "company",
            Category::Membership => "membership",
            Category::Health => "health",
            Category::Home => "home",
            Category::Other => "other",
        }
    }

    pub fn from_id(s: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.id() == s)
    }

    pub fn label(self) -> String {
        match self {
            Category::Identity => i18n::t!("core.cat.identity"),
            Category::Vehicle => i18n::t!("core.cat.vehicle"),
            Category::Insurance => i18n::t!("core.cat.insurance"),
            Category::Warranty => i18n::t!("core.cat.warranty"),
            Category::Digital => i18n::t!("core.cat.digital"),
            Category::Contract => i18n::t!("core.cat.contract"),
            Category::Company => i18n::t!("core.cat.company"),
            Category::Membership => i18n::t!("core.cat.membership"),
            Category::Health => i18n::t!("core.cat.health"),
            Category::Home => i18n::t!("core.cat.home"),
            Category::Other => i18n::t!("core.cat.other"),
        }
    }
}

/// Type templates: each sets a category, default reminder lead times, a recurrence and the
/// fields that matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum Kind {
    Passport,
    NationalId,
    DriverLicense,
    Visa,
    ResidencePermit,
    Soat,
    Tecnomecanica,
    VehicleRegistration,
    VehicleInsurance,
    InsurancePolicy,
    Warranty,
    Domain,
    Certificate,
    Subscription,
    Contract,
    WyomingReport,
    CamaraComercio,
    Rut,
    Membership,
    Vaccine,
    Checkup,
    GasInspection,
    Extinguisher,
    #[default]
    Custom,
}

impl Kind {
    pub const ALL: [Kind; 24] = [
        Kind::Passport,
        Kind::NationalId,
        Kind::DriverLicense,
        Kind::Visa,
        Kind::ResidencePermit,
        Kind::Soat,
        Kind::Tecnomecanica,
        Kind::VehicleRegistration,
        Kind::VehicleInsurance,
        Kind::InsurancePolicy,
        Kind::Warranty,
        Kind::Domain,
        Kind::Certificate,
        Kind::Subscription,
        Kind::Contract,
        Kind::WyomingReport,
        Kind::CamaraComercio,
        Kind::Rut,
        Kind::Membership,
        Kind::Vaccine,
        Kind::Checkup,
        Kind::GasInspection,
        Kind::Extinguisher,
        Kind::Custom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Kind::Passport => "passport",
            Kind::NationalId => "national_id",
            Kind::DriverLicense => "driver_license",
            Kind::Visa => "visa",
            Kind::ResidencePermit => "residence_permit",
            Kind::Soat => "soat",
            Kind::Tecnomecanica => "tecnomecanica",
            Kind::VehicleRegistration => "vehicle_registration",
            Kind::VehicleInsurance => "vehicle_insurance",
            Kind::InsurancePolicy => "insurance",
            Kind::Warranty => "warranty",
            Kind::Domain => "domain",
            Kind::Certificate => "certificate",
            Kind::Subscription => "subscription",
            Kind::Contract => "contract",
            Kind::WyomingReport => "wyoming_report",
            Kind::CamaraComercio => "camara_comercio",
            Kind::Rut => "rut",
            Kind::Membership => "membership",
            Kind::Vaccine => "vaccine",
            Kind::Checkup => "checkup",
            Kind::GasInspection => "gas_inspection",
            Kind::Extinguisher => "extinguisher",
            Kind::Custom => "custom",
        }
    }

    pub fn from_id(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == s)
    }

    pub fn label(self) -> String {
        match self {
            Kind::Passport => i18n::t!("core.kind.passport"),
            Kind::NationalId => i18n::t!("core.kind.national_id"),
            Kind::DriverLicense => i18n::t!("core.kind.driver_license"),
            Kind::Visa => i18n::t!("core.kind.visa"),
            Kind::ResidencePermit => i18n::t!("core.kind.residence_permit"),
            Kind::Soat => i18n::t!("core.kind.soat"),
            Kind::Tecnomecanica => i18n::t!("core.kind.tecnomecanica"),
            Kind::VehicleRegistration => i18n::t!("core.kind.vehicle_registration"),
            Kind::VehicleInsurance => i18n::t!("core.kind.vehicle_insurance"),
            Kind::InsurancePolicy => i18n::t!("core.kind.insurance"),
            Kind::Warranty => i18n::t!("core.kind.warranty"),
            Kind::Domain => i18n::t!("core.kind.domain"),
            Kind::Certificate => i18n::t!("core.kind.certificate"),
            Kind::Subscription => i18n::t!("core.kind.subscription"),
            Kind::Contract => i18n::t!("core.kind.contract"),
            Kind::WyomingReport => i18n::t!("core.kind.wyoming_report"),
            Kind::CamaraComercio => i18n::t!("core.kind.camara_comercio"),
            Kind::Rut => i18n::t!("core.kind.rut"),
            Kind::Membership => i18n::t!("core.kind.membership"),
            Kind::Vaccine => i18n::t!("core.kind.vaccine"),
            Kind::Checkup => i18n::t!("core.kind.checkup"),
            Kind::GasInspection => i18n::t!("core.kind.gas_inspection"),
            Kind::Extinguisher => i18n::t!("core.kind.extinguisher"),
            Kind::Custom => i18n::t!("core.kind.custom"),
        }
    }

    pub fn category(self) -> Category {
        use Kind::*;
        match self {
            Passport | NationalId | DriverLicense | Visa | ResidencePermit => Category::Identity,
            Soat | Tecnomecanica | VehicleRegistration | VehicleInsurance => Category::Vehicle,
            InsurancePolicy => Category::Insurance,
            Warranty => Category::Warranty,
            Domain | Certificate | Subscription => Category::Digital,
            Contract => Category::Contract,
            WyomingReport | CamaraComercio | Rut => Category::Company,
            Membership => Category::Membership,
            Vaccine | Checkup => Category::Health,
            GasInspection | Extinguisher => Category::Home,
            Custom => Category::Other,
        }
    }

    /// Default reminder lead times, in days before the deadline.
    pub fn lead_days(self) -> Vec<u32> {
        use Kind::*;
        match self {
            // many countries refuse entry with less than 6 months left: start at 9
            Passport => vec![270, 180, 90, 30],
            ResidencePermit | Visa => vec![120, 60, 30, 7],
            NationalId | DriverLicense => vec![90, 30, 7],
            Soat | Tecnomecanica | VehicleInsurance | Subscription => vec![30, 7, 1],
            VehicleRegistration | Membership | Checkup | Extinguisher | Warranty | Vaccine | Rut => vec![30, 7],
            InsurancePolicy | WyomingReport | CamaraComercio | GasInspection => vec![60, 30, 7],
            Domain => vec![60, 30, 7, 1],
            Certificate => vec![30, 14, 7, 1],
            Contract | Custom => DEFAULT_LEAD.to_vec(),
        }
    }

    /// Default recurrence for a new item of this type.
    pub fn recurrence(self) -> Recurrence {
        use Kind::*;
        match self {
            Soat | Tecnomecanica | VehicleRegistration | VehicleInsurance | InsurancePolicy | Domain | Subscription | Membership | Checkup | Extinguisher | Rut => Recurrence::Every { n: 1, unit: Unit::Year },
            WyomingReport => Recurrence::Wyoming { month: 1 },
            CamaraComercio => Recurrence::Yearly { month: 3, day: 31 },
            GasInspection => Recurrence::Every { n: 5, unit: Unit::Year },
            _ => Recurrence::None,
        }
    }

    /// Typical validity, used to suggest the next expiry when renewing a non-recurring item.
    pub fn term_months(self) -> Option<u32> {
        use Kind::*;
        match self {
            Passport | DriverLicense | NationalId => Some(120),
            Certificate => Some(3),
            Warranty => Some(12),
            Visa | ResidencePermit => Some(12),
            _ => None,
        }
    }

    /// Whether this type can be checked automatically over the network.
    pub fn checkable(self) -> bool {
        matches!(self, Kind::Domain | Kind::Certificate)
    }
}

pub const DEFAULT_LEAD: [u32; 4] = [90, 30, 7, 1];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Day,
    Week,
    Month,
    Year,
}

/// How an item comes back after it's renewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Recurrence {
    #[default]
    None,
    Every { n: u32, unit: Unit },
    /// Wyoming LLC annual report: due on the 1st day of the formation (anniversary) month.
    Wyoming { month: u32 },
    /// Same day every year (Cámara de Comercio renewal: by March 31 in Colombia).
    Yearly { month: u32, day: u32 },
}

impl Recurrence {
    /// `none`, `every:1y`, `every:6m`, `every:2w`, `every:30d`, `wyoming:3`, `yearly:03-31`.
    pub fn id(self) -> String {
        match self {
            Recurrence::None => "none".into(),
            Recurrence::Every { n, unit } => format!("every:{n}{}", match unit {
                Unit::Day => 'd',
                Unit::Week => 'w',
                Unit::Month => 'm',
                Unit::Year => 'y',
            }),
            Recurrence::Wyoming { month } => format!("wyoming:{month}"),
            Recurrence::Yearly { month, day } => format!("yearly:{month:02}-{day:02}"),
        }
    }

    pub fn parse(s: &str) -> Option<Recurrence> {
        let s = s.trim();
        if s.is_empty() || s == "none" {
            return Some(Recurrence::None);
        }
        let (k, v) = s.split_once(':')?;
        match k {
            "every" => {
                let unit = match v.chars().last()? {
                    'd' => Unit::Day,
                    'w' => Unit::Week,
                    'm' => Unit::Month,
                    'y' => Unit::Year,
                    _ => return None,
                };
                let n: u32 = v[..v.len() - 1].parse().ok().filter(|n| *n > 0)?;
                Some(Recurrence::Every { n, unit })
            }
            "wyoming" => v.parse().ok().filter(|m| (1..=12).contains(m)).map(|month| Recurrence::Wyoming { month }),
            "yearly" => {
                let (m, d) = v.split_once('-')?;
                let (month, day) = (m.parse().ok()?, d.parse().ok()?);
                ((1..=12).contains(&month) && (1..=date::last_day(2028, month)).contains(&day)).then_some(Recurrence::Yearly { month, day })
            }
            _ => None,
        }
    }

    /// The next deadline after `current` was met.
    pub fn next_after(self, current: Date) -> Option<Date> {
        match self {
            Recurrence::None => None,
            Recurrence::Every { n, unit } => Some(match unit {
                Unit::Day => date::add_days(current, n as i64),
                Unit::Week => date::add_days(current, n as i64 * 7),
                Unit::Month => date::add_months(current, n as i32),
                Unit::Year => date::add_years(current, n as i32),
            }),
            Recurrence::Wyoming { month } => Some(date::wyoming_next(month, date::add_days(current, 1))),
            Recurrence::Yearly { month, day } => Some(date::yearly_next(month, day, date::add_days(current, 1))),
        }
    }

    /// For rule-based recurrences, the deadline that applies from `today` when none is stored.
    pub fn first_on_or_after(self, today: Date) -> Option<Date> {
        match self {
            Recurrence::Wyoming { month } => Some(date::wyoming_next(month, today)),
            Recurrence::Yearly { month, day } => Some(date::yearly_next(month, day, today)),
            _ => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            Recurrence::None => i18n::t!("core.rec.none"),
            Recurrence::Every { n, unit: Unit::Day } => i18n::tn!("core.rec.days", n),
            Recurrence::Every { n, unit: Unit::Week } => i18n::tn!("core.rec.weeks", n),
            Recurrence::Every { n, unit: Unit::Month } => i18n::tn!("core.rec.months", n),
            Recurrence::Every { n, unit: Unit::Year } => i18n::tn!("core.rec.years", n),
            Recurrence::Wyoming { month } => i18n::t!("core.rec.wyoming", month = month_name(month)),
            Recurrence::Yearly { month, day } => i18n::t!("core.rec.yearly", day = day, month = month_name(month)),
        }
    }
}

pub fn month_name(m: u32) -> String {
    match m {
        1 => i18n::t!("core.month.1"),
        2 => i18n::t!("core.month.2"),
        3 => i18n::t!("core.month.3"),
        4 => i18n::t!("core.month.4"),
        5 => i18n::t!("core.month.5"),
        6 => i18n::t!("core.month.6"),
        7 => i18n::t!("core.month.7"),
        8 => i18n::t!("core.month.8"),
        9 => i18n::t!("core.month.9"),
        10 => i18n::t!("core.month.10"),
        11 => i18n::t!("core.month.11"),
        _ => i18n::t!("core.month.12"),
    }
}

/// Where an item is in its renewal cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum State {
    #[default]
    Active,
    /// The renewal was started (paperwork sent, appointment booked): lead reminders pause.
    InProgress,
    /// Kept for the record, no reminders, hidden from views by default.
    Archived,
}

impl State {
    pub fn id(self) -> &'static str {
        match self {
            State::Active => "active",
            State::InProgress => "in_progress",
            State::Archived => "archived",
        }
    }

    pub fn from_id(s: &str) -> State {
        match s {
            "in_progress" => State::InProgress,
            "archived" => State::Archived,
            _ => State::Active,
        }
    }
}

/// Urgency of an item on a given day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Status {
    Expired,
    /// due in 30 days or less
    Soon,
    /// due in 31–90 days
    Upcoming,
    Ok,
    /// no date known
    Undated,
}

impl Status {
    pub const ALL: [Status; 5] = [Status::Expired, Status::Soon, Status::Upcoming, Status::Ok, Status::Undated];

    pub fn of_days(days_left: Option<i64>) -> Status {
        match days_left {
            None => Status::Undated,
            Some(d) if d < 0 => Status::Expired,
            Some(d) if d <= 30 => Status::Soon,
            Some(d) if d <= 90 => Status::Upcoming,
            Some(_) => Status::Ok,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Status::Expired => "expired",
            Status::Soon => "soon",
            Status::Upcoming => "upcoming",
            Status::Ok => "ok",
            Status::Undated => "undated",
        }
    }

    pub fn from_id(s: &str) -> Option<Status> {
        Status::ALL.into_iter().find(|x| x.id() == s)
    }

    pub fn label(self) -> String {
        match self {
            Status::Expired => i18n::t!("core.status.expired"),
            Status::Soon => i18n::t!("core.status.soon"),
            Status::Upcoming => i18n::t!("core.status.upcoming"),
            Status::Ok => i18n::t!("core.status.ok"),
            Status::Undated => i18n::t!("core.status.undated"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub id: ItemId,
    pub kind: Kind,
    pub name: String,
    /// Whose it is: the name of its entity (a person, a vehicle, a company…). Saving resolves
    /// it to an existing entity or creates one.
    pub owner: String,
    /// The entity `owner` names, as loaded.
    pub entity: Option<crate::entity::EntityId>,
    /// The entity's kind as loaded; on save, the kind for a new entity when `entity` is `None`.
    pub owner_kind: Option<crate::entity::EntityKind>,
    pub category: Category,
    pub issued: Option<Date>,
    /// The expiry / due date currently in force.
    pub expires: Option<Date>,
    pub recurrence: Recurrence,
    /// Validity in months: warranty months (expiry = purchase + months), or the usual term used
    /// to suggest the next expiry when renewing.
    pub term_months: Option<u32>,
    /// Contracts: days of notice before the end date to cancel or renew.
    pub notice_days: Option<u32>,
    pub cost: Option<f64>,
    pub currency: String,
    pub provider: String,
    pub reference: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub renewal_url: String,
    pub auto_renews: bool,
    pub lead_days: Vec<u32>,
    pub state: State,
    pub snoozed_until: Option<Date>,
    /// Domain name or TLS host (`host` or `host:port`).
    pub host: String,
    /// Opt-in network check (RDAP for domains, TLS handshake for certificates).
    pub auto_check: bool,
    pub last_checked: Option<i64>,
    /// Outcome of the last check, shown as is (an error or a short summary).
    pub check_note: String,
    pub created: i64,
    pub updated: i64,
}

impl Item {
    /// A new item with the type's defaults.
    pub fn new(kind: Kind, name: impl Into<String>) -> Item {
        Item { kind, name: name.into(), category: kind.category(), recurrence: kind.recurrence(), lead_days: kind.lead_days(), term_months: if kind == Kind::Warranty { Some(12) } else { None }, ..Default::default() }
    }

    /// The end of validity: the stored expiry, else purchase + warranty months, else the next
    /// date of a rule-based recurrence.
    pub fn end_date(&self, today: Date) -> Option<Date> {
        if let Some(e) = self.expires {
            return Some(e);
        }
        if let (Kind::Warranty, Some(issued), Some(m)) = (self.kind, self.issued, self.term_months) {
            return Some(date::add_months(issued, m as i32));
        }
        self.recurrence.first_on_or_after(today)
    }

    /// The day something must be done: the end date, minus the notice period for contracts.
    pub fn due_date(&self, today: Date) -> Option<Date> {
        let end = self.end_date(today)?;
        Some(match self.notice_days {
            Some(n) if n > 0 => date::add_days(end, -(n as i64)),
            _ => end,
        })
    }

    pub fn days_left(&self, today: Date) -> Option<i64> {
        self.due_date(today).map(|d| date::days_between(today, d))
    }

    pub fn status(&self, today: Date) -> Status {
        Status::of_days(self.days_left(today))
    }

    /// Suggested expiry after renewing today.
    pub fn suggest_next(&self, today: Date) -> Date {
        let current = self.end_date(today).unwrap_or(today);
        if let Some(n) = self.recurrence.next_after(current) {
            // an item renewed long after it lapsed starts from today
            return if n < today { self.recurrence.next_after(today).unwrap_or(n) } else { n };
        }
        let base = current.max(today);
        if let Some(m) = self.term_months.or(self.kind.term_months()) {
            return date::add_months(if self.kind == Kind::Warranty { today } else { base }, m as i32);
        }
        if let (Some(i), Some(e)) = (self.issued, self.expires)
            && e > i
        {
            let months = ((e - i).num_days() as f64 / 30.44).round().max(1.) as i32;
            return date::add_months(base, months);
        }
        date::add_years(base, 1)
    }

    /// Lead times in use: the item's own, else the defaults (sorted, largest first).
    pub fn leads(&self) -> Vec<u32> {
        let mut v = if self.lead_days.is_empty() { DEFAULT_LEAD.to_vec() } else { self.lead_days.clone() };
        v.sort_unstable_by(|a, b| b.cmp(a));
        v.dedup();
        v
    }

    /// Lowercase text searched by the filter box.
    pub fn haystack(&self) -> String {
        [self.name.as_str(), &self.owner, &self.provider, &self.reference, &self.notes, &self.host, &self.tags.join(" "), &self.kind.label(), &self.category.label()].join(" ").to_lowercase()
    }

    /// Owner, else the item name: the label a reminder names.
    pub fn title(&self) -> String {
        if self.owner.is_empty() { self.name.clone() } else { i18n::t!("core.item.title", name = self.name, owner = self.owner) }
    }
}

/// Parse `90, 30, 7 1` into lead days.
pub fn parse_leads(s: &str) -> Vec<u32> {
    let mut v: Vec<u32> = s.split(|c: char| !c.is_ascii_digit()).filter_map(|p| p.parse().ok()).filter(|n| *n <= 3650).collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v.dedup();
    v
}

pub fn leads_text(v: &[u32]) -> String {
    v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    pub id: i64,
    pub item: ItemId,
    /// Relative to the files folder next to the database (`vigia-files/`), `/`-separated.
    pub rel_path: String,
    pub name: String,
    pub added: i64,
}

/// One renewal in an item's history.
#[derive(Debug, Clone, PartialEq)]
pub struct Renewal {
    pub id: i64,
    pub item: ItemId,
    pub on: Date,
    pub old_expiry: Option<Date>,
    pub new_expiry: Option<Date>,
    pub cost: Option<f64>,
    pub note: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    fn d(y: i32, m: u32, dd: u32) -> Date {
        ymd(y, m, dd).unwrap()
    }

    #[test]
    fn recurrence_ids_roundtrip() {
        for r in [Recurrence::None, Recurrence::Every { n: 1, unit: Unit::Year }, Recurrence::Every { n: 6, unit: Unit::Month }, Recurrence::Every { n: 2, unit: Unit::Week }, Recurrence::Every { n: 30, unit: Unit::Day }, Recurrence::Wyoming { month: 3 }, Recurrence::Yearly { month: 3, day: 31 }, Recurrence::Yearly { month: 2, day: 29 }] {
            assert_eq!(Recurrence::parse(&r.id()), Some(r), "{}", r.id());
        }
        assert_eq!(Recurrence::parse("every:0y"), None);
        assert_eq!(Recurrence::parse("wyoming:13"), None);
        assert_eq!(Recurrence::parse("yearly:02-30"), None);
        assert_eq!(Recurrence::parse("weird"), None);
        assert!(Kind::ALL.iter().all(|k| Kind::from_id(k.id()) == Some(*k)));
        assert!(Category::ALL.iter().all(|c| Category::from_id(c.id()) == Some(*c)));
    }

    #[test]
    fn next_dates() {
        let yearly = Recurrence::Every { n: 1, unit: Unit::Year };
        assert_eq!(yearly.next_after(d(2028, 2, 29)), Some(d(2029, 2, 28)));
        assert_eq!(Recurrence::Every { n: 1, unit: Unit::Month }.next_after(d(2027, 1, 31)), Some(d(2027, 2, 28)));
        assert_eq!(Recurrence::Wyoming { month: 3 }.next_after(d(2027, 3, 1)), Some(d(2028, 3, 1)));
        assert_eq!(Recurrence::Yearly { month: 3, day: 31 }.next_after(d(2027, 3, 31)), Some(d(2028, 3, 31)));
        assert_eq!(Recurrence::None.next_after(d(2027, 3, 31)), None);
    }

    #[test]
    fn deadlines_status_and_notice() {
        let today = d(2026, 11, 20);
        let mut c = Item::new(Kind::Contract, "Lease");
        c.expires = Some(d(2027, 1, 31));
        c.notice_days = Some(60);
        // notice must be given by Dec 2nd
        assert_eq!(c.due_date(today), Some(d(2026, 12, 2)));
        assert_eq!(c.days_left(today), Some(12));
        assert_eq!(c.status(today), Status::Soon);
        let mut w = Item::new(Kind::Warranty, "Laptop");
        w.issued = Some(d(2025, 12, 31));
        w.term_months = Some(24);
        assert_eq!(w.end_date(today), Some(d(2027, 12, 31)));
        assert_eq!(w.status(today), Status::Ok);
        let wy = Item { recurrence: Recurrence::Wyoming { month: 2 }, ..Item::new(Kind::WyomingReport, "Otherwise Technologies LLC") };
        assert_eq!(wy.due_date(today), Some(d(2027, 2, 1)));
        assert_eq!(wy.status(today), Status::Upcoming);
        let cc = Item::new(Kind::CamaraComercio, "Matrícula");
        assert_eq!(cc.due_date(today), Some(d(2027, 3, 31)));
        let mut p = Item::new(Kind::Passport, "Pasaporte");
        assert_eq!(p.status(today), Status::Undated);
        p.expires = Some(d(2026, 11, 19));
        assert_eq!(p.status(today), Status::Expired);
        p.expires = Some(today);
        assert_eq!(p.status(today), Status::Soon);
        assert_eq!(Status::of_days(Some(30)), Status::Soon);
        assert_eq!(Status::of_days(Some(31)), Status::Upcoming);
        assert_eq!(Status::of_days(Some(90)), Status::Upcoming);
        assert_eq!(Status::of_days(Some(91)), Status::Ok);
    }

    #[test]
    fn renewal_suggestions() {
        let today = d(2026, 11, 20);
        let mut soat = Item::new(Kind::Soat, "SOAT ABC123");
        soat.expires = Some(d(2026, 12, 5));
        assert_eq!(soat.suggest_next(today), d(2027, 12, 5));
        // lapsed for years: the next one counts from today
        soat.expires = Some(d(2023, 1, 10));
        assert_eq!(soat.suggest_next(today), d(2027, 11, 20));
        let mut pass = Item::new(Kind::Passport, "Pasaporte");
        pass.expires = Some(d(2027, 2, 1));
        assert_eq!(pass.suggest_next(today), d(2037, 2, 1));
        let mut sub = Item::new(Kind::Custom, "Thing");
        sub.issued = Some(d(2026, 6, 1));
        sub.expires = Some(d(2026, 12, 1));
        assert_eq!(sub.suggest_next(today), d(2027, 6, 1));
        let wy = Item { recurrence: Recurrence::Wyoming { month: 2 }, expires: Some(d(2027, 2, 1)), ..Item::new(Kind::WyomingReport, "LLC") };
        assert_eq!(wy.suggest_next(today), d(2028, 2, 1));
        assert_eq!(parse_leads("90, 30 7;1 30"), vec![90, 30, 7, 1]);
        assert_eq!(Item::new(Kind::Passport, "x").leads(), vec![270, 180, 90, 30]);
    }
}
