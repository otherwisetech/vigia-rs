//! Which fields each item type asks for, and what it calls them: a birthday asks for a date of
//! birth and nothing about cost or renewal; a domain for its name, expiry, registrar and the
//! automatic check; a passport for its dates and number. Data, like the reminder templates; the
//! form and the detail panel only read it. A field the type doesn't list still shows when the
//! item already has a value in it, so nothing typed before is hidden.
use crate::model::{Item, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Issued,
    Expires,
    /// Warranty months.
    Term,
    /// Contracts: notice days.
    Notice,
    Recurrence,
    BusinessDays,
    Host,
    AutoCheck,
    Cost,
    Provider,
    Reference,
    RenewalUrl,
    AutoRenews,
    Leads,
    Tags,
    Notes,
}

/// One type's form: its fields in order and the labels that differ from the generic ones
/// (catalog keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    pub fields: &'static [Field],
    pub issued: &'static str,
    pub expires: &'static str,
    pub provider: &'static str,
    pub reference: &'static str,
    pub host: &'static str,
}

use Field::*;

const BASE: Spec = Spec { fields: &[], issued: "core.field.issued", expires: "core.field.expires", provider: "core.field.provider", reference: "core.field.reference", host: "core.field.host" };

const fn spec(fields: &'static [Field]) -> Spec {
    Spec { fields, ..BASE }
}

const DOCUMENT: Spec = Spec { reference: "core.field.doc_number", provider: "core.field.issued_by", ..spec(&[Issued, Expires, Reference, Provider, Leads, Tags, Notes]) };
const VEHICLE: Spec = Spec { reference: "core.field.policy_number", ..spec(&[Issued, Expires, Recurrence, Provider, Reference, Cost, RenewalUrl, AutoRenews, Leads, Tags, Notes]) };
const FILING: Spec = Spec { expires: "core.field.due", ..spec(&[Expires, Recurrence, BusinessDays, Cost, Provider, Reference, RenewalUrl, Leads, Tags, Notes]) };
const CARE: Spec = Spec { issued: "core.field.last_done", expires: "core.field.next_due", ..spec(&[Issued, Expires, Recurrence, Provider, Reference, Cost, Leads, Tags, Notes]) };
const ALL: Spec = spec(&[Issued, Expires, Recurrence, BusinessDays, Cost, Provider, Reference, RenewalUrl, AutoRenews, Leads, Tags, Notes]);

impl Kind {
    pub fn form(self) -> Spec {
        use Kind::*;
        match self {
            Passport | NationalId | DriverLicense | Visa | ResidencePermit => DOCUMENT,
            Soat | Tecnomecanica | VehicleInsurance => VEHICLE,
            VehicleRegistration => Spec { reference: "core.field.doc_number", ..spec(&[Issued, Expires, Reference, Cost, Leads, Tags, Notes]) },
            InsurancePolicy => Spec { issued: "core.field.start", expires: "core.field.ends", provider: "core.field.insurer", reference: "core.field.policy_number", ..spec(&[Issued, Expires, Recurrence, Provider, Reference, Cost, AutoRenews, RenewalUrl, Leads, Tags, Notes]) },
            Warranty => Spec { issued: "core.field.purchased", provider: "core.field.store", reference: "core.field.serial", ..spec(&[Issued, Term, Expires, Provider, Reference, Cost, Leads, Tags, Notes]) },
            Domain => Spec { host: "core.field.domain", provider: "core.field.registrar", ..spec(&[Host, Expires, AutoCheck, Provider, Cost, AutoRenews, RenewalUrl, Recurrence, Leads, Tags, Notes]) },
            Certificate => Spec { provider: "core.field.issuer", ..spec(&[Host, Expires, AutoCheck, Issued, Provider, AutoRenews, Leads, Tags, Notes]) },
            Subscription => Spec { expires: "core.field.next_renewal", provider: "core.field.service", reference: "core.field.account", ..spec(&[Expires, Recurrence, Cost, Provider, AutoRenews, RenewalUrl, Reference, Leads, Tags, Notes]) },
            Contract => Spec { issued: "core.field.start", expires: "core.field.ends", provider: "core.field.other_party", ..spec(&[Issued, Expires, Notice, Recurrence, BusinessDays, Provider, Reference, Cost, AutoRenews, RenewalUrl, Leads, Tags, Notes]) },
            WyomingReport | CamaraComercio | Rut => FILING,
            Membership => Spec { reference: "core.field.member_number", ..spec(&[Issued, Expires, Recurrence, Cost, Provider, Reference, AutoRenews, RenewalUrl, Leads, Tags, Notes]) },
            Vaccine | Checkup => Spec { provider: "core.field.clinic", ..CARE },
            GasInspection | Extinguisher => CARE,
            Birthday => Spec { issued: "core.field.birth_date", ..spec(&[Issued, Leads, Tags, Notes]) },
            Anniversary => Spec { issued: "core.field.date", ..spec(&[Issued, Leads, Tags, Notes]) },
            Custom => ALL,
        }
    }
}

