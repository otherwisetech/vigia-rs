//! Made-up people, vehicles, companies and items to try Vigía with (`--demo`, only into an
//! empty database the user points at; never the default one).
use crate::date::{self, Date};
use crate::db::{Result, Store};
use crate::entity::{Entity, EntityKind, Role};
use crate::model::*;

pub fn entities() -> Vec<Entity> {
    let e = |kind, name: &str| Entity::new(kind, name);
    vec![
        e(EntityKind::Person, "Ana"),  // i18n-ok: demo person
        e(EntityKind::Person, "Juan"), // i18n-ok: demo person
        Entity { detail: "Mazda 3".into(), year: Some(2019), ..e(EntityKind::Vehicle, "Mazda ABC123") }, // i18n-ok: demo vehicle
        Entity { country: "US".into(), ..e(EntityKind::Company, "Otherwise Technologies LLC") },       // i18n-ok: demo company
        Entity { country: "CO".into(), ..e(EntityKind::Company, "Otherwise SAS") },                    // i18n-ok: demo company
        Entity { ident: "Calle 10 # 4-20, apto 301".into(), ..e(EntityKind::Property, "Apto 301") },   // i18n-ok: demo home
        Entity { detail: "Google".into(), ..e(EntityKind::Digital, "Google Workspace") },              // i18n-ok: demo account
        Entity { detail: "Criollo".into(), year: Some(2021), ..e(EntityKind::Pet, "Luna") },           // i18n-ok: demo pet
    ]
}

/// (from, to, role) by entity name.
pub fn relations() -> Vec<(&'static str, &'static str, Role)> {
    vec![
        ("Juan", "Otherwise Technologies LLC", Role::Member), // i18n-ok: demo names
        ("Juan", "Otherwise SAS", Role::Owner),               // i18n-ok: demo names
        ("Juan", "Mazda ABC123", Role::Owner),                // i18n-ok: demo names
        ("Juan", "Apto 301", Role::Uses),                     // i18n-ok: demo names
        ("Ana", "Juan", Role::Family),                        // i18n-ok: demo names
        ("Otherwise SAS", "Google Workspace", Role::Owner),   // i18n-ok: demo names
        ("Ana", "Luna", Role::Owner),                         // i18n-ok: demo names
        ("Ana", "Mazda ABC123", Role::Owner),                 // i18n-ok: demo names
    ]
}

