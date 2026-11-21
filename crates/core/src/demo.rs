//! A few made-up items to try Vigía with (`--demo`, only into an empty database the user
//! points at; never the default one).
use crate::date::{self, Date};
use crate::model::*;

pub fn items(today: Date) -> Vec<Item> {
    let at = |days: i64| Some(date::add_days(today, days));
    let mut v = Vec::new();
    let mut p = Item::new(Kind::Passport, i18n::t!("core.demo.passport"));
    p.owner = "Ana".into(); // i18n-ok: demo person
    p.expires = at(200);
    p.issued = Some(date::add_years(today, -10));
    p.reference = "AX4471982".into();
    v.push(p);
    let mut s = Item::new(Kind::Soat, i18n::t!("core.demo.soat"));
    s.owner = "Mazda ABC123".into(); // i18n-ok: demo plate
    s.expires = at(12);
    s.cost = Some(892_000.0);
    s.currency = "COP".into();
    v.push(s);
    let mut t = Item::new(Kind::Tecnomecanica, i18n::t!("core.demo.tecno"));
    t.owner = "Mazda ABC123".into(); // i18n-ok: demo plate
    t.expires = at(-3);
    v.push(t);
    let mut d = Item::new(Kind::Domain, "example.com");
    d.host = "example.com".into();
    d.expires = at(48);
    d.provider = "Example Registrar".into(); // i18n-ok: demo provider
    d.auto_renews = true;
    v.push(d);
    let mut c = Item::new(Kind::Certificate, i18n::t!("core.demo.cert"));
    c.host = "example.com".into();
    c.expires = at(25);
    v.push(c);
    let mut w = Item::new(Kind::Warranty, i18n::t!("core.demo.warranty"));
    w.issued = Some(date::add_months(today, -10));
    w.term_months = Some(12);
    w.provider = "Alkosto".into(); // i18n-ok: demo store
    v.push(w);
    let mut l = Item::new(Kind::Contract, i18n::t!("core.demo.lease"));
    l.expires = at(95);
    l.notice_days = Some(60);
    v.push(l);
    let mut wy = Item::new(Kind::WyomingReport, "Demo Ventures LLC"); // i18n-ok: demo company
    wy.recurrence = Recurrence::Wyoming { month: date::add_months(today, 4).format("%m").to_string().parse().unwrap_or(3) };
    v.push(wy);
    let cc = Item::new(Kind::CamaraComercio, i18n::t!("core.demo.camara"));
    v.push(cc);
    let mut n = Item::new(Kind::Subscription, i18n::t!("core.demo.sub"));
    n.expires = at(160);
    n.cost = Some(99.0);
    n.currency = "USD".into();
    n.auto_renews = true;
    v.push(n);
    let mut x = Item::new(Kind::Extinguisher, i18n::t!("core.demo.extinguisher"));
    x.expires = at(300);
    v.push(x);
    v
}
