//! The four views: dashboard, timeline (next 12 months), calendar month and filtered list.
use crate::app::VigiaApp;
use crate::state::Data;
use crate::theme::Tokens;
use crate::ui::{self, icon};
use chrono::Datelike;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{IconName, Sizable as _};
use vigia_core::date::{self, Date};
use vigia_core::model::{Category, Item, State, Status};
use vigia_core::settings::Settings;

/// `12 de mayo de 2031` / `May 12, 2031`.
pub fn long_date(d: Date) -> String {
    i18n::t!("app.date.long", day = d.day(), month = vigia_core::model::month_name(d.month()), year = d.year())
}

/// "24 December": a yearly day without its year.
pub fn long_day_month(month: u32, day: u32) -> String {
    i18n::t!("app.date.day_month", day = day, month = vigia_core::model::month_name(month))
}

pub fn month_title(d: Date) -> String {
    i18n::t!("app.date.month", month = vigia_core::model::month_name(d.month()), year = d.year())
}

/// "in 12 days", "today", "3 days ago", "no date".
pub fn days_text(days: Option<i64>) -> String {
    match days {
        None => i18n::t!("app.days.none"),
        Some(0) => i18n::t!("app.days.today"),
        Some(1) => i18n::t!("app.days.tomorrow"),
        Some(n) if n < 0 => i18n::tn!("app.days.ago", -n),
        Some(n) => i18n::tn!("app.days.in", n),
    }
}

fn weekday(i: usize) -> String {
    match i {
        0 => i18n::t!("app.wd.mon"),
        1 => i18n::t!("app.wd.tue"),
        2 => i18n::t!("app.wd.wed"),
        3 => i18n::t!("app.wd.thu"),
        4 => i18n::t!("app.wd.fri"),
        5 => i18n::t!("app.wd.sat"),
        _ => i18n::t!("app.wd.sun"),
    }
}

impl VigiaApp {
    /// Holiday names (true) and other calendars' events (false) per day, `from`..=`to`.
    pub fn day_marks(&self, from: Date, to: Date, cx: &App) -> std::collections::BTreeMap<Date, Vec<(String, bool)>> {
        let mut m: std::collections::BTreeMap<Date, Vec<(String, bool)>> = std::collections::BTreeMap::new();
        let many = self.holidays.len() > 1;
        for h in vigia_core::holidays::between(&self.holidays, from, to) {
            let name = if many { i18n::t!("app.cal.holiday_in", name = h.name, country = h.country.id().to_uppercase()) } else { h.name.to_string() };
            m.entry(h.date).or_default().push((name, true));
        }
        for o in vigia_core::calendars::overlay(&self.data.read(cx).calendars, from, to) {
            m.entry(o.date).or_default().push((o.summary, false));
        }
        m
    }

