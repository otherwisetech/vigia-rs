//! An entity's page (a person, a vehicle, a company…): its details, links to other entities,
//! items, coming deadlines, renewal history and files, plus its edit form in the side panel.
use crate::app::{VigiaApp, View};
use crate::dialogs::Dialog;
use crate::state::Data;
use crate::theme::Tokens;
use crate::ui::{self, icon};
use crate::views::{days_text, long_date, month_title};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{IconName, Sizable as _};
use vigia_core::entity::{Entity as Ent, EntityId, EntityKind, Role};
use vigia_core::model::{Item, State, Status};

/// Sidebar group title: PEOPLE, VEHICLES…
pub fn section_label(k: EntityKind) -> String {
    match k {
        EntityKind::Person => i18n::t!("app.side.people"),
        EntityKind::Vehicle => i18n::t!("app.side.vehicles"),
        EntityKind::Company => i18n::t!("app.side.companies"),
        EntityKind::Property => i18n::t!("app.side.properties"),
        EntityKind::Pet => i18n::t!("app.side.pets"),
        EntityKind::Digital => i18n::t!("app.side.digital"),
        EntityKind::Other => i18n::t!("app.side.other"),
    }
}

/// The same groups in a menu: People, Vehicles…
pub fn section_title_case(k: EntityKind) -> String {
    match k {
        EntityKind::Person => i18n::t!("app.entities.people"),
        EntityKind::Vehicle => i18n::t!("app.entities.vehicles"),
        EntityKind::Company => i18n::t!("app.entities.companies"),
        EntityKind::Property => i18n::t!("app.entities.properties"),
        EntityKind::Pet => i18n::t!("app.entities.pets"),
        EntityKind::Digital => i18n::t!("app.entities.digital"),
        EntityKind::Other => i18n::t!("app.entities.other"),
    }
}

/// The role a new link gets: a person is a member of a company, owns things, is family of
/// another person.
pub fn default_role(from: EntityKind, to: EntityKind) -> Role {
    match (from, to) {
        (EntityKind::Person, EntityKind::Person) => Role::Family,
        (EntityKind::Person, EntityKind::Company) => Role::Member,
        (EntityKind::Person | EntityKind::Company, _) => Role::Owner,
        _ => Role::Other,
    }
}

pub struct EntityForm {
    /// 0 for a new entity.
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: Entity<InputState>,
    pub ident: Entity<InputState>,
    pub detail: Entity<InputState>,
    pub year: Entity<InputState>,
    pub country: Entity<InputState>,
    pub notes: Entity<InputState>,
    pub base: Ent,
}

