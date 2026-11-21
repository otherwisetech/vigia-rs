//! Design tokens (same family as rama, nodo and diskmap) plus urgency colours.
use gpui::{App, Hsla, rgb};
use gpui_component::ActiveTheme;
use vigia_core::model::Status;

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub canvas: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub ink: Hsla,
    pub ink_muted: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub danger: Hsla,
    pub warn: Hsla,
    pub tag: Hsla,
    pub ok: Hsla,
    /// logo badge background (the icon's night blue)
    pub brand: Hsla,
    pub dark: bool,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn a(hex: u32, alpha: f32) -> Hsla {
    let mut h = c(hex);
    h.a = alpha;
    h
}

impl Tokens {
    pub fn light() -> Self {
        Tokens {
            canvas: c(0xF2F1EE),
            surface: c(0xFFFFFF),
            border: c(0xE2DFD8),
            border_strong: c(0xC9C5BB),
            ink: c(0x1C1B19),
            ink_muted: c(0x6B675F),
            accent: c(0x2B4C8C),
            accent_soft: c(0xE6ECF7),
            danger: c(0xB3361F),
            warn: c(0xC2620A),
            tag: c(0xA8780F),
            ok: c(0x1F8A4C),
            brand: c(0x1E2F5C),
            dark: false,
        }
    }

    pub fn dark() -> Self {
        Tokens {
            canvas: c(0x141413),
            surface: c(0x1C1B1A),
            border: c(0x302E2B),
            border_strong: c(0x4A4741),
            ink: c(0xF2F0EA),
            ink_muted: c(0xA19C93),
            accent: c(0x8FB0F0),
            accent_soft: a(0x5D82D6, 0.22),
            danger: c(0xE0705C),
            warn: c(0xF09A45),
            tag: c(0xE6BE5A),
            ok: c(0x52C485),
            brand: c(0x1E2F5C),
            dark: true,
        }
    }

    pub fn on_fill(&self) -> Hsla {
        if self.dark { c(0x141413) } else { c(0xFFFFFF) }
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
    if cx.theme().mode.is_dark() { Tokens::dark() } else { Tokens::light() }
}

pub const SANS: &str = "Inter";
