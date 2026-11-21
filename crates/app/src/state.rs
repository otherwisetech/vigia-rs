//! The open database and everything derived from it, held by the app rather than the window:
//! closing the window (macOS, with the menu-bar item) keeps reminders and checks running.
use gpui::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use vigia_core::date::{self, Date};
use vigia_core::db::{self, Store};
use vigia_core::calendars::Calendar;
use vigia_core::entity::{Entity as Ent, EntityId, Relation, Role};
use vigia_core::model::{Item, ItemId, State};
use vigia_core::settings::Settings;

pub struct Data {
    pub store: Option<Store>,
    pub path: PathBuf,
    /// Opened with `--db`: used for this session only, never remembered.
    pub temporary: bool,
    pub error: Option<String>,
    pub items: Vec<Item>,
    pub entities: Vec<Ent>,
    pub relations: Vec<Relation>,
    pub calendars: Vec<Calendar>,
    /// Entities with a live .ics file kept up to date: (entity, file).
    pub live_ics: Vec<(EntityId, PathBuf)>,
    pub attach_counts: HashMap<ItemId, usize>,
    pub generation: u64,
    pub checking: HashSet<ItemId>,
    /// Reminders sent this session, newest first: (item, title, body).
    pub recent: Vec<(ItemId, String, String)>,
    /// What was last written to the Today feed (JSON of the events, or "off").
    last_feed: String,
    /// An edit since the last sync with the shared registry; a sync is running.
    pub sync_dirty: bool,
    syncing: bool,
}

const LIVE_ICS: &str = "live_ics:";

pub struct Shared(pub Entity<Data>);

impl Global for Shared {}

pub fn data(cx: &App) -> Entity<Data> {
    cx.global::<Shared>().0.clone()
}

impl Data {
    pub fn open(path: PathBuf, temporary: bool) -> Data {
        let mut d = Data { store: None, path: path.clone(), temporary, error: None, items: Vec::new(), entities: Vec::new(), relations: Vec::new(), calendars: Vec::new(), live_ics: Vec::new(), attach_counts: HashMap::new(), generation: 0, checking: HashSet::new(), recent: Vec::new(), last_feed: String::new(), sync_dirty: true, syncing: false };
        match Store::open_or_create(&path) {
            Ok(s) => {
                d.store = Some(s);
                d.reload();
            }
            Err(e) => d.error = Some(e.to_string()),
        }
        d
    }

    pub fn today() -> Date {
        date::today()
    }

