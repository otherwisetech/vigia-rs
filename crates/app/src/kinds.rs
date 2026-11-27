//! Sections by kind of entity (Vehículos, Mascotas, Personas, Empresas, Hogar, Digital): each
//! lists its entities as cards with their next deadline, and offers suggested reminders as
//! chips; one click opens the new-item form prefilled from the template, with only the date
//! left to set. The top bar's template picker reaches the same from anywhere.
use crate::app::VigiaApp;
use crate::entities::EntityForm;
use crate::state::Data;
use crate::theme::Tokens;
use crate::ui::{self, icon};
use crate::views::{days_text, long_date};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::InputState;
use gpui_component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_component::{IconName, Sizable as _};
use vigia_core::entity::{Entity as Ent, EntityKind};
use vigia_core::model::{Item, State, leads_text};
use vigia_core::templates::{self, Template};

/// "Agregar vehículo", "Agregar mascota"…
pub fn add_label(k: EntityKind) -> String {
    match k {
        EntityKind::Person => i18n::t!("app.kinds.add.person"),
        EntityKind::Vehicle => i18n::t!("app.kinds.add.vehicle"),
        EntityKind::Company => i18n::t!("app.kinds.add.company"),
        EntityKind::Property => i18n::t!("app.kinds.add.property"),
        EntityKind::Pet => i18n::t!("app.kinds.add.pet"),
        EntityKind::Digital => i18n::t!("app.kinds.add.digital"),
        EntityKind::Other => i18n::t!("app.kinds.add.other"),
    }
}

/// The heading over a section's items that no entity of its kind holds.
pub fn loose_label(k: EntityKind) -> String {
    match k {
        EntityKind::Digital => i18n::t!("app.kinds.loose.digital"),
        EntityKind::Vehicle => i18n::t!("app.kinds.loose.vehicle"),
        EntityKind::Company => i18n::t!("app.kinds.loose.company"),
        EntityKind::Property => i18n::t!("app.kinds.loose.property"),
        _ => i18n::t!("app.kinds.loose.other"),
    }
}

/// One line under a section's title.
fn blurb(k: EntityKind) -> String {
    match k {
        EntityKind::Person => i18n::t!("app.kinds.blurb.person"),
        EntityKind::Vehicle => i18n::t!("app.kinds.blurb.vehicle"),
        EntityKind::Company => i18n::t!("app.kinds.blurb.company"),
        EntityKind::Property => i18n::t!("app.kinds.blurb.property"),
        EntityKind::Pet => i18n::t!("app.kinds.blurb.pet"),
        EntityKind::Digital => i18n::t!("app.kinds.blurb.digital"),
        EntityKind::Other => i18n::t!("app.kinds.blurb.other"),
    }
}

/// "Cada año · avisa 30, 7, 1 días antes" (and the template's note).
pub fn template_tip(tpl: &Template) -> String {
    let mut s = i18n::t!("app.tpl.tip", every = tpl.recurrence.label(), leads = leads_text(tpl.lead));
    if let Some(n) = tpl.note() {
        s = format!("{s}\n{n}"); // i18n-ok: two lines
    }
    s
}

/// The entities, by kind, the picker and the chips work with.
fn entities_of(d: &Data, k: EntityKind) -> Vec<Ent> {
    let mut v: Vec<Ent> = d.entities.iter().filter(|e| e.kind == k && !e.archived).cloned().collect();
    v.sort_by_key(|e| e.name.to_lowercase());
    v
}

impl VigiaApp {
    /// The new-item form filled from `tpl` for `owner` (or for an owner still to type, of the
    /// template's kind); the cursor waits in the date.
    pub fn start_from_template(&mut self, tpl: &'static Template, owner: Option<Ent>, kind: Option<EntityKind>, window: &mut Window, cx: &mut Context<Self>) {
        let mut it = tpl.item(owner.as_ref());
        // a section's kind is the owner's for its own papers (a pet's vaccine), not for a
        // domain or subscription, whose holder is a person or company
        if owner.is_none() && tpl.category != vigia_core::model::Category::Digital {
            it.owner_kind = kind.or(it.owner_kind);
        }
        it.auto_check = it.kind.checkable();
        self.start_new_with(it, window, cx);
        if let Some(f) = self.edit.as_ref() {
            let target = if owner.is_none() { f.owner.clone() } else { f.expires.clone() };
            target.update(cx, |s, cx| s.focus(window, cx));
        }
    }

