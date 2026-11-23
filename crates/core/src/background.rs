//! Vigía's background jobs for Otherwise Helper (otherwise-link `background`; the bundle ships
//! `Contents/Resources/otherwise-background.json`, the `vigia` CLI runs them as `vigia
//! background run <job>`), so reminders and checks keep coming with Vigía quit:
//!
//! - `reminders` (every 15 min): the same scheduler as the app ([`reminder::plan`]); due
//!   reminders become notifications and are recorded as sent.
//! - `checks` (hourly, network): the daily domain / certificate checks ([`checks::due`]); a
//!   notification when a check starts failing (the service is down or unreachable), when it
//!   recovers, and when an expiry moves (renewed).
//!
//! **Single writer**: while the app has the database open (its lock, a live process here) the
//! jobs leave it alone — the app does both itself. Otherwise they take the same
//! `<db>.lock.json` for the run; another computer's lock means hands off.
use crate::checks::{self, Checker, Outcome};
use crate::date::Date;
use crate::db::Store;
use crate::model::ItemId;
use otherwise_link::lock::{Holder, Lock, Scope};
use std::path::Path;

pub const REMINDERS: &str = "reminders";
pub const CHECKS: &str = "checks";
const CHECK_META: &str = "bg.check.";

pub enum Access {
    Mine(Lock),
    /// The app (or another process here) has it.
    AppOpen,
    /// Open on another computer.
    Elsewhere(String),
}

/// Takes the database's lock for one job run.
pub fn access(db: &Path) -> Access {
    match Lock::acquire(db, crate::sync::APP, Scope::Process) {
        Ok(l) => Access::Mine(l),
        Err(Holder::Elsewhere(i)) => Access::Elsewhere(i.host),
        Err(_) => Access::AppOpen,
    }
}

/// A notification to queue (the caller writes it to the outbox).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub item: ItemId,
}

/// Runs the reminder scheduler: returns what to notify and records everything that fired.
pub fn reminders(store: &mut Store, today: Date) -> crate::db::Result<Vec<Note>> {
    let items = store.items()?;
    let sent = store.sent()?;
    let plan = crate::reminder::plan(&items, today, &sent);
    let all: Vec<_> = plan.notify.iter().chain(plan.silent.iter()).copied().collect();
    if all.is_empty() {
        return Ok(vec![]);
    }
    store.mark_sent(&all)?;
    Ok(plan
        .notify
        .iter()
        .filter_map(|r| {
            let it = items.iter().find(|i| i.id == r.item)?;
            let (title, body) = crate::reminder::message(it, r, today);
            Some(Note { id: format!("remind-{}-{}-{}", r.item, crate::date::iso(r.deadline), r.lead), title, body, item: r.item })
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The check failed after passing (or for the first time).
    Down { item: ItemId, title: String, why: String },
    Recovered { item: ItemId, title: String },
    /// The expiry moved (a renewal the check found).
    Renewed { item: ItemId, title: String, expires: Date },
}

/// Runs the checks that are due, writes their outcomes, and reports what changed.
pub fn checks(store: &Store, net: &dyn Checker, now: i64) -> crate::db::Result<(usize, Vec<Change>)> {
    let mut items = store.items()?;
    let due = checks::due(&items, now);
    let mut changes = vec![];
    for it in items.iter_mut().filter(|i| due.contains(&i.id)) {
        let key = format!("{CHECK_META}{}", it.id);
        let was_failing = store.meta(&key)?.as_deref() == Some("fail");
        let o = checks::run(it, net);
        let expiry_moved = checks::apply(it, &o, now);
        store.update(it)?;
        let failing = matches!(o, Outcome::Failed(_));
        store.set_meta(&key, if failing { "fail" } else { "ok" })?;
        match (&o, was_failing) {
            (Outcome::Failed(why), false) => changes.push(Change::Down { item: it.id, title: it.title(), why: why.clone() }),
            (Outcome::Failed(_), true) => {}
            (_, true) => changes.push(Change::Recovered { item: it.id, title: it.title() }),
            _ => {}
        }
        if expiry_moved && let Some(e) = it.expires {
            changes.push(Change::Renewed { item: it.id, title: it.title(), expires: e });
        }
    }
    Ok((due.len(), changes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::CertInfo;
    use crate::model::{Item, Kind};
    use crate::rdap::DomainInfo;
    use std::cell::Cell;

    struct Fake {
        fail: Cell<bool>,
        expires: Cell<Option<Date>>,
    }

    impl Checker for Fake {
        fn domain(&self, _: &str) -> Result<DomainInfo, String> {
            if self.fail.get() {
                return Err("RDAP unreachable".into());
            }
            Ok(DomainInfo { expires: self.expires.get(), ..Default::default() })
        }
        fn cert(&self, _: &str) -> Result<CertInfo, String> {
            Err("no".into())
        }
    }

    fn store() -> (tempfile::TempDir, Store) {
        let t = tempfile::tempdir().unwrap();
        let s = Store::create(&t.path().join("vigia.db")).unwrap();
        (t, s)
    }

    #[test]
    fn checks_report_down_recovered_and_renewed() {
        let (_t, s) = store();
        let mut it = Item::new(Kind::Domain, "example.com");
        it.auto_check = true;
        it.expires = Date::from_ymd_opt(2027, 1, 1);
        let id = s.insert(&it).unwrap();
        let net = Fake { fail: Cell::new(true), expires: Cell::new(Date::from_ymd_opt(2027, 1, 1)) };
        let day = checks::EVERY_SECS;
        let (n, c) = checks(&s, &net, 1_000_000).unwrap();
        assert_eq!(n, 1);
        assert!(matches!(&c[..], [Change::Down { item, .. }] if *item == id), "{c:?}");
        // still failing tomorrow: no second "down"
        let (_, c) = checks(&s, &net, 1_000_000 + day).unwrap();
        assert!(c.is_empty(), "{c:?}");
        // back, and renewed
        net.fail.set(false);
        net.expires.set(Date::from_ymd_opt(2028, 1, 1));
        let (_, c) = checks(&s, &net, 1_000_000 + 2 * day).unwrap();
        assert!(c.iter().any(|x| matches!(x, Change::Recovered { .. })), "{c:?}");
        assert!(c.iter().any(|x| matches!(x, Change::Renewed { expires, .. } if *expires == Date::from_ymd_opt(2028, 1, 1).unwrap())), "{c:?}");
        // not due again within the day
        let (n, _) = checks(&s, &net, 1_000_000 + 2 * day + 60).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn reminders_fire_once_and_respect_the_lock() {
        let (t, mut s) = store();
        let today = Date::from_ymd_opt(2026, 11, 23).unwrap();
        let mut it = Item::new(Kind::Soat, "SOAT ABC123");
        it.expires = Some(today);
        s.insert(&it).unwrap();
        let first = reminders(&mut s, today).unwrap();
        assert_eq!(first.len(), 1, "{first:?}");
        assert!(reminders(&mut s, today).unwrap().is_empty(), "recorded as sent");
        let db = t.path().join("vigia.db");
        let Access::Mine(l) = access(&db) else { panic!("free") };
        // our own process holds it: a second open in this process is "mine" again
        assert!(matches!(access(&db), Access::Mine(_)));
        l.release();
    }
}