    /// Holidays per country, other calendars, import.
    pub fn calendars_button(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let _ = t;
        let on = self.holidays.clone();
        let cals: Vec<(i64, String, bool, bool)> = self.data.read(cx).calendars.iter().map(|c| (c.id, c.name.clone(), c.enabled, c.is_url())).collect();
        let weak = cx.entity().downgrade();
        Button::new("calendars").outline().small().icon(IconName::Calendar).label(i18n::t!("app.cal.calendars")).dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
            menu = menu.label(i18n::t!("app.cal.holidays"));
            for c in vigia_core::holidays::Country::ALL {
                let w = weak.clone();
                let checked = on.contains(&c);
                menu = menu.item(PopupMenuItem::new(c.label()).checked(checked).on_click(move |_, _, cx| {
                    w.update(cx, |this, cx| this.toggle_holidays(c, cx)).ok();
                }));
            }
            if !cals.is_empty() {
                menu = menu.separator().label(i18n::t!("app.cal.others"));
                for (id, name, enabled, _) in &cals {
                    let (w, id, enabled) = (weak.clone(), *id, *enabled);
                    menu = menu.item(PopupMenuItem::new(name.clone()).checked(enabled).on_click(move |_, _, cx| {
                        w.update(cx, |this, cx| this.data.update(cx, |d, cx| d.set_calendar_enabled(id, !enabled, cx))).ok();
                    }));
                }
                let w = weak.clone();
                menu = menu.item(PopupMenuItem::new(i18n::t!("app.cal.refresh")).on_click(move |_, _, cx| {
                    w.update(cx, |this, cx| this.refresh_calendars(cx)).ok();
                }));
                for (id, name, _, _) in &cals {
                    let (w, id) = (weak.clone(), *id);
                    menu = menu.item(PopupMenuItem::new(i18n::t!("app.cal.remove", name = name.clone())).on_click(move |_, _, cx| {
                        w.update(cx, |this, cx| this.data.update(cx, |d, cx| d.remove_calendar(id, cx))).ok();
                    }));
                }
            }
            let (w1, w2) = (weak.clone(), weak.clone());
            menu.separator()
                .item(PopupMenuItem::new(i18n::t!("app.cal.import_file")).on_click(move |_, window, cx| {
                    w1.update(cx, |this, cx| this.import_calendar_file(window, cx)).ok();
                }))
                .item(PopupMenuItem::new(i18n::t!("app.cal.import_url")).on_click(move |_, window, cx| {
                    w2.update(cx, |this, cx| this.ask_calendar_url(window, cx)).ok();
                }))
        })
        .into_any_element()
    }

    fn toggle_holidays(&mut self, c: vigia_core::holidays::Country, cx: &mut Context<Self>) {
        if let Some(i) = self.holidays.iter().position(|x| *x == c) {
            self.holidays.remove(i);
        } else {
            self.holidays.push(c);
            self.holidays.sort();
        }
        let ids: Vec<String> = self.holidays.iter().map(|c| c.id().to_string()).collect();
        Settings::update(|s| s.holidays = (!ids.is_empty()).then_some(ids));
        cx.notify();
    }

    fn import_calendar_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(i18n::t!("app.cal.import_file").into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(p) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| match this.data.update(cx, |d, cx| d.add_calendar_file(&p, cx)) {
                    Ok(name) => this.toast(i18n::t!("app.cal.added", name = name), cx),
                    Err(e) => this.toast(e, cx),
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn add_calendar_url(&mut self, url: String, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        self.toast(i18n::t!("app.cal.downloading"), cx);
        self.data.update(cx, |d, cx| {
            d.add_calendar_url(url, None, cx, move |r, cx| {
                weak.update(cx, |this, cx| match r {
                    Ok(name) => this.toast(i18n::t!("app.cal.added", name = name), cx),
                    Err(e) => this.toast(e, cx),
                })
                .ok();
            })
        });
    }

    fn refresh_calendars(&mut self, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        self.data.update(cx, |d, cx| {
            d.refresh_calendars(cx, move |r, cx| {
                if let Err(e) = r {
                    weak.update(cx, |this, cx| this.toast(e, cx)).ok();
                }
            })
        });
        self.toast(i18n::t!("app.cal.refreshed"), cx);
    }

    /// One item as a row: urgency bar, name and owner, type, due date and time left.
    pub fn item_row(&self, t: &Tokens, it: &Item, today: Date, key: impl Into<ElementId>, cx: &mut Context<Self>) -> Stateful<Div> {
        let status = it.status(today);
        let color = t.status(status);
        let selected = self.selected == Some(it.id);
        let hover = t.canvas;
        let id = it.id;
        let due = it.due_date(today);
        let attachments = self.data.read(cx).attach_counts.get(&id).copied().unwrap_or(0);
        let mut badges = div().flex().items_center().gap(px(6.));
        if it.state == State::InProgress {
            badges = badges.child(ui::chip(i18n::t!("app.state.in_progress"), t.accent, 11.));
        }
        if it.state == State::Archived {
            badges = badges.child(ui::chip(i18n::t!("app.state.archived"), t.ink_muted, 11.));
        }
        if it.snoozed_until.is_some_and(|s| s > today) {
            badges = badges.child(icon(IconName::Bell, 12., t.ink_muted));
        }
        if it.auto_renews {
            badges = badges.child(icon(IconName::Redo, 12., t.ink_muted));
        }
        if attachments > 0 {
            badges = badges.child(icon(IconName::File, 12., t.ink_muted));
        }
        let d = self.data.read(cx);
        // seen from an owner's list: the item may be theirs through a company they own
        if let Some(e) = self.filter.entity
            && let Some(Some(via)) = vigia_core::view::via(it, e, &self.filter.through)
            && let Some(x) = d.entity(via)
        {
            badges = badges.child(ui::chip(i18n::t!("app.list.via", name = x.name), t.ink_muted, 11.));
        }
        let owners = {
            let names: Vec<String> = it.owner_ids().into_iter().filter_map(|o| d.entity(o).map(|e| e.name.clone())).collect();
            if names.is_empty() { it.owner.clone() } else { names.join(", ") }
        };
        let sub = due.and_then(|d| it.celebration_text(d)).unwrap_or_else(|| it.kind.label());
        div()
            .id(key)
            .flex()
            .items_center()
            .gap(px(12.))
            .min_h(px(46.))
            .pr(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(if selected { t.accent } else { t.border })
            .bg(t.surface)
            .cursor_pointer()
            .overflow_hidden()
            .hover(move |s| s.bg(hover))
            .child(div().w(px(4.)).h(px(46.)).bg(color))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .py(px(6.))
                    .child(div().flex().items_center().gap(px(6.)).min_w_0().child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).truncate().child(it.name.clone())).when(!owners.is_empty(), |d| d.child(div().flex().items_center().gap(px(4.)).min_w_0().text_size(px(12.5)).text_color(t.ink_muted).child(ui::entity_icon(it.owner_kind.unwrap_or_default(), 11., t.ink_muted)).child(div().truncate().child(owners)))))
                    .child(div().flex().items_center().gap(px(5.)).text_size(px(11.5)).text_color(t.ink_muted).child(crate::icons::kind_icon(it.kind, 11., t.ink_muted)).child(div().truncate().child(sub))),
            )
            .child(badges)
            .child(div().flex().flex_col().items_end().flex_none().child(div().text_size(px(12.5)).child(due.map(long_date).unwrap_or_else(|| i18n::t!("app.days.none")))).child(div().text_size(px(11.5)).font_weight(FontWeight::MEDIUM).text_color(color).child(days_text(it.days_left(today)))))
            .on_click(cx.listener(move |this, _, _, cx| this.select(id, cx)))
    }

    fn scroll_col(id: &'static str) -> Stateful<Div> {
        div().id(id).flex().flex_col().size_full().overflow_y_scroll().px(px(28.)).py(px(22.)).gap(px(10.))
    }

    fn heading(t: &Tokens, title: String, sub: Option<String>) -> Div {
        div().flex().items_baseline().gap(px(10.)).child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child(title)).children(sub.map(|s| div().text_size(px(13.)).text_color(t.ink_muted).child(s)))
    }

    fn render_empty(&self, t: &Tokens, cx: &mut Context<Self>) -> Stateful<Div> {
        let example = |s: String| div().px(px(10.)).py(px(5.)).rounded(px(8.)).bg(t.surface).border_1().border_color(t.border).text_size(px(12.5)).child(s);
        div()
            .id("empty")
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(18.))
            .size_full()
            .overflow_y_scroll()
            .py(px(30.))
            .child(img(self.logo.clone()).size(px(104.)))
            .child(div().flex().flex_col().items_center().gap(px(6.)).child(div().text_size(px(24.)).font_weight(FontWeight::SEMIBOLD).child(i18n::t!("app.welcome.title"))).child(div().max_w(px(560.)).text_center().text_size(px(14.)).line_height(px(20.)).text_color(t.ink_muted).child(i18n::t!("app.welcome.subtitle"))))
            .child(div().flex().flex_col().items_center().gap(px(8.)).child(ui::section_title(t, i18n::t!("app.welcome.try"))).child(div().flex().flex_wrap().justify_center().max_w(px(620.)).gap(px(8.)).child(example(i18n::t!("app.welcome.ex1"))).child(example(i18n::t!("app.welcome.ex2"))).child(example(i18n::t!("app.welcome.ex3"))).child(example(i18n::t!("app.welcome.ex4")))))
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(ui::filled_button(t, "first-item", t.accent).child(icon(IconName::Plus, 15., t.on_fill())).child(i18n::t!("app.welcome.first")).on_click(cx.listener(|this, _, window, cx| this.new_item(window, cx))))
                    .child(ui::outline_button(t, "first-ocr").child(icon(IconName::File, 15., t.ink)).child(i18n::t!("app.ocr.button")).on_click(cx.listener(|this, _, window, cx| this.read_document_dialog(None, window, cx))))
                    .child(ui::outline_button(t, "first-csv").child(icon(IconName::FolderOpen, 15., t.ink)).child(i18n::t!("app.menu.import_csv")).on_click(cx.listener(|_, _, window, cx| window.dispatch_action(Box::new(crate::app::ImportCsv), cx)))),
            )
            .child(div().max_w(px(540.)).text_center().text_size(px(11.5)).line_height(px(16.)).text_color(t.ink_muted).child(i18n::t!("app.welcome.privacy")))
    }

    pub fn render_dashboard(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let today = Data::today();
        let items = self.data.read(cx).items.clone();
        if items.is_empty() {
            return self.render_empty(t, cx).into_any_element();
        }
        let c = vigia_core::view::counts(&items, today);
        let stat = |id: &'static str, n: usize, label: String, color: Hsla, s: Option<Status>| {
            let hover = t.border_strong;
            div()
                .id(id)
                .flex()
                .flex_col()
                .gap(px(4.))
                .flex_1()
                .min_w(px(120.))
                .p(px(14.))
                .rounded(px(12.))
                .border_1()
                .border_color(t.border)
                .bg(t.surface)
                .cursor_pointer()
                .hover(move |st| st.border_color(hover))
                .child(div().text_size(px(28.)).font_weight(FontWeight::SEMIBOLD).text_color(if n == 0 { t.ink_muted } else { color }).child(n.to_string()))
                .child(div().flex().items_center().gap(px(6.)).child(div().size(px(8.)).rounded_full().bg(color)).child(div().text_size(px(12.5)).text_color(t.ink_muted).child(label)))
                .on_click(cx.listener(move |this, _, _, cx| this.status_filter(s, cx)))
        };
        let next = vigia_core::view::next(&items, today, 5);
        let mut col = Self::scroll_col("dash")
            .child(Self::heading(t, i18n::t!("app.view.dashboard"), Some(long_date(today))))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(stat("c-exp", c.expired, Status::Expired.label(), t.danger, Some(Status::Expired)))
                    .child(stat("c-soon", c.soon, Status::Soon.label(), t.warn, Some(Status::Soon)))
                    .child(stat("c-up", c.upcoming, Status::Upcoming.label(), t.tag, Some(Status::Upcoming)))
                    .child(stat("c-ok", c.ok, Status::Ok.label(), t.ok, Some(Status::Ok)))
                    .child(stat("c-prog", c.in_progress, i18n::t!("app.state.in_progress"), t.accent, None)),
            )
            .child(div().pt(px(10.)).child(ui::section_title(t, i18n::t!("app.dash.next"))));
        for (k, it) in next.iter().enumerate() {
            col = col.child(self.item_row(t, it, today, ("dash-next", k), cx));
        }
        let soon = vigia_core::reminder::upcoming(&items, today, date::add_days(today, 30));
        if !soon.is_empty() {
            col = col.child(div().pt(px(12.)).child(ui::section_title(t, i18n::t!("app.dash.reminders"))));
            for (k, r) in soon.iter().take(8).enumerate() {
                let Some(it) = items.iter().find(|i| i.id == r.item) else { continue };
                let id = it.id;
                let when = if r.fires_on() == today { i18n::t!("app.days.today") } else { long_date(r.fires_on()) };
                let what = if r.lead == 0 { i18n::t!("app.dash.on_deadline") } else { i18n::tn!("app.dash.lead", r.lead) };
                let hover = t.canvas;
                col = col.child(
                    div()
                        .id(("rem", k))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(icon(IconName::Bell, 13., t.ink_muted))
                        .child(div().w(px(150.)).flex_none().text_size(px(12.5)).text_color(t.ink_muted).child(when))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(13.)).child(it.title()))
                        .child(div().text_size(px(12.)).text_color(t.ink_muted).child(what))
                        .on_click(cx.listener(move |this, _, _, cx| this.select(id, cx))),
                );
            }
        }
        let recent = self.data.read(cx).recent.clone();
        if !recent.is_empty() {
            col = col.child(div().pt(px(12.)).child(ui::section_title(t, i18n::t!("app.dash.sent"))));
            for (k, (id, title, body)) in recent.into_iter().take(6).enumerate() {
                let hover = t.canvas;
                col = col.child(div().id(("sent", k)).flex().gap(px(10.)).px(px(10.)).py(px(5.)).rounded(px(8.)).cursor_pointer().hover(move |s| s.bg(hover)).child(icon(IconName::CircleCheck, 13., t.ok)).child(div().text_size(px(13.)).child(title)).child(div().text_size(px(12.5)).text_color(t.ink_muted).child(body)).on_click(cx.listener(move |this, _, _, cx| this.select(id, cx))));
            }
        }
        let checked: Vec<&Item> = items.iter().filter(|i| i.auto_check && i.kind.checkable()).collect();
        if !checked.is_empty() {
            col = col.child(div().pt(px(12.)).child(ui::section_title(t, i18n::t!("app.dash.checks"))));
            let now = vigia_core::db::now_secs();
            for (k, it) in checked.into_iter().enumerate() {
                let id = it.id;
                let when = it.last_checked.map(|s| crate::panel::ago(s, now)).unwrap_or_else(|| i18n::t!("app.check.never"));
                let stale = vigia_core::checks::stale(it, now);
                let hover = t.canvas;
                col = col.child(
                    div()
                        .id(("chk", k))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(5.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(icon(IconName::Network, 13., if stale { t.warn } else { t.ink_muted }))
                        .child(div().w(px(220.)).flex_none().truncate().text_size(px(13.)).child(vigia_core::checks::target(it)))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(t.ink_muted).child(it.check_note.clone()))
                        .child(div().text_size(px(12.)).text_color(if stale { t.warn } else { t.ink_muted }).child(when))
                        .on_click(cx.listener(move |this, _, _, cx| this.select(id, cx))),
                );
            }
        }
        col.into_any_element()
    }

    pub fn render_timeline(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let today = Data::today();
        let items = self.data.read(cx).items.clone();
        if items.is_empty() {
            return self.render_empty(t, cx).into_any_element();
        }
        let refs: Vec<&Item> = items.iter().filter(|i| i.state != State::Archived).collect();
        let (overdue, months) = vigia_core::view::timeline(&refs, today, 12);
        let marks = self.day_marks(today, date::add_months(date::month_start(today), 12), cx);
        let mut col = Self::scroll_col("timeline").child(div().flex().items_center().child(div().flex_1().child(Self::heading(t, i18n::t!("app.view.timeline"), Some(i18n::t!("app.timeline.sub"))))).child(self.calendars_button(t, cx)));
        let day_badge = |d: Date, color: Hsla| div().flex().flex_col().items_center().justify_center().w(px(46.)).flex_none().child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(color).child(d.day().to_string())).child(div().text_size(px(10.5)).text_color(t.ink_muted).child(weekday(d.weekday().num_days_from_monday() as usize)));
        if !overdue.is_empty() {
            col = col.child(div().flex().items_center().gap(px(8.)).pt(px(8.)).child(icon(IconName::TriangleAlert, 14., t.danger)).child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(t.danger).child(i18n::tn!("app.timeline.overdue", overdue.len()))));
            for (k, it) in overdue.iter().enumerate() {
                let d = it.due_date(today).unwrap_or(today);
                col = col.child(div().flex().items_center().gap(px(8.)).child(day_badge(d, t.danger)).child(div().flex_1().min_w_0().child(self.item_row(t, it, today, ("over", k), cx))));
            }
        }
        for (m, month) in months.iter().enumerate() {
            col = col.child(div().flex().items_baseline().gap(px(8.)).pt(px(12.)).pb(px(2.)).border_b_1().border_color(t.border).child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(month_title(month.start))).child(div().text_size(px(12.)).text_color(t.ink_muted).child(if month.items.is_empty() { i18n::t!("app.timeline.nothing") } else { i18n::tn!("app.items.count", month.items.len()) })));
            let end = date::add_months(month.start, 1);
            let in_month: Vec<String> = marks.iter().filter(|(d, _)| **d >= month.start && **d < end).flat_map(|(d, v)| v.iter().map(move |(name, _)| i18n::t!("app.timeline.day_mark", day = d.day(), name = name.clone()))).collect();
            if !in_month.is_empty() {
                col = col.child(div().flex().flex_wrap().gap_x(px(12.)).gap_y(px(2.)).text_size(px(11.5)).text_color(t.ink_muted).children(in_month.into_iter().map(|m| div().child(m))));
            }
            for (k, it) in month.items.iter().enumerate() {
                let d = it.due_date(today).unwrap_or(today);
                let color = t.status(it.status(today));
                col = col.child(div().flex().items_center().gap(px(8.)).child(day_badge(d, color)).child(div().flex_1().min_w_0().child(self.item_row(t, it, today, ("tl", m * 1000 + k), cx))));
            }
        }
        let undated = refs.iter().filter(|i| i.due_date(today).is_none()).count();
        if undated > 0 {
            col = col.child(div().pt(px(14.)).text_size(px(12.)).text_color(t.ink_muted).child(i18n::tn!("app.timeline.undated", undated)));
        }
        col.into_any_element()
    }

    pub fn render_calendar(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let today = Data::today();
        let items = self.data.read(cx).items.clone();
        let refs: Vec<&Item> = items.iter().filter(|i| i.state != State::Archived).collect();
        let grid = vigia_core::view::month_grid(&refs, self.cal_month, today);
        let marks = self.day_marks(grid[0].0, grid[41].0, cx);
        let cal_btn = self.calendars_button(t, cx);
        let month = self.cal_month;
        let nav = |id: &'static str, ic: IconName, delta: i32| {
            ui::icon_button(t, id, ic, true).on_click(cx.listener(move |this, _, _, cx| {
                this.cal_month = if delta == 0 { date::month_start(Data::today()) } else { date::add_months(this.cal_month, delta) };
                cx.notify();
            }))
        };
        let header = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().flex_1().child(Self::heading(t, month_title(month), None)))
            .child(cal_btn)
            .child(nav("cal-prev", IconName::ChevronLeft, -1))
            .child(ui::outline_button(t, "cal-today").child(i18n::t!("app.calendar.today")).on_click(cx.listener(|this, _, _, cx| {
                this.cal_month = date::month_start(Data::today());
                cx.notify();
            })))
            .child(nav("cal-next", IconName::ChevronRight, 1));
        let mut weeks = div().flex().flex_col().flex_1().min_h_0().border_l_1().border_t_1().border_color(t.border).rounded(px(10.)).overflow_hidden();
        weeks = weeks.child(div().flex().children((0..7).map(|i| div().flex_1().py(px(6.)).px(px(8.)).border_r_1().border_b_1().border_color(t.border).bg(t.surface).text_size(px(11.5)).font_weight(FontWeight::SEMIBOLD).text_color(t.ink_muted).child(weekday(i)))));
        for w in 0..6 {
            let mut row = div().flex().flex_1().min_h(px(84.));
            for d in 0..7 {
                let (day, list) = &grid[w * 7 + d];
                let in_month = day.month() == month.month();
                let is_today = *day == today;
                let mut cell = div().flex().flex_col().gap(px(3.)).flex_1().min_w_0().p(px(6.)).border_r_1().border_b_1().border_color(t.border).bg(if in_month { t.surface } else { t.canvas });
                let day_marks = marks.get(day).cloned().unwrap_or_default();
                let holiday = day_marks.iter().any(|(_, h)| *h);
                cell = cell.child(div().flex().child(div().px(px(5.)).rounded(px(6.)).text_size(px(12.)).when(holiday && !is_today, |d| d.text_color(t.danger).font_weight(FontWeight::SEMIBOLD)).when(is_today, |d| d.bg(t.accent).text_color(t.on_fill()).font_weight(FontWeight::SEMIBOLD)).when(!is_today && !in_month, |d| d.text_color(t.ink_muted.opacity(0.6))).child(day.day().to_string())));
                for (name, h) in day_marks.iter().take(2) {
                    cell = cell.child(div().text_size(px(10.)).truncate().text_color(if *h { t.danger.opacity(0.85) } else { t.ink_muted }).child(name.clone()));
                }
                for (k, it) in list.iter().take(3).enumerate() {
                    let color = t.status(it.status(today));
                    let id = it.id;
                    let sel = self.selected == Some(id);
                    cell = cell.child(
                        div()
                            .id(("cal", (w * 7 + d) * 10 + k))
                            .px(px(5.))
                            .py(px(1.))
                            .rounded(px(5.))
                            .bg(color.opacity(if sel { 0.32 } else { 0.15 }))
                            .text_color(color)
                            .text_size(px(11.))
                            .truncate()
                            .cursor_pointer()
                            .child(it.celebration_text(*day).unwrap_or_else(|| it.title()))
                            .on_click(cx.listener(move |this, _, _, cx| this.select(id, cx))),
                    );
                }
                if list.len() > 3 {
                    cell = cell.child(div().text_size(px(10.5)).text_color(t.ink_muted).child(i18n::tn!("app.calendar.more", list.len() - 3)));
                }
                row = row.child(cell);
            }
            weeks = weeks.child(row);
        }
        div().flex().flex_col().size_full().px(px(24.)).py(px(18.)).gap(px(12.)).child(header).child(weeks).into_any_element()
    }

    pub fn render_list(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let today = Data::today();
        let items = self.data.read(cx).items.clone();
        let entities = self.data.read(cx).entities.clone();
        let rows = vigia_core::view::filtered(&items, &self.filter, today);
        let weak = cx.entity().downgrade();
        let mut seg = ui::seg_group(t).child(ui::seg(t, "st-all", i18n::t!("app.list.all"), self.filter.status.is_none()).on_click(cx.listener(|this, _, _, cx| {
            this.filter.status = None;
            cx.notify();
        })));
        for s in Status::ALL {
            seg = seg.child(ui::seg(t, ("st", s as usize), s.label(), self.filter.status == Some(s)).on_click(cx.listener(move |this, _, _, cx| {
                this.filter.status = Some(s);
                cx.notify();
            })));
        }
        let (cur_ent, cur_kind) = (self.filter.entity, self.filter.entity_kind);
        let owner_label = match (cur_ent.and_then(|id| entities.iter().find(|e| e.id == id)), cur_kind) {
            (Some(e), _) => e.name.clone(),
            (None, Some(k)) => crate::entities::section_title_case(k),
            (None, None) => i18n::t!("app.list.everyone"),
        };
        let w = weak.clone();
        let owner_btn = Button::new("f-owner").outline().small().icon(IconName::User).label(owner_label).dropdown_menu(move |mut menu, _, _| {
            let w0 = w.clone();
            menu = menu.item(PopupMenuItem::new(i18n::t!("app.list.everyone")).checked(cur_ent.is_none() && cur_kind.is_none()).on_click(move |_, _, cx| {
                w0.update(cx, |this, cx| {
                    this.filter.entity = None;
                    this.filter.through.clear();
                    this.filter.entity_kind = None;
                    cx.notify();
                })
                .ok();
            }));
            for k in vigia_core::entity::EntityKind::ALL {
                let mine: Vec<_> = entities.iter().filter(|e| e.kind == k && !e.archived).collect();
                if mine.is_empty() {
                    continue;
                }
                let wk = w.clone();
                menu = menu.separator().item(PopupMenuItem::new(crate::entities::section_title_case(k)).checked(cur_ent.is_none() && cur_kind == Some(k)).on_click(move |_, _, cx| {
                    wk.update(cx, |this, cx| {
                        this.filter.entity = None;
                        this.filter.through.clear();
                        this.filter.entity_kind = Some(k);
                        cx.notify();
                    })
                    .ok();
                }));
                for e in mine {
                    let (w1, id) = (w.clone(), e.id);
                    menu = menu.item(PopupMenuItem::new(e.name.clone()).checked(cur_ent == Some(id)).on_click(move |_, _, cx| {
                        w1.update(cx, |this, cx| {
                            this.filter.through = vigia_core::entity::owned_through(id, &this.data.read(cx).relations);
                            this.filter.entity = Some(id);
                            this.filter.entity_kind = None;
                            cx.notify();
                        })
                        .ok();
                    }));
                }
            }
            menu
        });
        let cur_cat = self.filter.category;
        let w = weak.clone();
        let cat_btn = Button::new("f-cat").outline().small().label(cur_cat.map(|c| c.label()).unwrap_or_else(|| i18n::t!("app.list.all_categories"))).dropdown_menu(move |mut menu, _, _| {
            let w0 = w.clone();
            menu = menu.item(PopupMenuItem::new(i18n::t!("app.list.all_categories")).checked(cur_cat.is_none()).on_click(move |_, _, cx| {
                w0.update(cx, |this, cx| {
                    this.filter.category = None;
                    cx.notify();
                })
                .ok();
            }));
            for c in Category::ALL {
                let w1 = w.clone();
                menu = menu.item(PopupMenuItem::new(c.label()).checked(cur_cat == Some(c)).on_click(move |_, _, cx| {
                    w1.update(cx, |this, cx| {
                        this.filter.category = Some(c);
                        cx.notify();
                    })
                    .ok();
                }));
            }
            menu
        });
        let search = div().flex().items_center().gap(px(6.)).w(px(260.)).h(px(30.)).pl(px(10.)).pr(px(4.)).rounded(px(8.)).border_1().border_color(t.border).bg(t.surface).child(icon(IconName::Search, 13., t.ink_muted)).child(div().flex_1().min_w_0().text_size(px(13.)).child(Input::new(&self.search).appearance(false).small()));
        let toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(search)
            .child(seg)
            .child(owner_btn)
            .child(cat_btn)
            .child(Checkbox::new("f-archived").small().label(i18n::t!("app.list.archived")).checked(self.filter.show_archived).on_click(cx.listener(|this, v: &bool, _, cx| {
                this.filter.show_archived = *v;
                cx.notify();
            })));
        let mut col = Self::scroll_col("list").child(Self::heading(t, i18n::t!("app.view.list"), Some(i18n::t!("app.list.shown", n = rows.len(), total = items.len())))).child(toolbar);
        if rows.is_empty() {
            col = col.child(div().pt(px(30.)).flex().justify_center().text_size(px(13.)).text_color(t.ink_muted).child(if items.is_empty() { i18n::t!("app.list.empty") } else { i18n::t!("app.list.no_match") }));
        }
        for (k, it) in rows.iter().enumerate() {
            col = col.child(self.item_row(t, it, today, ("row", k), cx));
        }
        col.into_any_element()
    }
}
