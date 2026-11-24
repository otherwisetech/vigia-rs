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
use otherwise_link::lock::{Lock, LockInfo, Scope};
use otherwise_link::storage;

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
    /// Held while the file is open, so another computer (synced folder) or process sees it.
    lock: Option<Lock>,
    /// The file is open on another computer / in another process: not opened.
    pub blocked: Option<LockInfo>,
    /// Cloud folder and conflicted copies of the open file.
    pub storage: Option<storage::Status>,
    /// The question of the first launch of 0.2.1 (move to the Otherwise data folder).
    pub migration: Option<vigia_core::paths::Migration>,
    last_beat: std::time::Instant,
}

/// Why another database couldn't be opened.
pub enum SwitchError {
    /// Open on another computer or in another process; the user may take it over.
    Locked(LockInfo),
    Failed(String),
}

pub fn locked_message(i: &LockInfo) -> String {
    if i.host == otherwise_link::lock::host_name() { i18n::t!("app.db.locked_here") } else { i18n::t!("app.db.locked_elsewhere", host = i.host.clone()) }
}

fn lock(path: &Path, force: bool) -> Result<Lock, SwitchError> {
    if force {
        return Lock::take_over(path, vigia_core::sync::APP, Scope::Process).map_err(|e| SwitchError::Failed(e.to_string()));
    }
    Lock::acquire(path, vigia_core::sync::APP, Scope::Process).map_err(|h| match h.info() {
        Some(i) => SwitchError::Locked(i.clone()),
        None => SwitchError::Failed(String::new()),
    })
}

const LIVE_ICS: &str = "live_ics:";

pub struct Shared(pub Entity<Data>);

impl Global for Shared {}

pub fn data(cx: &App) -> Entity<Data> {
    cx.global::<Shared>().0.clone()
}

impl Data {
    pub fn open(path: PathBuf, temporary: bool) -> Data {
        let mut d = Data { store: None, path, temporary, error: None, items: Vec::new(), entities: Vec::new(), relations: Vec::new(), calendars: Vec::new(), live_ics: Vec::new(), attach_counts: HashMap::new(), generation: 0, checking: HashSet::new(), recent: Vec::new(), last_feed: String::new(), sync_dirty: true, syncing: false, lock: None, blocked: None, storage: None, migration: None, last_beat: std::time::Instant::now() };
        d.open_store(false);
        if d.store.is_some() && !temporary {
            d.note_recent();
        }
        d
    }

    /// (Re)opens `self.path`: lock first, then the database.
    fn open_store(&mut self, force: bool) {
        self.store = None;
        self.lock = None;
        self.blocked = None;
        match lock(&self.path, force) {
            Ok(l) => match Store::open_or_create(&self.path) {
                Ok(s) => {
                    self.store = Some(s);
                    self.lock = Some(l);
                    self.error = None;
                    self.reload();
                }
                Err(e) => self.error = Some(e.to_string()),
            },
            Err(SwitchError::Locked(i)) => {
                self.error = Some(locked_message(&i));
                self.blocked = Some(i);
            }
            Err(SwitchError::Failed(e)) => self.error = Some(e),
        }
        self.storage = Some(storage::status(&self.path));
    }

    /// "Use it here anyway" after the open-elsewhere notice.
    pub fn take_over(&mut self, cx: &mut Context<Self>) {
        self.open_store(true);
        self.changed(cx);
    }

    /// Keeps the lock fresh; when another computer took the file over, stops using it.
    pub fn heartbeat(&mut self, cx: &mut Context<Self>) {
        if self.last_beat.elapsed() < otherwise_link::lock::HEARTBEAT_EVERY {
            return;
        }
        self.last_beat = std::time::Instant::now();
        if let Some(l) = self.lock.as_mut()
            && let Err(h) = l.heartbeat()
        {
            self.store = None;
            if let Some(l) = self.lock.take() {
                l.forget();
            }
            if let Some(i) = h.info() {
                self.error = Some(locked_message(i));
                self.blocked = Some(i.clone());
            }
            cx.notify();
        }
        if self.store.as_mut().is_some_and(|s| s.refresh().unwrap_or(false)) {
            self.reload();
            cx.notify();
        }
        let st = storage::status(&self.path);
        if self.storage.as_ref() != Some(&st) {
            self.storage = Some(st);
            cx.notify();
        }
    }