    pub fn reload(&mut self) {
        let Some(s) = &self.store else { return };
        match s.items() {
            Ok(v) => self.items = v,
            Err(e) => self.error = Some(e.to_string()),
        }
        self.entities = s.entities().unwrap_or_default();
        self.relations = s.relations().unwrap_or_default();
        self.calendars = s.calendars().unwrap_or_default();
        self.live_ics = s.metas(LIVE_ICS).unwrap_or_default().into_iter().filter_map(|(k, v)| Some((k.strip_prefix(LIVE_ICS)?.parse().ok()?, PathBuf::from(v)))).collect();
        self.attach_counts = s.attachment_counts().unwrap_or_default();
        self.generation += 1;
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn entity(&self, id: EntityId) -> Option<&Ent> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Inserts (id 0) or updates; returns the id.
    pub fn save_entity(&mut self, e: Ent, cx: &mut Context<Self>) -> Result<EntityId, String> {
        let s = self.store.as_ref().ok_or_else(|| i18n::t!("app.db.none"))?;
        let id = if e.id == 0 { s.insert_entity(&e).map_err(|x| x.to_string())? } else { s.update_entity(&e).map(|_| e.id).map_err(|x| x.to_string())? };
        self.changed(cx);
        Ok(id)
    }

    pub fn delete_entity(&mut self, id: EntityId, cx: &mut Context<Self>) {
        if let Some(s) = &self.store {
            let _ = s.delete_entity(id);
        }
        self.changed(cx);
    }

    pub fn relate(&mut self, from: EntityId, to: EntityId, role: Role, cx: &mut Context<Self>) -> Result<(), String> {
        let s = self.store.as_ref().ok_or_else(|| i18n::t!("app.db.none"))?;
        s.relate(from, to, role).map_err(|e| e.to_string())?;
        self.changed(cx);
        Ok(())
    }

    /// Keeps (or stops keeping) `path` up to date with the entity's items.
    pub fn set_live_ics(&mut self, entity: EntityId, path: Option<PathBuf>, cx: &mut Context<Self>) {
        let Some(s) = &self.store else { return };
        let key = format!("{LIVE_ICS}{entity}");
        let _ = match &path {
            Some(p) => s.set_meta(&key, &p.display().to_string()),
            None => s.delete_meta(&key),
        };
        self.changed(cx);
    }

    // ── other calendars ──────────────────────────────────────────────────────────────────────

    pub fn add_calendar_file(&mut self, path: &Path, cx: &mut Context<Self>) -> Result<String, String> {
        let body = vigia_core::calendars::read_file(path)?;
        let fallback = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let c = Calendar { name: vigia_core::calendars::name_of(&body, &fallback), source: path.display().to_string(), body, fetched: Some(db::now_secs()), enabled: true, id: 0 };
        let s = self.store.as_ref().ok_or_else(|| i18n::t!("app.db.none"))?;
        s.save_calendar(&c).map_err(|e| e.to_string())?;
        self.changed(cx);
        Ok(c.name)
    }

    /// Downloads in the background; `done` gets the calendar's name or the error.
    pub fn add_calendar_url(&mut self, url: String, existing: Option<i64>, cx: &mut Context<Self>, done: impl FnOnce(Result<String, String>, &mut App) + 'static) {
        let u = url.clone();
        let task = cx.background_executor().spawn(async move { vigia_core::calendars::download(&u) });
        cx.spawn(async move |this, cx| {
            let body = task.await;
            let r = this.update(cx, |this, cx| -> Result<String, String> {
                let body = body?;
                let prev = existing.and_then(|id| this.calendars.iter().find(|c| c.id == id).cloned());
                let fallback = url.split('/').next_back().unwrap_or(&url).trim_end_matches(".ics").to_string();
                let c = match prev {
                    Some(p) => Calendar { body, fetched: Some(db::now_secs()), ..p },
                    None => Calendar { id: 0, name: vigia_core::calendars::name_of(&body, &fallback), source: url.clone(), body, fetched: Some(db::now_secs()), enabled: true },
                };
                let s = this.store.as_ref().ok_or_else(|| i18n::t!("app.db.none"))?;
                s.save_calendar(&c).map_err(|e| e.to_string())?;
                this.changed(cx);
                Ok(c.name)
            });
            let r = r.unwrap_or_else(|e| Err(e.to_string()));
            cx.update(|cx| done(r, cx));
        })
        .detach();
    }

    /// Reads each calendar again from its file or URL.
    pub fn refresh_calendars(&mut self, cx: &mut Context<Self>, done: impl Fn(Result<String, String>, &mut App) + Clone + 'static) {
        for c in self.calendars.clone() {
            if c.is_url() {
                self.add_calendar_url(c.source.clone(), Some(c.id), cx, done.clone());
            } else if let Some(s) = &self.store {
                let r = vigia_core::calendars::read_file(Path::new(&c.source)).and_then(|body| s.save_calendar(&Calendar { body, fetched: Some(db::now_secs()), ..c.clone() }).map(|_| c.name.clone()).map_err(|e| e.to_string()));
                done(r, cx);
            }
        }
        self.changed(cx);
    }

    pub fn set_calendar_enabled(&mut self, id: i64, on: bool, cx: &mut Context<Self>) {
        if let (Some(s), Some(c)) = (&self.store, self.calendars.iter().find(|c| c.id == id)) {
            let _ = s.save_calendar(&Calendar { enabled: on, ..c.clone() });
        }
        self.changed(cx);
    }

    pub fn remove_calendar(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(s) = &self.store {
            let _ = s.delete_calendar(id);
        }
        self.changed(cx);
    }

    pub fn unrelate(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(s) = &self.store {
            let _ = s.unrelate(id);
        }
        self.changed(cx);
    }

    /// Something changed: reload, refresh the calendar file and the Today feed, tell the views.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.reload();
        self.sync_ics();
        self.publish_today();
        self.sync_dirty = true;
        cx.notify();
    }

    /// The local entity behind a shared card id (following merges in the registry when the
    /// card isn't linked here directly).
    pub fn entity_for_shared(&self, id: uuid::Uuid) -> Option<EntityId> {
        let find = |u: uuid::Uuid| self.entities.iter().find(|e| e.shared.as_deref() == Some(&u.to_string())).map(|e| e.id);
        if let Some(e) = find(id) {
            return Some(e);
        }
        let reg = otherwise_link::registry::Registry::open_shared(&otherwise_link::Root::system()).ok()?;
        let live = reg.resolve(id).ok().flatten()?;
        if let Some(e) = find(live) {
            return Some(e);
        }
        let uid = self.store.as_ref()?.meta("shared_uid").ok().flatten()?;
        let card = reg.get(live).ok().flatten()?;
        let local = vigia_core::sync::parse_ref(card.refs.get(vigia_core::sync::APP)?, &uid)?;
        self.entity(local).map(|e| e.id)
    }