pub fn items(today: Date) -> Vec<Item> {
    let at = |days: i64| Some(date::add_days(today, days));
    let of = |mut it: Item, owner: &str, kind: EntityKind| {
        it.owner = owner.into();
        it.owner_kind = Some(kind);
        it
    };
    let mut v = Vec::new();
    let mut p = of(Item::new(Kind::Passport, i18n::t!("core.demo.passport")), "Ana", EntityKind::Person); // i18n-ok: demo person
    p.expires = at(200);
    p.issued = Some(date::add_years(today, -10));
    p.reference = "AX4471982".into();
    v.push(p);
    let mut lic = of(Item::new(Kind::DriverLicense, Kind::DriverLicense.label()), "Juan", EntityKind::Person); // i18n-ok: demo person
    lic.expires = at(410);
    v.push(lic);
    let mut s = of(Item::new(Kind::Soat, i18n::t!("core.demo.soat")), "Mazda ABC123", EntityKind::Vehicle); // i18n-ok: demo plate
    s.expires = at(12);
    s.cost = Some(892_000.0);
    s.currency = "COP".into();
    v.push(s);
    let mut t = of(Item::new(Kind::Tecnomecanica, i18n::t!("core.demo.tecno")), "Mazda ABC123", EntityKind::Vehicle); // i18n-ok: demo plate
    t.expires = at(-3);
    v.push(t);
    let mut d = of(Item::new(Kind::Domain, "otherwisetech.com"), "Otherwise SAS", EntityKind::Company); // i18n-ok: demo company
    d.host = "otherwisetech.com".into();
    d.expires = at(48);
    d.provider = "Example Registrar".into(); // i18n-ok: demo provider
    d.auto_renews = true;
    v.push(d);
    let mut c = of(Item::new(Kind::Certificate, i18n::t!("core.demo.cert")), "Otherwise SAS", EntityKind::Company); // i18n-ok: demo company
    c.host = "otherwisetech.com".into();
    c.expires = at(25);
    v.push(c);
    let cc = of(Item::new(Kind::CamaraComercio, i18n::t!("core.demo.camara")), "Otherwise SAS", EntityKind::Company); // i18n-ok: demo company
    v.push(cc);
    let mut rut = of(Item::new(Kind::Rut, Kind::Rut.label()), "Otherwise SAS", EntityKind::Company); // i18n-ok: demo company
    rut.expires = at(140);
    v.push(rut);
    let mut wy = of(Item::new(Kind::WyomingReport, Kind::WyomingReport.label()), "Otherwise Technologies LLC", EntityKind::Company); // i18n-ok: demo company
    wy.recurrence = Recurrence::Wyoming { month: date::add_months(today, 4).format("%m").to_string().parse().unwrap_or(3) };
    v.push(wy);
    let mut w = of(Item::new(Kind::Warranty, i18n::t!("core.demo.warranty")), "Juan", EntityKind::Person); // i18n-ok: demo person
    w.issued = Some(date::add_months(today, -10));
    w.term_months = Some(12);
    w.provider = "Alkosto".into(); // i18n-ok: demo store
    v.push(w);
    let mut l = of(Item::new(Kind::Contract, i18n::t!("core.demo.lease")), "Apto 301", EntityKind::Property); // i18n-ok: demo home
    l.expires = at(95);
    l.notice_days = Some(60);
    v.push(l);
    let mut n = of(Item::new(Kind::Subscription, i18n::t!("core.demo.sub")), "Google Workspace", EntityKind::Digital); // i18n-ok: demo account
    n.expires = at(160);
    n.cost = Some(99.0);
    n.currency = "USD".into();
    n.auto_renews = true;
    v.push(n);
    let mut vc = of(Item::new(Kind::Domain, "vcity.io"), "Otherwise Technologies LLC", EntityKind::Company); // i18n-ok: demo domain
    vc.expires = at(230);
    v.push(vc);
    let mut b = of(Item::new(Kind::Birthday, Kind::Birthday.label()), "Juan", EntityKind::Person); // i18n-ok: demo person
    b.issued = Some(date::add_years(date::add_days(today, 9), -36));
    b.tidy(today);
    v.push(b);
    let mut x = of(Item::new(Kind::Extinguisher, i18n::t!("core.demo.extinguisher")), "Apto 301", EntityKind::Property); // i18n-ok: demo home
    x.expires = at(300);
    v.push(x);
    let luna = Entity { id: 0, kind: EntityKind::Pet, name: "Luna".into(), ..Default::default() }; // i18n-ok: demo pet
    for (id, days) in [("pet_rabies", 21), ("pet_deworming", -2)] {
        let mut it = crate::templates::by_id(id).map(|t| t.item(Some(&luna))).unwrap_or_default();
        it.entity = None;
        it.expires = at(days);
        v.push(it);
    }
    v
}

/// Fills an (empty) store with the demo entities, their relations and items.
pub fn seed(store: &Store, today: Date) -> Result<()> {
    for e in entities() {
        store.insert_entity(&e)?;
    }
    let all = store.entities()?;
    let id = |n: &str| all.iter().find(|e| e.name == n).map(|e| e.id);
    for (a, b, role) in relations() {
        if let (Some(a), Some(b)) = (id(a), id(b)) {
            store.relate(a, b, role)?;
        }
    }
    for mut it in items(today) {
        // a domain both companies hold
        if it.name == "otherwisetech.com" {
            it.co_owners = id("Otherwise Technologies LLC").into_iter().collect(); // i18n-ok: demo company
        }
        store.insert(&it)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn seeds_entities_relations_and_items() {
        let dir = tempfile::tempdir().unwrap();
        let s = crate::db::Store::create(&dir.path().join("d.db")).unwrap();
        super::seed(&s, crate::date::ymd(2026, 11, 20).unwrap()).unwrap();
        assert_eq!(s.entities().unwrap().len(), super::entities().len());
        assert_eq!(s.relations().unwrap().len(), super::relations().len());
        let items = s.items().unwrap();
        assert!(items.iter().all(|i| i.entity.is_some()));
        let sas = s.entities().unwrap().into_iter().find(|e| e.name == "Otherwise SAS").unwrap();
        assert!(items.iter().any(|i| i.kind == crate::model::Kind::Domain && i.entity == Some(sas.id)));
    }
}
