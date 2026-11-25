//! Reminder scheduling: which items deserve a notification today. Pure: the caller passes the
//! reminders already sent (kept in the database) and records the ones this returns.
use crate::date::{self, Date};
use crate::model::{Item, ItemId, State};
use std::collections::HashSet;

/// A reminder: item, the deadline it is about, and how many days before it (0 = the day it
/// lapses or later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Reminder {
    pub item: ItemId,
    pub deadline: Date,
    pub lead: u32,
}

impl Reminder {
    pub fn fires_on(&self) -> Date {
        date::add_days(self.deadline, -(self.lead as i64))
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Plan {
    /// Notify now (at most one per item: the most urgent one that is due).
    pub notify: Vec<Reminder>,
    /// Earlier lead times that also passed: record them as sent without a notification, so a
    /// first run or a week away doesn't produce a burst of stale reminders.
    pub silent: Vec<Reminder>,
}

/// The reminders of `item` for its current deadline: one per lead time, plus the deadline
/// itself (lead 0).
pub fn reminders_of(item: &Item, today: Date) -> Vec<Reminder> {
    let Some(deadline) = item.due_date(today) else { return Vec::new() };
    let mut leads = item.leads();
    if !leads.contains(&0) {
        leads.push(0);
    }
    leads.into_iter().map(|lead| Reminder { item: item.id, deadline, lead }).collect()
}

pub fn plan(items: &[Item], today: Date, sent: &HashSet<Reminder>) -> Plan {
    let mut p = Plan::default();
    for item in items {
        if item.state == State::Archived {
            continue;
        }
        if item.snoozed_until.is_some_and(|s| s > today) {
            continue;
        }
        let mut due: Vec<Reminder> = reminders_of(item, today).into_iter().filter(|r| r.fires_on() <= today && !sent.contains(r)).collect();
        if item.state == State::InProgress {
            // the renewal is under way: only the deadline itself still warrants a nudge
            due.retain(|r| r.lead == 0);
        }
        if due.is_empty() {
            continue;
        }
        // smallest lead = the most recent / most urgent
        due.sort_by_key(|r| r.lead);
        let first = due.remove(0);
        // a lead the item has passed already got (or skipped) its moment
        let already = reminders_of(item, today).iter().any(|r| r.lead < first.lead && sent.contains(r));
        if !already {
            p.notify.push(first);
        } else {
            p.silent.push(first);
        }
        p.silent.extend(due);
    }
    p.notify.sort();
    p.silent.sort();
    p
}

/// Reminders that will fire between `from` and `to` (inclusive), soonest first — "what will
/// Vigía tell me next".
pub fn upcoming(items: &[Item], from: Date, to: Date) -> Vec<Reminder> {
    let mut v: Vec<Reminder> = items.iter().filter(|i| i.state == State::Active).flat_map(|i| reminders_of(i, from)).filter(|r| (from..=to).contains(&r.fires_on())).collect();
    v.sort_by_key(|r| (r.fires_on(), r.item));
    v
}

/// The text of a reminder: "Pasaporte (Ana) expires in 30 days".
pub fn message(item: &Item, r: &Reminder, today: Date) -> (String, String) {
    let days = date::days_between(today, r.deadline);
    if item.kind.is_celebration() {
        let title = item.celebration_text(r.deadline).unwrap_or_else(|| item.title());
        let body = if days <= 0 { i18n::t!("core.remind.celebration_today") } else { i18n::tn!("core.remind.celebration_in", days) };
        return (title, body);
    }
    let title = item.title();
    let body = if item.notice_days.is_some_and(|n| n > 0) {
        if days < 0 {
            i18n::tn!("core.remind.notice_passed", -days)
        } else if days == 0 {
            i18n::t!("core.remind.notice_today")
        } else {
            i18n::tn!("core.remind.notice_in", days)
        }
    } else if days < 0 {
        i18n::tn!("core.remind.expired_ago", -days)
    } else if days == 0 {
        i18n::t!("core.remind.today")
    } else {
        i18n::tn!("core.remind.in", days)
    };
    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;
    use crate::model::Kind;

    fn d(y: i32, m: u32, dd: u32) -> Date {
        ymd(y, m, dd).unwrap()
    }

    fn item(id: ItemId, expires: Date, leads: &[u32]) -> Item {
        Item { id, expires: Some(expires), lead_days: leads.to_vec(), ..Item::new(Kind::Custom, format!("item {id}")) }
    }

    /// Run the scheduler every day between two dates, as the app would.
    fn simulate(items: &[Item], from: Date, to: Date) -> Vec<(Date, Reminder)> {
        let mut sent = HashSet::new();
        let mut out = Vec::new();
        let mut day = from;
        while day <= to {
            let p = plan(items, day, &sent);
            for r in &p.notify {
                out.push((day, *r));
            }
            sent.extend(p.notify);
            sent.extend(p.silent);
            day = date::add_days(day, 1);
        }
        out
    }

    #[test]
    fn fires_once_per_lead_and_on_the_day() {
        let it = item(1, d(2027, 3, 31), &[30, 7, 1]);
        let got: Vec<(Date, u32)> = simulate(&[it], d(2027, 1, 1), d(2027, 4, 30)).into_iter().map(|(day, r)| (day, r.lead)).collect();
        assert_eq!(got, vec![(d(2027, 3, 1), 30), (d(2027, 3, 24), 7), (d(2027, 3, 30), 1), (d(2027, 3, 31), 0)]);
    }

    #[test]
    fn first_run_does_not_burst() {
        // app installed 5 days before the deadline: only the 7-day reminder, not the 90 and 30
        let it = item(1, d(2027, 3, 31), &[90, 30, 7]);
        let p = plan(std::slice::from_ref(&it), d(2027, 3, 26), &HashSet::new());
        assert_eq!(p.notify.iter().map(|r| r.lead).collect::<Vec<_>>(), vec![7]);
        assert_eq!(p.silent.iter().map(|r| r.lead).collect::<Vec<_>>(), vec![30, 90]);
        // app closed for weeks after the 7-day one: the next run fires the deadline once
        let got = simulate(std::slice::from_ref(&it), d(2027, 4, 20), d(2027, 4, 25));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].1.lead, 0);
    }