impl Spec {
    pub fn has(&self, f: Field) -> bool {
        self.fields.contains(&f)
    }

    /// Whether the form and the detail show `f` for `it`: the type asks for it, or the item
    /// already holds something there.
    pub fn shows(&self, f: Field, it: &Item) -> bool {
        self.has(f) || has_value(f, it)
    }

    pub fn label(&self, f: Field) -> String {
        let key = match f {
            Issued => self.issued,
            Expires => self.expires,
            Provider => self.provider,
            Reference => self.reference,
            Host => self.host,
            Term => "core.field.term",
            Notice => "core.field.notice",
            Recurrence => "core.field.recurrence",
            BusinessDays => "core.field.business_days",
            AutoCheck => "core.field.auto_check",
            Cost => "core.field.cost",
            RenewalUrl => "core.field.url",
            AutoRenews => "core.field.auto_renews",
            Leads => "core.field.leads",
            Tags => "core.field.tags",
            Notes => "core.field.notes",
        };
        i18n::tr(key, &[])
    }
}

/// Whether `it` has something in `f` worth keeping on screen.
pub fn has_value(f: Field, it: &Item) -> bool {
    match f {
        Issued => it.issued.is_some(),
        Expires => it.expires.is_some(),
        Term => it.term_months.is_some() && it.kind == Kind::Warranty,
        Notice => it.notice_days.is_some_and(|n| n > 0),
        Recurrence => !it.kind.is_celebration() && it.recurrence != crate::model::Recurrence::None,
        BusinessDays => it.business_days.is_some(),
        Host => !it.host.trim().is_empty(),
        AutoCheck => it.auto_check && it.kind.checkable(),
        Cost => it.cost.is_some(),
        Provider => !it.provider.trim().is_empty(),
        Reference => !it.reference.trim().is_empty(),
        RenewalUrl => !it.renewal_url.trim().is_empty(),
        AutoRenews => it.auto_renews,
        Leads | Tags | Notes => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_type_asks_for_what_it_needs() {
        let b = Kind::Birthday.form();
        assert_eq!(b.fields, &[Issued, Leads, Tags, Notes]);
        let bday = Item::new(Kind::Birthday, "Cumpleaños");
        for f in [Cost, Provider, RenewalUrl, AutoRenews, Expires, Recurrence] {
            assert!(!b.shows(f, &bday), "{f:?}");
        }
        // a value typed before (a cost on an old birthday) stays visible
        assert!(b.shows(Cost, &Item { cost: Some(10.), ..bday.clone() }));
        let d = Kind::Domain.form();
        for f in [Host, Expires, Provider, Cost, AutoRenews, AutoCheck] {
            assert!(d.has(f), "{f:?}");
        }
        assert!(!d.has(Issued));
        assert!(Kind::Passport.form().has(Reference) && !Kind::Passport.form().has(Cost));
        assert!(Kind::Contract.form().has(Notice) && Kind::Warranty.form().has(Term));
        assert!(Kind::Custom.form().fields.len() > 10);
        for k in Kind::ALL {
            let s = k.form();
            assert!(s.has(Notes) && s.has(Leads), "{k:?}");
            assert_eq!(s.has(AutoCheck), k.checkable(), "{k:?}");
            assert_eq!(s.has(Host), k.checkable(), "{k:?}");
            for lang in i18n::available().into_iter().map(|(c, _)| c) {
                let keys = i18n::keys(lang);
                for key in [s.issued, s.expires, s.provider, s.reference, s.host] {
                    assert!(keys.contains(&key), "{lang}: {key}");
                }
            }
        }
    }
}
