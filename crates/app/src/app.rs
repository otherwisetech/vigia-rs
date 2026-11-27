//! The window: top bar with quick add, a sidebar of views and filters, the current view, the
//! selected item's panel, a footer, and the About modal.
use crate::dialogs::Dialog;
use crate::state::{self, Data};
use crate::theme::{SANS, Tokens, tokens};
use crate::ui::{self, icon};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{IconName, Sizable as _};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vigia_core::date::{self, Date};
use vigia_core::model::{Category, Item, ItemId, Status};
use vigia_core::settings::Settings;
use vigia_core::view::Filter;

pub(crate) const CONTEXT: &str = "VigiaApp";
const TOAST_FOR: Duration = otherwise_ui::tokens::motion::TOAST;
pub const SIDEBAR_W: f32 = otherwise_ui::tokens::size::SIDEBAR;

actions!(
    vigia,
    [
        NewItem,
        FocusQuickAdd,
        ReadDocument,
        ImportCsv,
        ExportCsv,
        ExportIcs,
        Backup,
        Restore,
        OpenDb,
        MoveDb,
        SaveDbAs,
        RevealDb,
        ClearRecent,
        OpenRecent0,
        OpenRecent1,
        OpenRecent2,
        OpenRecent3,
        OpenRecent4,
        OpenRecent5,
        OpenRecent6,
        OpenRecent7,
        CloseWindow,
        ShowDashboard,
        ShowTimeline,
        ShowCalendar,
        ShowList,
        ShowGraph,
        FocusSearch,
        Escape,
        RenewSelected,
        EditSelected,
        ToggleInProgress,
        SnoozeSelected,
        CheckSelected,
        DeleteSelected,
        ShowAbout,
        OpenSettings
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-n", NewItem, Some(CONTEXT)),
        KeyBinding::new("secondary-l", FocusQuickAdd, Some(CONTEXT)),
        KeyBinding::new("secondary-f", FocusSearch, Some(CONTEXT)),
        KeyBinding::new("secondary-1", ShowDashboard, Some(CONTEXT)),
        KeyBinding::new("secondary-2", ShowTimeline, Some(CONTEXT)),
        KeyBinding::new("secondary-3", ShowCalendar, Some(CONTEXT)),
        KeyBinding::new("secondary-4", ShowList, Some(CONTEXT)),
        KeyBinding::new("secondary-5", ShowGraph, Some(CONTEXT)),
        KeyBinding::new("secondary-r", RenewSelected, Some(CONTEXT)),
        KeyBinding::new("secondary-e", EditSelected, Some(CONTEXT)),
        KeyBinding::new("secondary-,", OpenSettings, Some(CONTEXT)),
        KeyBinding::new("secondary-w", CloseWindow, Some(CONTEXT)),
        KeyBinding::new("escape", Escape, Some(CONTEXT)),
    ]);
}

pub(crate) fn sync_theme(window: Option<&mut Window>, cx: &mut App) {
    crate::theme::apply(window, cx);
}

fn system_locale() -> Option<String> {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var)
            && !v.is_empty()
            && v != "C"
            && v != "POSIX"
        {
            return Some(v);
        }
    }
    if cfg!(target_os = "macos") {
        let out = std::process::Command::new("defaults").args(["read", "-g", "AppleLocale"]).output().ok()?;
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    None
}

/// Saved language, else the system one if bundled, else the default.
pub fn initial_language(settings: &Settings) -> &'static str {
    let saved = settings.language.as_deref().and_then(|l| i18n::available().into_iter().map(|(c, _)| c).find(|c| *c == l));
    saved.or_else(|| system_locale().as_deref().and_then(i18n::match_system)).unwrap_or(i18n::DEFAULT)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Timeline,
    Calendar,
    List,
    Graph,
    /// One person / vehicle / company: its items, deadlines, files and links.
    Entity(vigia_core::entity::EntityId),
    /// A section: every vehicle, pet, person… as a card, with suggested reminders.
    Kind(vigia_core::entity::EntityKind),
}

impl View {
    fn id(self) -> String {
        if let View::Kind(k) = self {
            return format!("kind:{}", k.id()); // i18n-ok: settings value
        }
        match self {
            View::Dashboard => "dashboard",
            View::Timeline => "timeline",
            View::Calendar => "calendar",
            View::List | View::Entity(_) => "list",
            View::Graph => "graph",
            View::Kind(_) => "dashboard",
        }
        .to_string()
    }

    fn from_id(s: &str) -> View {
        if let Some(k) = s.strip_prefix("kind:").and_then(vigia_core::entity::EntityKind::from_id) {
            return View::Kind(k);
        }
        match s {
            "timeline" => View::Timeline,
            "calendar" => View::Calendar,
            "list" => View::List,
            "graph" => View::Graph,
            _ => View::Dashboard,
        }
    }
}

pub struct VigiaApp {
    pub data: Entity<Data>,
    pub view: View,
    pub filter: Filter,
    pub search: Entity<InputState>,
    pub quick: Entity<InputState>,
    pub selected: Option<ItemId>,
    pub edit: Option<crate::panel::EditForm>,
    pub entity_edit: Option<crate::entities::EntityForm>,
    pub graph: crate::graph::GraphState,
    /// Countries whose public holidays show in the calendar and timeline.
    pub holidays: Vec<vigia_core::holidays::Country>,
    pub dialog: Option<Dialog>,
    pub about: bool,
    toast: Option<(String, Instant)>,
    /// The toast says an assistant card was applied: it offers Undo.
    pub toast_undo: bool,
    pub logo: Arc<Image>,
    pub focus: FocusHandle,
    pub cal_month: Date,
    placeholder_locale: &'static str,
    pub ocr_busy: bool,
    /// the AI panel and its cards
    pub agent: crate::assistant::AgentState,
    _subs: Vec<Subscription>,
}

