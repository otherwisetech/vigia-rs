//! Small styled building blocks shared by the top bar, panel and screens.
use crate::theme::Tokens;
use gpui::*;
use gpui_component::{Icon, IconName};
use otherwise_ui::tokens::icon as icon_size;
pub use otherwise_ui::{filled_button, icon, outline_button, section_title};

pub const CAR_SVG: &str = "icons/vigia-car.svg";
pub const HOUSE_SVG: &str = "icons/vigia-house.svg";
pub const PAW_SVG: &str = "icons/vigia-paw.svg";
pub const GRAPH_SVG: &str = "icons/vigia-graph.svg";

/// The icon of a kind of entity (person, car, building, house, paw, globe, folder).
pub fn entity_icon(kind: vigia_core::entity::EntityKind, size: f32, color: Hsla) -> Icon {
    use vigia_core::entity::EntityKind as K;
    let i = match kind {
        K::Person => Icon::new(IconName::User),
        K::Vehicle => Icon::empty().path(CAR_SVG),
        K::Company => Icon::new(IconName::Building2),
        K::Property => Icon::empty().path(HOUSE_SVG),
        K::Pet => Icon::empty().path(PAW_SVG),
        K::Digital => Icon::new(IconName::Globe),
        K::Other => Icon::new(IconName::Folder),
    };
    i.size(px(size)).text_color(color)
}

pub fn svg_icon(path: &'static str, size: f32, color: Hsla) -> Icon {
    Icon::empty().path(path).size(px(size)).text_color(color)
}

/// The form's Save: dimmed and inert while the database can't be written.
pub fn save_button(t: &Tokens, id: impl Into<ElementId>, read_only: bool) -> Stateful<Div> {
    if read_only { filled_button(t, id, t.ink_muted).opacity(0.45).cursor_default() } else { filled_button(t, id, t.accent) }
}

/// Above a form's buttons when nothing can be saved.
pub fn read_only_note(t: &Tokens) -> Div {
    div().flex().items_start().gap(px(6.)).text_size(px(12.)).text_color(t.danger).child(icon(IconName::Info, 13., t.danger)).child(div().flex_1().child(i18n::t!("app.db.read_only_hint")))
}

/// The family's square icon-only button with one of gpui-component's icons.
pub fn icon_button(t: &Tokens, id: impl Into<ElementId>, name: IconName, enabled: bool) -> Stateful<Div> {
    otherwise_ui::icon_button(t, id, enabled).child(icon(name, icon_size::TOOLBAR, if enabled { t.ink } else { t.ink_faint }))
}

/// Small rounded label.
pub fn chip(text: impl Into<SharedString>, color: Hsla, size: f32) -> Div {
    div().flex_none().px(px(6.)).rounded(px(otherwise_ui::tokens::radius::CHIP)).bg(color.opacity(0.14)).text_color(color).text_size(px(size)).child(text.into())
}

/// Segmented control of two or more options.
pub fn seg(t: &Tokens, id: impl Into<ElementId>, label: String, on: bool) -> Stateful<Div> {
    otherwise_ui::segment(t, id, on).child(label)
}

pub fn seg_group(t: &Tokens) -> Div {
    otherwise_ui::segmented(t)
}

pub fn backdrop(t: &Tokens, id: &'static str) -> Stateful<Div> {
    div().id(id).absolute().inset_0().flex().items_center().justify_center().bg(t.backdrop)
}

