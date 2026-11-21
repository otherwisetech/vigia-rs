//! Side panel: the selected item (dates, status, details, attachments, history, actions) or
//! the edit form for a new or existing item.
use crate::app::VigiaApp;
use crate::state::Data;
use crate::theme::Tokens;
use crate::ui::{self, icon};
use crate::views::{days_text, long_date};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{IconName, Sizable as _};
use vigia_core::date::{self, Date};
use vigia_core::entity::{self, EntityKind};
use vigia_core::holidays::{BusinessDays, Country, Shift};
use vigia_core::model::{Category, Item, ItemId, Kind, Recurrence, State, Unit, leads_text, parse_leads};

pub const PANEL_W: f32 = 380.;

/// "3 hours ago", "2 days ago".
pub fn ago(secs: i64, now: i64) -> String {
    let d = (now - secs).max(0);
    match d {
        0..60 => i18n::t!("app.ago.now"),
        60..3600 => i18n::tn!("app.ago.minutes", d / 60),
        3600..86_400 => i18n::tn!("app.ago.hours", d / 3600),
        _ => i18n::tn!("app.ago.days", d / 86_400),
    }
}

pub struct EditForm {
    /// 0 for a new item.
    pub id: ItemId,
    pub kind: Kind,
    pub category: Category,
    pub recurrence: Recurrence,
    pub auto_renews: bool,
    pub auto_check: bool,
    pub business_days: Option<BusinessDays>,
    pub name: Entity<InputState>,
    pub owner: Entity<InputState>,
    /// The kind picked for a new owner; `None` until the user picks (then it's guessed).
    pub owner_kind: Option<EntityKind>,
    pub issued: Entity<InputState>,
    pub expires: Entity<InputState>,
    pub term: Entity<InputState>,
    pub notice: Entity<InputState>,
    pub cost: Entity<InputState>,
    pub currency: Entity<InputState>,
    pub provider: Entity<InputState>,
    pub reference: Entity<InputState>,
    pub url: Entity<InputState>,
    pub tags: Entity<InputState>,
    pub leads: Entity<InputState>,
    pub host: Entity<InputState>,
    pub notes: Entity<TextareaState>,
    /// The item as it was, so fields the form doesn't show are kept.
    pub base: Item,
    _subs: Vec<Subscription>,
}

fn d2s(d: Option<Date>) -> String {
    d.map(date::iso).unwrap_or_default()
}

/// Recurrence choices in the form; rule-based ones take their month / day from the dates.
fn recurrence_presets(base: Date) -> Vec<Recurrence> {
    use chrono::Datelike;
    vec![
        Recurrence::None,
        Recurrence::Every { n: 1, unit: Unit::Year },
        Recurrence::Every { n: 6, unit: Unit::Month },
        Recurrence::Every { n: 3, unit: Unit::Month },
        Recurrence::Every { n: 1, unit: Unit::Month },
        Recurrence::Every { n: 2, unit: Unit::Year },
        Recurrence::Every { n: 5, unit: Unit::Year },
        Recurrence::Every { n: 10, unit: Unit::Year },
        Recurrence::Wyoming { month: base.month() },
        Recurrence::Yearly { month: base.month(), day: base.day() },
    ]
}