impl VigiaApp {
    pub fn start_entity_edit(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.data.read(cx).entity(id).cloned() else { return };
        let mk = |v: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).default_value(v));
        let name = mk(e.name.clone(), window, cx);
        name.update(cx, |s, cx| s.focus(window, cx));
        self.edit = None;
        self.selected = None;
        self.entity_edit = Some(EntityForm {
            id,
            kind: e.kind,
            name,
            ident: mk(e.ident.clone(), window, cx),
            detail: mk(e.detail.clone(), window, cx),
            year: mk(e.year.map(|y| y.to_string()).unwrap_or_default(), window, cx),
            country: mk(e.country.clone(), window, cx),
            notes: mk(e.notes.clone(), window, cx),
            base: e,
        });
        cx.notify();
    }

    fn save_entity_edit(&mut self, cx: &mut Context<Self>) {
        let Some(f) = self.entity_edit.as_ref() else { return };
        let val = |e: &Entity<InputState>| e.read(cx).value().trim().to_string();
        let mut e = f.base.clone();
        e.kind = f.kind;
        let name = val(&f.name);
        if !name.is_empty() {
            e.name = name;
        } else if f.id == 0 {
            return self.toast(i18n::t!("app.kinds.needs_name"), cx);
        }
        let is_new = f.id == 0;
        e.ident = val(&f.ident);
        e.detail = val(&f.detail);
        e.country = val(&f.country).to_uppercase();
        e.notes = val(&f.notes);
        let y = val(&f.year);
        e.year = if y.is_empty() {
            None
        } else {
            match y.parse() {
                Ok(v) => Some(v),
                Err(_) => return self.toast(i18n::t!("app.edit.bad_number", value = y), cx),
            }
        };
        match self.data.update(cx, |d, cx| d.save_entity(e, cx)) {
            Ok(id) => {
                self.entity_edit = None;
                if is_new {
                    self.open_entity(id, cx);
                }
                cx.notify();
            }
            Err(err) => self.toast(err, cx),
        }
    }

    pub fn ask_delete_entity(&mut self, id: EntityId, cx: &mut Context<Self>) {
        let d = self.data.read(cx);
        let Some(e) = d.entity(id) else { return };
        let items = d.items.iter().filter(|i| i.entity == Some(id)).count();
        self.dialog = Some(Dialog::DeleteEntity { id, name: e.name.clone(), items });
        cx.notify();
    }

    /// Writes the calendar of this entity's items (`Otherwise SAS.ics`) once, or (`live`) keeps
    /// that file up to date from now on.
    fn pick_entity_ics(&mut self, id: EntityId, live: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.data.read(cx).entity(id).cloned() else { return };
        let name = format!("{}.ics", vigia_core::db::sanitize(&e.name)); // i18n-ok: file name
        let dir = crate::app::documents_dir();
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(p))) = rx.await {
                this.update(cx, |this, cx| {
                    if live {
                        this.data.update(cx, |d, cx| d.set_live_ics(id, Some(p.clone()), cx));
                        return this.toast(i18n::t!("app.entity.live_ics_on", file = p.display()), cx);
                    }
                    let items: Vec<Item> = this.data.read(cx).items.iter().filter(|i| i.entity == Some(id)).cloned().collect();
                    let text = vigia_core::ics::export(&items, Data::today(), vigia_core::db::now_secs());
                    match std::fs::write(&p, text) {
                        Ok(()) => this.toast(i18n::t!("app.export.done", file = p.display()), cx),
                        Err(e) => this.toast(e.to_string(), cx),
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn render_entity_page(&self, t: &Tokens, id: EntityId, cx: &mut Context<Self>) -> AnyElement {
        let today = Data::today();
        let d = self.data.read(cx);
        let Some(e) = d.entity(id).cloned() else { return div().into_any_element() };
        let all = d.entities.clone();
        let rels: Vec<_> = d.relations.iter().filter(|r| r.from == id || r.to == id).cloned().collect();
        let items: Vec<Item> = d.items.iter().filter(|i| i.entity == Some(id)).cloned().collect();
        let files: Vec<(String, vigia_core::model::Attachment)> = d.store.as_ref().map(|s| items.iter().flat_map(|i| s.attachments(i.id).unwrap_or_default().into_iter().map(|a| (i.name.clone(), a))).collect()).unwrap_or_default();
        let mut history: Vec<(String, vigia_core::model::Renewal)> = d.store.as_ref().map(|s| items.iter().flat_map(|i| s.renewals(i.id).unwrap_or_default().into_iter().map(|r| (i.name.clone(), r))).collect()).unwrap_or_default();
        history.sort_by_key(|h| std::cmp::Reverse(h.1.on));
        let files_dir = d.files_dir();
        let name_of = |x: EntityId| all.iter().find(|o| o.id == x).cloned();

        let weak = cx.entity().downgrade();
        let live = d.live_ics.iter().find(|(e, _)| *e == id).map(|(_, p)| p.display().to_string());
        let more = Button::new("ent-more").ghost().small().icon(IconName::Ellipsis).dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            let (w1, w2, w3) = (weak.clone(), weak.clone(), weak.clone());
            let live_item = match &live {
                Some(p) => PopupMenuItem::new(i18n::t!("app.entity.live_ics_stop", file = p.clone())).on_click(move |_, _, cx| {
                    w3.update(cx, |this, cx| this.data.update(cx, |d, cx| d.set_live_ics(id, None, cx))).ok();
                }),
                None => PopupMenuItem::new(i18n::t!("app.entity.live_ics")).on_click(move |_, window, cx| {
                    w3.update(cx, |this, cx| this.pick_entity_ics(id, true, window, cx)).ok();
                }),
            };
            menu.item(PopupMenuItem::new(i18n::t!("app.entity.export_ics")).on_click(move |_, window, cx| {
                w1.update(cx, |this, cx| this.pick_entity_ics(id, false, window, cx)).ok();
            }))
            .item(live_item)
            .separator()
            .item(PopupMenuItem::new(i18n::t!("app.entity.delete")).on_click(move |_, _, cx| {
                w2.update(cx, |this, cx| this.ask_delete_entity(id, cx)).ok();
            }))
        });
        let mut facts = div().flex().flex_wrap().gap(px(16.)).text_size(px(12.5));
        let fact = |label: String, v: String| div().flex().flex_col().gap(px(1.)).child(div().text_size(px(11.)).text_color(t.ink_muted).child(label)).child(div().child(v));
        if let Some(l) = e.kind.ident_label().filter(|_| !e.ident.is_empty()) {
            facts = facts.child(fact(l, e.shown_ident()));
        }
        if let Some(l) = e.kind.detail_label().filter(|_| !e.detail.is_empty()) {
            facts = facts.child(fact(l, e.detail.clone()));
        }
        if let Some(y) = e.year.filter(|_| e.kind.has_year()) {
            facts = facts.child(fact(i18n::t!("app.entity.year"), y.to_string()));
        }
        if !e.country.is_empty() && e.kind.has_country() {
            facts = facts.child(fact(i18n::t!("app.entity.country"), e.country.clone()));
        }
        let live: Vec<&Item> = items.iter().filter(|i| i.state != State::Archived).collect();
        let worst = vigia_core::view::worst_status(&items, id, today);
        let head = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().flex().items_center().justify_center().size(px(44.)).rounded_full().border_2().border_color(worst.map(|s| t.status(s)).unwrap_or(t.border)).bg(t.surface).child(ui::entity_icon(e.kind, 22., t.accent)))
            .child(div().flex().flex_col().flex_1().min_w_0().child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).truncate().child(e.name.clone())).child(div().flex().gap(px(6.)).child(ui::chip(e.kind.label(), t.accent, 11.5)).child(ui::chip(i18n::tn!("app.items.count", live.len()), t.ink_muted, 11.5))))
            .child(ui::outline_button(t, "ent-new").child(icon(IconName::Plus, 14., t.ink)).child(i18n::t!("app.entity.add_item")).on_click(cx.listener(move |this, _, window, cx| {
                let e = this.data.read(cx).entity(id).cloned();
                this.start_new_for(e.as_ref(), window, cx);
            })))
            .child(ui::outline_button(t, "ent-graph").child(ui::svg_icon(ui::GRAPH_SVG, 14., t.ink)).child(i18n::t!("app.entity.show_graph")).on_click(cx.listener(move |this, _, _, cx| {
                this.graph.center = Some(id);
                this.graph.invalidate();
                this.set_view(View::Graph, cx);
            })))
            .child(ui::outline_button(t, "ent-edit").child(i18n::t!("app.item.edit")).on_click(cx.listener(move |this, _, window, cx| this.start_entity_edit(id, window, cx))))
            .child(more);
        let mut col = div().id("entity-page").flex().flex_col().size_full().overflow_y_scroll().px(px(28.)).py(px(22.)).gap(px(10.)).child(head).child(facts);
        if !e.notes.is_empty() {
            col = col.child(div().text_size(px(12.5)).text_color(t.ink_muted).child(e.notes.clone()));
        }
        // counts by urgency
        let mut stats = div().flex().flex_wrap().gap(px(6.));
        for s in [Status::Expired, Status::Soon, Status::Upcoming, Status::Ok] {
            let n = live.iter().filter(|i| i.status(today) == s).count();
            if n > 0 {
                stats = stats.child(ui::chip(format!("{n} · {}", s.label()), t.status(s), 12.)); // i18n-ok: separator
            }
        }
        col = col.child(stats);
        if !vigia_core::templates::missing(e.kind, &live).is_empty() {
            col = col.child(div().pt(px(6.)).child(ui::section_title(t, i18n::t!("app.tpl.suggested")))).child(self.template_chips(t, e.kind, Some(&e), &live, 9000, 8, cx));
        }

        // links to other entities
        let weak = cx.entity().downgrade();
        let me_kind = e.kind;
        let others: Vec<Ent> = all.iter().filter(|o| o.id != id && !rels.iter().any(|r| r.from == o.id || r.to == o.id)).cloned().collect();
        let add_link = Button::new("ent-link").outline().xsmall().icon(IconName::Plus).label(i18n::t!("app.entity.link")).dropdown_menu(move |mut menu, _, _| {
            for k in EntityKind::ALL {
                let of: Vec<&Ent> = others.iter().filter(|o| o.kind == k).collect();
                if of.is_empty() {
                    continue;
                }
                menu = menu.label(section_title_case(k));
                for o in of {
                    let (w, oid, role) = (weak.clone(), o.id, default_role(me_kind, o.kind));
                    menu = menu.item(PopupMenuItem::new(o.name.clone()).on_click(move |_, _, cx| {
                        w.update(cx, |this, cx| {
                            if let Err(e) = this.data.update(cx, |d, cx| d.relate(id, oid, role, cx)) {
                                this.toast(e, cx);
                            }
                        })
                        .ok();
                    }));
                }
            }
            menu
        });
        col = col.child(div().flex().items_center().gap(px(8.)).pt(px(10.)).child(ui::section_title(t, i18n::t!("app.entity.links"))).child(div().flex_1()).child(add_link));
        if rels.is_empty() {
            col = col.child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.entity.no_links")));
        }
        for (k, r) in rels.iter().enumerate() {
            let other_id = if r.from == id { r.to } else { r.from };
            let Some(o) = name_of(other_id) else { continue };
            let text = if r.from == id { i18n::t!("app.entity.link_out", role = r.role.label(), other = o.name.clone()) } else { i18n::t!("app.entity.link_in", role = r.role.label(), other = o.name.clone()) };
            let (rid, from, to, cur) = (r.id, r.from, r.to, r.role);
            let weak = cx.entity().downgrade();
            let role_btn = Button::new(("ent-role", k)).ghost().xsmall().label(cur.label()).dropdown_menu(move |mut menu, _, _| {
                for role in Role::ALL {
                    let w = weak.clone();
                    menu = menu.item(PopupMenuItem::new(role.label()).checked(role == cur).on_click(move |_, _, cx| {
                        w.update(cx, |this, cx| {
                            let _ = this.data.update(cx, |d, cx| d.relate(from, to, role, cx));
                        })
                        .ok();
                    }));
                }
                menu
            });
            let hover = t.canvas;
            col = col.child(
                div()
                    .id(("ent-rel", k))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(8.))
                    .hover(move |s| s.bg(hover))
                    .child(ui::entity_icon(o.kind, 13., t.ink_muted))
                    .child(div().id(("ent-rel-open", k)).flex_1().min_w_0().truncate().text_size(px(13.)).cursor_pointer().child(text).on_click(cx.listener(move |this, _, _, cx| this.open_entity(other_id, cx))))
                    .child(role_btn)
                    .child(div().id(("ent-rel-rm", k)).cursor_pointer().child(icon(IconName::Close, 12., t.ink_muted)).on_click(cx.listener(move |this, _, _, cx| this.data.update(cx, |d, cx| d.unrelate(rid, cx))))),
            );
        }

        // items
        col = col.child(div().pt(px(10.)).child(ui::section_title(t, i18n::t!("app.entity.items"))));
        if items.is_empty() {
            col = col.child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.entity.no_items")));
        }
        for (k, it) in vigia_core::view::sorted(items.iter(), today).into_iter().enumerate() {
            col = col.child(self.item_row(t, it, today, ("ent-item", k), cx));
        }

        // coming deadlines, by month
        let (overdue, months) = vigia_core::view::timeline(&live, today, 24);
        let months: Vec<_> = months.into_iter().filter(|m| !m.items.is_empty()).collect();
        if !overdue.is_empty() || !months.is_empty() {
            col = col.child(div().pt(px(10.)).child(ui::section_title(t, i18n::t!("app.entity.timeline"))));
            let line = |k: usize, it: &Item, cx: &mut Context<Self>| {
                let iid = it.id;
                let color = t.status(it.status(today));
                div()
                    .id(("ent-tl", k))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(3.))
                    .rounded(px(7.))
                    .cursor_pointer()
                    .child(div().size(px(8.)).rounded_full().bg(color))
                    .child(div().w(px(170.)).flex_none().text_size(px(12.5)).child(it.due_date(today).map(long_date).unwrap_or_default()))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(12.5)).child(it.name.clone()))
                    .child(div().text_size(px(12.)).text_color(color).child(days_text(it.days_left(today))))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(iid, cx)))
            };
            let mut k = 0;
            for it in &overdue {
                col = col.child(line(k, it, cx));
                k += 1;
            }
            for m in &months {
                col = col.child(div().pt(px(4.)).px(px(10.)).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(t.ink_muted).child(month_title(m.start)));
                for it in &m.items {
                    col = col.child(line(k, it, cx));
                    k += 1;
                }
            }
        }
        if !history.is_empty() {
            col = col.child(div().pt(px(10.)).child(ui::section_title(t, i18n::t!("app.history.title"))));
            for (name, r) in history.iter().take(12) {
                col = col.child(div().px(px(10.)).text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.entity.renewed", on = long_date(r.on), name = name.clone(), to = r.new_expiry.map(long_date).unwrap_or_default())));
            }
        }
        // files of all its items
        col = col.child(div().pt(px(10.)).child(ui::section_title(t, i18n::t!("app.attach.title"))));
        if files.is_empty() {
            col = col.child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.entity.no_files")));
        }
        for (k, (item_name, a)) in files.iter().enumerate() {
            let path = files_dir.join(a.rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
            let hover = t.canvas;
            col = col.child(
                div()
                    .id(("ent-file", k))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(7.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .child(icon(IconName::File, 13., t.ink_muted))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(12.5)).child(a.name.clone()))
                    .child(div().text_size(px(11.5)).text_color(t.ink_muted).child(item_name.clone()))
                    .on_click(move |_, _, cx| cx.open_with_system(&path)),
            );
        }
        col.into_any_element()
    }

    pub fn render_entity_form(&self, t: &Tokens, cx: &mut Context<Self>) -> Stateful<Div> {
        let f = self.entity_edit.as_ref().expect("entity form");
        let label = |s: String| div().text_size(px(11.5)).font_weight(FontWeight::MEDIUM).text_color(t.ink_muted).child(s);
        let field = |s: String, e: &Entity<InputState>| div().flex().flex_col().gap(px(3.)).child(label(s)).child(Input::new(e).small());
        let weak = cx.entity().downgrade();
        let cur = f.kind;
        let kind_btn = Button::new("ent-form-kind").outline().small().w_full().label(cur.label()).dropdown_menu(move |mut menu, _, _| {
            for k in EntityKind::ALL {
                let w = weak.clone();
                menu = menu.item(PopupMenuItem::new(k.label()).checked(k == cur).on_click(move |_, _, cx| {
                    w.update(cx, |this, cx| {
                        if let Some(f) = this.entity_edit.as_mut() {
                            f.kind = k;
                        }
                        cx.notify();
                    })
                    .ok();
                }));
            }
            menu
        });
        let mut form = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(18.))
            .pt(px(16.))
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(if f.id == 0 { crate::kinds::add_label(cur) } else { i18n::t!("app.entity.edit") }))
            .child(div().flex().flex_col().gap(px(3.)).child(label(i18n::t!("app.entity.kind"))).child(kind_btn))
            .child(field(i18n::t!("app.field.name"), &f.name));
        if let Some(l) = cur.ident_label() {
            form = form.child(field(l, &f.ident));
        }
        if let Some(l) = cur.detail_label() {
            form = form.child(field(l, &f.detail));
        }
        if cur.has_year() {
            form = form.child(field(i18n::t!("app.entity.year"), &f.year));
        }
        if cur.has_country() {
            form = form.child(field(i18n::t!("app.entity.country"), &f.country));
        }
        form = form.child(field(i18n::t!("app.field.notes"), &f.notes)).child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.))
                .pt(px(4.))
                .child(ui::outline_button(t, "ent-form-cancel").child(i18n::t!("app.dialog.cancel")).on_click(cx.listener(|this, _, _, cx| {
                    this.entity_edit = None;
                    cx.notify();
                })))
                .child(ui::filled_button(t, "ent-form-save", t.accent).child(i18n::t!("app.dialog.save")).on_click(cx.listener(|this, _, _, cx| this.save_entity_edit(cx)))),
        );
        div().id("entity-form").flex().flex_col().flex_none().w(px(crate::panel::PANEL_W)).h_full().bg(t.surface).border_l_1().border_color(t.border).overflow_y_scroll().pb(px(20.)).child(form)
    }
}

#[cfg(test)]
mod tests {
    use vigia_core::entity::{EntityKind as K, Role};

    #[test]
    fn link_roles() {
        assert_eq!(super::default_role(K::Person, K::Company), Role::Member);
        assert_eq!(super::default_role(K::Person, K::Vehicle), Role::Owner);
        assert_eq!(super::default_role(K::Company, K::Digital), Role::Owner);
        assert_eq!(super::default_role(K::Person, K::Person), Role::Family);
        assert_eq!(super::default_role(K::Vehicle, K::Person), Role::Other);
    }
}