impl VigiaApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        sync_theme(Some(window), cx);
        let data = state::data(cx);
        let search = cx.new(|cx| InputState::new(window, cx));
        let quick = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![
            window.observe_window_appearance(|window, cx| sync_theme(Some(window), cx)),
            cx.observe(&data, |this, _, cx| {
                if this.selected.is_some_and(|id| this.data.read(cx).item(id).is_none()) {
                    this.selected = None;
                    this.edit = None;
                }
                if let View::Entity(id) = this.view
                    && this.data.read(cx).entity(id).is_none()
                {
                    this.view = View::Dashboard;
                    this.entity_edit = None;
                }
                cx.notify();
            }),
            cx.subscribe_in(&search, window, |this, _, ev: &InputEvent, _, cx| {
                if let InputEvent::Change = ev {
                    this.filter.query = this.search.read(cx).value().to_string();
                    if this.view != View::List && !this.filter.query.trim().is_empty() {
                        this.view = View::List;
                    }
                    cx.notify();
                }
            }),
            cx.subscribe_in(&quick, window, |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => this.quick_add(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            }),
        ];
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        crate::star::refresh(Settings::load().github_stars(), cx);
        let title = window_title(data.read(cx));
        window.set_window_title(&title);
        let migration = data.read(cx).migration.clone();
        let mut this = VigiaApp {
            data,
            view: View::from_id(Settings::load().view.as_deref().unwrap_or_default()),
            filter: Filter::default(),
            search,
            quick,
            selected: None,
            edit: None,
            entity_edit: None,
            graph: crate::graph::GraphState::default(),
            holidays: Settings::load().holiday_countries(),
            dialog: migration.map(Dialog::Migrate),
            about: false,
            toast: None,
            toast_undo: false,
            logo: Arc::new(Image::from_bytes(ImageFormat::Png, crate::ICON_PNG.to_vec())),
            focus,
            cal_month: date::month_start(Data::today()),
            placeholder_locale: "",
            ocr_busy: false,
            agent: crate::assistant::AgentState::new(),
            _subs: subs,
        };
        this.start_agent_pump(cx);
        this
    }

    // ── small actions ────────────────────────────────────────────────────────────────────────

    pub fn toast(&mut self, text: String, cx: &mut Context<Self>) {
        self.toast_for(text, TOAST_FOR, cx);
    }

    pub fn toast_for(&mut self, text: String, wait: Duration, cx: &mut Context<Self>) {
        let at = Instant::now();
        self.toast = Some((text, at));
        self.toast_undo = false;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|(_, t)| *t == at) {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn set_view(&mut self, v: View, cx: &mut Context<Self>) {
        self.view = v;
        if !matches!(v, View::Entity(_)) {
            Settings::update(|s| s.view = Some(v.id()));
        }
        cx.notify();
    }

    pub fn open_entity(&mut self, id: vigia_core::entity::EntityId, cx: &mut Context<Self>) {
        if self.entity_edit.as_ref().is_some_and(|f| f.id != id) {
            self.entity_edit = None;
        }
        self.view = View::Entity(id);
        cx.notify();
    }

    pub fn select(&mut self, id: ItemId, cx: &mut Context<Self>) {
        if self.selected != Some(id) {
            self.edit = None;
        }
        self.selected = Some(id);
        cx.notify();
    }

    /// From the menu bar: show this item.
    pub fn select_and_show(&mut self, id: ItemId, cx: &mut Context<Self>) {
        if self.data.read(cx).item(id).is_some() {
            self.select(id, cx);
        }
    }

    /// A deep link (`otherwise://vigia/…`, from another app or the Today view).
    pub fn open_link(&mut self, link: &otherwise_link::Link, window: &mut Window, cx: &mut Context<Self>) {
        use vigia_core::links::Route;
        match vigia_core::links::route(link) {
            Route::Focus => {}
            Route::Item(id) => {
                if self.data.read(cx).item(id).is_some() {
                    self.select(id, cx);
                } else {
                    self.toast(i18n::t!("app.link.not_found"), cx);
                }
            }
            Route::Owner(id) => {
                if self.data.read(cx).entity(id).is_some() {
                    self.open_entity(id, cx);
                } else {
                    self.toast(i18n::t!("app.link.not_found"), cx);
                }
            }
            Route::Section(k) => self.set_view(View::Kind(k), cx),
            Route::Shared(uuid) => match self.data.read(cx).entity_for_shared(uuid) {
                Some(id) => self.open_entity(id, cx),
                None => self.toast(i18n::t!("app.link.not_found"), cx),
            },
            Route::New(p) => {
                let it = {
                    let d = self.data.read(cx);
                    p.to_item(Data::today(), Self::order(), |u| d.entity_for_shared(u).and_then(|id| d.entity(id)).map(|e| (e.id, e.name.clone(), e.kind)))
                };
                self.start_new_with(it, window, cx);
            }
            Route::Unknown => self.toast(i18n::t!("app.link.unknown", url = link.to_url()), cx),
        }
    }

    pub fn show_about(&mut self, cx: &mut Context<Self>) {
        self.about = true;
        cx.notify();
    }

    pub fn focus_quick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.update(cx, |s, cx| s.focus(window, cx));
    }

    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // the search field lives in the list view
        self.set_view(View::List, cx);
        self.search.update(cx, |s, cx| s.focus(window, cx));
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.about {
            self.about = false;
        } else if self.dialog.is_some() {
            self.dialog = None;
        } else if self.entity_edit.is_some() {
            self.entity_edit = None;
        } else if self.edit.is_some() {
            self.edit = None;
        } else if !self.search.read(cx).value().is_empty() {
            self.search.update(cx, |s, cx| s.set_value("", window, cx));
            self.filter.query.clear();
            self.focus.focus(window, cx);
        } else {
            self.selected = None;
        }
        cx.notify();
    }

    fn set_language(&mut self, code: &'static str, cx: &mut Context<Self>) {
        if i18n::set_locale(code) {
            Settings::update(|s| s.language = Some(code.to_string()));
            gpui_component::set_locale(code);
            crate::assistant::sync_locale();
            crate::set_app_menus(cx);
            crate::sync_tray(cx);
            cx.refresh_windows();
        }
    }

    pub fn set_github_stars(&mut self, on: bool, cx: &mut Context<Self>) {
        Settings::update(|s| s.github_stars = (!on).then_some(false));
        crate::star::refresh(on, cx);
        cx.refresh_windows();
    }

    fn sync_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let loc = i18n::locale();
        if self.placeholder_locale != loc {
            self.placeholder_locale = loc;
            let q = i18n::t!("app.quick.placeholder");
            self.quick.update(cx, |s, cx| s.set_placeholder(q, window, cx));
            let s = i18n::t!("app.search.placeholder");
            self.search.update(cx, |st, cx| st.set_placeholder(s, window, cx));
        }
    }

    pub fn order() -> date::Order {
        date::order_for(i18n::locale())
    }

    // ── adding ───────────────────────────────────────────────────────────────────────────────

    fn quick_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.quick.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        let item = vigia_core::quick::parse(&text, Data::today(), Self::order());
        let has_date = item.end_date(Data::today()).is_some();
        match self.data.update(cx, |d, cx| d.save(item, cx)) {
            Ok(id) => {
                self.quick.update(cx, |s, cx| s.set_value("", window, cx));
                self.select(id, cx);
                let it = self.data.read(cx).item(id).cloned();
                if let Some(it) = it {
                    let msg = if has_date { i18n::t!("app.quick.added", name = it.title(), kind = it.kind.label()) } else { i18n::t!("app.quick.added_no_date", name = it.title()) };
                    self.toast(msg, cx);
                    if !has_date {
                        self.start_edit(id, window, cx);
                    }
                }
            }
            Err(e) => self.toast(e, cx),
        }
    }

    /// What the quick-add line would create, shown under it while typing.
    fn quick_preview(&self, cx: &App) -> Option<String> {
        let text = self.quick.read(cx).value().trim().to_string();
        if text.is_empty() {
            return None;
        }
        let it = vigia_core::quick::parse(&text, Data::today(), Self::order());
        let when = it.end_date(Data::today()).map(crate::views::long_date).unwrap_or_else(|| i18n::t!("app.quick.no_date"));
        let mut out = i18n::t!("app.quick.preview", kind = it.kind.label(), name = it.name, date = when);
        if !it.owner.is_empty() {
            let d = self.data.read(cx);
            out = match vigia_core::entity::find(&d.entities, &it.owner) {
                Some(e) => i18n::t!("app.quick.preview_owner", rest = out, kind = e.kind.label(), owner = e.name),
                None => i18n::t!("app.quick.preview_new_owner", rest = out, kind = it.owner_kind.unwrap_or_default().label(), owner = it.owner),
            };
        }
        Some(out)
    }

    pub fn new_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = None;
        self.start_new(window, cx);
    }

    // ── files ────────────────────────────────────────────────────────────────────────────────

    fn pick_file(&mut self, prompt: String, window: &mut Window, cx: &mut Context<Self>, then: impl FnOnce(&mut Self, PathBuf, &mut Window, &mut Context<Self>) + 'static) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(prompt.into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(p) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| then(this, p, window, cx)).ok();
            }
        })
        .detach();
    }

    fn save_file(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>, then: impl FnOnce(&mut Self, PathBuf, &mut Window, &mut Context<Self>) + 'static) {
        let dir = documents_dir();
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(p))) = rx.await {
                this.update_in(cx, |this, window, cx| then(this, p, window, cx)).ok();
            }
        })
        .detach();
    }

    fn import_csv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_file(i18n::t!("app.menu.import_csv"), window, cx, |this, p, _, cx| {
            let text = match std::fs::read_to_string(&p) {
                Ok(t) => t,
                Err(e) => return this.toast(e.to_string(), cx),
            };
            let r = vigia_core::csv::import(&text, Data::today());
            let skipped = r.skipped.len();
            match this.data.update(cx, |d, cx| d.import(r.items, cx)) {
                Ok(n) => this.toast(if skipped > 0 { i18n::tn!("app.import.done_skipped", n, skipped = skipped) } else { i18n::tn!("app.import.done", n) }, cx),
                Err(e) => this.toast(e, cx),
            }
        });
    }

    fn export(&mut self, ics: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = if ics { "vigia.ics" } else { "vigia.csv" }; // i18n-ok: file names
        self.save_file(name.into(), window, cx, move |this, p, _, cx| {
            let items = this.data.read(cx).items.clone();
            let text = if ics { vigia_core::ics::export(&items, Data::today(), vigia_core::db::now_secs()) } else { vigia_core::csv::export(&items) };
            match std::fs::write(&p, text) {
                Ok(()) => this.toast(i18n::t!("app.export.done", file = p.display()), cx),
                Err(e) => this.toast(e.to_string(), cx),
            }
        });
    }

    fn backup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = format!("vigia-backup-{}.zip", date::iso(Data::today())); // i18n-ok: file name
        self.save_file(name, window, cx, |this, p, _, cx| {
            let r = this.data.read(cx).store.as_ref().map(|s| vigia_core::backup::create(s, &p));
            match r {
                Some(Ok(n)) => this.toast(i18n::tn!("app.backup.done", n, file = p.display()), cx),
                Some(Err(e)) => this.toast(e.to_string(), cx),
                None => {}
            }
        });
    }

    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_file(i18n::t!("app.menu.restore"), window, cx, |this, zip, window, cx| {
            let stem = zip.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "vigia".into());
            let dir = zip.parent().map(|p| p.join(stem)).unwrap_or_default();
            match vigia_core::backup::restore(&zip, &dir) {
                Ok(db) => this.switch_db(db, window, cx),
                Err(e) => this.toast(e.to_string(), cx),
            }
        });
    }

    pub fn open_db(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_file(i18n::t!("app.menu.open_db"), window, cx, |this, p, window, cx| this.switch_db(p, window, cx));
    }

    pub fn new_db(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_file("vigia.db".into(), window, cx, |this, p, window, cx| { // i18n-ok: file name
            if p.exists() {
                return this.toast(i18n::t!("app.db.exists"), cx);
            }
            this.switch_db(p, window, cx);
        });
    }

    pub fn move_db(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.data.read(cx).path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| vigia_core::paths::DB_FILE.into());
        self.save_file(name, window, cx, |this, p, window, cx| {
            let p = if p.extension().is_none() { p.with_extension(vigia_core::db::EXTENSION) } else { p };
            if p.exists() {
                return this.toast(i18n::t!("app.db.exists"), cx);
            }
            match this.data.update(cx, |d, cx| d.move_to(p.clone(), cx)) {
                Ok(()) => {
                    this.toast(i18n::t!("app.db.moved", path = p.display()), cx);
                    let t = window_title(this.data.read(cx));
                    window.set_window_title(&t);
                    crate::set_app_menus(cx);
                }
                Err(e) => this.toast(e, cx),
            }
        });
    }

    pub fn switch_db(&mut self, p: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.switch_db_forced(p, false, window, cx);
    }

    pub fn switch_db_forced(&mut self, p: PathBuf, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        if p.exists() && !vigia_core::db::is_vigia(&p) {
            return self.toast(i18n::t!("app.db.not_vigia"), cx);
        }
        match self.data.update(cx, |d, cx| d.switch_to(p.clone(), force, cx)) {
            Ok(()) => {
                self.selected = None;
                self.edit = None;
                self.dialog = None;
                self.toast(i18n::t!("app.db.opened", path = p.display()), cx);
                let t = window_title(self.data.read(cx));
                window.set_window_title(&t);
                crate::set_app_menus(cx);
            }
            Err(state::SwitchError::Locked(info)) => {
                self.dialog = Some(Dialog::Locked { path: p, info });
                cx.notify();
            }
            Err(state::SwitchError::Failed(e)) => self.toast(e, cx),
        }
    }

    // ── OCR assist ───────────────────────────────────────────────────────────────────────────

    pub fn read_document_dialog(&mut self, target: Option<ItemId>, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_file(i18n::t!("app.ocr.menu"), window, cx, move |this, p, _, cx| this.read_document(p, target, cx));
    }

    /// Reads dates and numbers from a document; `target` is the item to fill, else a new one.
    pub fn read_document(&mut self, path: PathBuf, target: Option<ItemId>, cx: &mut Context<Self>) {
        if !vigia_core::ocr::readable(&path) {
            return self.toast(vigia_core::ocr::OcrError::Unsupported.to_string(), cx);
        }
        let is_pdf = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        if !is_pdf && !vigia_core::ocr::models_present() {
            self.dialog = Some(Dialog::OcrModels { path, target });
            cx.notify();
            return;
        }
        self.ocr_busy = true;
        cx.notify();
        let p = path.clone();
        let task = cx.background_executor().spawn(async move { vigia_core::ocr::read_text(&p) });
        cx.spawn(async move |this, cx| {
            let text = task.await;
            this.update(cx, |this, cx| {
                this.ocr_busy = false;
                match text {
                    Ok(text) => {
                        let ex = vigia_core::extract::extract(&text, Data::today(), Self::order());
                        this.dialog = Some(Dialog::Ocr { path, target, ex, text });
                    }
                    Err(e) => this.toast(e.to_string(), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn download_ocr_models(&mut self, path: PathBuf, target: Option<ItemId>, cx: &mut Context<Self>) {
        self.ocr_busy = true;
        self.dialog = None;
        self.toast(i18n::t!("app.ocr.downloading"), cx);
        let task = cx.background_executor().spawn(async move { vigia_core::ocr::download_models() });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |this, cx| {
                this.ocr_busy = false;
                match r {
                    Ok(()) => this.read_document(path, target, cx),
                    Err(e) => this.toast(e.to_string(), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    /// Files dropped on the window: attach to the selected item (and read them), or read the
    /// first one to start a new item.
    fn dropped(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let first_readable = paths.iter().find(|p| vigia_core::ocr::readable(p)).cloned();
        if let Some(id) = self.selected {
            match self.data.update(cx, |d, cx| d.attach(id, &paths, cx)) {
                Ok(n) => self.toast(i18n::tn!("app.attach.done", n), cx),
                Err(e) => self.toast(e, cx),
            }
            return;
        }
        match first_readable {
            Some(p) => self.read_document(p, None, cx),
            None => self.toast(i18n::t!("app.drop.select_first"), cx),
        }
    }

    // ── rendering ────────────────────────────────────────────────────────────────────────────

    fn render_topbar(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let current = i18n::locale();
        let weak = cx.entity().downgrade();
        let stars_on = crate::star::enabled(cx);
        let lang = Button::new("lang").ghost().icon(IconName::Globe).with_size(px(otherwise_ui::tokens::size::CONTROL)).tooltip(i18n::t!("common.language")).dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
            menu = menu.label(i18n::t!("common.language"));
            for (code, name) in i18n::available() {
                let w = weak.clone();
                menu = menu.item(PopupMenuItem::new(name).checked(code == current).on_click(move |_, _, cx| {
                    w.update(cx, |this, cx| this.set_language(code, cx)).ok();
                }));
            }
            let w = weak.clone();
            menu = menu.separator().item(PopupMenuItem::new(i18n::t!("app.star.setting")).checked(stars_on).on_click(move |_, _, cx| {
                w.update(cx, |this, cx| this.set_github_stars(!stars_on, cx)).ok();
            }));
            menu = menu.label(i18n::t!("app.star.setting_hint"));
            let w = weak.clone();
            menu = menu.separator().item(PopupMenuItem::new(i18n::t!("app.menu.settings")).on_click(move |_, window, cx| {
                w.update(cx, |this, cx| this.open_settings(window, cx)).ok();
            }));
            let w = weak.clone();
            menu.item(PopupMenuItem::new(i18n::t!("app.about.menu", app = crate::APP_NAME)).on_click(move |_, _, cx| {
                w.update(cx, |this, cx| this.show_about(cx)).ok();
            }))
        });
        let preview = self.quick_preview(cx);
        let quick = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(6.))
            .w(px(520.))
            .min_w(px(200.))
            .flex_shrink(1.)
            .h(px(otherwise_ui::tokens::size::CONTROL))
            .pl(px(10.))
            .pr(px(4.))
            .rounded(px(otherwise_ui::tokens::radius::CONTROL))
            .border_1()
            .border_color(t.border)
            .bg(t.canvas)
            .child(icon(IconName::Plus, 14., t.accent))
            .child(div().flex_1().min_w_0().text_size(px(13.)).child(Input::new(&self.quick).appearance(false).small()))
            .children(preview.map(|p| deferred(div().absolute().top(px(38.)).left_0().right_0().px(px(10.)).py(px(6.)).rounded(px(8.)).bg(t.ink).text_color(t.on_fill()).text_size(px(12.)).child(p)).with_priority(1)));
        let busy = self.ocr_busy.then(|| div().flex().items_center().gap(px(6.)).text_size(px(12.)).text_color(t.ink_muted).child(gpui_component::spinner::Spinner::new().small().color(t.accent)).child(i18n::t!("app.ocr.reading")));
        let mut header = otherwise_ui::AppHeader::new(otherwise_ui::logo_mark(crate::LOGO_SVG), crate::APP_NAME).center(quick);
        if let Some(b) = busy {
            header = header.action(b);
        }
        header
            .action(ui::outline_button(t, "top-ocr").child(icon(IconName::File, 15., t.ink)).child(i18n::t!("app.ocr.button")).tooltip(|w, cx| gpui_component::tooltip::Tooltip::new(i18n::t!("app.ocr.tooltip")).build(w, cx)).on_click(cx.listener(|this, _, window, cx| this.read_document_dialog(None, window, cx))))
            .action(self.template_picker(cx))
            .action(ui::filled_button(t, "top-new", t.accent).child(icon(IconName::Plus, 15., t.on_fill())).child(i18n::t!("app.item.new_short")).on_click(cx.listener(|this, _, window, cx| this.new_item(window, cx))))
            .action(ui::icon_button(t, "top-assistant", IconName::Bot, true).tooltip(|w, cx| gpui_component::tooltip::Tooltip::new(i18n::t!("app.assistant.toggle")).build(w, cx)).on_click(cx.listener(|this, _, window, cx| this.toggle_agent_panel(window, cx))))
            .action(lang)
    }

    fn render_sidebar(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Data::today();
        let d = self.data.read(cx);
        let counts = vigia_core::view::counts(&d.items, today);
        let used = vigia_core::view::sections(&d.entities, &d.items, &d.relations);
        // every section shows, empty ones too (to add the first car or pet); other only when used
        let sections: Vec<_> = vigia_core::templates::SECTIONS.into_iter().chain([vigia_core::entity::EntityKind::Other]).filter_map(|k| {
            let ents = used.iter().find(|(u, _)| *u == k).map(|(_, v)| v.clone()).unwrap_or_default();
            (vigia_core::templates::has_section(k) || !ents.is_empty()).then_some((k, ents))
        }).collect();
        let mut cats: Vec<(Category, usize)> = Category::ALL.iter().map(|c| (*c, d.items.iter().filter(|i| i.category == *c && i.state != vigia_core::model::State::Archived).count())).filter(|(_, n)| *n > 0).collect();
        cats.sort_by_key(|(c, _)| *c);
        let hover = t.canvas;
        let nav = |id: &'static str, ic: IconName, label: String, v: View, badge: Option<(usize, Hsla)>, on: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(9.))
                .h(px(otherwise_ui::tokens::size::NAV_ITEM))
                .px(px(10.))
                .rounded(px(otherwise_ui::tokens::radius::CONTROL))
                .text_size(px(13.))
                .cursor_pointer()
                .when(on, |d| d.bg(t.accent_soft).text_color(t.accent).font_weight(FontWeight::SEMIBOLD))
                .when(!on, |d| d.hover(move |s| s.bg(hover)))
                .child(icon(ic, 15., if on { t.accent } else { t.ink_muted }))
                .child(div().flex_1().child(label))
                .children(badge.filter(|(n, _)| *n > 0).map(|(n, c)| ui::chip(n.to_string(), c, 11.)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter = Filter { query: this.filter.query.clone(), ..Default::default() };
                    this.set_view(v, cx);
                }))
        };
        let filter_row = |id: ElementId, label: String, n: usize, on: bool, set: Box<dyn Fn(&mut Filter)>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .px(px(10.))
                .rounded(px(otherwise_ui::tokens::radius::CONTROL))
                .text_size(px(otherwise_ui::tokens::text::SMALL))
                .cursor_pointer()
                .when(on, |d| d.bg(t.accent_soft).font_weight(FontWeight::SEMIBOLD))
                .when(!on, |d| d.hover(move |s| s.bg(hover)))
                .child(div().flex_1().truncate().child(label))
                .child(div().text_size(px(11.)).text_color(t.ink_muted).child(n.to_string()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    set(&mut this.filter);
                    this.view = View::List;
                    cx.notify();
                }))
        };
        let v = self.view;
        let mut side = div()
            .id("sidebar-items")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(2.))
            .overflow_y_scroll()
            .child(nav("nav-dash", IconName::LayoutDashboard, i18n::t!("app.view.dashboard"), View::Dashboard, Some((counts.attention(), t.warn)), v == View::Dashboard))
            .child(nav("nav-time", IconName::Map, i18n::t!("app.view.timeline"), View::Timeline, None, v == View::Timeline))
            .child(nav("nav-cal", IconName::Calendar, i18n::t!("app.view.calendar"), View::Calendar, None, v == View::Calendar))
            .child(nav("nav-list", IconName::Inbox, i18n::t!("app.view.list"), View::List, Some((counts.total, t.ink_muted)), v == View::List && self.filter.entity.is_none() && self.filter.entity_kind.is_none() && self.filter.category.is_none() && self.filter.status.is_none()))
            .child({
                let on = v == View::Graph;
                div()
                    .id("nav-graph")
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .h(px(otherwise_ui::tokens::size::NAV_ITEM))
                    .px(px(10.))
                    .rounded(px(otherwise_ui::tokens::radius::CONTROL))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(t.accent_soft).text_color(t.accent).font_weight(FontWeight::SEMIBOLD))
                    .when(!on, |d| d.hover(move |s| s.bg(hover)))
                    .child(ui::svg_icon(ui::GRAPH_SVG, 15., if on { t.accent } else { t.ink_muted }))
                    .child(div().flex_1().child(i18n::t!("app.view.graph")))
                    .on_click(cx.listener(|this, _, _, cx| this.set_view(View::Graph, cx)))
            });
        for (kind, ents) in sections {
            let kind_on = v == View::Kind(kind);
            let plus_hover = t.border;
            let header = div()
                .id(("side-kind", kind as usize))
                .flex()
                .items_center()
                .gap(px(6.))
                .mt(px(10.))
                .h(px(26.))
                .px(px(10.))
                .rounded(px(otherwise_ui::tokens::radius::CONTROL))
                .cursor_pointer()
                .when(kind_on, |d| d.bg(t.accent_soft))
                .when(!kind_on, |d| d.hover(move |s| s.bg(hover)))
                .child(div().flex_1().child(ui::section_title(t, crate::entities::section_label(kind)).when(kind_on, |d| d.text_color(t.accent))))
                .child(div().id(("side-kind-add", kind as usize)).flex().items_center().justify_center().size(px(18.)).rounded(px(5.)).hover(move |s| s.bg(plus_hover)).child(icon(IconName::Plus, 11., t.ink_muted)).tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(crate::kinds::add_label(kind)).build(w, cx)).on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.start_new_entity(kind, window, cx);
                })))
                .on_click(cx.listener(move |this, _, _, cx| this.set_view(View::Kind(kind), cx)));
            side = side.child(header);
            // what the section holds that no entity of its kind does (a company's domains)
            let loose = vigia_core::view::loose(&d.items, &d.entities, kind, today);
            if !loose.is_empty() {
                let ring = loose.iter().map(|i| i.status(today)).filter(|s| *s != vigia_core::model::Status::Undated).min().map(|s| t.status(s));
                let on = v == View::Kind(kind);
                side = side.child(
                    div()
                        .id(("side-loose", kind as usize))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .px(px(10.))
                        .rounded(px(otherwise_ui::tokens::radius::CONTROL))
                        .text_size(px(otherwise_ui::tokens::text::SMALL))
                        .text_color(t.ink_muted)
                        .cursor_pointer()
                        .when(!on, |d| d.hover(move |s| s.bg(hover)))
                        .child(icon(IconName::Inbox, 13., t.ink_muted))
                        .child(div().flex_1().min_w_0().truncate().child(crate::kinds::loose_label(kind)))
                        .children(ring.map(|c| div().size(px(6.)).rounded_full().bg(c)))
                        .child(div().text_size(px(11.)).child(loose.len().to_string()))
                        .on_click(cx.listener(move |this, _, _, cx| this.set_view(View::Kind(kind), cx))),
                );
            }
            for (e, n) in ents {
                let id = e.id;
                let on = v == View::Entity(id);
                let ring = vigia_core::view::worst_status(&d.items, id, today).map(|s| t.status(s));
                side = side.child(
                    div()
                        .id(("ent", id as usize))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .px(px(10.))
                        .rounded(px(otherwise_ui::tokens::radius::CONTROL))
                        .text_size(px(otherwise_ui::tokens::text::SMALL))
                        .cursor_pointer()
                        .when(on, |d| d.bg(t.accent_soft).font_weight(FontWeight::SEMIBOLD))
                        .when(!on, |d| d.hover(move |s| s.bg(hover)))
                        .child(ui::entity_icon(kind, 13., if on { t.accent } else { t.ink_muted }))
                        .child(div().flex_1().min_w_0().truncate().child(e.name.clone()))
                        .children(ring.map(|c| div().size(px(6.)).rounded_full().bg(c)))
                        .child(div().text_size(px(11.)).text_color(t.ink_muted).child(n.to_string()))
                        .on_click(cx.listener(move |this, _, _, cx| this.open_entity(id, cx))),
                );
            }
        }
        if !cats.is_empty() {
            side = side.child(div().pt(px(14.)).pb(px(4.)).px(px(10.)).child(ui::section_title(t, i18n::t!("app.side.categories"))));
            for (c, n) in cats {
                let on = v == View::List && self.filter.category == Some(c);
                side = side.child(filter_row(("cat", c as usize).into(), c.label(), n, on, Box::new(move |f| *f = Filter { category: Some(c), ..Default::default() })));
            }
        }
        div()
            .id("sidebar")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(SIDEBAR_W))
            .h_full()
            .gap(px(otherwise_ui::tokens::space::S4))
            .p(px(10.))
            .border_r_1()
            .border_color(t.border)
            .bg(t.surface)
            .child(side)
            .child(otherwise_ui::privacy_note(t, i18n::t!("app.privacy.local"), None))
    }

    fn render_failed(&self, t: &Tokens, message: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let path = self.data.read(cx).path.display().to_string();
        let blocked = self.data.read(cx).blocked.is_some();
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .size_full()
            .child(icon(IconName::TriangleAlert, 32., t.danger))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(i18n::t!("app.error.title")))
            .child(div().text_size(px(13.)).text_color(t.ink_muted).child(path))
            .child(div().max_w(px(520.)).text_center().text_size(px(13.)).child(message.to_string()))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::outline_button(t, "err-new").child(i18n::t!("app.db.new")).on_click(cx.listener(|this, _, window, cx| this.new_db(window, cx))))
                    .child(ui::filled_button(t, "err-open", t.accent).child(i18n::t!("app.menu.open_db")).on_click(cx.listener(|this, _, window, cx| this.open_db(window, cx))))
                    .when(blocked, |d| d.child(ui::outline_button(t, "err-take-over").child(i18n::t!("app.db.take_over")).on_click(cx.listener(|this, _, _, cx| this.data.update(cx, |d, cx| d.take_over(cx)))))),
            )
            .when(blocked, |d| d.child(div().max_w(px(520.)).text_center().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.db.locked_hint"))))
    }

    /// Slim bar at the bottom: the database on the left, About on the right.
    fn render_footer(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let d = self.data.read(cx);
        let n = d.items.len();
        let mut left = format!("{} · {}", d.path.display(), i18n::tn!("app.items.count", n)); // i18n-ok: separator
        if d.temporary {
            left = i18n::t!("app.db.temporary", rest = left);
        }
        let note = self.storage_note(cx);
        let read_only = d.read_only();
        let checking = d.checking.len();
        let mut footer = otherwise_ui::AppFooter::new(crate::APP_NAME, i18n::t!("app.about.footer_link")).version(crate::VERSION).status(div().truncate().child(left));
        if let Some((n, warn)) = note {
            footer = footer.item(div().flex_none().text_color(if warn { t.danger } else { t.ink_muted }).child(n));
        }
        if read_only {
            footer = footer.item(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(t.danger).child(i18n::t!("app.db.read_only_badge")));
        }
        if checking > 0 {
            footer = footer.item(div().flex_none().child(i18n::tn!("app.check.running", checking)));
        }
        footer.item(crate::star::footer_link(t, cx)).on_about(cx.listener(|this, _, _, cx| this.show_about(cx)))
    }

    fn render_about(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let links = div().flex().items_center().gap(px(14.)).child(otherwise_ui::link(t, "about-source").text_size(px(otherwise_ui::tokens::text::SMALL)).child(i18n::t!("app.about.source")).on_click(|_, _, cx| cx.open_url(crate::REPO_URL))).child(crate::star::about_button(t, cx));
        otherwise_ui::AboutModal::new(crate::APP_NAME, crate::VERSION, i18n::t!("app.about.by", company = crate::COMPANY))
            .logo(self.logo.clone())
            .description(i18n::t!("app.about.description"))
            .place(i18n::t!("app.about.place"))
            .website(i18n::t!("app.about.visit"), crate::COMPANY_URL)
            .vcity(i18n::t!("app.vcity.invite"), i18n::t!("app.vcity.menu"), crate::VCITY_URL)
            .extra(links)
            .extra(crate::star::about_ask(t))
            .license(i18n::t!("app.about.license"))
            .on_dismiss(cx.listener(|this, _: &otherwise_ui::Dismiss, _, cx| {
                this.about = false;
                cx.notify();
            }))
    }

    fn render_toast(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (text, _) = self.toast.as_ref()?;
        let undo = (self.toast_undo && self.agent.last.is_some()).then(|| {
            div().id("toast-undo").text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(t.accent_soft).cursor_pointer().hover(|s| s.underline()).child(i18n::t!("app.assistant.undo")).on_click(cx.listener(|this, _, _, cx| this.undo_last_proposal(cx)))
        });
        Some(div().absolute().bottom(px(36.)).left_0().right_0().flex().justify_center().child(otherwise_ui::toast(t, text.clone()).gap(px(14.)).max_w(px(720.)).children(undo)))
    }

    pub fn selected_item(&self, cx: &App) -> Option<Item> {
        self.selected.and_then(|id| self.data.read(cx).item(id).cloned())
    }

    fn with_selected(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Self, Item, &mut Window, &mut Context<Self>)) {
        if let Some(it) = self.selected_item(cx) {
            f(self, it, window, cx);
        }
    }
}

