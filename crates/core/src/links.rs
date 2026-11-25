//! Deep links into Vigía (`otherwise://vigia/…` or `otherwise-vigia://…`, see otherwise-link's
//! SPEC): what each path means, and the prefilled new item of `new?…`.
//!
//! | Path | Opens |
//! |------|-------|
//! | (empty) | the window |
//! | `item/<id>` | that item |
//! | `owner/<id>` | that entity's page (Vigía's own id) |
//! | `entity/<uuid>` | the entity linked to that shared card |
//! | `new?…` | the new-item form, prefilled (nothing is saved until the user does) |
//!
//! `new` takes `text` (a quick-add line, parsed first), then `kind` (a type id such as
//! `certificate`, `domain`, `warranty`), `name`, `owner`, `owner_kind`, `entity` (a shared card
//! id), `expires` / `issued` (`YYYY-MM-DD`), `months`, `host`, `provider`, `reference`, `url`,
//! `notes`, `tags` (comma separated) and `check=1` (daily RDAP / TLS check).
use crate::date::{self, Date, Order};
use crate::entity::{EntityId, EntityKind};
use crate::model::{Item, ItemId, Kind};
use otherwise_link::Link;
use uuid::Uuid;

pub const APP: &str = crate::sync::APP;
pub const BUNDLE_ID: &str = "io.github.otherwisetech.vigia-app";
pub const SCHEME: &str = "otherwise-vigia";
/// The Linux `.desktop` file, named after the bundle id as otherwise-link expects.
pub const DESKTOP_FILE: &str = "io.github.otherwisetech.vigia-app.desktop";

#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Focus,
    Item(ItemId),
    Owner(EntityId),
    Shared(Uuid),
    /// A section page: `section/digital`, `section/vehicle`…
    Section(EntityKind),
    New(Box<Prefill>),
    /// Someone else's link, or a path this version doesn't know.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prefill {
    pub text: Option<String>,
    pub kind: Option<Kind>,
    pub name: Option<String>,
    pub owner: Option<String>,
    pub owner_kind: Option<EntityKind>,
    pub entity: Option<Uuid>,
    pub expires: Option<Date>,
    pub issued: Option<Date>,
    pub months: Option<u32>,
    pub host: Option<String>,
    pub provider: Option<String>,
    pub reference: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub check: bool,
}

pub fn route(l: &Link) -> Route {
    if l.app != APP {
        return Route::Unknown;
    }
    let id = |i: usize| l.at(i).and_then(|s| s.parse::<i64>().ok());
    match l.at(0) {
        None => Route::Focus,
        Some("item") => id(1).map_or(Route::Unknown, Route::Item),
        Some("owner") => id(1).map_or(Route::Unknown, Route::Owner),
        Some("entity") => l.at(1).and_then(|s| Uuid::parse_str(s).ok()).map_or(Route::Unknown, Route::Shared),
        Some("new") => Route::New(Box::new(Prefill::from_link(l))),
        Some("section") => l.at(1).and_then(EntityKind::from_id).map_or(Route::Unknown, Route::Section),
        Some(_) => Route::Unknown,
    }
}

pub fn item_link(id: ItemId) -> Link {
    Link::new(APP).segment("item").segment(id.to_string())
}

pub fn owner_link(id: EntityId) -> Link {
    Link::new(APP).segment("owner").segment(id.to_string())
}

