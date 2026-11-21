//! What the views show, computed without UI: filters, dashboard counts, the timeline's months
//! and the calendar's grid.
use crate::date::{self, Date};
use crate::model::{Category, Item, State, Status};
use chrono::Datelike;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    pub query: String,
    pub owner: Option<String>,
    pub category: Option<Category>,
    pub status: Option<Status>,
    pub show_archived: bool,
}

impl Filter {
    pub fn matches(&self, it: &Item, today: Date) -> bool {
        if it.state == State::Archived && !self.show_archived {
            return false;
        }
        if self.owner.as_ref().is_some_and(|o| !it.owner.eq_ignore_ascii_case(o)) {
            return false;
        }
        if self.category.is_some_and(|c| it.category != c) {
            return false;
        }
        if self.status.is_some_and(|s| it.status(today) != s) {
            return false;
        }
        let q = self.query.trim().to_lowercase();
        q.is_empty() || q.split_whitespace().all(|w| it.haystack().contains(w))
    }

    pub fn is_empty(&self) -> bool {
        *self == Filter::default()
    }
}

/// Sorted by due date (undated last), then name.
pub fn sorted<'a>(items: impl IntoIterator<Item = &'a Item>, today: Date) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = items.into_iter().collect();
    v.sort_by_key(|a| (a.due_date(today).is_none(), a.due_date(today), a.name.to_lowercase()));
    v
}

pub fn filtered<'a>(items: &'a [Item], f: &Filter, today: Date) -> Vec<&'a Item> {
    sorted(items.iter().filter(|i| f.matches(i, today)), today)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub expired: usize,
    pub soon: usize,
    pub upcoming: usize,
    pub ok: usize,
    pub undated: usize,
    pub in_progress: usize,
    pub total: usize,
}

impl Counts {
    /// Number on the menu-bar badge: what needs attention (expired or due within 30 days).
    pub fn attention(&self) -> usize {
        self.expired + self.soon
    }
}

pub fn counts(items: &[Item], today: Date) -> Counts {
    let mut c = Counts::default();
    for it in items.iter().filter(|i| i.state != State::Archived) {
        c.total += 1;
        if it.state == State::InProgress {
            c.in_progress += 1;
        }
        match it.status(today) {
            Status::Expired => c.expired += 1,
            Status::Soon => c.soon += 1,
            Status::Upcoming => c.upcoming += 1,
            Status::Ok => c.ok += 1,
            Status::Undated => c.undated += 1,
        }
    }
    c
}

/// The next `n` deadlines (overdue first), not archived.
pub fn next(items: &[Item], today: Date, n: usize) -> Vec<&Item> {
    sorted(items.iter().filter(|i| i.state != State::Archived && i.due_date(today).is_some()), today).into_iter().take(n).collect()
}

/// Owners in use, sorted, case-insensitively unique.
pub fn owners(items: &[Item]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for it in items {
        let o = it.owner.trim();
        if !o.is_empty() && !v.iter().any(|x| x.eq_ignore_ascii_case(o)) {
            v.push(o.to_string());
        }
    }
    v.sort_by_key(|s| s.to_lowercase());
    v
}

/// One month on the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Month<'a> {
    /// First day of the month.
    pub start: Date,
    pub items: Vec<&'a Item>,
}

/// Overdue items, then the next `months` months from today's month (empty months included so
/// the timeline reads as a calendar).
pub fn timeline<'a>(items: &[&'a Item], today: Date, months: u32) -> (Vec<&'a Item>, Vec<Month<'a>>) {
    let overdue: Vec<&Item> = items.iter().copied().filter(|i| i.due_date(today).is_some_and(|d| d < today)).collect();
    let first = date::month_start(today);
    let out = (0..months)
        .map(|k| {
            let start = date::add_months(first, k as i32);
            let end = date::add_months(start, 1);
            let mut v: Vec<&Item> = items.iter().copied().filter(|i| i.due_date(today).is_some_and(|d| d >= today.max(start) && d < end)).collect();
            v.sort_by_key(|i| (i.due_date(today), i.name.to_lowercase()));
            Month { start, items: v }
        })
        .collect();
    (overdue, out)
}

