//! Suggested reminders per kind of entity (a vehicle's SOAT, a pet's vaccines, a company's
//! filings): one click fills the new-item form with the name, category, a typical interval and
//! lead times. Colombian-first. Templates never set a date: where a date is set by law or by an
//! authority (SOAT expiry, tax calendars), the user enters the real one, and filings carry a
//! "check the dates with your accountant" note.
use crate::entity::{Entity, EntityKind};
use crate::model::{Category, Item, Kind, Recurrence, Unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template {
    /// Stable id (`soat`, `pet_rabies`), used by the AI tools.
    pub id: &'static str,
    /// The kinds of entity it is offered for; the first is its home section.
    pub owners: &'static [EntityKind],
    pub kind: Kind,
    pub category: Category,
    /// A suggestion: the form lets the user change it.
    pub recurrence: Recurrence,
    /// Reminder lead times, in days before the deadline, largest first.
    pub lead: &'static [u32],
    pub term_months: Option<u32>,
    pub notice_days: Option<u32>,
    label: &'static str,
    note: Option<&'static str>,
}

const fn every(n: u32, unit: Unit) -> Recurrence {
    Recurrence::Every { n, unit }
}

const YEARLY: Recurrence = every(1, Unit::Year);
const NONE: Recurrence = Recurrence::None;

use EntityKind::{Company as Co, Digital as Dg, Other as Ot, Person as Pe, Pet as Pt, Property as Pr, Vehicle as Ve};

#[allow(clippy::too_many_arguments)]
const fn t(id: &'static str, owners: &'static [EntityKind], kind: Kind, category: Category, recurrence: Recurrence, lead: &'static [u32], label: &'static str, note: Option<&'static str>) -> Template {
    Template { id, owners, kind, category, recurrence, lead, term_months: None, notice_days: None, label, note }
}

const ACCOUNTANT: Option<&str> = Some("core.tplnote.accountant");
const CHECK: Option<&str> = Some("core.tplnote.check");

/// The catalogue, grouped by home section.
pub const ALL: &[Template] = &[
    // vehicles
    t("soat", &[Ve], Kind::Soat, Category::Vehicle, YEARLY, &[30, 7, 1], "core.tpl.soat", Some("core.tplnote.real_date")),
    t("tecnomecanica", &[Ve], Kind::Tecnomecanica, Category::Vehicle, YEARLY, &[30, 7, 1], "core.tpl.tecnomecanica", Some("core.tplnote.runt")),
    t("vehicle_insurance", &[Ve], Kind::VehicleInsurance, Category::Vehicle, YEARLY, &[30, 7, 1], "core.tpl.vehicle_insurance", None),
    t("vehicle_tax", &[Ve], Kind::Custom, Category::Vehicle, YEARLY, &[30, 7], "core.tpl.vehicle_tax", Some("core.tplnote.local_tax")),
    t("oil_change", &[Ve], Kind::Custom, Category::Vehicle, every(6, Unit::Month), &[14, 3], "core.tpl.oil_change", Some("core.tplnote.km")),
    t("brakes", &[Ve], Kind::Custom, Category::Vehicle, YEARLY, &[14], "core.tpl.brakes", Some("core.tplnote.km")),
    t("vehicle_registration", &[Ve], Kind::VehicleRegistration, Category::Vehicle, NONE, &[30, 7], "core.tpl.vehicle_registration", None),
    // pets
    t("pet_rabies", &[Pt], Kind::Vaccine, Category::Health, YEARLY, &[14, 3], "core.tpl.pet_rabies", Some("core.tplnote.vet")),
    t("pet_multiple", &[Pt], Kind::Vaccine, Category::Health, YEARLY, &[14, 3], "core.tpl.pet_multiple", Some("core.tplnote.vet")),
    t("pet_deworming", &[Pt], Kind::Custom, Category::Health, every(3, Unit::Month), &[7, 1], "core.tpl.pet_deworming", Some("core.tplnote.vet")),
    t("pet_fleas", &[Pt], Kind::Custom, Category::Health, every(1, Unit::Month), &[3, 1], "core.tpl.pet_fleas", None),
    t("pet_checkup", &[Pt], Kind::Checkup, Category::Health, YEARLY, &[14, 3], "core.tpl.pet_checkup", None),
    t("pet_grooming", &[Pt], Kind::Custom, Category::Other, every(1, Unit::Month), &[2], "core.tpl.pet_grooming", None),
    // people
    t("birthday", &[Pe], Kind::Custom, Category::Other, YEARLY, &[7, 1], "core.tpl.birthday", None),
    t("national_id", &[Pe], Kind::NationalId, Category::Identity, NONE, &[90, 30, 7], "core.tpl.national_id", Some("core.tplnote.private")),
    t("passport", &[Pe], Kind::Passport, Category::Identity, NONE, &[270, 180, 90, 30], "core.tpl.passport", Some("core.tplnote.private")),
    t("visa", &[Pe], Kind::Visa, Category::Identity, NONE, &[120, 60, 30, 7], "core.tpl.visa", None),
    t("driver_license", &[Pe], Kind::DriverLicense, Category::Identity, NONE, &[90, 30, 7], "core.tpl.driver_license", None),
    t("medical", &[Pe], Kind::Custom, Category::Health, NONE, &[7, 1], "core.tpl.medical", None),
    t("dentist", &[Pe], Kind::Custom, Category::Health, every(6, Unit::Month), &[7, 1], "core.tpl.dentist", None),
    t("lab_tests", &[Pe], Kind::Checkup, Category::Health, YEARLY, &[14, 3], "core.tpl.lab_tests", None),
    t("health_plan", &[Pe], Kind::Membership, Category::Health, YEARLY, &[30, 7], "core.tpl.health_plan", None),
    // companies
    t("camara_comercio", &[Co], Kind::CamaraComercio, Category::Company, YEARLY, &[60, 30, 7], "core.tpl.camara_comercio", ACCOUNTANT),
    t("rut", &[Co, Pe], Kind::Rut, Category::Company, NONE, &[30, 7], "core.tpl.rut", ACCOUNTANT),
    t("income_tax", &[Co, Pe], Kind::Custom, Category::Company, YEARLY, &[30, 7, 1], "core.tpl.income_tax", ACCOUNTANT),
    t("vat", &[Co], Kind::Custom, Category::Company, NONE, &[14, 3, 1], "core.tpl.vat", ACCOUNTANT),
    t("withholding", &[Co], Kind::Custom, Category::Company, every(1, Unit::Month), &[7, 3, 1], "core.tpl.withholding", ACCOUNTANT),
    t("annual_report", &[Co], Kind::WyomingReport, Category::Company, YEARLY, &[60, 30, 7], "core.tpl.annual_report", ACCOUNTANT),
    t("policy", &[Co, Pr, Pe], Kind::InsurancePolicy, Category::Insurance, YEARLY, &[60, 30, 7], "core.tpl.policy", None),
    Template { notice_days: Some(30), ..t("contract", &[Co, Pr, Ot], Kind::Contract, Category::Contract, NONE, &[90, 30, 7, 1], "core.tpl.contract", None) },
    t("domain", &[Dg, Co], Kind::Domain, Category::Digital, YEARLY, &[60, 30, 7, 1], "core.tpl.domain", CHECK),
    t("certificate", &[Dg, Co], Kind::Certificate, Category::Digital, NONE, &[30, 14, 7, 1], "core.tpl.certificate", CHECK),
    t("software_license", &[Dg, Co], Kind::Subscription, Category::Digital, YEARLY, &[30, 7, 1], "core.tpl.software_license", None),
    // home and property
    t("property_tax", &[Pr], Kind::Custom, Category::Home, YEARLY, &[30, 7], "core.tpl.property_tax", Some("core.tplnote.local_tax")),
    t("hoa_fee", &[Pr], Kind::Custom, Category::Home, every(1, Unit::Month), &[5, 1], "core.tpl.hoa_fee", None),
    t("utilities", &[Pr], Kind::Custom, Category::Home, every(1, Unit::Month), &[5, 1], "core.tpl.utilities", None),
    t("home_insurance", &[Pr], Kind::InsurancePolicy, Category::Insurance, YEARLY, &[60, 30, 7], "core.tpl.home_insurance", None),
    t("gas_inspection", &[Pr], Kind::GasInspection, Category::Home, every(5, Unit::Year), &[60, 30, 7], "core.tpl.gas_inspection", Some("core.tplnote.real_date")),
    t("water_heater", &[Pr], Kind::Custom, Category::Home, YEARLY, &[14], "core.tpl.water_heater", None),
    t("air_conditioning", &[Pr], Kind::Custom, Category::Home, every(6, Unit::Month), &[14], "core.tpl.air_conditioning", None),
    t("extinguisher", &[Pr, Co, Ve], Kind::Extinguisher, Category::Home, YEARLY, &[30, 7], "core.tpl.extinguisher", None),
    Template { term_months: Some(12), ..t("appliance_warranty", &[Pr, Ot], Kind::Warranty, Category::Warranty, NONE, &[30, 7], "core.tpl.appliance_warranty", None) },
    // digital
    t("subscription", &[Dg, Pe, Ot], Kind::Subscription, Category::Digital, YEARLY, &[30, 7, 1], "core.tpl.subscription", None),
];

/// The kinds of entity that get their own section, in sidebar order.
pub const SECTIONS: [EntityKind; 6] = [Ve, Pt, Pe, Co, Pr, Dg];

impl Template {
    pub fn label(&self) -> String {
        i18n::tr(self.label, &[])
    }

    pub fn note(&self) -> Option<String> {
        self.note.map(|k| i18n::tr(k, &[]))
    }

    /// The new item it suggests, belonging to `owner` when given; not saved, and without a
    /// date: the user sets it.
    pub fn item(&self, owner: Option<&Entity>) -> Item {
        let mut it = Item::new(self.kind, self.label());
        it.category = self.category;
        it.recurrence = self.recurrence;
        it.lead_days = self.lead.to_vec();
        it.term_months = self.term_months.or(it.term_months);
        it.notice_days = self.notice_days;
        it.notes = self.note().unwrap_or_default();
        if let Some(e) = owner {
            it.owner = e.name.clone();
            it.entity = Some(e.id);
            it.owner_kind = Some(e.kind);
        } else {
            it.owner_kind = Some(self.owners[0]);
        }
        it
    }

    /// Whether `items` (an entity's) already have this reminder: the same name, or the same
    /// non-generic type.
    pub fn present_in<'a>(&self, items: impl IntoIterator<Item = &'a Item>) -> bool {
        let label = self.label().to_lowercase();
        let unique = self.kind != Kind::Custom && ALL.iter().filter(|o| o.kind == self.kind).count() == 1;
        items.into_iter().any(|i| i.name.trim().to_lowercase() == label || (unique && i.kind == self.kind))
    }
}

