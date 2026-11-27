//! The family's tokens come from otherwise-ui (github.com/otherwisetech/otherwise-ui, by tag):
//! Inter, the type scale, the warm neutrals and status colours, with Vigía's night-blue accent
//! (`otherwise_ui::accents::VIGIA`, set in main.rs). `Tokens` derefs to `otherwise_ui::Theme`
//! (so `&Tokens` passes wherever the crate wants `&Theme`) and adds the urgency scale.
use gpui::{App, Hsla, Window};
use vigia_core::model::Status;
pub use otherwise_ui::tokens::SANS;

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub ui: otherwise_ui::Theme,
    /// "upcoming" (≤ 90 days): an amber between warn and ok, content only
    pub tag: Hsla,
}

impl std::ops::Deref for Tokens {
    type Target = otherwise_ui::Theme;
    fn deref(&self) -> &otherwise_ui::Theme {
        &self.ui
    }
}

impl Tokens {
    pub fn new(ui: otherwise_ui::Theme) -> Self {
        let tag = otherwise_ui::theme::hex(if ui.dark { 0xE6BE5A } else { 0xA8780F });
        Tokens { ui, tag }
    }

    /// Urgency colour: red expired, orange ≤ 30 days, amber ≤ 90 days, green further away.
    pub fn status(&self, s: Status) -> Hsla {
        match s {
            Status::Expired => self.danger,
            Status::Soon => self.warn,
            Status::Upcoming => self.tag,
            Status::Ok => self.ok,
            Status::Undated => self.ink_muted,
        }
    }
}

pub fn tokens(cx: &App) -> Tokens {
    Tokens::new(otherwise_ui::theme(cx))
}

/// Follows the system's light / dark appearance and sets the family font on gpui-component.
pub fn apply(window: Option<&mut Window>, cx: &mut App) {
    otherwise_ui::apply(otherwise_ui::Appearance::System, window, cx);
}