impl VigiaApp {
    fn form_from(&mut self, it: Item, window: &mut Window, cx: &mut Context<Self>) -> EditForm {
        let mk = |v: String, ph: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).placeholder(ph).default_value(v));
        let date_ph = i18n::t!("app.edit.date_ph");
        let name = mk(it.name.clone(), i18n::t!("app.edit.name_ph"), window, cx);
        name.update(cx, |s, cx| s.focus(window, cx));
        let expires = mk(d2s(it.expires), date_ph.clone(), window, cx);
        let issued = mk(d2s(it.issued), date_ph, window, cx);
        let owner = mk(it.owner.clone(), i18n::t!("app.edit.owner_ph"), window, cx);
        let subs = vec![cx.observe(&expires, |_, _, cx| cx.notify()), cx.observe(&issued, |_, _, cx| cx.notify()), cx.observe(&owner, |_, _, cx| cx.notify())];
        EditForm {
            id: it.id,
            kind: it.kind,
            category: it.category,
            recurrence: it.recurrence,
            auto_renews: it.auto_renews,
            auto_check: it.auto_check,
            business_days: it.business_days,
            name,
            owner,
            owner_kind: None,
            issued,
            expires,
            term: mk(it.term_months.map(|v| v.to_string()).unwrap_or_default(), String::new(), window, cx),
            notice: mk(it.notice_days.map(|v| v.to_string()).unwrap_or_default(), String::new(), window, cx),
            cost: mk(it.cost.map(|v| format!("{v}")).unwrap_or_default(), String::new(), window, cx),
            currency: mk(it.currency.clone(), i18n::t!("app.edit.currency_ph"), window, cx),
            provider: mk(it.provider.clone(), String::new(), window, cx),
            reference: mk(it.reference.clone(), String::new(), window, cx),
            url: mk(it.renewal_url.clone(), i18n::t!("app.edit.url_ph"), window, cx),
            tags: mk(it.tags.join(", "), i18n::t!("app.edit.tags_ph"), window, cx),
            leads: mk(leads_text(&it.leads()), String::new(), window, cx),
            host: mk(it.host.clone(), i18n::t!("app.edit.host_ph"), window, cx),
            notes: cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 10).default_value(it.notes.clone())),
            base: it,
            _subs: subs,
        }
    }

    pub fn start_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_new_for(None, window, cx);
    }

    /// A new item, already belonging to `owner` (from an entity's page).
    pub fn start_new_for(&mut self, owner: Option<&vigia_core::entity::Entity>, window: &mut Window, cx: &mut Context<Self>) {
        let mut it = Item::new(Kind::Custom, "");
        if let Some(e) = owner {
            it.owner = e.name.clone();
            it.entity = Some(e.id);
            it.owner_kind = Some(e.kind);
        }
        self.entity_edit = None;
        let f = self.form_from(it, window, cx);
        self.edit = Some(f);
        cx.notify();
    }

    pub fn start_edit(&mut self, id: ItemId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(it) = self.data.read(cx).item(id).cloned() else { return };
        self.selected = Some(id);
        let f = self.form_from(it, window, cx);
        self.edit = Some(f);
        cx.notify();
    }

    /// Changing the type of a new item brings that type's defaults.
    fn set_kind(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.edit.as_mut() else { return };
        f.kind = kind;
        f.category = kind.category();
        if f.id == 0 {
            f.recurrence = kind.recurrence();
            let leads = leads_text(&kind.lead_days());
            f.leads.update(cx, |s, cx| s.set_value(leads, window, cx));
            if kind == Kind::Warranty && f.term.read(cx).value().is_empty() {
                f.term.update(cx, |s, cx| s.set_value("12", window, cx));
            }
            if kind.checkable() {
                f.auto_check = true;
            }
        }
        cx.notify();
    }

    pub fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some(f) = self.edit.as_ref() else { return };
        let today = Data::today();
        let order = Self::order();
        let val = |e: &Entity<InputState>| e.read(cx).value().trim().to_string();
        let parse_date = |e: &Entity<InputState>| -> Result<Option<Date>, String> {
            let v = val(e);
            if v.is_empty() {
                return Ok(None);
            }
            date::parse_loose(&v, today, order).map(Some).ok_or_else(|| i18n::t!("app.edit.bad_date", value = v))
        };
        let num = |e: &Entity<InputState>| -> Result<Option<u32>, String> {
            let v = val(e);
            if v.is_empty() { Ok(None) } else { v.parse().map(Some).map_err(|_| i18n::t!("app.edit.bad_number", value = v)) }
        };
        let result = (|| -> Result<Item, String> {
            let mut it = f.base.clone();
            it.kind = f.kind;
            it.category = f.category;
            it.recurrence = f.recurrence;
            it.auto_renews = f.auto_renews;
            it.auto_check = f.auto_check && f.kind.checkable();
            it.business_days = f.business_days;
            it.name = val(&f.name);
            if it.name.is_empty() {
                it.name = f.kind.label();
            }
            let owner = val(&f.owner);
            if owner.to_lowercase() != f.base.owner.trim().to_lowercase() {
                it.entity = None;
                it.owner_kind = Some(f.owner_kind.unwrap_or_else(|| entity::guess(&owner, Some(f.kind))));
            }
            it.owner = owner;
            it.issued = parse_date(&f.issued)?;
            it.expires = parse_date(&f.expires)?;
            it.term_months = num(&f.term)?;
            it.notice_days = num(&f.notice)?;
            let cost = val(&f.cost);
            it.cost = if cost.is_empty() { None } else { Some(cost.replace(',', ".").parse().map_err(|_| i18n::t!("app.edit.bad_number", value = cost))?) };
            it.currency = val(&f.currency).to_uppercase();
            it.provider = val(&f.provider);
            it.reference = val(&f.reference);
            it.renewal_url = val(&f.url);
            it.tags = val(&f.tags).split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
            it.lead_days = parse_leads(&val(&f.leads));
            it.host = val(&f.host);
            it.notes = f.notes.read(cx).value().to_string();
            Ok(it)
        })();
        let it = match result {
            Ok(it) => it,
            Err(e) => return self.toast(e, cx),
        };
        let check_now = it.auto_check && (it.id == 0 || !f.base.auto_check || it.host != f.base.host);
        match self.data.update(cx, |d, cx| d.save(it, cx)) {
            Ok(id) => {
                self.edit = None;
                self.selected = Some(id);
                if check_now {
                    self.data.update(cx, |d, cx| d.check(vec![id], cx));
                }
                cx.notify();
            }
            Err(e) => self.toast(e, cx),
        }
    }

    pub fn render_panel(&mut self, t: &Tokens, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let _ = window;
        if self.entity_edit.is_some() {
            return Some(self.render_entity_form(t, cx).into_any_element());
        }
        if self.edit.is_some() {
            return Some(self.render_form(t, cx).into_any_element());
        }
        let it = self.selected_item(cx)?;
        Some(self.render_detail(t, &it, cx).into_any_element())
    }

    fn panel_shell(t: &Tokens, id: &'static str) -> Stateful<Div> {
        div().id(id).flex().flex_col().flex_none().w(px(PANEL_W)).h_full().bg(t.surface).border_l_1().border_color(t.border).overflow_y_scroll()
    }

    fn render_detail(&self, t: &Tokens, it: &Item, cx: &mut Context<Self>) -> Stateful<Div> {
        let today = Data::today();
        let status = it.status(today);
        let color = t.status(status);
        let id = it.id;
        let d = self.data.read(cx);
        let store_files = d.store.as_ref().map(|s| (s.attachments(id).unwrap_or_default(), s.renewals(id).unwrap_or_default()));
        let checking = d.checking.contains(&id);
        let (files, history) = store_files.unwrap_or_default();
        let row = |label: String, value: String| div().flex().gap(px(10.)).py(px(3.)).child(div().w(px(118.)).flex_none().text_size(px(12.)).text_color(t.ink_muted).child(label)).child(div().flex_1().min_w_0().text_size(px(12.5)).child(value));
        let mut body = div().flex().flex_col().gap(px(2.)).px(px(18.));
        let end = it.end_date(today);
        if it.notice_days.is_some_and(|n| n > 0) {
            body = body.child(row(i18n::t!("app.field.end"), end.map(long_date).unwrap_or_default()));
            body = body.child(row(i18n::t!("app.field.notice"), i18n::tn!("app.field.notice_days", it.notice_days.unwrap_or(0))));
        }
        if let Some(i) = it.issued {
            body = body.child(row(if it.kind == Kind::Warranty { i18n::t!("app.field.purchased") } else { i18n::t!("app.field.issued") }, long_date(i)));
        }
        if let Some(m) = it.term_months.filter(|_| it.kind == Kind::Warranty) {
            body = body.child(row(i18n::t!("app.field.term"), i18n::tn!("app.field.months", m)));
        }
        body = body.child(row(i18n::t!("app.field.recurrence"), it.recurrence.label()));
        if let Some(b) = it.business_days {
            body = body.child(row(i18n::t!("app.field.business_days"), b.label()));
        }
        if let Some(c) = it.cost {
            body = body.child(row(i18n::t!("app.field.cost"), format!("{} {}", fmt_money(c), it.currency).trim().to_string()));
        }
        for (label, v) in [(i18n::t!("app.field.provider"), &it.provider), (i18n::t!("app.field.reference"), &it.reference), (i18n::t!("app.field.host"), &it.host)] {
            if !v.is_empty() {
                body = body.child(row(label, v.clone()));
            }
        }
        if !it.tags.is_empty() {
            body = body.child(row(i18n::t!("app.field.tags"), it.tags.join(", ")));
        }
        body = body.child(row(i18n::t!("app.field.reminders"), i18n::t!("app.field.reminders_value", days = leads_text(&it.leads()))));
        if it.auto_renews {
            body = body.child(row(i18n::t!("app.field.auto_renews"), i18n::t!("app.yes")));
        }
        if let Some(s) = it.snoozed_until.filter(|s| *s > today) {
            body = body.child(row(i18n::t!("app.field.snoozed"), long_date(s)));
        }
        let accent = t.accent;
        if !it.renewal_url.is_empty() {
            let url = it.renewal_url.clone();
            body = body.child(div().flex().gap(px(10.)).py(px(3.)).child(div().w(px(118.)).flex_none().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.field.url"))).child(div().id("renew-url").flex_1().min_w_0().truncate().text_size(px(12.5)).text_color(accent).cursor_pointer().hover(|s| s.underline()).child(url.clone()).on_click(move |_, _, cx| cx.open_url(&url))));
        }
        // automatic check
        let mut check = div().flex().flex_col().gap(px(6.)).px(px(18.));
        if it.kind.checkable() {
            let now = vigia_core::db::now_secs();
            let last = it.last_checked.map(|s| i18n::t!("app.check.last", when = ago(s, now))).unwrap_or_else(|| i18n::t!("app.check.never"));
            check = check
                .child(ui::section_title(t, i18n::t!("app.check.title")))
                .child(div().text_size(px(12.)).text_color(t.ink_muted).child(if it.auto_check { i18n::t!("app.check.on", target = vigia_core::checks::target(it)) } else { i18n::t!("app.check.off") }))
                .child(div().text_size(px(12.)).child(last))
                .when(!it.check_note.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(t.ink_muted).child(it.check_note.clone())))
                .child(
                    div().flex().child(
                        ui::outline_button(t, "check-now")
                            .when(checking, |d| d.opacity(0.5))
                            .child(icon(IconName::Network, 14., t.ink))
                            .child(if checking { i18n::t!("app.check.running_one") } else { i18n::t!("app.item.check_now") })
                            .when(!checking, |d| d.on_click(cx.listener(move |this, _, _, cx| this.data.update(cx, |d, cx| d.check(vec![id], cx))))),
                    ),
                );
        }
        // attachments
        let mut att = div().flex().flex_col().gap(px(4.)).px(px(18.)).child(ui::section_title(t, i18n::t!("app.attach.title")));
        let files_dir = d.files_dir();
        for (k, a) in files.iter().enumerate() {
            let path = files_dir.join(a.rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
            let (p1, p2) = (path.clone(), path.clone());
            let aid = a.id;
            let readable = vigia_core::ocr::readable(&path);
            let hover = t.canvas;
            att = att.child(
                div()
                    .id(("att", k))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(7.))
                    .hover(move |s| s.bg(hover))
                    .child(icon(IconName::File, 13., t.ink_muted))
                    .child(div().id(("att-open", k)).flex_1().min_w_0().truncate().text_size(px(12.5)).cursor_pointer().child(a.name.clone()).on_click(move |_, _, cx| cx.open_with_system(&p1)))
                    .when(readable, |d| d.child(div().id(("att-ocr", k)).text_size(px(11.5)).text_color(accent).cursor_pointer().child(i18n::t!("app.attach.read")).on_click(cx.listener(move |this, _, _, cx| this.read_document(p2.clone(), Some(id), cx)))))
                    .child(div().id(("att-rm", k)).cursor_pointer().child(icon(IconName::Close, 12., t.ink_muted)).on_click(cx.listener(move |this, _, _, cx| this.data.update(cx, |d, cx| d.detach(aid, cx))))),
            );
        }
        let border = t.border_strong;
        att = att.child(
            div()
                .id("att-drop")
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .h(px(44.))
                .rounded(px(8.))
                .border_1()
                .border_dashed()
                .border_color(t.border)
                .text_size(px(12.))
                .text_color(t.ink_muted)
                .cursor_pointer()
                .hover(move |s| s.border_color(border))
                .drag_over::<ExternalPaths>(move |s, _, _, _| s.border_color(accent).bg(accent.opacity(0.08)))
                .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                    let p = paths.paths().to_vec();
                    match this.data.update(cx, |d, cx| d.attach(id, &p, cx)) {
                        Ok(n) => this.toast(i18n::tn!("app.attach.done", n), cx),
                        Err(e) => this.toast(e, cx),
                    }
                    cx.stop_propagation();
                }))
                .child(icon(IconName::Plus, 13., t.ink_muted))
                .child(i18n::t!("app.attach.drop"))
                .on_click(cx.listener(move |_, _, window, cx| {
                    let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some(i18n::t!("app.attach.pick").into()) });
                    cx.spawn_in(window, async move |this, cx| {
                        if let Ok(Ok(Some(paths))) = rx.await {
                            this.update(cx, |this, cx| {
                                if let Err(e) = this.data.update(cx, |d, cx| d.attach(id, &paths, cx)) {
                                    this.toast(e, cx);
                                }
                            })
                            .ok();
                        }
                    })
                    .detach();
                })),
        );
        // history
        let mut hist = div().flex().flex_col().gap(px(3.)).px(px(18.));
        if !history.is_empty() {
            hist = hist.child(ui::section_title(t, i18n::t!("app.history.title")));
            for r in &history {
                let mut line = i18n::t!("app.history.row", on = long_date(r.on), from = r.old_expiry.map(long_date).unwrap_or_else(|| "—".into()), to = r.new_expiry.map(long_date).unwrap_or_default()); // i18n-ok: placeholder
                if let Some(c) = r.cost {
                    line = format!("{line} · {}", fmt_money(c)); // i18n-ok: separator
                }
                if !r.note.is_empty() {
                    line = format!("{line} · {}", r.note); // i18n-ok: separator
                }
                hist = hist.child(div().text_size(px(12.)).text_color(t.ink_muted).child(line));
            }
        }
        let weak = cx.entity().downgrade();
        let in_progress = it.state == State::InProgress;
        let archived = it.state == State::Archived;
        let more = Button::new("more").ghost().small().icon(IconName::Ellipsis).dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            let (w1, w2, w3, w4, w5, w6) = (weak.clone(), weak.clone(), weak.clone(), weak.clone(), weak.clone(), weak.clone());
            menu.item(PopupMenuItem::new(i18n::t!("app.item.snooze_day")).on_click(move |_, _, cx| {
                w1.update(cx, |this, cx| this.snooze(id, 1, cx)).ok();
            }))
            .item(PopupMenuItem::new(i18n::t!("app.item.snooze_week")).on_click(move |_, _, cx| {
                w2.update(cx, |this, cx| this.snooze(id, 7, cx)).ok();
            }))
            .item(PopupMenuItem::new(i18n::t!("app.item.snooze_month")).on_click(move |_, _, cx| {
                w3.update(cx, |this, cx| this.snooze(id, 30, cx)).ok();
            }))
            .separator()
            .item(PopupMenuItem::new(i18n::t!("app.ocr.menu")).on_click(move |_, window, cx| {
                w6.update(cx, |this, cx| this.read_document_dialog(Some(id), window, cx)).ok();
            }))
            .item(PopupMenuItem::new(if archived { i18n::t!("app.item.unarchive") } else { i18n::t!("app.item.archive") }).on_click(move |_, _, cx| {
                w4.update(cx, |this, cx| this.data.update(cx, |d, cx| d.set_state(id, if archived { State::Active } else { State::Archived }, cx))).ok();
            }))
            .separator()
            .item(PopupMenuItem::new(i18n::t!("app.item.delete")).on_click(move |_, _, cx| {
                w5.update(cx, |this, cx| this.ask_delete(id, cx)).ok();
            }))
        });
        let actions = div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .px(px(18.))
            .child(ui::filled_button(t, "renew", t.accent).child(icon(IconName::Redo, 14., t.on_fill())).child(i18n::t!("app.item.renew")).on_click(cx.listener(move |this, _, window, cx| this.open_renew(id, window, cx))))
            .child(ui::outline_button(t, "progress").when(in_progress, |d| d.border_color(t.accent).text_color(t.accent)).child(if in_progress { i18n::t!("app.item.in_progress_on") } else { i18n::t!("app.item.in_progress") }).on_click(cx.listener(move |this, _, _, cx| this.data.update(cx, |d, cx| d.set_state(id, if in_progress { State::Active } else { State::InProgress }, cx)))))
            .child(ui::outline_button(t, "edit").child(i18n::t!("app.item.edit")).on_click(cx.listener(move |this, _, window, cx| this.start_edit(id, window, cx))));
        let due = it.due_date(today);
        let head = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(18.))
            .pt(px(16.))
            .child(div().flex().items_start().gap(px(8.)).child(div().flex_1().min_w_0().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(it.name.clone())).child(more).child(ui::icon_button(t, "panel-close", IconName::Close, true).size(px(26.)).on_click(cx.listener(|this, _, _, cx| {
                this.selected = None;
                cx.notify();
            }))))
            .when(!it.owner.is_empty(), |d| {
                let ent = it.entity;
                let accent = t.accent;
                d.child(
                    div()
                        .id("detail-owner")
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .text_color(t.ink_muted)
                        .child(ui::entity_icon(it.owner_kind.unwrap_or_default(), 13., t.ink_muted))
                        .child(it.owner.clone())
                        .child(div().text_size(px(11.5)).child(format!("· {}", it.owner_kind.unwrap_or_default().label()))) // i18n-ok: separator
                        .when_some(ent, |d, id| d.cursor_pointer().hover(move |s| s.text_color(accent).underline()).on_click(cx.listener(move |this, _, _, cx| this.open_entity(id, cx)))),
                )
            })
            .child(div().flex().flex_wrap().gap(px(6.)).child(ui::chip(it.kind.label(), t.accent, 11.5)).child(ui::chip(it.category.label(), t.ink_muted, 11.5)).when(in_progress, |d| d.child(ui::chip(i18n::t!("app.state.in_progress"), t.accent, 11.5))).when(archived, |d| d.child(ui::chip(i18n::t!("app.state.archived"), t.ink_muted, 11.5))))
            .child(
                div()
                    .mt(px(6.))
                    .p(px(12.))
                    .rounded(px(12.))
                    .bg(color.opacity(0.1))
                    .border_1()
                    .border_color(color.opacity(0.35))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(11.5)).text_color(t.ink_muted).child(if it.notice_days.is_some_and(|n| n > 0) { i18n::t!("app.field.notice_by") } else { i18n::t!("app.field.expires") }))
                    .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(due.map(long_date).unwrap_or_else(|| i18n::t!("app.days.none"))))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(color).child(format!("{} · {}", days_text(it.days_left(today)), it.status(today).label()))), // i18n-ok: separator
            );
        let notes = (!it.notes.is_empty()).then(|| div().px(px(18.)).flex().flex_col().gap(px(4.)).child(ui::section_title(t, i18n::t!("app.field.notes"))).child(div().text_size(px(12.5)).line_height(px(18.)).child(it.notes.clone())));
        Self::panel_shell(t, "panel").gap(px(14.)).pb(px(20.)).child(head).child(actions).child(body).children(notes).child(check).child(att).child(hist)
    }

    fn render_form(&self, t: &Tokens, cx: &mut Context<Self>) -> Stateful<Div> {
        let f = self.edit.as_ref().expect("form");
        let today = Data::today();
        let label = |s: String| div().text_size(px(11.5)).font_weight(FontWeight::MEDIUM).text_color(t.ink_muted).child(s);
        let field = |s: String, e: &Entity<InputState>| div().flex().flex_col().gap(px(3.)).child(label(s)).child(Input::new(e).small());
        let half = |a: Div, b: Div| div().flex().gap(px(8.)).child(a.flex_1().min_w_0()).child(b.flex_1().min_w_0());
        let order = Self::order();
        let preview = |e: &Entity<InputState>| -> Option<(String, bool)> {
            let v = e.read(cx).value().trim().to_string();
            if v.is_empty() {
                return None;
            }
            Some(match date::parse_loose(&v, today, order) {
                Some(d) => (long_date(d), true),
                None => (i18n::t!("app.edit.bad_date", value = v), false),
            })
        };
        let date_field = |s: String, e: &Entity<InputState>| {
            let p = preview(e);
            div().flex().flex_col().gap(px(3.)).child(label(s)).child(Input::new(e).small()).children(p.map(|(txt, ok)| div().text_size(px(11.)).text_color(if ok { t.ink_muted } else { t.danger }).child(txt)))
        };
        let weak = cx.entity().downgrade();
        let cur = f.kind;
        let kind_btn = Button::new("form-kind").outline().small().w_full().label(cur.label()).dropdown_menu(move |mut menu, _, _| {
            for c in Category::ALL {
                let kinds: Vec<Kind> = Kind::ALL.iter().copied().filter(|k| k.category() == c).collect();
                if kinds.is_empty() {
                    continue;
                }
                menu = menu.label(c.label());
                for k in kinds {
                    let w = weak.clone();
                    menu = menu.item(PopupMenuItem::new(k.label()).checked(k == cur).on_click(move |_, window, cx| {
                        w.update(cx, |this, cx| this.set_kind(k, window, cx)).ok();
                    }));
                }
            }
            menu
        });
        let weak = cx.entity().downgrade();
        let cur_cat = f.category;
        let cat_btn = Button::new("form-cat").outline().small().w_full().label(cur_cat.label()).dropdown_menu(move |mut menu, _, _| {
            for c in Category::ALL {
                let w = weak.clone();
                menu = menu.item(PopupMenuItem::new(c.label()).checked(c == cur_cat).on_click(move |_, _, cx| {
                    w.update(cx, |this, cx| {
                        if let Some(f) = this.edit.as_mut() {
                            f.category = c;
                        }
                        cx.notify();
                    })
                    .ok();
                }));
            }
            menu
        });
        let base_date = date::parse_loose(&f.expires.read(cx).value(), today, order).or_else(|| date::parse_loose(&f.issued.read(cx).value(), today, order)).unwrap_or(today);
        let weak = cx.entity().downgrade();
        let cur_rec = f.recurrence;
        let rec_btn = Button::new("form-rec").outline().small().w_full().label(cur_rec.label()).dropdown_menu(move |mut menu, _, _| {
            for r in recurrence_presets(base_date) {
                let w = weak.clone();
                menu = menu.item(PopupMenuItem::new(r.label()).checked(r == cur_rec).on_click(move |_, _, cx| {
                    w.update(cx, |this, cx| {
                        if let Some(f) = this.edit.as_mut() {
                            f.recurrence = r;
                        }
                        cx.notify();
                    })
                    .ok();
                }));
            }
            menu
        });
        let typed = f.owner.read(cx).value().trim().to_string();
        let owner_hint: Option<AnyElement> = if typed.is_empty() {
            None
        } else {
            let ents = &self.data.read(cx).entities;
            match entity::find(ents, &typed) {
                Some(e) => Some(div().flex().items_center().gap(px(5.)).text_size(px(11.5)).text_color(t.ink_muted).child(ui::entity_icon(e.kind, 12., t.ink_muted)).child(i18n::t!("app.edit.owner_existing", name = e.name, kind = e.kind.label())).into_any_element()),
                None => {
                    let cur = f.owner_kind.unwrap_or_else(|| entity::guess(&typed, Some(f.kind)));
                    let weak = cx.entity().downgrade();
                    let picker = Button::new("form-owner-kind").outline().xsmall().label(cur.label()).dropdown_menu(move |mut menu, _, _| {
                        for k in EntityKind::ALL {
                            let w = weak.clone();
                            menu = menu.item(PopupMenuItem::new(k.label()).checked(k == cur).on_click(move |_, _, cx| {
                                w.update(cx, |this, cx| {
                                    if let Some(f) = this.edit.as_mut() {
                                        f.owner_kind = Some(k);
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }));
                        }
                        menu
                    });
                    Some(div().flex().items_center().gap(px(6.)).text_size(px(11.5)).text_color(t.ink_muted).child(ui::entity_icon(cur, 12., t.ink_muted)).child(i18n::t!("app.edit.owner_new")).child(picker).into_any_element())
                }
            }
        };
        let kind = f.kind;
        let mut form = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(18.))
            .pt(px(16.))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(if f.id == 0 { i18n::t!("app.item.new") } else { i18n::t!("app.item.edit") }))
            .child(half(div().flex().flex_col().gap(px(3.)).child(label(i18n::t!("app.field.kind"))).child(kind_btn), div().flex().flex_col().gap(px(3.)).child(label(i18n::t!("app.field.category"))).child(cat_btn)))
            .child(field(i18n::t!("app.field.name"), &f.name))
            .child(field(i18n::t!("app.field.owner"), &f.owner))
            .children(owner_hint)
            .child(half(date_field(if kind == Kind::Warranty { i18n::t!("app.field.purchased") } else { i18n::t!("app.field.issued") }, &f.issued), date_field(i18n::t!("app.field.expires"), &f.expires)));
        if kind == Kind::Warranty {
            form = form.child(field(i18n::t!("app.field.term_months"), &f.term)).child(div().text_size(px(11.)).text_color(t.ink_muted).child(i18n::t!("app.edit.warranty_hint")));
        }
        if kind == Kind::Contract || f.base.notice_days.is_some() {
            form = form.child(field(i18n::t!("app.field.notice_days_label"), &f.notice));
        }
        form = form.child(div().flex().flex_col().gap(px(3.)).child(label(i18n::t!("app.field.recurrence"))).child(rec_btn));
        let cur_b = f.business_days;
        let weak = cx.entity().downgrade();
        let set_b = move |w: &WeakEntity<VigiaApp>, b: Option<BusinessDays>, cx: &mut App| {
            w.update(cx, |this, cx| {
                if let Some(f) = this.edit.as_mut() {
                    f.business_days = b;
                }
                cx.notify();
            })
            .ok();
        };
        let shift_label = match cur_b.map(|b| b.shift) {
            None => i18n::t!("app.edit.biz_none"),
            Some(Shift::Previous) => i18n::t!("app.edit.biz_prev"),
            Some(Shift::Next) => i18n::t!("app.edit.biz_next"),
        };
        let w0 = weak.clone();
        let shift_btn = Button::new("form-biz").outline().small().w_full().label(shift_label).dropdown_menu(move |mut menu, _, _| {
            let default_country = cur_b.and_then(|b| b.country).or(Some(Country::Co));
            for (lbl, v) in [(i18n::t!("app.edit.biz_none"), None), (i18n::t!("app.edit.biz_prev"), Some(Shift::Previous)), (i18n::t!("app.edit.biz_next"), Some(Shift::Next))] {
                let w = w0.clone();
                menu = menu.item(PopupMenuItem::new(lbl).checked(cur_b.map(|b| b.shift) == v).on_click(move |_, _, cx| set_b(&w, v.map(|shift| BusinessDays { shift, country: default_country }), cx)));
            }
            menu
        });
        let country_btn = cur_b.map(|b| {
            let w0 = weak.clone();
            Button::new("form-biz-c").outline().small().w_full().label(b.country.map(|c| c.label()).unwrap_or_else(|| i18n::t!("app.edit.biz_weekends"))).dropdown_menu(move |mut menu, _, _| {
                let w = w0.clone();
                menu = menu.item(PopupMenuItem::new(i18n::t!("app.edit.biz_weekends")).checked(b.country.is_none()).on_click(move |_, _, cx| set_b(&w, Some(BusinessDays { country: None, ..b }), cx)));
                for c in Country::ALL {
                    let w = w0.clone();
                    menu = menu.item(PopupMenuItem::new(c.label()).checked(b.country == Some(c)).on_click(move |_, _, cx| set_b(&w, Some(BusinessDays { country: Some(c), ..b }), cx)));
                }
                menu
            })
        });
        form = form.child(div().flex().flex_col().gap(px(3.)).child(label(i18n::t!("app.field.business_days"))).child(div().flex().gap(px(8.)).child(div().flex_1().min_w_0().child(shift_btn)).children(country_btn.map(|b| div().flex_1().min_w_0().child(b)))));
        if matches!(kind, Kind::WyomingReport | Kind::CamaraComercio) {
            form = form.child(div().text_size(px(11.)).text_color(t.ink_muted).child(if kind == Kind::WyomingReport { i18n::t!("app.edit.wyoming_hint") } else { i18n::t!("app.edit.camara_hint") }));
        }
        if kind.checkable() {
            form = form.child(field(if kind == Kind::Domain { i18n::t!("app.field.domain") } else { i18n::t!("app.field.host") }, &f.host)).child(Checkbox::new("form-check").small().label(i18n::t!("app.edit.auto_check")).checked(f.auto_check).on_click(cx.listener(|this, v: &bool, _, cx| {
                if let Some(f) = this.edit.as_mut() {
                    f.auto_check = *v;
                }
                cx.notify();
            }))).child(div().text_size(px(11.)).text_color(t.ink_muted).child(if kind == Kind::Domain { i18n::t!("app.edit.rdap_hint") } else { i18n::t!("app.edit.tls_hint") }));
        }
        form = form
            .child(field(i18n::t!("app.field.reminders_label"), &f.leads))
            .child(half(field(i18n::t!("app.field.cost"), &f.cost), field(i18n::t!("app.field.currency"), &f.currency)))
            .child(field(i18n::t!("app.field.provider"), &f.provider))
            .child(field(i18n::t!("app.field.reference"), &f.reference))
            .child(field(i18n::t!("app.field.url"), &f.url))
            .child(field(i18n::t!("app.field.tags"), &f.tags))
            .child(Checkbox::new("form-auto").small().label(i18n::t!("app.field.auto_renews")).checked(f.auto_renews).on_click(cx.listener(|this, v: &bool, _, cx| {
                if let Some(f) = this.edit.as_mut() {
                    f.auto_renews = *v;
                }
                cx.notify();
            })))
            .child(div().flex().flex_col().gap(px(3.)).child(label(i18n::t!("app.field.notes"))).child(div().rounded(px(6.)).border_1().border_color(t.border).bg(t.canvas).px(px(6.)).py(px(4.)).text_size(px(12.5)).child(Textarea::new(&f.notes))))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .pt(px(4.))
                    .child(ui::outline_button(t, "form-cancel").child(i18n::t!("app.dialog.cancel")).on_click(cx.listener(|this, _, _, cx| {
                        this.edit = None;
                        cx.notify();
                    })))
                    .child(ui::filled_button(t, "form-save", t.accent).child(i18n::t!("app.dialog.save")).on_click(cx.listener(|this, _, _, cx| this.save_edit(cx)))),
            );
        Self::panel_shell(t, "form-panel").pb(px(20.)).child(form)
    }
}

/// `1234567.5` → `1,234,567.50` (whole numbers without decimals).
pub fn fmt_money(v: f64) -> String {
    let neg = v < 0.;
    let v = v.abs();
    let whole = v.trunc() as u64;
    let cents = ((v - v.trunc()) * 100.).round() as u64;
    let digits = whole.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if cents > 0 {
        out = format!("{out}.{cents:02}");
    }
    if neg { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    #[test]
    fn money() {
        assert_eq!(super::fmt_money(892000.0), "892,000");
        assert_eq!(super::fmt_money(12.5), "12.50");
        assert_eq!(super::fmt_money(999.0), "999");
        assert_eq!(super::fmt_money(-1234.0), "-1,234");
    }
}