    pub fn today() -> Date {
        date::today()
    }

    /// The file can't be written (read-only file or folder): the forms say so instead of
    /// failing on Save.
    pub fn read_only(&self) -> bool {
        self.store.as_ref().is_some_and(Store::is_read_only)
    }

    /// One write on the open database, on a fresh connection when the sync client replaced
    /// the file (see [`Store::write`]).
    fn write<T>(&mut self, f: impl FnMut(&mut Store) -> db::Result<T>) -> Result<T, String> {
        let s = self.store.as_mut().ok_or_else(|| i18n::t!("app.db.none"))?;
        s.write(f).map_err(|e| e.to_string())
    }

    pub fn reload(&mut self) {
        let Some(s) = self.store.as_mut() else { return };
        // a sync client may have put another file at the path: read that one
        let _ = s.refresh();
        let s = &*s;
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
        let id = self.write(|s| if e.id == 0 { s.insert_entity(&e) } else { s.update_entity(&e).map(|_| e.id) })?;
        self.changed(cx);
        Ok(id)
    }

    pub fn delete_entity(&mut self, id: EntityId, cx: &mut Context<Self>) {
        let _ = self.write(|s| s.delete_entity(id));
        self.changed(cx);
    }

    pub fn relate(&mut self, from: EntityId, to: EntityId, role: Role, cx: &mut Context<Self>) -> Result<(), String> {
        self.write(|s| s.relate(from, to, role))?;
        self.changed(cx);
        Ok(())
    }

    /// Keeps (or stops keeping) `path` up to date with the entity's items.
    pub fn set_live_ics(&mut self, entity: EntityId, path: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.store.is_none() {
            return;
        }
        let key = format!("{LIVE_ICS}{entity}");
        let _ = self.write(|s| match &path {
            Some(p) => s.set_meta(&key, &p.display().to_string()),
            None => s.delete_meta(&key),
        });
        self.changed(cx);
    }

    // ── other calendars ──────────────────────────────────────────────────────────────────────