impl Prefill {
    pub fn from_link(l: &Link) -> Prefill {
        let s = |k: &str| l.get(k).map(str::trim).filter(|v| !v.is_empty()).map(String::from);
        Prefill {
            text: s("text"),
            kind: s("kind").and_then(|k| Kind::from_id(&k)),
            name: s("name"),
            owner: s("owner"),
            owner_kind: s("owner_kind").and_then(|k| EntityKind::from_id(&k)),
            entity: s("entity").and_then(|u| Uuid::parse_str(&u).ok()),
            expires: s("expires").and_then(|d| date::parse_iso(&d)),
            issued: s("issued").and_then(|d| date::parse_iso(&d)),
            months: s("months").and_then(|m| m.parse().ok()),
            host: s("host"),
            provider: s("provider"),
            reference: s("reference"),
            url: s("url"),
            notes: s("notes"),
            tags: s("tags").map(|t| t.split(',').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default(),
            check: matches!(l.get("check"), Some("1" | "true" | "yes")),
        }
    }

    /// The link that opens this prefill (what other apps build).
    pub fn to_link(&self) -> Link {
        let mut l = Link::new(APP).segment("new");
        let mut put = |k: &str, v: Option<String>| {
            if let Some(v) = v {
                l = std::mem::take(&mut l).param(k, v);
            }
        };
        put("text", self.text.clone());
        put("kind", self.kind.map(|k| k.id().to_string()));
        put("name", self.name.clone());
        put("owner", self.owner.clone());
        put("owner_kind", self.owner_kind.map(|k| k.id().to_string()));
        put("entity", self.entity.map(|u| u.to_string()));
        put("expires", self.expires.map(date::iso));
        put("issued", self.issued.map(date::iso));
        put("months", self.months.map(|m| m.to_string()));
        put("host", self.host.clone());
        put("provider", self.provider.clone());
        put("reference", self.reference.clone());
        put("url", self.url.clone());
        put("notes", self.notes.clone());
        put("tags", (!self.tags.is_empty()).then(|| self.tags.join(",")));
        put("check", self.check.then(|| "1".to_string()));
        l
    }

    /// The unsaved item the form starts from. `owner_of` names the local entity linked to a
    /// shared card id (`entity=`), when there is one.
    pub fn to_item(&self, today: Date, order: Order, owner_of: impl Fn(Uuid) -> Option<(EntityId, String, EntityKind)>) -> Item {
        let mut it = match (&self.text, self.kind) {
            (Some(t), _) => {
                let mut it = crate::quick::parse(t, today, order);
                if let Some(k) = self.kind.filter(|k| *k != it.kind) {
                    it = Item { kind: k, category: k.category(), recurrence: k.recurrence(), lead_days: k.lead_days(), ..it };
                }
                it
            }
            (None, Some(k)) => Item::new(k, ""),
            (None, None) => Item::new(Kind::Custom, ""),
        };
        if let Some(n) = &self.name {
            it.name = n.clone();
        }
        if let Some((id, name, kind)) = self.entity.and_then(&owner_of) {
            it.entity = Some(id);
            it.owner = name;
            it.owner_kind = Some(kind);
        } else if let Some(o) = &self.owner {
            it.owner = o.clone();
            it.entity = None;
            it.owner_kind = self.owner_kind.or(it.owner_kind);
        }
        if self.expires.is_some() {
            it.expires = self.expires;
        }
        if self.issued.is_some() {
            it.issued = self.issued;
        }
        if self.months.is_some() {
            it.term_months = self.months;
        }
        let set = |dst: &mut String, v: &Option<String>| {
            if let Some(v) = v {
                *dst = v.clone();
            }
        };
        set(&mut it.host, &self.host);
        set(&mut it.provider, &self.provider);
        set(&mut it.reference, &self.reference);
        set(&mut it.renewal_url, &self.url);
        set(&mut it.notes, &self.notes);
        if !self.tags.is_empty() {
            it.tags = self.tags.clone();
        }
        if self.check && it.kind.checkable() {
            it.auto_check = true;
        }
        if it.name.trim().is_empty() {
            it.name = it.host.clone();
        }
        it
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    #[test]
    fn routes() {
        let r = |s: &str| route(&Link::parse(s).unwrap());
        assert_eq!(r("otherwise://vigia"), Route::Focus);
        assert_eq!(r("otherwise-vigia://item/42"), Route::Item(42));
        assert_eq!(r("otherwise://vigia/owner/7"), Route::Owner(7));
        let u = Uuid::new_v4();
        assert_eq!(r(&format!("otherwise://vigia/entity/{u}")), Route::Shared(u));
        assert_eq!(r("otherwise://vigia/entity/nope"), Route::Unknown);
        assert_eq!(r("otherwise://vigia/item/x"), Route::Unknown);
        assert_eq!(r("otherwise://nodo/item/1"), Route::Unknown);
        assert_eq!(r("otherwise://vigia/weird"), Route::Unknown);
        assert_eq!(r("otherwise://vigia/section/digital"), Route::Section(EntityKind::Digital));
        assert_eq!(r("otherwise://vigia/section/spaceship"), Route::Unknown);
        assert_eq!(item_link(42).to_url(), "otherwise://vigia/item/42");
        assert_eq!(item_link(42).app_url(), "otherwise-vigia://item/42");
    }

    #[test]
    fn prefilled_new_item() {
        let today = ymd(2026, 11, 21).unwrap();
        let p = Prefill { kind: Some(Kind::Certificate), name: Some("SSL otherwisetech.com".into()), owner: Some("otherwisetech.com".into()), owner_kind: Some(EntityKind::Digital), expires: ymd(2027, 1, 15), host: Some("otherwisetech.com:443".into()), check: true, ..Default::default() };
        let l = p.to_link();
        assert!(l.to_url().starts_with("otherwise://vigia/new?kind=certificate&name=SSL%20otherwisetech.com"));
        let Route::New(back) = route(&Link::parse(&l.to_url()).unwrap()) else { panic!("not a new route") };
        assert_eq!(*back, p);
        let it = back.to_item(today, Order::DayFirst, |_| None);
        assert_eq!((it.kind, it.name.as_str(), it.owner.as_str(), it.owner_kind, it.expires, it.host.as_str(), it.auto_check, it.id), (Kind::Certificate, "SSL otherwisetech.com", "otherwisetech.com", Some(EntityKind::Digital), ymd(2027, 1, 15), "otherwisetech.com:443", true, 0));
        // a quick-add line, then the explicit fields on top; a shared card names the owner
        let u = Uuid::new_v4();
        let l = Link::parse(&format!("otherwise://vigia/new?text=Pasaporte+%40Ana+12%2F05%2F2031&entity={u}&notes=x")).unwrap();
        let Route::New(p) = route(&l) else { panic!() };
        let it = p.to_item(today, Order::DayFirst, |id| (id == u).then(|| (3, "Ana María".to_string(), EntityKind::Person)));
        assert_eq!((it.kind, it.expires, it.entity, it.owner.as_str(), it.notes.as_str()), (Kind::Passport, ymd(2031, 5, 12), Some(3), "Ana María", "x"));
        // a warranty from a device: name falls back to nothing, months kept
        let it = Prefill { kind: Some(Kind::Warranty), name: Some("MacBook".into()), issued: ymd(2026, 1, 2), months: Some(24), ..Default::default() }.to_item(today, Order::DayFirst, |_| None);
        assert_eq!(it.end_date(today), ymd(2028, 1, 2));
    }
}
