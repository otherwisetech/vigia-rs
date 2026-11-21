//! Two-way sync of Vigía's entities with the shared registry of the Otherwise apps
//! (`otherwise_link::registry`), only when the user turned the registry on in Otherwise Apps.
//!
//! - **Push**: every entity of a shared kind (person, company, vehicle, property, digital) whose
//!   revision changed since the last sync becomes an upsert, by its card id once known, else by
//!   its ref (`entity-<id>@<database uid>`, so two Vigía databases never claim each other's
//!   cards), else by natural key (tax id, plate, domain). The card id is stored with the entity.
//! - **Pull**: what other apps changed since Vigía's cursor. A card already linked (by id or ref)
//!   updates the entity; else one with the same identifier or the same name and kind is linked
//!   instead of creating a twin; else a new entity is created (except genealogy-only people, so a
//!   family tree doesn't flood the sidebar). Merges repoint the link to the survivor; a deleted
//!   card leaves the entity in place, detached for good.
//! - Deleting an entity in Vigía unlinks its ref; the card stays for the other apps.
//!
//! Every registry call is one short transaction; the caller runs this off the UI thread.
use crate::db::{self, Store, StoreError};
use crate::entity::{Entity, EntityKind, normalize_ident};
use otherwise_link::entity::{self as card, Entity as Card, Kind as CardKind};
use otherwise_link::registry::{Error as RegError, Op, Registry};
use otherwise_link::tax::TaxId;
use otherwise_link::Root;
use serde_json::json;
use uuid::Uuid;

pub const APP: &str = "vigia";
/// `shared_id` of an entity whose card was deleted in the registry: never pushed again.
pub const DETACHED: &str = "detached";
const UID_KEY: &str = "shared_uid";
const SYNCED_KEY: &str = "shared_synced";
const BATCH: usize = 500;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub pushed: usize,
    pub pulled: usize,
    /// Entities created here from other apps' cards.
    pub created: usize,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub enum SyncError {
    Store(StoreError),
    Registry(RegError),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Store(e) => write!(f, "{e}"),
            SyncError::Registry(e) => write!(f, "{}", i18n::t!("core.sync.error", detail = e)),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<StoreError> for SyncError {
    fn from(e: StoreError) -> Self {
        SyncError::Store(e)
    }
}

impl From<RegError> for SyncError {
    fn from(e: RegError) -> Self {
        SyncError::Registry(e)
    }
}

pub type Result<T> = std::result::Result<T, SyncError>;

pub fn card_kind(k: EntityKind) -> Option<CardKind> {
    match k {
        EntityKind::Person => Some(CardKind::Person),
        EntityKind::Company => Some(CardKind::Company),
        EntityKind::Vehicle => Some(CardKind::Vehicle),
        EntityKind::Property => Some(CardKind::Property),
        EntityKind::Digital => Some(CardKind::Digital),
        EntityKind::Pet | EntityKind::Other => None,
    }
}

pub fn local_kind(k: CardKind) -> Option<EntityKind> {
    match k {
        CardKind::Person => Some(EntityKind::Person),
        CardKind::Company => Some(EntityKind::Company),
        CardKind::Vehicle => Some(EntityKind::Vehicle),
        CardKind::Property => Some(EntityKind::Property),
        CardKind::Digital => Some(EntityKind::Digital),
        _ => None,
    }
}

/// This database's id in refs: random, made on the first sync, kept in `meta` (it moves with
/// the file).
pub fn database_uid(store: &Store) -> Result<String> {
    if let Some(u) = store.meta(UID_KEY)? {
        return Ok(u);
    }
    let u = Uuid::new_v4().simple().to_string()[..8].to_string();
    store.set_meta(UID_KEY, &u)?;
    Ok(u)
}

pub fn ref_for(id: i64, uid: &str) -> String {
    format!("entity-{id}@{uid}")
}

/// The local id in one of this database's refs.
pub fn parse_ref(r: &str, uid: &str) -> Option<i64> {
    let (id, u) = r.strip_prefix("entity-")?.split_once('@')?;
    if u == uid { id.parse().ok() } else { None }
}

/// The national-id / tax-id scheme usually meant by an identifier in `country`.
fn scheme(kind: EntityKind, country: &str) -> &'static str {
    match (kind, country) {
        (EntityKind::Company, "CO") => "NIT",
        (EntityKind::Company, "US") => "EIN",
        (EntityKind::Company, "BR") => "CNPJ",
        (EntityKind::Company, "FR") => "SIREN",
        (EntityKind::Company, "MX") | (EntityKind::Person, "MX") => "RFC",
        (EntityKind::Person, "CO") => "CC",
        (EntityKind::Person, "US") => "SSN",
        (EntityKind::Person, "BR") => "CPF",
        (EntityKind::Company, _) => "TAX",
        _ => "ID",
    }
}

