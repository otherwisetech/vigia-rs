//! Vigía's part of Otherwise Apps' Calendar (`<data root>/vigia/events.json`, otherwise-link
//! `events`): each live item's deadline (expiry, or the notice deadline of a contract) and its
//! next reminder day, from 60 days back to [`AHEAD_DAYS`] ahead, each linking to the item.
//! Health items are busy-only (the Calendar shows them; exports and assistants see "Busy").
//!
//! Kinds (otherwise-link `events::Kind`, serialized lowercase): the thing itself is `due`
//! (an expiry or filing), `renewal` (it comes back or renews), `warranty`, `birthday` or
//! `anniversary`, titled with what happens ("SOAT vence (Mercedes CQC925)", "Cumpleaños de Ana
//! (30 años)"); the lead reminder before it is a separate `reminder` event (id
//! `item-<id>-reminder`) that a calendar can hide, since Vigía notifies on its own.
use crate::date::{self, Date};
use crate::entity::Entity;
use crate::model::{Category, Item, Recurrence, State};
use otherwise_link::Root;
use otherwise_link::events::{self, Event, Kind, Status};

pub const AHEAD_DAYS: i64 = 548;
const BACK_DAYS: i64 = 60;

fn kind(it: &Item) -> Kind {
    if it.kind == crate::model::Kind::Birthday {
        Kind::Birthday
    } else if it.kind == crate::model::Kind::Anniversary {
        Kind::Anniversary
    } else if it.category == Category::Warranty {
        Kind::Warranty
    } else if it.recurrence != Recurrence::None || it.auto_renews || !it.renewal_url.is_empty() {
        Kind::Renewal
    } else {
        Kind::Due
    }
}

/// Who an event is about: the main owner's name, with the plate for a vehicle whose name
/// doesn't carry it ("Mercedes CQC925").
fn owner_label(it: &Item, entities: &[Entity]) -> String {
    match it.entity.and_then(|id| entities.iter().find(|e| e.id == id)) {
        Some(e) if e.kind == crate::entity::EntityKind::Vehicle && !e.ident.trim().is_empty() && !crate::entity::normalize_ident(&e.name).contains(&crate::entity::normalize_ident(&e.ident)) => format!("{} {}", e.name.trim(), e.ident.trim()), // i18n-ok: name and plate
        Some(e) => e.name.trim().to_string(),
        None => it.owner.trim().to_string(),
    }
}

fn with_owner(what: String, owner: &str) -> String {
    if owner.is_empty() { what } else { i18n::t!("core.cal.with_owner", what = what, owner = owner) }
}

/// What happens, without its owner: "SOAT vence", "Garantía de Nevera vence".
fn what(it: &Item) -> String {
    if it.notice_days.is_some_and(|n| n > 0) {
        i18n::t!("core.cal.notice", name = it.name)
    } else if it.category == Category::Warranty {
        i18n::t!("core.cal.warranty", name = it.name)
    } else if it.auto_renews {
        i18n::t!("core.cal.renews", name = it.name)
    } else {
        i18n::t!("core.cal.expires", name = it.name)
    }
}

/// The event on its real date: "SOAT vence (Mercedes CQC925)", "Cumpleaños de Ana (30 años)".
pub fn title(it: &Item, on: Date, entities: &[Entity]) -> String {
    let owner = owner_label(it, entities);
    if it.kind.is_celebration() {
        let base = match (it.kind, owner.is_empty()) {
            (crate::model::Kind::Birthday, false) => i18n::t!("core.cal.birthday", name = owner),
            _ => it.name.clone(),
        };
        return match it.years_on(on) {
            Some(n) => i18n::tn!("core.cal.years", n, title = base),
            None => base,
        };
    }
    with_owner(what(it), &owner)
}

/// The lead reminder before it: "Recordatorio: SOAT vence el 27 nov (Mercedes CQC925)".
pub fn reminder_title(it: &Item, due: Date, entities: &[Entity]) -> String {
    use chrono::Datelike;
    let month: String = crate::model::month_name(due.month()).chars().take(3).collect();
    let when = i18n::t!("core.cal.short_date", day = due.day(), month = month);
    let what = if it.kind.is_celebration() { title(it, due, entities) } else { what(it) };
    let line = i18n::t!("core.cal.reminder_on", what = what, date = when);
    if it.kind.is_celebration() { line } else { with_owner(line, &owner_label(it, entities)) }
}