    /// Rewrites Vigía's file of the Today view when what it would say changed. Only for the
    /// user's own database (never one opened with `--db`).
    pub fn publish_today(&mut self) {
        if self.temporary || self.store.is_none() {
            return;
        }
        let root = otherwise_link::Root::system();
        if !Settings::load().today_feed() {
            if self.last_feed != "off" {
                let _ = vigia_core::feed::clear(&root);
                self.last_feed = "off".into();
            }
            return;
        }
        let events = vigia_core::feed::events(&self.items, &self.entities, Self::today());
        let sig = serde_json::to_string(&events).unwrap_or_default();
        if sig != self.last_feed && otherwise_link::today::publish(&root, vigia_core::sync::APP, &events).is_ok() {
            self.last_feed = sig;
        }
    }

    /// Syncs entities with the shared registry in the background (nothing happens unless the
    /// user turned it on in Otherwise Apps). Results land in the database and reload the views.
    pub fn sync_shared(&mut self, cx: &mut Context<Self>) {
        if self.temporary || self.syncing || self.store.is_none() {
            return;
        }
        self.syncing = true;
        self.sync_dirty = false;
        let path = self.path.clone();
        let task = cx.background_executor().spawn(async move {
            let store = Store::open(&path).map_err(|e| e.to_string())?;
            vigia_core::sync::sync_shared(&store, &otherwise_link::Root::system()).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |this, cx| {
                this.syncing = false;
                match r {
                    Ok(Some(rep)) if rep.pulled + rep.created > 0 => {
                        this.reload();
                        this.sync_ics();
                        this.publish_today();
                        cx.notify();
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("{e}"),
                }
            })
            .ok();
        })
        .detach();
    }

    /// Inserts (id 0) or updates; returns the id.
    pub fn save(&mut self, item: Item, cx: &mut Context<Self>) -> Result<ItemId, String> {
        let s = self.store.as_ref().ok_or_else(|| i18n::t!("app.db.none"))?;
        let id = if item.id == 0 { s.insert(&item).map_err(|e| e.to_string())? } else { s.update(&item).map(|_| item.id).map_err(|e| e.to_string())? };
        self.changed(cx);
        Ok(id)
    }

    pub fn delete(&mut self, id: ItemId, cx: &mut Context<Self>) {
        if let Some(s) = &self.store {
            let _ = s.delete(id);
        }
        self.changed(cx);
    }

    pub fn renew(&mut self, id: ItemId, new_expiry: Date, cost: Option<f64>, note: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let s = self.store.as_mut().ok_or_else(|| i18n::t!("app.db.none"))?;
        s.renew(id, new_expiry, Self::today(), cost, note).map_err(|e| e.to_string())?;
        self.changed(cx);
        Ok(())
    }

    pub fn modify(&mut self, id: ItemId, f: impl FnOnce(&mut Item), cx: &mut Context<Self>) {
        let Some(mut it) = self.item(id).cloned() else { return };
        f(&mut it);
        let _ = self.save(it, cx);
    }

    pub fn set_state(&mut self, id: ItemId, state: State, cx: &mut Context<Self>) {
        self.modify(id, |i| i.state = state, cx);
    }

    pub fn snooze(&mut self, id: ItemId, until: Option<Date>, cx: &mut Context<Self>) {
        self.modify(id, |i| i.snoozed_until = until, cx);
    }

    pub fn attach(&mut self, id: ItemId, paths: &[PathBuf], cx: &mut Context<Self>) -> Result<usize, String> {
        let s = self.store.as_ref().ok_or_else(|| i18n::t!("app.db.none"))?;
        let mut n = 0;
        for p in paths.iter().filter(|p| p.is_file()) {
            s.attach(id, p).map_err(|e| e.to_string())?;
            n += 1;
        }
        self.changed(cx);
        Ok(n)
    }

    pub fn detach(&mut self, attachment: i64, cx: &mut Context<Self>) {
        if let Some(s) = &self.store {
            let _ = s.detach(attachment);
        }
        self.changed(cx);
    }

    // ── which database ───────────────────────────────────────────────────────────────────────

    fn remember(&self) {
        if !self.temporary {
            let p = self.path.display().to_string();
            Settings::update(|s| s.db_path = Some(p));
        }
    }