/// The days of a month grid: 6 weeks starting on Monday, each with the items due that day.
pub fn month_grid<'a>(items: &[&'a Item], month: Date, today: Date) -> Vec<(Date, Vec<&'a Item>)> {
    let first = date::month_start(month);
    let back = first.weekday().num_days_from_monday() as i64;
    let start = date::add_days(first, -back);
    (0..42)
        .map(|k| {
            let d = date::add_days(start, k);
            (d, items.iter().copied().filter(|i| i.due_date(today) == Some(d)).collect())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;
    use crate::model::Kind;

    fn it(id: i64, name: &str, exp: Option<Date>) -> Item {
        Item { id, expires: exp, ..Item::new(Kind::Custom, name) }
    }

    #[test]
    fn counts_filters_timeline_grid() {
        let today = ymd(2026, 11, 20).unwrap();
        let mut items = vec![
            it(1, "Expired", ymd(2026, 11, 1)),
            it(2, "Soon", ymd(2026, 12, 5)),
            it(3, "Upcoming", ymd(2027, 1, 30)),
            it(4, "Later", ymd(2027, 9, 1)),
            it(5, "Undated", None),
            Item { state: State::Archived, ..it(6, "Old", ymd(2026, 12, 1)) },
            Item { owner: "Ana".into(), state: State::InProgress, kind: Kind::Passport, category: Category::Identity, ..it(7, "Pasaporte", ymd(2026, 11, 20)) },
        ];
        let c = counts(&items, today);
        assert_eq!(c, Counts { expired: 1, soon: 2, upcoming: 1, ok: 1, undated: 1, in_progress: 1, total: 6 });
        assert_eq!(c.attention(), 3);
        assert_eq!(next(&items, today, 3).iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 7, 2]);
        let f = Filter { owner: Some("ana".into()), ..Default::default() };
        assert_eq!(filtered(&items, &f, today).len(), 1);
        let f = Filter { status: Some(Status::Soon), ..Default::default() };
        assert_eq!(filtered(&items, &f, today).iter().map(|i| i.id).collect::<Vec<_>>(), vec![7, 2]);
        let f = Filter { query: "pasa ana".into(), ..Default::default() };
        assert_eq!(filtered(&items, &f, today).len(), 1);
        let f = Filter { category: Some(Category::Identity), ..Default::default() };
        assert_eq!(filtered(&items, &f, today)[0].id, 7);
        assert_eq!(filtered(&items, &Filter { show_archived: true, ..Default::default() }, today).len(), 7);
        assert_eq!(owners(&items), vec!["Ana".to_string()]);
        items.retain(|i| i.state != State::Archived);
        let refs: Vec<&Item> = items.iter().collect();
        let (overdue, months) = timeline(&refs, today, 12);
        assert_eq!(overdue.iter().map(|i| i.id).collect::<Vec<_>>(), vec![1]);
        assert_eq!(months.len(), 12);
        assert_eq!(months[0].start, ymd(2026, 11, 1).unwrap());
        assert_eq!(months[0].items.iter().map(|i| i.id).collect::<Vec<_>>(), vec![7]);
        assert_eq!(months[1].items[0].id, 2);
        assert_eq!(months[2].items[0].id, 3);
        assert_eq!(months[10].items[0].id, 4);
        let grid = month_grid(&refs, today, today);
        assert_eq!(grid.len(), 42);
        // November 2026 starts on a Sunday: the grid starts Monday Oct 26
        assert_eq!(grid[0].0, ymd(2026, 10, 26).unwrap());
        let day = grid.iter().find(|(d, _)| *d == today).unwrap();
        assert_eq!(day.1[0].id, 7);
    }
}
