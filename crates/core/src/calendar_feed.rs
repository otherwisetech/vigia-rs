//! Vigía's part of Otherwise Apps' Calendar (`<data root>/vigia/events.json`, otherwise-link
//! `events`): each live item's deadline (expiry, or the notice deadline of a contract) and its
//! next reminder day, from 60 days back to [`AHEAD_DAYS`] ahead, each linking to the item.
//! Health items are busy-only (the Calendar shows them; exports and assistants see "Busy").
use crate::date::{self, Date};
use crate::entity::Entity;
use crate::model::{Category, Item, Recurrence, State};
use otherwise_link::Root;
use otherwise_link::events::{self, Event, Kind, Status};

pub const AHEAD_DAYS: i64 = 548;
const BACK_DAYS: i64 = 60;

fn kind(it: &Item) -> Kind {
    if it.category == Category::Warranty {
        Kind::Warranty
    } else if it.recurrence != Recurrence::None || it.auto_renews || !it.renewal_url.is_empty() {
        Kind::Renewal
    } else {
        Kind::Due
    }
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
        let mut e = Event::day(format!("item-{}", it.id), kind(it), crate::ics::summary(it), due).link(&link).category(it.category.label()).busy_only(private);
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
                let mut rem = Event::day(format!("item-{}-reminder", it.id), Kind::Reminder, i18n::t!("core.cal.reminder", title = it.title()), r).link(&link).category(it.category.label()).busy_only(private);
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
