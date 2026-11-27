//! Modal dialogs: renew (asks the new expiry), delete, settings (database location, reminders,
//! menu bar, login, checks, calendar file, OCR models, stars) and the OCR result.
use crate::app::VigiaApp;
use crate::state::Data;
use crate::theme::Tokens;
use crate::ui::{self, icon};
use crate::views::long_date;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::{IconName, Sizable as _};
use std::path::PathBuf;
use vigia_core::date;
use vigia_core::extract::Extracted;
use vigia_core::model::{Item, ItemId, Kind};
use vigia_core::settings::Settings;

pub struct SettingsForm {
    pub notify: bool,
    pub tray: bool,
    pub login: bool,
    pub auto_checks: bool,
    pub ics_sync: bool,
    pub today: bool,
    pub stars: bool,
    pub hour: Entity<InputState>,
}

pub enum Dialog {
    Renew { id: ItemId, expires: Entity<InputState>, cost: Entity<InputState>, note: Entity<InputState>, _sub: Subscription },
    Delete { id: ItemId, name: String },
    CalendarUrl { url: Entity<InputState> },
    DeleteEntity { id: vigia_core::entity::EntityId, name: String, items: usize },
    Settings(SettingsForm),
    /// The models must be downloaded before reading an image.
    OcrModels { path: PathBuf, target: Option<ItemId> },
    Ocr { path: PathBuf, target: Option<ItemId>, ex: Extracted, text: String },
    /// 0.2.1: move the database from the old default to the Otherwise data folder?
    Migrate(vigia_core::paths::Migration),
    /// The database to open is in use on another computer / in another process.
    Locked { path: PathBuf, info: otherwise_link::lock::LockInfo },
    /// A recent database that isn't there any more.
    Missing { path: PathBuf },
}

/// Title, body, OK label, danger, OK action.
pub(crate) type Parts = (String, AnyElement, String, bool, fn(&mut VigiaApp, &mut Window, &mut Context<VigiaApp>));

impl VigiaApp {
    pub fn open_renew(&mut self, id: ItemId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(it) = self.data.read(cx).item(id).cloned() else { return };
        let next = it.suggest_next(Data::today());
        let expires = cx.new(|cx| InputState::new(window, cx).default_value(date::iso(next)));
        let cost = cx.new(|cx| InputState::new(window, cx).placeholder(i18n::t!("app.renew.cost_ph")).default_value(it.cost.map(|c| format!("{c}")).unwrap_or_default()));
        let note = cx.new(|cx| InputState::new(window, cx).placeholder(i18n::t!("app.renew.note_ph")));
        expires.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.observe(&expires, |_, _, cx| cx.notify());
        self.dialog = Some(Dialog::Renew { id, expires, cost, note, _sub: sub });
        cx.notify();
    }