pub fn window_title(d: &Data) -> String {
    let name = d.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if name == "vigia" { crate::APP_NAME.to_string() } else { format!("{name} — {}", crate::APP_NAME) } // i18n-ok: separator
}

/// `~/Documents` if it exists, else home.
pub fn documents_dir() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    let docs = home.join("Documents");
    if docs.is_dir() { docs } else { home }
}

impl Render for VigiaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_placeholders(window, cx);
        let t = tokens(cx);
        VigiaApp::sync_agent_style(&t, cx);
        let error = self.data.read(cx).error.clone();
        let body: AnyElement = match error {
            Some(e) if self.data.read(cx).store.is_none() => self.render_failed(&t, &e, cx).into_any_element(),
            _ => {
                let center = match self.view {
                    View::Dashboard => self.render_dashboard(&t, cx).into_any_element(),
                    View::Timeline => self.render_timeline(&t, cx).into_any_element(),
                    View::Calendar => self.render_calendar(&t, cx).into_any_element(),
                    View::List => self.render_list(&t, cx).into_any_element(),
                    View::Graph => self.render_graph(window, cx),
                    View::Entity(id) => self.render_entity_page(&t, id, cx),
                    View::Kind(k) => self.render_kind_page(&t, k, cx),
                };
                let accent = t.accent;
                let center = div()
                    .id("center")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .drag_over::<ExternalPaths>(move |s, _, _, _| s.bg(accent.opacity(0.08)))
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.dropped(paths.paths().to_vec(), cx)))
                    .child(center);
                div().flex().size_full().min_h_0().child(self.render_sidebar(&t, cx)).child(center).children(self.render_panel(&t, window, cx)).into_any_element()
            }
        };
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NewItem, window, cx| this.new_item(window, cx)))
            .on_action(cx.listener(|this, _: &FocusQuickAdd, window, cx| this.focus_quick(window, cx)))
            .on_action(cx.listener(|this, _: &ReadDocument, window, cx| {
                let target = this.selected;
                this.read_document_dialog(target, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ImportCsv, window, cx| this.import_csv(window, cx)))
            .on_action(cx.listener(|this, _: &ExportCsv, window, cx| this.export(false, window, cx)))
            .on_action(cx.listener(|this, _: &ExportIcs, window, cx| this.export(true, window, cx)))
            .on_action(cx.listener(|this, _: &Backup, window, cx| this.backup(window, cx)))
            .on_action(cx.listener(|this, _: &Restore, window, cx| this.restore(window, cx)))
            .on_action(cx.listener(|this, _: &OpenDb, window, cx| this.open_db(window, cx)))
            .on_action(cx.listener(|this, _: &MoveDb, window, cx| this.move_db(window, cx)))
            .on_action(cx.listener(|this, _: &SaveDbAs, window, cx| this.save_db_as(window, cx)))
            .on_action(cx.listener(|this, _: &RevealDb, _, cx| this.reveal_db(cx)))
            .on_action(cx.listener(|this, _: &ClearRecent, _, cx| this.clear_recent(cx)))
            .on_action(cx.listener(|this, _: &OpenRecent0, window, cx| this.open_recent(0, window, cx)))
            .on_action(cx.listener(|this, _: &OpenRecent1, window, cx| this.open_recent(1, window, cx)))
            .on_action(cx.listener(|this, _: &OpenRecent2, window, cx| this.open_recent(2, window, cx)))
            .on_action(cx.listener(|this, _: &OpenRecent3, window, cx| this.open_recent(3, window, cx)))
            .on_action(cx.listener(|this, _: &OpenRecent4, window, cx| this.open_recent(4, window, cx)))
            .on_action(cx.listener(|this, _: &OpenRecent5, window, cx| this.open_recent(5, window, cx)))
            .on_action(cx.listener(|this, _: &OpenRecent6, window, cx| this.open_recent(6, window, cx)))
            .on_action(cx.listener(|this, _: &OpenRecent7, window, cx| this.open_recent(7, window, cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|this, _: &ShowDashboard, _, cx| this.set_view(View::Dashboard, cx)))
            .on_action(cx.listener(|this, _: &ShowTimeline, _, cx| this.set_view(View::Timeline, cx)))
            .on_action(cx.listener(|this, _: &ShowCalendar, _, cx| this.set_view(View::Calendar, cx)))
            .on_action(cx.listener(|this, _: &ShowList, _, cx| this.set_view(View::List, cx)))
            .on_action(cx.listener(|this, _: &ShowGraph, _, cx| this.set_view(View::Graph, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)))
            .on_action(cx.listener(|this, _: &Escape, window, cx| this.escape(window, cx)))
            .on_action(cx.listener(|this, _: &RenewSelected, window, cx| this.with_selected(window, cx, |this, it, window, cx| this.open_renew(it.id, window, cx))))
            .on_action(cx.listener(|this, _: &EditSelected, window, cx| this.with_selected(window, cx, |this, it, window, cx| this.start_edit(it.id, window, cx))))
            .on_action(cx.listener(|this, _: &ToggleInProgress, window, cx| {
                this.with_selected(window, cx, |this, it, _, cx| {
                    let s = if it.state == vigia_core::model::State::InProgress { vigia_core::model::State::Active } else { vigia_core::model::State::InProgress };
                    this.data.update(cx, |d, cx| d.set_state(it.id, s, cx));
                })
            }))
            .on_action(cx.listener(|this, _: &SnoozeSelected, window, cx| this.with_selected(window, cx, |this, it, _, cx| this.snooze(it.id, 7, cx))))
            .on_action(cx.listener(|this, _: &CheckSelected, window, cx| {
                this.with_selected(window, cx, |this, it, _, cx| {
                    if it.kind.checkable() {
                        this.data.update(cx, |d, cx| d.check(vec![it.id], cx));
                    }
                })
            }))
            .on_action(cx.listener(|this, _: &DeleteSelected, window, cx| this.with_selected(window, cx, |this, it, _, cx| this.ask_delete(it.id, cx))))
            .on_action(cx.listener(|this, _: &ShowAbout, _, cx| this.show_about(cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.canvas)
            .text_color(t.ink)
            .font_family(SANS)
            .child(self.render_topbar(&t, cx))
            .child(div().flex_1().min_h_0().child(body))
            .child(self.render_footer(&t, cx))
            .children(self.render_toast(&t, cx))
            .children(self.render_dialog(&t, cx))
            .when(self.about, |d| d.child(self.render_about(&t, cx)))
    }
}

impl VigiaApp {
    pub fn snooze(&mut self, id: ItemId, days: i64, cx: &mut Context<Self>) {
        let until = date::add_days(Data::today(), days);
        self.data.update(cx, |d, cx| d.snooze(id, Some(until), cx));
        self.toast(i18n::t!("app.item.snoozed", date = crate::views::long_date(until)), cx);
    }

    pub fn status_filter(&mut self, s: Option<Status>, cx: &mut Context<Self>) {
        self.filter.status = s;
        self.view = View::List;
        cx.notify();
    }
}
