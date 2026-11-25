//! What the views show, computed without UI: filters, dashboard counts, the timeline's months
//! and the calendar's grid.
use crate::date::{self, Date};
use crate::entity::{Entity, EntityId, EntityKind, Relation};
use crate::model::{Category, Item, State, Status};
use chrono::Datelike;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    pub query: String,
    pub entity: Option<EntityId>,
    /// With `entity`: what it owns through ownership links (its companies, their cars), whose
    /// items count as its own too. See [`Filter::for_entity`].
    pub through: Vec<EntityId>,
    pub entity_kind: Option<EntityKind>,
    pub category: Option<Category>,
    pub status: Option<Status>,
    pub show_archived: bool,
}

impl Filter {
    pub fn matches(&self, it: &Item, today: Date) -> bool {
        if it.state == State::Archived && !self.show_archived {
            return false;
        }
        if let Some(e) = self.entity
            && !it.owned_by(e)
            && !self.through.iter().any(|t| it.owned_by(*t))
        {
            return false;
        }
        if self.entity_kind.is_some() && it.owner_kind != self.entity_kind && it.category.section() != self.entity_kind {
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

    /// Everything `e` has: its own items and those of what it owns.
    pub fn for_entity(e: EntityId, rels: &[Relation]) -> Filter {
        Filter { entity: Some(e), through: crate::entity::owned_through(e, rels), ..Default::default() }
    }
}

/// How `it` belongs to `e`: `None` when it doesn't, `Some(None)` directly, `Some(Some(x))`
/// through `x` (a company `e` owns).
pub fn via(it: &Item, e: EntityId, through: &[EntityId]) -> Option<Option<EntityId>> {
    if it.owned_by(e) {
        return Some(None);
    }
    through.iter().find(|t| it.owned_by(**t)).map(|t| Some(*t))
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

/// The sidebar's groups: each entity kind in use, with its entities (by name, archived ones
/// left out) and how many items (not archived) each has, its own and those of what it owns.
pub fn sections<'a>(entities: &'a [Entity], items: &[Item], rels: &[Relation]) -> Vec<(EntityKind, Vec<(&'a Entity, usize)>)> {
    EntityKind::ALL
        .into_iter()
        .filter_map(|k| {
            let mut v: Vec<(&Entity, usize)> = entities.iter().filter(|e| e.kind == k && !e.archived).map(|e| (e, items_of(items, e.id, rels).len())).collect();
            v.sort_by_key(|(e, _)| e.name.to_lowercase());
            (!v.is_empty()).then_some((k, v))
        })
        .collect()
}

/// An entity's live items: its own, then those of what it owns.
pub fn items_of<'a>(items: &'a [Item], e: EntityId, rels: &[Relation]) -> Vec<&'a Item> {
    let f = Filter::for_entity(e, rels);
    items.iter().filter(|i| i.state != State::Archived && f.matches(i, chrono::NaiveDate::MIN)).collect()
}

/// The items a section lists on their own: their category belongs to `kind`'s section (a
/// domain to Digital) but no owner of that kind holds them (a company's domain, a person's
/// subscription), so no card of the section shows them. Not archived, most urgent first.
pub fn loose<'a>(items: &'a [Item], entities: &[Entity], kind: EntityKind, today: Date) -> Vec<&'a Item> {
    let kind_of = |id: EntityId| entities.iter().find(|e| e.id == id).filter(|e| !e.archived).map(|e| e.kind);
    sorted(items.iter().filter(|i| i.state != State::Archived && i.category.section() == Some(kind) && !i.owner_ids().into_iter().any(|o| kind_of(o) == Some(kind))), today)
}

/// The worst status among an entity's items (not archived), for its colour.
pub fn worst_status(items: &[Item], entity: EntityId, today: Date) -> Option<Status> {
    items.iter().filter(|i| i.owned_by(entity) && i.state != State::Archived).map(|i| i.status(today)).filter(|s| *s != Status::Undated).min()
}