    /// The entity form for a new vehicle, pet, person…
    pub fn start_new_entity(&mut self, kind: EntityKind, window: &mut Window, cx: &mut Context<Self>) {
        let mk = |window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx));
        let name = mk(window, cx);
        name.update(cx, |s, cx| s.focus(window, cx));
        self.edit = None;
        self.selected = None;
        self.entity_edit = Some(EntityForm { id: 0, kind, name, ident: mk(window, cx), detail: mk(window, cx), year: mk(window, cx), country: mk(window, cx), notes: mk(window, cx), base: Ent::new(kind, "") });
        cx.notify();
    }

    /// A row of suggestion chips for an entity (or, with `None`, for a new one of `kind`).
    #[allow(clippy::too_many_arguments)]
    pub fn template_chips(&self, t: &Tokens, kind: EntityKind, owner: Option<&Ent>, have: &[&Item], key: usize, limit: usize, cx: &mut Context<Self>) -> Div {
        let missing = templates::missing(kind, have);
        let mut row = div().flex().flex_wrap().items_center().gap(px(6.));
        let hover = t.accent_soft;
        for (k, tpl) in missing.iter().take(limit).enumerate() {
            let tip = template_tip(tpl);
            let (tpl, owner) = (*tpl, owner.cloned());
            row = row.child(
                div()
                    .id(("tpl-chip", key * 100 + k))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .h(px(24.))
                    .px(px(8.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface)
                    .text_size(px(12.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .child(icon(IconName::Plus, 11., t.accent))
                    .child(tpl.label())
                    .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    .on_click(cx.listener(move |this, _, window, cx| this.start_from_template(tpl, owner.clone(), Some(kind), window, cx))),
            );
        }
        let rest: Vec<&'static Template> = missing.iter().skip(limit).copied().collect();
        if !rest.is_empty() {
            let weak = cx.entity().downgrade();
            let owner = owner.cloned();
            row = row.child(Button::new(("tpl-more", key)).ghost().xsmall().label(i18n::tn!("app.tpl.more", rest.len())).dropdown_menu(move |mut menu, _, _| {
                for tpl in rest.iter().copied() {
                    let (w, owner) = (weak.clone(), owner.clone());
                    menu = menu.item(PopupMenuItem::new(tpl.label()).on_click(move |_, window, cx| {
                        w.update(cx, |this, cx| this.start_from_template(tpl, owner.clone(), Some(kind), window, cx)).ok();
                    }));
                }
                menu
            }));
        }
        row
    }

    pub fn render_kind_page(&self, t: &Tokens, kind: EntityKind, cx: &mut Context<Self>) -> AnyElement {
        let today = Data::today();
        let d = self.data.read(cx);
        let items = d.items.clone();
        let entities = d.entities.clone();
        let cards = vigia_core::view::cards(&entities, &items, kind, today);
        let loose: Vec<Item> = vigia_core::view::loose(&items, &entities, kind, today).into_iter().cloned().collect();
        let head = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().flex().items_center().justify_center().size(px(44.)).rounded_full().bg(t.accent_soft).child(ui::entity_icon(kind, 22., t.accent)))
            .child(div().flex().flex_col().flex_1().min_w_0().child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child(crate::entities::section_title_case(kind))).child(div().text_size(px(13.)).text_color(t.ink_muted).child(blurb(kind))))
            .child(ui::filled_button(t, "kind-add", t.accent).child(icon(IconName::Plus, 15., t.on_fill())).child(add_label(kind)).on_click(cx.listener(move |this, _, window, cx| this.start_new_entity(kind, window, cx))));
        let mut col = div().id("kind-page").flex().flex_col().size_full().overflow_y_scroll().px(px(28.)).py(px(22.)).gap(px(14.)).child(head);
        if !loose.is_empty() {
            col = col.child(self.loose_list(t, kind, &loose, cx));
        }
        if cards.is_empty() && !loose.is_empty() {
            return col.child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.kinds.loose_hint", kind = add_label(kind)))).child(self.template_chips(t, kind, None, &[], 0, 12, cx)).into_any_element();
        }
        if cards.is_empty() {
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .p(px(18.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface)
                    .child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).child(i18n::t!("app.kinds.empty")))
                    .child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.kinds.empty_hint")))
                    .child(self.template_chips(t, kind, None, &[], 0, 12, cx)),
            );
            return col.into_any_element();
        }
        let mut grid = div().flex().flex_wrap().gap(px(12.));
        for (k, c) in cards.iter().enumerate() {
            let e = c.entity.clone();
            let id = e.id;
            let ring = c.status.map(|s| t.status(s)).unwrap_or(t.border);
            let have: Vec<&Item> = items.iter().filter(|i| i.owned_by(id) && i.state != State::Archived).collect();
            let sub = [e.shown_ident(), e.detail.clone()].into_iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(" · "); // i18n-ok: separator
            let next = match c.next {
                Some(it) => {
                    let color = t.status(it.status(today));
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().size(px(8.)).flex_none().rounded_full().bg(color))
                        .child(div().flex().flex_col().flex_1().min_w_0().child(div().text_size(px(13.)).truncate().child(it.name.clone())).child(div().text_size(px(12.)).text_color(t.ink_muted).child(it.due_date(today).map(long_date).unwrap_or_default())))
                        .child(ui::chip(days_text(it.days_left(today)), color, 11.))
                }
                None => div().text_size(px(12.)).text_color(t.ink_muted).child(if c.items == 0 { i18n::t!("app.kinds.no_items") } else { i18n::t!("app.kinds.no_dates") }),
            };
            grid = grid.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .w(px(300.))
                    .p(px(14.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface)
                    .child(
                        div()
                            .id(("kind-card", k))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .cursor_pointer()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(div().flex().flex_none().items_center().justify_center().size(px(34.)).rounded_full().border_2().border_color(ring).child(ui::entity_icon(kind, 16., t.accent)))
                                    .child(div().flex().flex_col().flex_1().min_w_0().child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).truncate().child(e.name.clone())).when(!sub.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(t.ink_muted).truncate().child(sub))))
                                    .children(c.status.map(|s| ui::chip(s.label(), t.status(s), 11.)))
                                    .child(div().text_size(px(11.)).text_color(t.ink_muted).child(i18n::tn!("app.items.count", c.items))),
                            )
                            .child(next)
                            .on_click(cx.listener(move |this, _, _, cx| this.open_entity(id, cx))),
                    )
                    .child(self.template_chips(t, kind, Some(&e), &have, k + 1, 3, cx)),
            );
        }
        col.child(grid).into_any_element()
    }

    /// The items of the section no entity of its kind holds (a company's domains in Digital),
    /// each with its owners.
    fn loose_list(&self, t: &Tokens, kind: EntityKind, items: &[Item], cx: &mut Context<Self>) -> Div {
        let today = Data::today();
        let d = self.data.read(cx);
        let mut list = div().flex().flex_col().rounded(px(12.)).border_1().border_color(t.border).bg(t.surface);
        for (k, it) in items.iter().enumerate() {
            let id = it.id;
            let color = t.status(it.status(today));
            let owners = it.owner_ids().into_iter().filter_map(|o| d.entity(o).map(|e| e.name.clone())).collect::<Vec<_>>().join(", ");
            let hover = t.canvas;
            list = list.child(
                div()
                    .id(("loose", k))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(9.))
                    .when(k > 0, |d| d.border_t_1().border_color(t.border))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .child(div().size(px(8.)).flex_none().rounded_full().bg(color))
                    .child(crate::icons::kind_icon(it.kind, 14., t.ink_muted))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child(it.name.clone()))
                            .child(div().text_size(px(12.)).text_color(t.ink_muted).truncate().child(if owners.is_empty() { i18n::t!("app.kinds.no_owner") } else { owners })),
                    )
                    .child(div().text_size(px(12.)).text_color(t.ink_muted).child(it.due_date(today).map(long_date).unwrap_or_else(|| i18n::t!("app.days.none"))))
                    .child(ui::chip(days_text(it.days_left(today)), color, 11.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = Some(id);
                        this.edit = None;
                        cx.notify();
                    })),
            );
        }
        div().flex().flex_col().gap(px(6.)).child(ui::section_title(t, loose_label(kind))).child(list)
    }

    /// The top bar's "from a template" menu: kind → entity → template, or a template for an
    /// owner still to name.
    pub fn template_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = self.data.read(cx);
        let groups: Vec<(EntityKind, Vec<Ent>)> = templates::SECTIONS.iter().map(|k| (*k, entities_of(d, *k))).collect();
        let weak = cx.entity().downgrade();
        Button::new("top-template").outline().icon(IconName::Plus).label(i18n::t!("app.tpl.picker")).tooltip(i18n::t!("app.tpl.picker_tip")).dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu: PopupMenu, window, cx| {
            menu = menu.label(i18n::t!("app.tpl.pick_kind"));
            for (kind, ents) in groups.clone() {
                let weak = weak.clone();
                menu = menu.submenu(crate::entities::section_title_case(kind), window, cx, move |mut m, window, cx| {
                    for e in ents.clone() {
                        let weak = weak.clone();
                        let ent = e.clone();
                        m = m.submenu(e.name.clone(), window, cx, move |mut mm, _, _| {
                            for tpl in templates::for_kind(kind) {
                                let (w, owner) = (weak.clone(), ent.clone());
                                mm = mm.item(PopupMenuItem::new(tpl.label()).on_click(move |_, window, cx| {
                                    w.update(cx, |this, cx| this.start_from_template(tpl, Some(owner.clone()), Some(kind), window, cx)).ok();
                                }));
                            }
                            mm
                        });
                    }
                    if !ents.is_empty() {
                        m = m.separator();
                    }
                    let w = weak.clone();
                    m = m.item(PopupMenuItem::new(add_label(kind)).on_click(move |_, window, cx| {
                        w.update(cx, |this, cx| this.start_new_entity(kind, window, cx)).ok();
                    }));
                    if ents.is_empty() {
                        m = m.separator().label(i18n::t!("app.tpl.for_new"));
                        for tpl in templates::for_kind(kind) {
                            let w = weak.clone();
                            m = m.item(PopupMenuItem::new(tpl.label()).on_click(move |_, window, cx| {
                                w.update(cx, |this, cx| this.start_from_template(tpl, None, Some(kind), window, cx)).ok();
                            }));
                        }
                    }
                    m
                });
            }
            menu
        })
    }
}
