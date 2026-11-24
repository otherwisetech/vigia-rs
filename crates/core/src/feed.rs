//! Vigía's part of the Otherwise apps' Today view (`today/vigia.json` in the shared folder):
//! what is overdue or due in the next [`HORIZON`] days, each with a link to the item. Written
//! whole and atomically by otherwise-link; rewritten when the list changes.
use crate::date::{self, Date};
use crate::entity::Entity;
use crate::model::{Item, State};
use crate::reminder::Reminder;
use otherwise_link::Root;
use otherwise_link::today::{self, Event, EventKind, Severity};

pub const HORIZON: i64 = 30;
/// `meta` key: the language the app shows, so the command line and Otherwise Helper publish
/// Today and Calendar in it too (their own environment may say English).
pub const LANG_META: &str = "feed.lang";

/// Runs `f` (a publish) in the language the app last used on this database.
pub fn in_app_language<T>(store: &crate::db::Store, f: impl FnOnce() -> T) -> T {
    let prev = i18n::locale();
    let lang = store.meta(LANG_META).ok().flatten();
    if let Some(l) = &lang {
        i18n::set_locale(l);
    }
    let out = f();
    if lang.is_some() {
        i18n::set_locale(prev);
    }
    out
}
/// An event left behind by an app that stopped running disappears this long after its date.
const STALE_DAYS: i64 = 30;

pub fn severity(days_left: i64) -> Severity {
    match days_left {
        d if d < 0 => Severity::Critical,
        0..=3 => Severity::High,
        4..=14 => Severity::Normal,
        _ => Severity::Low,
    }
}

/// The events for `items` on `today`, most urgent first. Archived and snoozed items are left
/// out, like their reminders. The id carries the due date, so a renewed item is a new event
/// (a dismissal of the old cycle doesn't hide it).
pub fn events(items: &[Item], entities: &[Entity], today: Date) -> Vec<Event> {
    let mut v: Vec<(&Item, Date, i64)> = items
        .iter()
        .filter(|i| i.state != State::Archived && i.snoozed_until.is_none_or(|s| s <= today))
        .filter_map(|i| {
            let due = i.due_date(today)?;
            let left = date::days_between(today, due);
            (left <= HORIZON).then_some((i, due, left))
        })
        .collect();
    v.sort_by_key(|(i, due, _)| (*due, i.id));
    v.into_iter()
        .map(|(it, due, left)| {
            let (title, body) = crate::reminder::message(it, &Reminder { item: it.id, deadline: due, lead: 0 }, today);
            let mut e = Event::new(format!("item-{}-{}", it.id, date::iso(due)), EventKind::Due, title).when(date::iso(due)).severity(severity(left)).body(body).link(&crate::links::item_link(it.id)).expires(date::iso(date::add_days(due.max(today), STALE_DAYS)));
            e.entity = it.entity.and_then(|id| entities.iter().find(|x| x.id == id)).and_then(|x| x.shared.as_deref()).and_then(|s| uuid::Uuid::parse_str(s).ok());
            e
        })
        .collect()
}

/// Rewrites Vigía's feed under `root`.
pub fn publish(root: &Root, items: &[Item], entities: &[Entity], today: Date) -> std::io::Result<Vec<Event>> {
    let v = events(items, entities, today);
    today::publish(root, crate::sync::APP, &v)?;
    Ok(v)
}

pub fn clear(root: &Root) -> std::io::Result<()> {
    today::clear(root, crate::sync::APP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;
    use crate::model::Kind;

    #[test]
    fn due_and_overdue_within_thirty_days() {
        let today = ymd(2026, 11, 21).unwrap();
        let mk = |id, name: &str, exp| Item { id, expires: exp, ..Item::new(Kind::Soat, name) };
        let mut snoozed = mk(5, "snoozed", ymd(2026, 11, 25));
        snoozed.snoozed_until = ymd(2026, 11, 24);
        let mut archived = mk(6, "archived", ymd(2026, 11, 22));
        archived.state = State::Archived;
        let mut owned = mk(7, "owned", ymd(2026, 12, 21));
        owned.entity = Some(1);
        owned.owner = "Mazda".into();
        let items = vec![mk(1, "late", ymd(2026, 11, 1)), mk(2, "soon", ymd(2026, 11, 23)), mk(3, "far", ymd(2027, 3, 1)), mk(4, "undated", None), snoozed, archived, owned];
        let shared = uuid::Uuid::new_v4();
        let ents = vec![Entity { id: 1, shared: Some(shared.to_string()), ..Entity::new(crate::entity::EntityKind::Vehicle, "Mazda") }];
        let v = events(&items, &ents, today);
        let ids: Vec<&str> = v.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["item-1-2026-11-01", "item-2-2026-11-23", "item-7-2026-12-21"]);
        assert_eq!((v[0].severity, v[1].severity, v[2].severity), (Severity::Critical, Severity::High, Severity::Low));
        assert_eq!(v[1].link.as_deref(), Some("otherwise://vigia/item/2"));
        assert_eq!(v[0].expires.as_deref(), Some("2026-12-21"));
        assert_eq!(v[2].entity, Some(shared));
        assert!(v[2].title.contains("Mazda"));
        // the snoozed one comes back once the snooze is over
        assert!(events(&items, &ents, ymd(2026, 11, 24).unwrap()).iter().any(|e| e.id == "item-5-2026-11-25"));
    }

    #[test]
    fn published_atomically_to_the_shared_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = Root::at(dir.path());
        let today = ymd(2026, 11, 21).unwrap();
        let items = vec![Item { id: 9, expires: ymd(2026, 11, 30), ..Item::new(Kind::Domain, "otherwisetech.com") }];
        publish(&root, &items, &[], today).unwrap();
        let f = dir.path().join("today").join("vigia.json");
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!((v["schema"].clone(), v["app"].clone(), v["events"][0]["kind"].clone()), (serde_json::json!(1), serde_json::json!("vigia"), serde_json::json!("due")));
        let merged = today::read(&root, chrono::Utc::now());
        assert!(merged.events.iter().all(|e| e.app == "vigia"));
        // no temp files left behind
        assert_eq!(std::fs::read_dir(dir.path().join("today")).unwrap().count(), 1);
        publish(&root, &[], &[], today).unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["events"], serde_json::json!([]));
        clear(&root).unwrap();
        assert!(!f.exists());
    }
}