fn some(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// The card for a Vigía entity (None for pets and "other").
pub fn to_card(e: &Entity, uid: &str) -> Option<Card> {
    let kind = card_kind(e.kind)?;
    let mut c = Card::new(kind, e.name.trim()).with_ref(APP, ref_for(e.id, uid));
    c.notes = some(&e.notes);
    let country = e.country.trim().to_ascii_uppercase();
    let tax = || -> Vec<TaxId> {
        match (some(&e.ident), country.is_empty()) {
            (Some(v), false) => vec![TaxId::new(&country, scheme(e.kind, &country), &v)],
            _ => vec![],
        }
    };
    match e.kind {
        EntityKind::Person => c.person = Some(card::Person { tax_ids: tax(), country: some(&country), ..Default::default() }),
        EntityKind::Company => c.company = Some(card::Company { tax_ids: tax(), country: some(&country), ..Default::default() }),
        EntityKind::Vehicle => {
            let plate = some(&e.ident).map(|p| normalize_ident(&p)).or_else(|| crate::entity::plate_in(&e.name));
            c.vehicle = Some(card::Vehicle { plate, model: some(&e.detail), year: e.year.and_then(|y| u16::try_from(y).ok()), ..Default::default() })
        }
        EntityKind::Property => c.property = Some(card::Property { address: some(&e.ident), ..Default::default() }),
        EntityKind::Digital => {
            let ident = some(&e.ident);
            let url = ident.clone().filter(|i| i.contains("://"));
            let domain = ident.as_deref().and_then(crate::rdap::normalize).or_else(|| (!e.name.contains(char::is_whitespace)).then(|| crate::rdap::normalize(&e.name)).flatten());
            c.digital = Some(card::Digital { domain, url, service: some(&e.detail).map(|s| s.to_lowercase()), ..Default::default() })
        }
        EntityKind::Pet | EntityKind::Other => {}
    }
    // what has no place in the card's sections, kept for the next Vigía that reads it
    c.ext.insert(APP.into(), json!({"ident": e.ident, "detail": e.detail, "year": e.year}));
    Some(c)
}

/// The identifier Vigía shows for a card (plate, tax id, address, domain / URL).
fn ident_of(c: &Card) -> Option<String> {
    let first_tax = |v: &[TaxId]| v.first().map(|t| t.value.clone());
    match c.kind {
        CardKind::Person => c.person.as_ref().and_then(|p| first_tax(&p.tax_ids)),
        CardKind::Company => c.company.as_ref().and_then(|p| first_tax(&p.tax_ids)),
        CardKind::Vehicle => c.vehicle.as_ref().and_then(|v| v.plate.clone().or(v.vin.clone())),
        CardKind::Property => c.property.as_ref().and_then(|p| p.address.clone().or(p.cadastral_id.clone())),
        CardKind::Digital => c.digital.as_ref().and_then(|d| d.url.clone().or(d.domain.clone())),
        _ => None,
    }
}

fn country_of(c: &Card) -> Option<String> {
    match c.kind {
        CardKind::Person => c.person.as_ref().and_then(|p| p.country.clone()),
        CardKind::Company => c.company.as_ref().and_then(|p| p.country.clone()),
        _ => None,
    }
}

fn detail_of(c: &Card) -> Option<String> {
    match c.kind {
        CardKind::Vehicle => c.vehicle.as_ref().and_then(|v| match (&v.make, &v.model) {
            (Some(a), Some(b)) if !b.to_lowercase().starts_with(&a.to_lowercase()) => Some(format!("{a} {b}")),
            (_, Some(b)) => Some(b.clone()),
            (Some(a), None) => Some(a.clone()),
            _ => None,
        }),
        CardKind::Digital => c.digital.as_ref().and_then(|d| d.service.clone()),
        _ => None,
    }
}

/// `card`'s values over `e` where the card has them (an empty field never erases one).
pub fn apply_card(e: &mut Entity, c: &Card) {
    if let Some(n) = some(&c.name) {
        e.name = n;
    }
    if let Some(n) = c.notes.as_deref().and_then(some) {
        e.notes = n;
    }
    if let Some(i) = ident_of(c).as_deref().and_then(some) {
        // the stored form ("900.123.456-7") stays when it is the same identifier
        if normalize_ident(&i) != normalize_ident(&e.ident) {
            e.ident = i;
        }
    }
    if let Some(d) = detail_of(c).as_deref().and_then(some)
        && e.kind.detail_label().is_some()
        && d.to_lowercase() != e.detail.to_lowercase()
    {
        e.detail = d;
    }
    if e.kind.has_country()
        && let Some(k) = country_of(c).as_deref().and_then(some)
    {
        e.country = k;
    }
    if e.kind.has_year()
        && let Some(y) = c.vehicle.as_ref().and_then(|v| v.year)
    {
        e.year = Some(y as i32);
    }
}

/// The local entity a card names, if any: linked by id, by this database's ref, by the same
/// identifier, else by the same name (case- and accent-insensitive) and kind.
fn find_local(all: &[Entity], c: &Card, kind: EntityKind, uid: &str) -> Option<i64> {
    let id = c.id.to_string();
    if let Some(e) = all.iter().find(|e| e.shared.as_deref() == Some(&id)) {
        return Some(e.id);
    }
    if let Some(r) = c.refs.get(APP).and_then(|r| parse_ref(r, uid))
        && all.iter().any(|e| e.id == r)
    {
        return Some(r);
    }
    let free = |e: &&Entity| e.kind == kind && e.shared.as_deref().is_none_or(|s| s == DETACHED);
    if let Some(i) = ident_of(c).map(|i| normalize_ident(&i)).filter(|i| !i.is_empty())
        && let Some(e) = all.iter().filter(free).find(|e| normalize_ident(&e.ident) == i)
    {
        return Some(e.id);
    }
    let name = otherwise_link::text::fold(&c.name);
    all.iter().filter(free).find(|e| otherwise_link::text::fold(&e.name) == name).map(|e| e.id)
}

/// A same-named entity of the kind is already linked to another card: the two cards are
/// duplicates for the user to merge in Otherwise Apps, not a reason for a second entity here.
fn has_twin(all: &[Entity], c: &Card, kind: EntityKind) -> bool {
    let name = otherwise_link::text::fold(&c.name);
    all.iter().any(|e| e.kind == kind && otherwise_link::text::fold(&e.name) == name)
}

/// People only a family tree knows about stay out of Vigía.
fn worth_importing(c: &Card) -> bool {
    !(c.kind == CardKind::Person && !c.refs.is_empty() && c.refs.keys().all(|a| a == "genealogy"))
}

/// Syncs `store` with the registry at `root` when the user enabled it; `Ok(None)` when not.
pub fn sync_shared(store: &Store, root: &Root) -> Result<Option<Report>> {
    match Registry::open_shared(root) {
        Ok(mut reg) => sync(store, &mut reg).map(Some),
        Err(RegError::Disabled) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn sync(store: &Store, reg: &mut Registry) -> Result<Report> {
    let uid = database_uid(store)?;
    let mut rep = Report::default();
    unlink_deleted(store, reg, &uid)?;
    push(store, reg, &uid, &mut rep)?;
    if store.meta(SYNCED_KEY)?.is_none() {
        // a database that never synced (new, or switched to) takes everything there is
        for kind in [CardKind::Person, CardKind::Company, CardKind::Vehicle, CardKind::Property, CardKind::Digital] {
            for c in reg.list(Some(kind), 100_000, 0)? {
                pull_card(store, reg, &c, &uid, &mut rep)?;
            }
        }
    }
    loop {
        let batch = reg.pull(APP, BATCH)?;
        for s in &batch.items {
            match s.change.op {
                Op::Upsert => {
                    if let Some(c) = &s.entity {
                        pull_card(store, reg, c, &uid, &mut rep)?;
                    }
                }
                Op::Merge => {
                    if let Some(into) = s.change.into {
                        rep.pulled += store.repoint_shared(&s.change.id.to_string(), &into.to_string())?;
                        if let Some(c) = reg.get(into)? {
                            pull_card(store, reg, &c, &uid, &mut rep)?;
                        }
                    }
                }
                Op::Delete => {
                    if let Some(e) = store.entity_by_shared(&s.change.id.to_string())? {
                        store.set_shared(e.id, Some(DETACHED))?;
                        rep.pulled += 1;
                    }
                }
            }
        }
        reg.ack(APP, batch.cursor)?;
        if !batch.more {
            break;
        }
    }
    store.set_meta(SYNCED_KEY, &db::now_secs().to_string())?;
    Ok(rep)
}

fn unlink_deleted(store: &Store, reg: &mut Registry, uid: &str) -> Result<()> {
    for (k, local) in store.metas(db::UNLINK_PREFIX)? {
        if let Some(id) = k.strip_prefix(db::UNLINK_PREFIX).and_then(|s| Uuid::parse_str(s).ok())
            && let Some(c) = reg.get(id)?
            && let Ok(local) = local.parse::<i64>()
            && c.refs.get(APP).is_some_and(|r| *r == ref_for(local, uid))
        {
            reg.unlink(APP, c.id)?;
        }
        store.delete_meta(&k)?;
    }
    Ok(())
}

fn push(store: &Store, reg: &mut Registry, uid: &str, rep: &mut Report) -> Result<()> {
    let revs = store.entity_revs()?;
    for e in store.entities()? {
        if e.shared.as_deref() == Some(DETACHED) {
            continue;
        }
        let dirty = revs.iter().find(|(id, _, _)| *id == e.id).is_some_and(|(_, rev, seen)| rev != seen);
        if !(dirty || e.shared.is_none()) {
            continue;
        }
        let Some(mut c) = to_card(&e, uid) else { continue };
        if let Some(id) = e.shared.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
            c.id = id;
            // another entity (merged here, or another Vigía database) holds the card's ref:
            // update the card without fighting over it
            if reg.get(id)?.and_then(|now| now.refs.get(APP).cloned()).is_some_and(|r| r != ref_for(e.id, uid)) {
                c.refs.remove(APP);
            }
        }
        match reg.upsert(APP, c) {
            Ok(up) => {
                store.set_shared(e.id, Some(&up.id.to_string()))?;
                rep.pushed += 1;
            }
            Err(err) => rep.errors.push(format!("{}: {err}", e.name)), // i18n-ok: name: error
        }
    }
    Ok(())
}

fn pull_card(store: &Store, reg: &mut Registry, c: &Card, uid: &str, rep: &mut Report) -> Result<()> {
    let Some(kind) = local_kind(c.kind) else { return Ok(()) };
    let all = store.entities()?;
    let shared = c.id.to_string();
    let local = find_local(&all, c, kind, uid);
    let id = match local {
        Some(id) => {
            let Some(mut e) = all.iter().find(|e| e.id == id).cloned() else { return Ok(()) };
            if e.shared.as_deref() == Some(DETACHED) {
                return Ok(());
            }
            let before = e.clone();
            apply_card(&mut e, c);
            if e != before {
                store.update_entity(&e)?;
            }
            id
        }
        None => {
            if !worth_importing(c) || has_twin(&all, c, kind) {
                return Ok(());
            }
            let mut e = Entity::new(kind, c.name.trim());
            apply_card(&mut e, c);
            rep.created += 1;
            store.insert_entity(&e)?
        }
    };
    store.set_shared(id, Some(&shared))?;
    rep.pulled += 1;
    if !c.refs.contains_key(APP) {
        reg.link(APP, c.id, &ref_for(id, uid))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_and_cards() {
        assert_eq!(parse_ref(&ref_for(12, "ab12cd34"), "ab12cd34"), Some(12));
        assert_eq!(parse_ref(&ref_for(12, "ab12cd34"), "ffffffff"), None);
        assert_eq!(parse_ref("owner-3", "ab12cd34"), None);
        let co = Entity { ident: "900.123.456-7".into(), country: "co".into(), ..Entity::new(EntityKind::Company, "Otherwise SAS") };
        let mut c = to_card(&co, "u").unwrap();
        c.normalize().unwrap();
        assert_eq!(c.natural_keys(), vec!["tax:CO:NIT:9001234567".to_string()]);
        let car = Entity { detail: "Mazda 3".into(), year: Some(2019), ..Entity::new(EntityKind::Vehicle, "Mazda ABC123") };
        let c = to_card(&car, "u").unwrap();
        assert_eq!(c.vehicle.as_ref().unwrap().plate.as_deref(), Some("ABC123"));
        let dom = to_card(&Entity::new(EntityKind::Digital, "otherwisetech.com"), "u").unwrap();
        assert_eq!(dom.digital.unwrap().domain.as_deref(), Some("otherwisetech.com"));
        assert!(to_card(&Entity::new(EntityKind::Pet, "Luna"), "u").is_none());
        // a card's values land on the entity, empty ones never erase
        let mut e = Entity { notes: "keep".into(), ..Entity::new(EntityKind::Vehicle, "Mazda") };
        let mut from = Card::new(CardKind::Vehicle, "Mazda 3 ABC123");
        from.vehicle = Some(card::Vehicle { plate: Some("ABC123".into()), make: Some("Mazda".into()), model: Some("3".into()), year: Some(2019), ..Default::default() });
        apply_card(&mut e, &from);
        assert_eq!((e.name.as_str(), e.ident.as_str(), e.detail.as_str(), e.year, e.notes.as_str()), ("Mazda 3 ABC123", "ABC123", "Mazda 3", Some(2019), "keep"));
        let mut tree = Card::new(CardKind::Person, "Abuela").with_ref("genealogy", "I12");
        assert!(!worth_importing(&tree));
        tree.refs.insert("expense".into(), "c1".into());
        assert!(worth_importing(&tree));
    }
}
