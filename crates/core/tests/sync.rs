//! Two-way sync with the shared registry, on temp dirs only.
use otherwise_link::entity::{Company, Entity as Card, Kind as CardKind, Vehicle};
use otherwise_link::registry::Registry;
use otherwise_link::settings::HubSettings;
use otherwise_link::tax::TaxId;
use otherwise_link::Root;
use vigia_core::db::Store;
use vigia_core::entity::{Entity, EntityKind};
use vigia_core::sync::{self, DETACHED};

fn setup() -> (tempfile::TempDir, Store, Registry) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::create(&dir.path().join("vigia.db")).unwrap();
    let reg = Registry::open(&dir.path().join("shared/entities.db")).unwrap();
    (dir, store, reg)
}

#[test]
fn disabled_registry_is_silent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::create(&dir.path().join("v.db")).unwrap();
    let root = Root::at(dir.path().join("shared"));
    assert_eq!(sync::sync_shared(&store, &root).unwrap(), None);
    assert!(!root.entities_db().exists(), "nothing is created before the user opts in");
    HubSettings::update(&root, |s| s.entities = true).unwrap();
    store.insert_entity(&Entity::new(EntityKind::Person, "Ana")).unwrap();
    assert_eq!(sync::sync_shared(&store, &root).unwrap().unwrap().pushed, 1);
}

#[test]
fn push_pull_without_duplicates() {
    let (_d, store, mut reg) = setup();
    let co = store.insert_entity(&Entity { ident: "900.123.456-7".into(), country: "CO".into(), ..Entity::new(EntityKind::Company, "Otherwise SAS") }).unwrap();
    let car = store.insert_entity(&Entity { detail: "Mazda 3".into(), ..Entity::new(EntityKind::Vehicle, "Mazda ABC123") }).unwrap();
    store.insert_entity(&Entity::new(EntityKind::Pet, "Luna")).unwrap();
    let r = sync::sync(&store, &mut reg).unwrap();
    assert_eq!((r.pushed, r.created), (2, 0), "{r:?}");
    let shared_co = store.entity(co).unwrap().shared.unwrap();
    // nothing changed: nothing pushed again, nothing pulled back
    let r = sync::sync(&store, &mut reg).unwrap();
    assert_eq!((r.pushed, r.pulled), (0, 0));

    // expense knows the same company by its NIT: same card, no twin
    let mut ex = Card::new(CardKind::Company, "Otherwise S.A.S.").with_ref("expense", "vendor-9");
    ex.company = Some(Company { tax_ids: vec![TaxId::new("CO", "NIT", "9001234567")], website: Some("otherwisetech.com".into()), ..Default::default() });
    let up = reg.upsert("expense", ex).unwrap();
    assert_eq!(up.id.to_string(), shared_co);
    // and a new vehicle, plus one with the same plate as ours
    let mut moto = Card::new(CardKind::Vehicle, "Moto XYZ12A").with_ref("expense", "v-2");
    moto.vehicle = Some(Vehicle { plate: Some("XYZ12A".into()), year: Some(2021), ..Default::default() });
    reg.upsert("expense", moto).unwrap();
    let r = sync::sync(&store, &mut reg).unwrap();
    assert_eq!((r.created, r.pushed), (1, 0), "{r:?}");
    let es = store.entities().unwrap();
    assert_eq!(es.iter().filter(|e| e.kind == EntityKind::Company).count(), 1);
    assert_eq!(store.entity(co).unwrap().name, "Otherwise S.A.S.", "the other app's rename lands here");
    let moto = es.iter().find(|e| e.name == "Moto XYZ12A").unwrap();
    assert_eq!((moto.ident.as_str(), moto.year), ("XYZ12A", Some(2021)));
    // the imported entity got Vigía's ref, so the card points back here
    let card = reg.get(uuid::Uuid::parse_str(moto.shared.as_deref().unwrap()).unwrap()).unwrap().unwrap();
    assert_eq!(sync::parse_ref(&card.refs["vigia"], &sync::database_uid(&store).unwrap()), Some(moto.id));

    // a local edit goes out; the card keeps the other app's data
    let mut e = store.entity(car).unwrap();
    e.detail = "Mazda 3 Touring".into();
    store.update_entity(&e).unwrap();
    assert_eq!(sync::sync(&store, &mut reg).unwrap().pushed, 1);
    let c = reg.get(uuid::Uuid::parse_str(&store.entity(car).unwrap().shared.unwrap()).unwrap()).unwrap().unwrap();
    assert_eq!(c.vehicle.unwrap().model.as_deref(), Some("Mazda 3 Touring"));
}