    /// Opens another database (creating it when the file doesn't exist) and remembers it.
    pub fn switch_to(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Result<(), String> {
        let store = Store::open_or_create(&path).map_err(|e| e.to_string())?;
        self.store = Some(store);
        self.path = path;
        self.temporary = false;
        self.error = None;
        self.remember();
        self.changed(cx);
        Ok(())
    }

    /// Moves the database and its attachments to `dest` and keeps using it there.
    pub fn move_to(&mut self, dest: PathBuf, cx: &mut Context<Self>) -> Result<(), String> {
        let from = self.path.clone();
        // close it first: Windows can't copy / delete an open file
        self.store = None;
        match db::move_database(&from, &dest) {
            Ok(s) => {
                self.store = Some(s);
                self.path = dest;
                self.remember();
                self.changed(cx);
                Ok(())
            }
            Err(e) => {
                self.store = Store::open(&from).ok();
                cx.notify();
                Err(e.to_string())
            }
        }
    }

    pub fn import(&mut self, items: Vec<Item>, cx: &mut Context<Self>) -> Result<usize, String> {
        let s = self.store.as_ref().ok_or_else(|| i18n::t!("app.db.none"))?;
        for it in &items {
            s.insert(it).map_err(|e| e.to_string())?;
        }
        self.changed(cx);
        Ok(items.len())
    }

    // ── reminders, checks, calendar file ─────────────────────────────────────────────────────

    /// Runs the scheduler: returns the notifications to show and records them as sent.
    pub fn tick_reminders(&mut self, cx: &mut Context<Self>) -> Vec<(ItemId, String, String)> {
        let today = Self::today();
        let Some(s) = self.store.as_mut() else { return Vec::new() };
        let Ok(sent) = s.sent() else { return Vec::new() };
        let plan = vigia_core::reminder::plan(&self.items, today, &sent);
        if plan.notify.is_empty() && plan.silent.is_empty() {
            return Vec::new();
        }
        let all: Vec<_> = plan.notify.iter().chain(plan.silent.iter()).copied().collect();
        if s.mark_sent(&all).is_err() {
            return Vec::new();
        }
        let out: Vec<(ItemId, String, String)> = plan
            .notify
            .iter()
            .filter_map(|r| {
                let it = self.items.iter().find(|i| i.id == r.item)?;
                let (t, b) = vigia_core::reminder::message(it, r, today);
                Some((it.id, t, b))
            })
            .collect();
        for n in out.iter().rev() {
            self.recent.insert(0, n.clone());
        }
        self.recent.truncate(20);
        cx.notify();
        out
    }

    /// Starts the network checks of `ids` in the background; results land in the database.
    pub fn check(&mut self, ids: Vec<ItemId>, cx: &mut Context<Self>) {
        for id in ids {
            if self.checking.contains(&id) {
                continue;
            }
            let Some(item) = self.item(id).cloned() else { continue };
            self.checking.insert(id);
            let task = cx.background_executor().spawn(async move { vigia_core::checks::run(&item, &vigia_core::checks::Net) });
            cx.spawn(async move |this, cx| {
                let outcome = task.await;
                this.update(cx, |this, cx| {
                    this.checking.remove(&id);
                    if let Some(mut it) = this.item(id).cloned() {
                        vigia_core::checks::apply(&mut it, &outcome, db::now_secs());
                        let _ = this.save(it, cx);
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// Daily checks of the items that opted in.
    pub fn run_due_checks(&mut self, cx: &mut Context<Self>) {
        if !Settings::load().auto_checks() {
            return;
        }
        let ids = vigia_core::checks::due(&self.items, db::now_secs());
        if !ids.is_empty() {
            self.check(ids, cx);
        }
    }

    pub fn ics_path() -> Option<PathBuf> {
        let s = Settings::load();
        s.ics_path.map(PathBuf::from).or_else(vigia_core::paths::default_ics)
    }

    /// Rewrites the subscribed calendar file when that's on.
    pub fn sync_ics(&self) {
        // per-entity files were asked for on this database, so they follow it
        for (id, p) in &self.live_ics {
            let mine: Vec<Item> = self.items.iter().filter(|i| i.entity == Some(*id)).cloned().collect();
            write_ics(p, &mine);
        }
        if self.temporary || !Settings::load().ics_sync() {
            return;
        }
        if let Some(p) = Self::ics_path() {
            write_ics(&p, &self.items);
        }
    }

    pub fn files_dir(&self) -> PathBuf {
        db::files_dir(&self.path)
    }
}

pub fn write_ics(path: &Path, items: &[Item]) {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(path, vigia_core::ics::export(items, date::today(), db::now_secs()));
}