/// One entity on its kind's section page: its most urgent item and how it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct Card<'a> {
    pub entity: &'a Entity,
    /// The next deadline (overdue first) among its items, not archived.
    pub next: Option<&'a Item>,
    pub status: Option<Status>,
    pub items: usize,
}

/// The cards of a section, most urgent first, then by name.
pub fn cards<'a>(entities: &'a [Entity], items: &'a [Item], kind: EntityKind, today: Date) -> Vec<Card<'a>> {
    let mut v: Vec<Card> = entities
        .iter()
        .filter(|e| e.kind == kind && !e.archived)
        .map(|e| {
            let own: Vec<&Item> = items.iter().filter(|i| i.owned_by(e.id) && i.state != State::Archived).collect();
            let next = sorted(own.iter().copied().filter(|i| i.due_date(today).is_some()), today).into_iter().next();
            Card { entity: e, next, status: worst_status(items, e.id, today), items: own.len() }
        })
        .collect();
    v.sort_by_key(|c| (c.status.unwrap_or(Status::Undated), c.entity.name.to_lowercase()));
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
            Item { owner: "Ana".into(), entity: Some(1), owner_kind: Some(EntityKind::Person), state: State::InProgress, kind: Kind::Passport, category: Category::Identity, ..it(7, "Pasaporte", ymd(2026, 11, 20)) },
        ];
        let c = counts(&items, today);
        assert_eq!(c, Counts { expired: 1, soon: 2, upcoming: 1, ok: 1, undated: 1, in_progress: 1, total: 6 });
        assert_eq!(c.attention(), 3);
        assert_eq!(next(&items, today, 3).iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 7, 2]);
        let f = Filter { entity: Some(1), ..Default::default() };
        assert_eq!(filtered(&items, &f, today).len(), 1);
        let f = Filter { entity_kind: Some(EntityKind::Person), ..Default::default() };
        assert_eq!(filtered(&items, &f, today)[0].id, 7);
        let f = Filter { entity_kind: Some(EntityKind::Vehicle), ..Default::default() };
        assert!(filtered(&items, &f, today).is_empty());
        let f = Filter { status: Some(Status::Soon), ..Default::default() };
        assert_eq!(filtered(&items, &f, today).iter().map(|i| i.id).collect::<Vec<_>>(), vec![7, 2]);
        let f = Filter { query: "pasa ana".into(), ..Default::default() };
        assert_eq!(filtered(&items, &f, today).len(), 1);
        let f = Filter { category: Some(Category::Identity), ..Default::default() };
        assert_eq!(filtered(&items, &f, today)[0].id, 7);
        assert_eq!(filtered(&items, &Filter { show_archived: true, ..Default::default() }, today).len(), 7);
        let ents = vec![Entity { id: 1, name: "Ana".into(), ..Default::default() }, Entity { id: 2, kind: EntityKind::Vehicle, name: "Mazda ABC123".into(), ..Default::default() }, Entity { id: 3, name: "Luis".into(), ..Default::default() }];
        let sec = sections(&ents, &items, &[]);
        assert_eq!(sec.iter().map(|(k, v)| (*k, v.iter().map(|(e, n)| (e.id, *n)).collect::<Vec<_>>())).collect::<Vec<_>>(), vec![(EntityKind::Person, vec![(1, 1), (3, 0)]), (EntityKind::Vehicle, vec![(2, 0)])]);
        assert_eq!(worst_status(&items, 1, today), Some(Status::Soon));
        assert_eq!(worst_status(&items, 2, today), None);
        let cs = cards(&ents, &items, EntityKind::Person, today);
        assert_eq!(cs.iter().map(|c| (c.entity.id, c.next.map(|i| i.id), c.status, c.items)).collect::<Vec<_>>(), vec![(1, Some(7), Some(Status::Soon), 1), (3, None, None, 0)]);
        assert_eq!(cards(&ents, &items, EntityKind::Vehicle, today).len(), 1);
        assert!(cards(&ents, &items, EntityKind::Pet, today).is_empty());
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
        assert!(!grid.is_empty());
        assert_eq!(grid.len(), 42);
        // November 2026 starts on a Sunday: the grid starts Monday Oct 26
        assert_eq!(grid[0].0, ymd(2026, 10, 26).unwrap());
        let day = grid.iter().find(|(d, _)| *d == today).unwrap();
        assert_eq!(day.1[0].id, 7);
    }

    #[test]
    fn owners_through_companies_and_loose_section_items() {
        use crate::entity::Role;
        let today = ymd(2026, 11, 24).unwrap();
        let ent = |id, kind, name: &str| Entity { id, kind, name: name.into(), ..Default::default() };
        let ents = vec![ent(1, EntityKind::Person, "Juan"), ent(2, EntityKind::Company, "Otherwise SAS"), ent(3, EntityKind::Vehicle, "Mazda"), ent(4, EntityKind::Person, "Ana"), ent(5, EntityKind::Digital, "vcity.io"), Entity { archived: true, ..ent(6, EntityKind::Vehicle, "Old car") }];
        let rels = vec![Relation { id: 1, from: 1, to: 2, role: Role::Owner }, Relation { id: 2, from: 2, to: 3, role: Role::Owner }];
        let mk = |id, kind, name: &str, main: Option<i64>, co: Vec<i64>| Item { id, entity: main, co_owners: co, expires: ymd(2027, 1, id as u32), ..Item::new(kind, name) };
        let items = vec![
            mk(1, Kind::Domain, "otherwise.co", Some(2), vec![]),
            mk(2, Kind::Soat, "SOAT", Some(3), vec![]),
            mk(3, Kind::Passport, "Pasaporte", Some(1), vec![]),
            mk(4, Kind::Domain, "vcity.io", Some(5), vec![]),
            mk(5, Kind::Tecnomecanica, "Tecno", Some(4), vec![1]),
            mk(6, Kind::Domain, "nobody.dev", None, vec![]),
        ];
        let f = Filter::for_entity(1, &rels);
        assert_eq!(f.through, vec![2, 3]);
        assert_eq!(filtered(&items, &f, today).iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 2, 3, 5], "direct, through the company and its car, and co-owned");
        assert_eq!(via(&items[1], 1, &f.through), Some(Some(3)));
        assert_eq!(via(&items[2], 1, &f.through), Some(None));
        assert_eq!(via(&items[3], 1, &f.through), None);
        let ana = Filter::for_entity(4, &rels);
        assert_eq!(filtered(&items, &ana, today).len(), 1);
        let sec = sections(&ents, &items, &rels);
        let juan = sec.iter().find(|(k, _)| *k == EntityKind::Person).unwrap().1.iter().find(|(e, _)| e.id == 1).unwrap().1;
        assert_eq!(juan, 4);
        assert!(!sec.iter().any(|(_, v)| v.iter().any(|(e, _)| e.archived)), "archived entities are out of the sidebar");
        // Digital lists the domains no digital entity holds; the vcity.io card shows its own
        assert_eq!(loose(&items, &ents, EntityKind::Digital, today).iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 6]);
        assert_eq!(cards(&ents, &items, EntityKind::Digital, today).len(), 1);
        assert_eq!(loose(&items, &ents, EntityKind::Vehicle, today).iter().map(|i| i.id).collect::<Vec<_>>(), vec![5], "the SOAT is on the car's card; papers held only by people are listed");
        assert_eq!(cards(&ents, &items, EntityKind::Person, today).iter().find(|c| c.entity.id == 1).unwrap().items, 2, "passport and the co-owned tecno");
        let digital = Filter { entity_kind: Some(EntityKind::Digital), ..Default::default() };
        assert_eq!(filtered(&items, &digital, today).len(), 3, "the category agrees with the section");
    }
}