    fn submit_renew(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Renew { id, expires, cost, note, .. }) = self.dialog.as_ref() else { return };
        let today = Data::today();
        let v = expires.read(cx).value().trim().to_string();
        let Some(new) = date::parse_loose(&v, today, Self::order()) else { return self.toast(i18n::t!("app.edit.bad_date", value = v), cx) };
        let c = cost.read(cx).value().trim().replace(',', ".");
        let cost = if c.is_empty() { None } else { c.parse::<f64>().ok() };
        let note = note.read(cx).value().trim().to_string();
        let id = *id;
        match self.data.update(cx, |d, cx| d.renew(id, new, cost, &note, cx)) {
            Ok(()) => {
                self.dialog = None;
                self.toast(i18n::t!("app.renew.done", date = long_date(new)), cx);
            }
            Err(e) => self.toast(e, cx),
        }
    }

    pub fn ask_calendar_url(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = cx.new(|cx| InputState::new(window, cx).placeholder(i18n::t!("app.cal.url_ph")));
        url.update(cx, |s, cx| s.focus(window, cx));
        self.dialog = Some(Dialog::CalendarUrl { url });
        cx.notify();
    }

    pub fn ask_delete(&mut self, id: ItemId, cx: &mut Context<Self>) {
        let Some(it) = self.data.read(cx).item(id) else { return };
        self.dialog = Some(Dialog::Delete { id, name: it.title() });
        cx.notify();
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = Settings::load();
        let hour = cx.new(|cx| InputState::new(window, cx).default_value(s.remind_hour().to_string()));
        self.dialog = Some(Dialog::Settings(SettingsForm { notify: s.notify(), tray: s.tray(), login: crate::system::launch_at_login(), auto_checks: s.auto_checks(), ics_sync: s.ics_sync(), today: s.today_feed(), stars: s.github_stars(), hour }));
        cx.notify();
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Settings(f)) = self.dialog.take() else { return };
        let hour = f.hour.read(cx).value().trim().parse::<u32>().ok().map(|h| h.min(23));
        let before = Settings::load();
        Settings::update(|s| {
            s.notify = (!f.notify).then_some(false);
            s.tray = (!f.tray).then_some(false);
            s.auto_checks = (!f.auto_checks).then_some(false);
            s.ics_sync = f.ics_sync.then_some(true);
            s.today_feed = (!f.today).then_some(false);
            s.remind_hour = hour.filter(|h| *h != 9);
        });
        if f.login != crate::system::launch_at_login()
            && let Err(e) = crate::system::set_launch_at_login(f.login)
        {
            self.toast(i18n::t!("app.settings.login_failed", detail = e), cx);
        }
        if f.stars != before.github_stars() {
            self.set_github_stars(f.stars, cx);
        }
        if f.ics_sync && !before.ics_sync() {
            self.data.read(cx).sync_ics();
        }
        if f.today != before.today_feed() {
            self.data.update(cx, |d, _| d.publish_today());
        }
        crate::sync_tray(cx);
        if f.auto_checks && !before.auto_checks() {
            self.data.update(cx, |d, cx| d.run_due_checks(cx));
        }
        cx.notify();
    }

    /// The OCR result becomes a new item (with the document attached) or fills the target.
    fn apply_ocr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Ocr { path, target, ex, .. }) = self.dialog.take() else { return };
        match target {
            Some(id) => {
                let attached = self.data.read(cx).store.as_ref().map(|s| s.attachments(id).unwrap_or_default().iter().any(|a| s.attachment_path(a) == path)).unwrap_or(false);
                self.data.update(cx, |d, cx| {
                    d.modify(
                        id,
                        |it| {
                            if let Some(e) = ex.expires {
                                it.expires = Some(e);
                            }
                            if let Some(i) = ex.issued {
                                it.issued = Some(i);
                            }
                            if let Some(n) = &ex.number
                                && it.reference.is_empty()
                            {
                                it.reference = n.clone();
                            }
                        },
                        cx,
                    );
                    if !attached {
                        let _ = d.attach(id, std::slice::from_ref(&path), cx);
                    }
                });
                self.toast(i18n::t!("app.ocr.applied"), cx);
            }
            None => {
                let kind = ex.kind.unwrap_or(Kind::Custom);
                let mut it = Item::new(kind, kind.label());
                it.expires = ex.expires;
                it.issued = ex.issued;
                it.reference = ex.number.clone().unwrap_or_default();
                if let Some(h) = &ex.holder {
                    it.owner = h.clone();
                    it.owner_kind = Some(vigia_core::entity::EntityKind::Person);
                }
                match self.data.update(cx, |d, cx| d.save(it, cx)) {
                    Ok(id) => {
                        let _ = self.data.update(cx, |d, cx| d.attach(id, std::slice::from_ref(&path), cx));
                        self.start_edit(id, window, cx);
                        self.toast(i18n::t!("app.ocr.created"), cx);
                    }
                    Err(e) => self.toast(e, cx),
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn render_dialog(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.dialog.as_ref()?;
        let text = |s: String| div().text_size(px(13.)).line_height(px(19.)).text_color(t.ink_muted).child(s);
        let label = |s: String| div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(s);
        let alt = Self::storage_dialog_alt(d);
        let (title, body, ok_label, danger, ok): Parts = match d {
            Dialog::Migrate(_) | Dialog::Locked { .. } | Dialog::Missing { .. } => self.storage_dialog(d, t)?,
            Dialog::Renew { id, expires, cost, note, .. } => {
                let it = self.data.read(cx).item(*id).cloned().unwrap_or_default();
                let today = Data::today();
                let v = expires.read(cx).value().to_string();
                let parsed = date::parse_loose(&v, today, Self::order());
                let b = div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(text(i18n::t!("app.renew.body", name = it.title(), date = it.end_date(today).map(long_date).unwrap_or_else(|| i18n::t!("app.days.none")))))
                    .child(label(i18n::t!("app.renew.new_expiry")))
                    .child(Input::new(expires))
                    .child(div().text_size(px(12.)).text_color(if parsed.is_some() { t.ink_muted } else { t.danger }).child(match parsed {
                        Some(d) => i18n::t!("app.renew.suggested", date = long_date(d), rule = it.recurrence.label()),
                        None => i18n::t!("app.edit.bad_date", value = v),
                    }))
                    .child(div().flex().gap(px(8.)).child(div().flex_1().flex().flex_col().gap(px(4.)).child(label(i18n::t!("app.field.cost"))).child(Input::new(cost))).child(div().flex_1().flex().flex_col().gap(px(4.)).child(label(i18n::t!("app.renew.note"))).child(Input::new(note))))
                    .child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.renew.history")));
                (i18n::t!("app.renew.title"), b.into_any_element(), i18n::t!("app.renew.ok"), false, |this, _, cx| this.submit_renew(cx))
            }
            Dialog::Delete { name, .. } => (i18n::t!("app.delete.title"), text(i18n::t!("app.delete.body", name = name.clone())).into_any_element(), i18n::t!("app.item.delete"), true, |this, _, cx| {
                if let Some(Dialog::Delete { id, .. }) = this.dialog.take() {
                    this.data.update(cx, |d, cx| d.delete(id, cx));
                    this.selected = None;
                    this.edit = None;
                }
                cx.notify();
            }),
            Dialog::CalendarUrl { url } => {
                let b = div().flex().flex_col().gap(px(10.)).child(text(i18n::t!("app.cal.url_body"))).child(Input::new(url).small()).child(text(i18n::t!("app.cal.url_privacy")));
                (i18n::t!("app.cal.import_url"), b.into_any_element(), i18n::t!("app.cal.add"), false, |this, _, cx| {
                    if let Some(Dialog::CalendarUrl { url }) = this.dialog.take() {
                        let u = url.read(cx).value().trim().to_string();
                        if vigia_core::calendars::is_url(&u) {
                            this.add_calendar_url(u, cx);
                        } else {
                            this.toast(i18n::t!("app.cal.bad_url"), cx);
                        }
                    }
                    cx.notify();
                })
            }
            Dialog::DeleteEntity { name, items, .. } => (i18n::t!("app.entity.delete_title"), text(i18n::tn!("app.entity.delete_body", *items, name = name.clone())).into_any_element(), i18n::t!("app.entity.delete"), true, |this, _, cx| {
                if let Some(Dialog::DeleteEntity { id, .. }) = this.dialog.take() {
                    this.data.update(cx, |d, cx| d.delete_entity(id, cx));
                    this.entity_edit = None;
                    this.view = crate::app::View::Dashboard;
                }
                cx.notify();
            }),
            Dialog::OcrModels { .. } => {
                let b = div().flex().flex_col().gap(px(10.)).child(text(i18n::t!("app.ocr.models_body"))).child(text(i18n::t!("app.ocr.models_privacy")));
                (i18n::t!("app.ocr.models_title"), b.into_any_element(), i18n::t!("app.ocr.models_ok"), false, |this, _, cx| {
                    if let Some(Dialog::OcrModels { path, target }) = this.dialog.take() {
                        this.download_ocr_models(path, target, cx);
                    }
                })
            }
            Dialog::Ocr { path, target, ex, text: raw } => {
                let row = |l: String, v: Option<String>| div().flex().gap(px(10.)).child(div().w(px(130.)).text_size(px(13.)).text_color(t.ink_muted).child(l)).child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(v.unwrap_or_else(|| i18n::t!("app.ocr.not_found"))));
                let found = ex.expires.is_some() || ex.number.is_some() || ex.issued.is_some();
                let b = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(text(i18n::t!("app.ocr.body", file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())))
                    .child(row(i18n::t!("app.field.kind"), ex.kind.map(|k| k.label())))
                    .child(row(i18n::t!("app.field.expires"), ex.expires.map(long_date)))
                    .child(row(i18n::t!("app.field.issued"), ex.issued.map(long_date)))
                    .child(row(i18n::t!("app.field.reference"), ex.number.clone()))
                    .when(ex.holder.is_some(), |d| d.child(row(i18n::t!("app.field.owner"), ex.holder.clone())))
                    .when(!found, |d| d.child(div().text_size(px(13.)).text_color(t.warn).child(i18n::t!("app.ocr.nothing"))))
                    .child(div().id("ocr-text").max_h(px(140.)).overflow_y_scroll().p(px(8.)).rounded(px(6.)).bg(t.canvas).text_size(px(11.)).text_color(t.ink_muted).child(if raw.trim().is_empty() { i18n::t!("app.ocr.no_text") } else { raw.chars().take(2000).collect::<String>() }))
                    .child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.ocr.local")));
                let ok = if target.is_some() { i18n::t!("app.ocr.apply") } else { i18n::t!("app.ocr.create") };
                (i18n::t!("app.ocr.title"), b.into_any_element(), ok, false, |this, window, cx| this.apply_ocr(window, cx))
            }
            Dialog::Settings(f) => {
                let hint = |s: String| div().text_size(px(12.)).line_height(px(16.)).text_color(t.ink_muted).child(s);
                let check = |id: &'static str, l: String, on: bool, set: fn(&mut SettingsForm, bool)| {
                    Checkbox::new(id).small().label(l).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                        if let Some(Dialog::Settings(f)) = this.dialog.as_mut() {
                            set(f, *v);
                        }
                        cx.notify();
                    }))
                };
                let data = self.data.read(cx);
                let db_path = data.path.display().to_string();
                let files = data.files_dir().display().to_string();
                let temporary = data.temporary;
                let storage_note = self.storage_note(cx);
                let ics = Data::ics_path().map(|p| p.display().to_string()).unwrap_or_default();
                let small_btn = |id: &'static str| ui::outline_button(t, id).h(px(28.)).px(px(10.)).text_size(px(12.));
                let models = vigia_core::ocr::models_present();
                let b = div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(ui::section_title(t, i18n::t!("app.settings.db")))
                    .child(div().p(px(8.)).rounded(px(6.)).bg(t.canvas).text_size(px(12.)).child(db_path))
                    .children(storage_note.map(|(n, warn)| div().text_size(px(12.)).text_color(if warn { t.danger } else { t.ink_muted }).child(n)))
                    .child(hint(i18n::t!("app.settings.db_hint", files = files)))
                    .when(temporary, |d| d.child(hint(i18n::t!("app.settings.db_temporary"))))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(small_btn("db-save-as").child(i18n::t!("app.menu.save_as")).on_click(cx.listener(|this, _, window, cx| this.save_db_as(window, cx))))
                            .child(small_btn("db-move").child(i18n::t!("app.settings.move")).on_click(cx.listener(|this, _, window, cx| this.move_db(window, cx))))
                            .child(small_btn("db-open").child(i18n::t!("app.settings.open")).on_click(cx.listener(|this, _, window, cx| this.open_db(window, cx))))
                            .child(small_btn("db-new").child(i18n::t!("app.db.new")).on_click(cx.listener(|this, _, window, cx| this.new_db(window, cx))))
                            .child(small_btn("db-backup").child(i18n::t!("app.menu.backup")).on_click(cx.listener(|_, _, window, cx| window.dispatch_action(Box::new(crate::app::Backup), cx))))
                            .child(small_btn("db-reveal").child(crate::storage_ui::reveal_label()).on_click(cx.listener(|this, _, _, cx| this.reveal_db(cx)))),
                    )
                    .child(ui::section_title(t, i18n::t!("app.settings.reminders")))
                    .child(check("set-notify", i18n::t!("app.settings.notify"), f.notify, |f, v| f.notify = v))
                    .child(div().flex().items_center().gap(px(8.)).child(label(i18n::t!("app.settings.hour"))).child(div().w(px(60.)).child(Input::new(&f.hour).small())))
                    .when(cfg!(target_os = "macos"), |d| d.child(check("set-tray", i18n::t!("app.settings.tray"), f.tray, |f, v| f.tray = v)).child(hint(i18n::t!("app.settings.tray_hint"))))
                    .child(check("set-login", i18n::t!("app.settings.login"), f.login, |f, v| f.login = v))
                    .child(ui::section_title(t, i18n::t!("app.settings.checks")))
                    .child(check("set-checks", i18n::t!("app.settings.auto_checks"), f.auto_checks, |f, v| f.auto_checks = v))
                    .child(hint(i18n::t!("app.settings.checks_hint")))
                    .child(ui::section_title(t, i18n::t!("app.settings.calendar")))
                    .child(check("set-ics", i18n::t!("app.settings.ics"), f.ics_sync, |f, v| f.ics_sync = v))
                    .child(hint(i18n::t!("app.settings.ics_hint", path = ics)))
                    .child(ui::section_title(t, i18n::t!("app.settings.otherwise")))
                    .child(check("set-today", i18n::t!("app.settings.today"), f.today, |f, v| f.today = v))
                    .child(hint(if otherwise_link::settings::HubSettings::load(&otherwise_link::Root::system()).entities { i18n::t!("app.settings.registry_on") } else { i18n::t!("app.settings.registry_off") }))
                    .child(ui::section_title(t, i18n::t!("app.settings.ocr")))
                    .child(div().flex().items_center().gap(px(8.)).child(icon(if models { IconName::CircleCheck } else { IconName::Info }, 14., if models { t.ok } else { t.ink_muted })).child(div().text_size(px(13.)).child(if models { i18n::t!("app.settings.ocr_ready") } else { i18n::t!("app.settings.ocr_missing") })))
                    .child(hint(i18n::t!("app.ocr.models_privacy")))
                    .child(ui::section_title(t, i18n::t!("app.settings.other")))
                    .child(check("set-stars", i18n::t!("app.star.setting"), f.stars, |f, v| f.stars = v))
                    .child(hint(i18n::t!("app.star.setting_hint")));
                (i18n::t!("app.menu.settings"), b.into_any_element(), i18n::t!("app.dialog.save"), false, |this, _, cx| this.save_settings(cx))
            }
        };
        let close = |this: &mut VigiaApp, cx: &mut Context<VigiaApp>| {
            this.dialog = None;
            cx.notify();
        };
        let ok_btn = ui::filled_button(t, "dialog-ok", if danger { t.danger } else { t.accent }).child(ok_label).on_click(cx.listener(move |this, _, window, cx| ok(this, window, cx)));
        let card = div()
            .id("dialog")
            .flex()
            .flex_col()
            .gap(px(14.))
            .w(px(540.))
            .max_h(relative(0.9))
            .overflow_y_scroll()
            .p(px(22.))
            .rounded(px(16.))
            .border_1()
            .border_color(t.border)
            .bg(t.surface)
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(title))
            .child(body)
            .child(div().flex().justify_end().gap(px(8.)).child(ui::outline_button(t, "dialog-cancel").child(i18n::t!("app.dialog.cancel")).on_click(cx.listener(move |this, _, _, cx| close(this, cx)))).children(alt.map(|(l, f)| ui::outline_button(t, "dialog-alt").child(l).on_click(cx.listener(move |this, _, window, cx| f(this, window, cx))))).child(ok_btn));
        Some(ui::backdrop(t, "dialog-backdrop").on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| close(this, cx))).child(card).into_any_element())
    }
}