#[test]
fn merges_deletes_and_twins() {
    let (_d, store, mut reg) = setup();
    let a = store.insert_entity(&Entity::new(EntityKind::Person, "Ana")).unwrap();
    sync::sync(&store, &mut reg).unwrap();
    let ana = uuid::Uuid::parse_str(&store.entity(a).unwrap().shared.unwrap()).unwrap();
    // another app has "Ana" too, without anything in common: a twin card, not a second Ana here
    let other = reg.upsert("expense", Card::new(CardKind::Person, "Ana").with_ref("expense", "c-1")).unwrap().id;
    sync::sync(&store, &mut reg).unwrap();
    assert_eq!(store.entities().unwrap().len(), 1);
    // the user merges them in Otherwise Apps, keeping expense's card: the link follows
    reg.merge("otherwise-apps", other, ana).unwrap();
    sync::sync(&store, &mut reg).unwrap();
    assert_eq!(store.entity(a).unwrap().shared, Some(other.to_string()));
    assert_eq!(store.entities().unwrap().len(), 1);
    // the card is deleted there: the entity stays, detached, never pushed again
    reg.delete("otherwise-apps", other).unwrap();
    sync::sync(&store, &mut reg).unwrap();
    assert_eq!(store.entity(a).unwrap().shared.as_deref(), Some(DETACHED));
    let mut e = store.entity(a).unwrap();
    e.notes = "x".into();
    store.update_entity(&e).unwrap();
    assert_eq!(sync::sync(&store, &mut reg).unwrap().pushed, 0);

    // deleting in Vigía unlinks the ref; the card stays for the others
    let b = store.insert_entity(&Entity::new(EntityKind::Company, "Acme Inc")).unwrap();
    sync::sync(&store, &mut reg).unwrap();
    let acme = uuid::Uuid::parse_str(&store.entity(b).unwrap().shared.unwrap()).unwrap();
    store.delete_entity(b).unwrap();
    sync::sync(&store, &mut reg).unwrap();
    let c = reg.get(acme).unwrap().unwrap();
    assert!(!c.refs.contains_key("vigia"));
    // and it isn't pulled back in
    sync::sync(&store, &mut reg).unwrap();
    assert!(store.entities().unwrap().iter().all(|e| e.name != "Acme Inc"));
}

#[test]
fn a_second_database_never_takes_the_first_ones_cards() {
    let dir = tempfile::tempdir().unwrap();
    let mut reg = Registry::open(&dir.path().join("entities.db")).unwrap();
    let one = Store::create(&dir.path().join("one.db")).unwrap();
    let two = Store::create(&dir.path().join("two.db")).unwrap();
    // same local id 1 in both, different things
    one.insert_entity(&Entity::new(EntityKind::Company, "Acme Inc")).unwrap();
    two.insert_entity(&Entity::new(EntityKind::Company, "Globex Corp")).unwrap();
    sync::sync(&one, &mut reg).unwrap();
    sync::sync(&two, &mut reg).unwrap();
    assert_eq!(one.entity(1).unwrap().name, "Acme Inc");
    assert_eq!(two.entity(1).unwrap().name, "Globex Corp");
    assert_ne!(one.entity(1).unwrap().shared, two.entity(1).unwrap().shared);
    // the second (never synced before) took the first one's company as a new entity
    assert!(two.entities().unwrap().iter().any(|e| e.name == "Acme Inc"));
}