    #[test]
    fn snooze_in_progress_archived() {
        let today = d(2027, 3, 26);
        let mut it = item(1, d(2027, 3, 31), &[7]);
        it.snoozed_until = Some(d(2027, 3, 28));
        assert!(plan(std::slice::from_ref(&it), today, &HashSet::new()).notify.is_empty());
        // snooze over: the reminder comes back
        assert_eq!(plan(std::slice::from_ref(&it), d(2027, 3, 28), &HashSet::new()).notify.len(), 1);
        it.snoozed_until = None;
        it.state = State::InProgress;
        let p = plan(std::slice::from_ref(&it), today, &HashSet::new());
        assert!(p.notify.is_empty());
        assert_eq!(plan(std::slice::from_ref(&it), d(2027, 4, 1), &HashSet::new()).notify[0].lead, 0);
        it.state = State::Archived;
        assert_eq!(plan(&[it], d(2027, 4, 1), &HashSet::new()), Plan::default());
    }

    #[test]
    fn renewal_starts_a_new_cycle() {
        let mut it = item(1, d(2027, 3, 31), &[7]);
        let mut sent = HashSet::new();
        let p = plan(std::slice::from_ref(&it), d(2027, 3, 25), &sent);
        sent.extend(p.notify);
        it.expires = Some(d(2028, 3, 31));
        assert!(plan(std::slice::from_ref(&it), d(2027, 3, 26), &sent).notify.is_empty());
        assert_eq!(plan(std::slice::from_ref(&it), d(2028, 3, 24), &sent).notify, vec![Reminder { item: 1, deadline: d(2028, 3, 31), lead: 7 }]);
    }

    #[test]
    fn notice_period_moves_the_deadline() {
        let mut it = item(1, d(2027, 6, 30), &[30]);
        it.kind = Kind::Contract;
        it.notice_days = Some(30);
        let got = simulate(&[it], d(2027, 4, 1), d(2027, 7, 31));
        assert_eq!(got.iter().map(|(day, r)| (*day, r.lead)).collect::<Vec<_>>(), vec![(d(2027, 5, 1), 30), (d(2027, 5, 31), 0)]);
    }

    #[test]
    fn upcoming_window() {
        let items = [item(1, d(2027, 3, 31), &[30, 7]), item(2, d(2027, 2, 10), &[7])];
        let v = upcoming(&items, d(2027, 2, 1), d(2027, 3, 10));
        assert_eq!(v.iter().map(|r| (r.item, r.lead)).collect::<Vec<_>>(), vec![(2, 7), (2, 0), (1, 30)]);
        i18n::set_locale("en");
        let (t, b) = message(&items[0], &Reminder { item: 1, deadline: d(2027, 3, 31), lead: 30 }, d(2027, 3, 1));
        assert_eq!((t.as_str(), b.as_str()), ("item 1", "Expires in 30 days"));
        i18n::set_locale(i18n::DEFAULT);
    }
}