/// The calendar events for `items` on `today`.
pub fn events(items: &[Item], entities: &[Entity], today: Date) -> Vec<Event> {
    let (lo, hi) = (date::add_days(today, -BACK_DAYS), date::add_days(today, AHEAD_DAYS));
    let mut out = Vec::new();
    let mut v: Vec<(&Item, Date)> = items.iter().filter(|i| i.state != State::Archived).filter_map(|i| i.due_date(today).map(|d| (i, d))).filter(|(_, d)| *d >= lo && *d <= hi).collect();
    v.sort_by_key(|(i, d)| (*d, i.id));
    for (it, due) in v {
        let shared = it.entity.and_then(|id| entities.iter().find(|x| x.id == id)).and_then(|x| x.shared.as_deref()).and_then(|s| uuid::Uuid::parse_str(s).ok());
        let private = it.category == Category::Health;
        let link = crate::links::item_link(it.id);
        let mut e = Event::day(format!("item-{}", it.id), kind(it), title(it, due, entities), due).link(&link).category(it.category.label()).busy_only(private);
        if it.state == State::InProgress {
            e = e.status(Status::Tentative);
        }
        let detail: Vec<&str> = [it.provider.as_str(), it.reference.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
        if !private && !detail.is_empty() {
            e = e.body(detail.join(" · "));
        }
        if let Some(u) = shared {
            e = e.entity(u);
        }
        // the next reminder still ahead (active, not snoozed past it)
        if it.state == State::Active {
            let next = it.leads().into_iter().filter(|l| *l > 0).map(|l| date::add_days(due, -(l as i64))).filter(|d| *d >= today && it.snoozed_until.is_none_or(|s| s <= *d)).min();
            if let Some(r) = next {
                let mut rem = Event::day(format!("item-{}-reminder", it.id), Kind::Reminder, reminder_title(it, due, entities), r).link(&link).category(it.category.label()).busy_only(private);
                if let Some(u) = shared {
                    rem = rem.entity(u);
                }
                out.push(rem);
            }
        }
        out.push(e);
    }
    out
}

/// Rewrites Vigía's events file under `root` (skipped by otherwise-link when unchanged).
pub fn publish(root: &Root, items: &[Item], entities: &[Entity], today: Date) -> std::io::Result<usize> {
    let v = events(items, entities, today);
    events::publish(root, crate::sync::APP, &v, None)?;
    Ok(v.len())
}

pub fn clear(root: &Root) -> std::io::Result<()> {
    events::clear(root, crate::sync::APP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;
    use crate::model::Kind as ItemKind;

    #[test]
    fn deadlines_reminders_and_privacy() {
        let today = ymd(2026, 11, 21).unwrap();
        let mk = |id, k, name: &str, exp| Item { id, expires: exp, ..Item::new(k, name) };
        let soat = mk(1, ItemKind::Soat, "SOAT", ymd(2027, 1, 10));
        let mut archived = mk(2, ItemKind::Soat, "old", ymd(2027, 1, 10));
        archived.state = State::Archived;
        let far = mk(3, ItemKind::Passport, "far", ymd(2031, 1, 1));
        let mut health = mk(4, ItemKind::Vaccine, "Fiebre amarilla", ymd(2026, 12, 1));
        health.provider = "Clínica".into();
        let mut busy = mk(5, ItemKind::Soat, "en trámite", ymd(2026, 12, 5));
        busy.state = State::InProgress;
        let v = events(&[soat, archived, far, health, busy], &[], today);
        let ids: Vec<(&str, String)> = v.iter().map(|e| (e.id.as_str(), e.start.clone())).collect();
        assert!(ids.iter().any(|(i, _)| *i == "item-1"));
        assert!(!ids.iter().any(|(i, _)| *i == "item-2" || *i == "item-3"));
        let h = v.iter().find(|e| e.id == "item-4").unwrap();
        assert!(h.busy_only && h.body.is_none());
        assert_eq!(h.link.as_deref(), Some("otherwise://vigia/item/4"));
        assert_eq!(v.iter().find(|e| e.id == "item-5").unwrap().status, Status::Tentative);
        assert!(!v.iter().any(|e| e.id == "item-5-reminder"), "in-progress items get no lead reminder");
        let r = v.iter().find(|e| e.id == "item-1-reminder").unwrap();
        assert_eq!(r.kind, Kind::Reminder);
        assert!(r.start.as_str() >= "2026-11-21" && r.start.as_str() < "2027-01-10");
    }

    #[test]
    fn titles_say_what_happens_and_kinds_tell_reminders_apart() {
        let today = ymd(2026, 11, 21).unwrap();
        let car = Entity { id: 7, kind: crate::entity::EntityKind::Vehicle, name: "Mercedes".into(), ident: "CQC925".into(), ..Default::default() };
        let soat = Item { id: 1, owner: "Mercedes".into(), entity: Some(7), expires: ymd(2027, 1, 10), ..Item::new(ItemKind::Soat, "SOAT") };
        let bday = Item { id: 2, owner: "Juan José".into(), issued: ymd(1990, 12, 3), ..Item::new(ItemKind::Birthday, "Cumpleaños") };
        let mut no_year = Item { id: 3, owner: "Ana".into(), ..Item::new(ItemKind::Birthday, "Cumpleaños") };
        no_year.recurrence = Recurrence::Yearly { month: 12, day: 24 };
        let mut b2 = bday.clone();
        b2.normalize_celebration(today);
        let v = events(&[soat, b2, no_year], std::slice::from_ref(&car), today);
        let main = v.iter().find(|e| e.id == "item-1").unwrap();
        assert_eq!(main.kind, Kind::Renewal);
        assert!(main.title.contains("SOAT") && main.title.contains("Mercedes CQC925"), "{}", main.title);
        assert_eq!(main.start, "2027-01-10");
        let r = v.iter().find(|e| e.id == "item-1-reminder").unwrap();
        assert_eq!(r.kind, Kind::Reminder);
        assert!(r.title.contains("SOAT") && r.title.contains("CQC925") && r.title.contains("10"), "{}", r.title);
        i18n::set_locale("es");
        let es = title(&Item { id: 1, owner: "Mercedes".into(), entity: Some(7), expires: ymd(2027, 1, 10), ..Item::new(ItemKind::Soat, "SOAT") }, today, std::slice::from_ref(&car));
        assert_eq!(es, "SOAT vence (Mercedes CQC925)");
        i18n::set_locale(i18n::DEFAULT);
        let b = v.iter().find(|e| e.id == "item-2").unwrap();
        assert_eq!((b.kind, b.start.as_str()), (Kind::Birthday, "2026-12-03"));
        assert!(b.title.contains("Juan José") && b.title.contains("36"), "{}", b.title);
        let n = v.iter().find(|e| e.id == "item-3").unwrap();
        assert!(n.title.contains("Ana") && !n.title.contains('('), "{}", n.title);
        // every main entry is not a reminder; reminders are only the -reminder ones
        assert!(v.iter().all(|e| (e.kind == Kind::Reminder) == e.id.ends_with("-reminder")));
    }

    #[test]
    fn published_to_the_apps_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = Root::at(dir.path());
        let today = ymd(2026, 11, 21).unwrap();
        let items = vec![Item { id: 9, expires: ymd(2026, 11, 30), ..Item::new(ItemKind::Domain, "otherwisetech.com") }];
        assert!(publish(&root, &items, &[], today).unwrap() >= 1);
        let m = events::read_in(&root, today, ymd(2026, 12, 31).unwrap(), &chrono::Utc);
        assert!(m.occurrences.iter().any(|o| o.event.id == "item-9" && o.event.app == "vigia"));
        clear(&root).unwrap();
        assert!(!dir.path().join("vigia/events.json").exists());
    }
}
