//! Icons of item types (a cake for birthdays, rings for anniversaries, a globe for domains…),
//! shown in lists and section pages. Vigía's own SVGs are served by `AppAssets` in main.rs.
use gpui::*;
use gpui_component::{Icon, IconName};
use vigia_core::model::{Category, Kind};

pub const CAKE_SVG: &str = "icons/vigia-cake.svg";
pub const RINGS_SVG: &str = "icons/vigia-rings.svg";

pub fn kind_icon(kind: Kind, size: f32, color: Hsla) -> Icon {
    let i = match kind {
        Kind::Birthday => Icon::empty().path(CAKE_SVG),
        Kind::Anniversary => Icon::empty().path(RINGS_SVG),
        Kind::Domain => Icon::new(IconName::Globe),
        Kind::Certificate => Icon::new(IconName::Network),
        _ => match kind.category() {
            Category::Identity => Icon::new(IconName::User),
            Category::Vehicle => Icon::empty().path(crate::ui::CAR_SVG),
            Category::Company => Icon::new(IconName::Building2),
            Category::Home => Icon::empty().path(crate::ui::HOUSE_SVG),
            Category::Digital => Icon::new(IconName::Globe),
            Category::Health => Icon::new(IconName::Heart),
            Category::Celebration => Icon::empty().path(CAKE_SVG),
            _ => Icon::new(IconName::File),
        },
    };
    i.size(px(size)).text_color(color)
}