pub fn by_id(id: &str) -> Option<&'static Template> {
    let id = id.trim().to_ascii_lowercase();
    ALL.iter().find(|t| t.id == id)
}

/// The templates offered for `kind`: its own first, then shared ones.
pub fn for_kind(kind: EntityKind) -> Vec<&'static Template> {
    let mut v: Vec<&Template> = ALL.iter().filter(|t| t.owners.contains(&kind)).collect();
    v.sort_by_key(|t| t.owners[0] != kind);
    v
}

/// The suggestions still missing for an entity of `kind` with `items`.
pub fn missing(kind: EntityKind, items: &[&Item]) -> Vec<&'static Template> {
    for_kind(kind).into_iter().filter(|t| !t.present_in(items.iter().copied())).collect()
}

/// The section a kind of entity is shown in; `Other` has none of its own.
pub fn has_section(kind: EntityKind) -> bool {
    SECTIONS.contains(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::State;

    #[test]
    fn every_template_is_valid() {
        let mut ids = std::collections::BTreeSet::new();
        for t in ALL {
            assert!(ids.insert(t.id), "duplicate id {}", t.id);
            assert_eq!(by_id(t.id).map(|x| x.id), Some(t.id));
            assert!(!t.owners.is_empty() && EntityKind::ALL.contains(&t.owners[0]), "{}: kind", t.id);
            assert!(Kind::ALL.contains(&t.kind) && Category::ALL.contains(&t.category), "{}", t.id);
            assert_eq!(Recurrence::parse(&t.recurrence.id()), Some(t.recurrence), "{}: interval", t.id);
            if let Recurrence::Every { n, .. } = t.recurrence {
                assert!((1..=10).contains(&n), "{}: interval", t.id);
            }
            assert!(!matches!(t.recurrence, Recurrence::Wyoming { .. } | Recurrence::Yearly { .. }), "{}: templates never fix a calendar date", t.id);
            assert!(!t.lead.is_empty() && t.lead.iter().all(|d| (1..=365).contains(d)), "{}: lead", t.id);
            assert!(t.lead.windows(2).all(|w| w[0] > w[1]), "{}: lead largest first", t.id);
            for lang in i18n::available().into_iter().map(|(c, _)| c) {
                let keys = i18n::keys(lang);
                assert!(keys.contains(&t.label), "{lang}: {}", t.label);
                if let Some(n) = t.note {
                    assert!(keys.contains(&n), "{lang}: {n}");
                }
            }
        }
        for k in SECTIONS {
            assert!(for_kind(k).len() >= 3, "{k:?} has suggestions");
            assert_eq!(for_kind(k)[0].owners[0], k, "own templates first");
        }
        assert!(by_id("SOAT").is_some() && by_id("nope").is_none());
    }

    #[test]
    fn filings_say_to_check_with_the_accountant() {
        for id in ["camara_comercio", "rut", "income_tax", "vat", "withholding"] {
            assert_eq!(by_id(id).unwrap().note, ACCOUNTANT, "{id}");
        }
    }

    #[test]
    fn creating_from_a_template() {
        let car = Entity { id: 3, kind: Ve, name: "Mazda ABC123".into(), ..Default::default() };
        let soat = by_id("soat").unwrap();
        let it = soat.item(Some(&car));
        assert_eq!((it.kind, it.category, it.recurrence), (Kind::Soat, Category::Vehicle, YEARLY));
        assert_eq!((it.entity, it.owner.as_str(), it.owner_kind), (Some(3), "Mazda ABC123", Some(Ve)));
        assert_eq!(it.lead_days, vec![30, 7, 1]);
        assert_eq!((it.id, it.expires, it.issued), (0, None, None), "not saved, the user sets the date");
        assert!(!it.name.is_empty() && !it.notes.is_empty());
        // Cámara de Comercio's own rule (March 31) is not prefilled: the user enters the date
        let cc = by_id("camara_comercio").unwrap().item(None);
        assert_eq!((cc.recurrence, cc.owner_kind, cc.entity), (YEARLY, Some(Co), None));
        assert_eq!(cc.end_date(crate::date::ymd(2026, 11, 20).unwrap()), None);
        let w = by_id("appliance_warranty").unwrap().item(None);
        assert_eq!(w.term_months, Some(12));
        assert_eq!(by_id("contract").unwrap().item(None).notice_days, Some(30));
        // what's already there isn't suggested again
        let have = [it, Item { state: State::Active, ..Item::new(Kind::Custom, by_id("oil_change").unwrap().label()) }];
        let refs: Vec<&Item> = have.iter().collect();
        let left: Vec<&str> = missing(Ve, &refs).iter().map(|t| t.id).collect();
        assert!(!left.contains(&"soat") && !left.contains(&"oil_change") && left.contains(&"tecnomecanica"));
        // a generic type (custom) never counts as present by type alone
        assert!(!by_id("brakes").unwrap().present_in(refs.iter().copied()));
    }
}
