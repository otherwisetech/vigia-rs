//! Small styled building blocks shared by the top bar, panel and screens.
use crate::theme::Tokens;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{Icon, IconName};

pub fn icon(name: IconName, size: f32, color: Hsla) -> Icon {
    Icon::new(name).size(px(size)).text_color(color)
}

/// Outlined 34 px control.
pub fn outline_button(t: &Tokens, id: impl Into<ElementId>) -> Stateful<Div> {
    let hover = t.border_strong;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(34.))
        .px(px(12.))
        .rounded(px(10.))
        .border_1()
        .border_color(t.border)
        .bg(t.surface)
        .text_color(t.ink)
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |s| s.border_color(hover))
}

/// Filled 34 px control.
pub fn filled_button(t: &Tokens, id: impl Into<ElementId>, bg: Hsla) -> Stateful<Div> {
    let hover = bg.opacity(0.88);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(34.))
        .px(px(14.))
        .rounded(px(10.))
        .bg(bg)
        .text_color(t.on_fill())
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
}

/// Square icon-only 34 px control.
pub fn icon_button(t: &Tokens, id: impl Into<ElementId>, name: IconName, enabled: bool) -> Stateful<Div> {
    let hover = t.canvas;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(34.))
        .rounded(px(10.))
        .child(icon(name, 16., if enabled { t.ink } else { t.ink_muted.opacity(0.5) }))
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
}

pub fn section_title(t: &Tokens, text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(t.ink_muted).child(text.into())
}


/// The app's own mark.
pub fn logo_mark(size: f32, color: Hsla) -> Svg {
    svg().path(crate::LOGO_SVG).flex_none().size(px(size)).text_color(color)
}

/// Small rounded label.
pub fn chip(text: impl Into<SharedString>, color: Hsla, size: f32) -> Div {
    div().flex_none().px(px(6.)).rounded(px(5.)).bg(color.opacity(0.14)).text_color(color).text_size(px(size)).child(text.into())
}

/// Segmented control of two or more options.
pub fn seg(t: &Tokens, id: impl Into<ElementId>, label: String, on: bool) -> Stateful<Div> {
    div().id(id).flex().items_center().gap(px(5.)).h(px(26.)).px(px(10.)).rounded(px(7.)).text_size(px(12.)).cursor_pointer().when(on, |d| d.bg(t.surface).font_weight(FontWeight::SEMIBOLD).shadow_sm()).when(!on, |d| d.text_color(t.ink_muted)).child(label)
}

pub fn seg_group(t: &Tokens) -> Div {
    div().flex().flex_none().items_center().gap(px(2.)).p(px(2.)).rounded(px(9.)).bg(t.border.opacity(0.6))
}

pub fn backdrop(t: &Tokens, id: &'static str) -> Stateful<Div> {
    div().id(id).absolute().inset_0().flex().items_center().justify_center().bg(gpui::black().opacity(if t.dark { 0.55 } else { 0.32 }))
}

