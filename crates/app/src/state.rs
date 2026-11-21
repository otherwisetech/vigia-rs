//! The open database and everything derived from it, held by the app rather than the window:
//! closing the window (macOS, with the menu-bar item) keeps reminders and checks running.
use gpui::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use vigia_core::date::{self, Date};
use vigia_core::db::{self, Store};
use vigia_core::model::{Item, ItemId, State};
use vigia_core::settings::Settings;

pub struct Data {
    pub store: Option<Store>,
    pub path: PathBuf,
    /// Opened with `--db`: used for this session only, never remembered.
    pub temporary: bool,
    pub error: Option<String>,
    pub items: Vec<Item>,
    pub attach_counts: HashMap<ItemId, usize>,
    pub generation: u64,
    pub checking: HashSet<ItemId>,
    /// Reminders sent this session, newest first: (item, title, body).
    pub recent: Vec<(ItemId, String, String)>,
}

pub struct Shared(pub Entity<Data>);

impl Global for Shared {}

pub fn data(cx: &App) -> Entity<Data> {
    cx.global::<Shared>().0.clone()
}

impl Data {
    pub fn open(path: PathBuf, temporary: bool) -> Data {
        let mut d = Data { store: None, path: path.clone(), temporary, error: None, items: Vec::new(), attach_counts: HashMap::new(), generation: 0, checking: HashSet::new(), recent: Vec::new() };
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
        self.attach_counts = s.attachment_counts().unwrap_or_default();
        self.generation += 1;
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Something changed: reload, refresh the calendar file, tell the views.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.reload();
        self.sync_ics();
        cx.notify();
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