    pub fn add_calendar_file(&mut self, path: &Path, cx: &mut Context<Self>) -> Result<String, String> {
        let body = vigia_core::calendars::read_file(path)?;
        let fallback = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let c = Calendar { name: vigia_core::calendars::name_of(&body, &fallback), source: path.display().to_string(), body, fetched: Some(db::now_secs()), enabled: true, id: 0 };
        self.write(|s| s.save_calendar(&c))?;
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
                this.write(|s| s.save_calendar(&c))?;
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
            } else if self.store.is_some() {
                let r = vigia_core::calendars::read_file(Path::new(&c.source)).and_then(|body| self.write(|s| s.save_calendar(&Calendar { body: body.clone(), fetched: Some(db::now_secs()), ..c.clone() })).map(|_| c.name.clone()));
                done(r, cx);
            }
        }
        self.changed(cx);
    }

    pub fn set_calendar_enabled(&mut self, id: i64, on: bool, cx: &mut Context<Self>) {
        if let Some(c) = self.calendars.iter().find(|c| c.id == id).cloned() {
            let _ = self.write(|s| s.save_calendar(&Calendar { enabled: on, ..c.clone() }));
        }
        self.changed(cx);
    }

    pub fn remove_calendar(&mut self, id: i64, cx: &mut Context<Self>) {
        let _ = self.write(|s| s.delete_calendar(id));
        self.changed(cx);
    }

    pub fn unrelate(&mut self, id: i64, cx: &mut Context<Self>) {
        let _ = self.write(|s| s.unrelate(id));
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
                let _ = vigia_core::calendar_feed::clear(&root);
                self.last_feed = "off".into();
            }
            return;
        }
        // the command line and Otherwise Helper publish in this language too
        let lang = i18n::locale();
        if self.store.as_ref().is_some_and(|s| s.meta(vigia_core::feed::LANG_META).ok().flatten().as_deref() != Some(lang)) && !self.read_only() {
            let _ = self.write(|s| s.set_meta(vigia_core::feed::LANG_META, lang));
        }
        // the Calendar's file; otherwise-link skips the write when nothing changed
        let _ = vigia_core::calendar_feed::publish(&root, &self.items, &self.entities, Self::today());
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
    pub fn save(&mut self, mut item: Item, cx: &mut Context<Self>) -> Result<ItemId, String> {
        item.tidy(Self::today());
        let before = self.item(item.id).cloned();
        let id = self.write(|s| if item.id == 0 { s.insert(&item) } else { s.update(&item).map(|_| item.id) })?;
        self.changed(cx);
        // a domain or certificate added, renamed or still without a date is looked up now
        // a domain or certificate added or edited is looked up now (one still without a date
        // even when its daily check is off); a save of a check's own result is not
        let look = item.kind.checkable()
            && (item.auto_check || item.expires.is_none())
            && before.as_ref().is_none_or(|b| b.last_checked == item.last_checked && (b.host != item.host || b.kind != item.kind || !b.auto_check && item.auto_check || item.expires.is_none()));
        if look {
            self.check(vec![id], cx);
        }
        Ok(id)
    }

    /// Applies an accepted proposal; returns how to undo it.
    pub fn apply_plan(&mut self, plan: &vigia_core::agent::Plan, cx: &mut Context<Self>) -> Result<vigia_core::agent::Applied, String> {
        let today = Self::today();
        let done = self.write(|s| vigia_core::agent::apply(s, plan, today))?;
        self.changed(cx);
        let look: Vec<ItemId> = done.items.iter().copied().filter(|id| plan.check.contains(id) || self.item(*id).is_some_and(|i| i.auto_check && i.kind.checkable() && i.expires.is_none() && i.last_checked.is_none())).collect();
        if !look.is_empty() {
            self.check(look, cx);
        }
        Ok(done)
    }

    pub fn undo(&mut self, steps: &[vigia_core::agent::Undo], cx: &mut Context<Self>) -> Result<(), String> {
        self.write(|s| vigia_core::agent::undo(s, steps))?;
        self.changed(cx);
        Ok(())
    }

    pub fn delete(&mut self, id: ItemId, cx: &mut Context<Self>) {
        let _ = self.write(|s| s.delete(id));
        self.changed(cx);
    }

    pub fn renew(&mut self, id: ItemId, new_expiry: Date, cost: Option<f64>, note: &str, cx: &mut Context<Self>) -> Result<(), String> {
        self.write(|s| s.renew(id, new_expiry, Self::today(), cost, note))?;
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
        let mut n = 0;
        for p in paths.iter().filter(|p| p.is_file()) {
            self.write(|s| s.attach(id, p))?;
            n += 1;
        }
        self.changed(cx);
        Ok(n)
    }

    pub fn detach(&mut self, attachment: i64, cx: &mut Context<Self>) {
        let _ = self.write(|s| s.detach(attachment));
        self.changed(cx);
    }

    // ── which database ───────────────────────────────────────────────────────────────────────

    fn note_recent(&self) {
        let p = self.path.clone();
        Settings::update(|s| {
            let mut r = s.recent_dbs();
            vigia_core::paths::push_recent(&mut r, &p);
            s.recent_dbs = Some(r);
        });
    }

    /// The default database is remembered as "no choice", so it follows the Otherwise data
    /// folder when that moves.
    fn remember(&self) {
        if self.temporary {
            return;
        }
        let p = self.path.clone();
        let default = vigia_core::paths::default_db();
        Settings::update(|s| {
            s.db_path = (default.as_deref() != Some(p.as_path())).then(|| p.display().to_string());
            let mut r = s.recent_dbs();
            vigia_core::paths::push_recent(&mut r, &p);
            s.recent_dbs = Some(r);
        });
    }

    fn adopt(&mut self, store: Store, lock: Lock, path: PathBuf, cx: &mut Context<Self>) {
        self.store = Some(store);
        self.lock = Some(lock);
        self.path = path;
        self.temporary = false;
        self.error = None;
        self.blocked = None;
        self.storage = Some(storage::status(&self.path));
        self.remember();
        self.changed(cx);
    }

    /// Opens another database (creating it when the file doesn't exist) and remembers it.
    /// `force`: take it over even if it's open elsewhere (after the user confirmed).
    pub fn switch_to(&mut self, path: PathBuf, force: bool, cx: &mut Context<Self>) -> Result<(), SwitchError> {
        let l = if path == self.path && self.lock.is_some() { self.lock.take().ok_or(SwitchError::Failed(String::new()))? } else { lock(&path, force)? };
        let store = Store::open_or_create(&path).map_err(|e| SwitchError::Failed(e.to_string()))?;
        self.adopt(store, l, path, cx);
        Ok(())
    }

    /// "Save As": a verified copy (with the attachments) at `dest`, which becomes the open one;
    /// the original stays as it was.
    pub fn save_as(&mut self, dest: PathBuf, cx: &mut Context<Self>) -> Result<(), String> {
        let l = lock(&dest, false).map_err(|e| match e {
            SwitchError::Locked(i) => locked_message(&i),
            SwitchError::Failed(m) => m,
        })?;
        let store = db::copy_database(&self.path, &dest).map_err(|e| e.to_string())?;
        self.adopt(store, l, dest, cx);
        Ok(())
    }

    /// Moves the database and its attachments to `dest` (verified copy, then the originals are
    /// removed) and keeps using it there.
    pub fn move_to(&mut self, dest: PathBuf, cx: &mut Context<Self>) -> Result<(), String> {
        let from = self.path.clone();
        let new_lock = lock(&dest, false).map_err(|e| match e {
            SwitchError::Locked(i) => locked_message(&i),
            SwitchError::Failed(m) => m,
        })?;
        // close it first: Windows can't copy / delete an open file
        self.store = None;
        let old_lock = self.lock.take();
        match db::move_database(&from, &dest) {
            Ok(s) => {
                drop(old_lock);
                self.adopt(s, new_lock, dest, cx);
                Ok(())
            }
            Err(e) => {
                drop(new_lock);
                self.store = Store::open(&from).ok();
                self.lock = old_lock;
                cx.notify();
                Err(e.to_string())
            }
        }
    }

    pub fn import(&mut self, items: Vec<Item>, cx: &mut Context<Self>) -> Result<usize, String> {
        // one write per item, so a retry never inserts one twice
        for it in &items {
            self.write(|s| s.insert(it))?;
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
        if s.write(|s| s.mark_sent(&all)).is_err() {
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
        let mut ids = vigia_core::checks::due(&self.items, db::now_secs());
        // 0.7.0: domains added without a date (or by the assistant, before checks were on by
        // default) are looked up once
        const BACKFILL: &str = "checks.backfill_070"; // i18n-ok: meta key
        if self.store.as_ref().is_some_and(|s| s.meta(BACKFILL).ok().flatten().is_none()) && !self.read_only() {
            for id in vigia_core::checks::backfill(&self.items) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let _ = self.write(|s| s.set_meta(BACKFILL, &db::now_secs().to_string()));
        }
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
            let mine: Vec<Item> = self.items.iter().filter(|i| i.owned_by(*id)).cloned().collect();
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
