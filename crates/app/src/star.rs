//! «Star on GitHub» in the footer and the About modal, with the repo's stargazer count. The count
//! is an unauthenticated GET to the public repos API (nothing about the user or their files is
//! sent), at most once a day, cached on disk; any failure just hides it. Off with the
//! «Show GitHub stars» item of the globe menu or Settings. With the opt-in item checks, the only requests Vigía makes.
use crate::theme::Tokens;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::IconName;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

const REPO: &str = "otherwisetech/vigia-rs";
pub const REFRESH_SECS: u64 = 24 * 60 * 60;

pub fn repo_url() -> String {
    format!("https://github.com/{REPO}")
}

pub fn api_url() -> String {
    format!("https://api.github.com/repos/{REPO}")
}

pub fn user_agent() -> String {
    format!("Vigia/{} (+https://otherwisetech.com)", env!("CARGO_PKG_VERSION")) // i18n-ok: HTTP header (ASCII)
}

/// `stargazers_count` of a `repos/{owner}/{repo}` body.
pub fn parse_stars(json: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(json).ok()?.get("stargazers_count")?.as_u64()
}

/// 12 → «12», 1234 → «1.2k», 12345 → «12k».
pub fn short_count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..10_000 => format!("{:.1}k", n as f64 / 1_000.0).replace(".0k", "k"), // i18n-ok: number
        _ => format!("{}k", n / 1_000), // i18n-ok: number
    }
}

/// Last attempt (unix seconds) and its answer; `stars: None` when it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cache {
    pub checked: u64,
    pub stars: Option<u64>,
}

impl Cache {
    /// A clock moved backwards counts as stale.
    pub fn fresh(&self, now: u64) -> bool {
        now >= self.checked && now - self.checked < REFRESH_SECS
    }

    pub fn parse(s: &str) -> Option<Cache> {
        let v = serde_json::from_str::<serde_json::Value>(s).ok()?;
        Some(Cache { checked: v.get("checked")?.as_u64()?, stars: v.get("stars").and_then(|s| s.as_u64()) })
    }

    pub fn to_json(self) -> String {
        match self.stars {
            Some(n) => format!("{{\"checked\": {}, \"stars\": {n}}}\n", self.checked),
            None => format!("{{\"checked\": {}, \"stars\": null}}\n", self.checked),
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn cache_file() -> Option<PathBuf> {
    vigia_core::paths::data_dir().map(|d| d.join("github-stars.json"))
}

fn load_cache() -> Option<Cache> {
    Cache::parse(&std::fs::read_to_string(cache_file()?).ok()?)
}

fn save_cache(c: Cache) {
    let Some(f) = cache_file() else { return };
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(f, c.to_json());
}

/// Blocking: run it on the background executor.
fn fetch() -> Option<u64> {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    let agent = AGENT.get_or_init(|| ureq::Agent::config_builder().user_agent(user_agent()).timeout_global(Some(Duration::from_secs(15))).max_redirects(3).build().into());
    let mut resp = agent.get(&api_url()).header("Accept", "application/vnd.github+json").call().ok()?;
    parse_stars(&resp.body_mut().with_config().limit(1 << 20).read_to_string().ok()?)
}

/// Whether the count may be fetched, and the count shown next to the star, if known.
struct Stars {
    on: bool,
    n: Option<u64>,
}

impl Global for Stars {}

pub fn count(cx: &App) -> Option<u64> {
    cx.try_global::<Stars>().and_then(|s| s.n)
}

pub fn enabled(cx: &App) -> bool {
    cx.try_global::<Stars>().is_none_or(|s| s.on)
}

/// Shows the cached count and refreshes it when a day old. `on: false` hides it.
pub fn refresh<V: 'static>(on: bool, cx: &mut Context<V>) {
    if !on {
        cx.set_global(Stars { on: false, n: None });
        return;
    }
    let now = now_secs();
    let cached = load_cache();
    cx.set_global(Stars { on: true, n: cached.and_then(|c| c.stars) });
    if cached.is_some_and(|c| c.fresh(now)) {
        return;
    }
    let task = cx.background_executor().spawn(async move {
        let stars = fetch();
        save_cache(Cache { checked: now, stars });
        stars
    });
    cx.spawn(async move |this, cx| {
        let stars = task.await;
        this.update(cx, |_, cx| {
            if enabled(cx) {
                cx.set_global(Stars { on: true, n: stars });
                cx.refresh_windows();
            }
        })
        .ok();
    })
    .detach();
}

fn open_repo(_: &ClickEvent, _: &mut Window, cx: &mut App) {
    cx.open_url(&repo_url());
}

fn star_count(color: Hsla, size: f32, n: Option<u64>) -> Div {
    div().flex().items_center().gap(px(3.)).child(crate::ui::icon(IconName::Star, size, color)).when_some(n, |d, n| d.child(short_count(n)))
}

/// Small footer link: GitHub mark, «Star on GitHub», ★ count.
pub fn footer_link(t: &Tokens, cx: &App) -> Stateful<Div> {
    let (muted, hover) = (t.ink_muted, t.ink);
    div()
        .id("star-footer")
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .text_size(px(11.))
        .text_color(muted)
        .cursor_pointer()
        .hover(move |s| s.text_color(hover))
        .child(crate::ui::icon(IconName::Github, 11. + 1., muted))
        .child(i18n::t!("app.star.label"))
        .child(star_count(muted, 11., count(cx)))
        .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new(i18n::t!("app.star.tooltip")).build(w, cx))
        .on_click(open_repo)
}

/// About modal button: outlined, GitHub mark, «Star on GitHub», ★ count.
pub fn about_button(t: &Tokens, cx: &App) -> Stateful<Div> {
    let hover = t.border_strong;
    div()
        .id("about-star")
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(30.))
        .px(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(t.border)
        .bg(t.surface)
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.ink)
        .cursor_pointer()
        .hover(move |s| s.border_color(hover))
        .child(crate::ui::icon(IconName::Github, 14., t.ink))
        .child(i18n::t!("app.star.label"))
        .child(div().pl(px(6.)).border_l_1().border_color(t.border).text_color(t.ink_muted).child(star_count(t.tag, 13., count(cx))))
        .on_click(open_repo)
}

/// One-line ask under the About button.
pub fn about_ask(t: &Tokens) -> Div {
    div().text_size(px(12.)).text_color(t.ink_muted).text_center().child(i18n::t!("app.star.ask"))
}

#[cfg(test)]
mod tests {
    use super::{Cache, REFRESH_SECS, api_url, parse_stars, repo_url, short_count, user_agent};

    #[test]
    fn urls() {
        assert_eq!(repo_url(), "https://github.com/otherwisetech/vigia-rs");
        assert_eq!(api_url(), "https://api.github.com/repos/otherwisetech/vigia-rs");
        assert_eq!(user_agent(), format!("Vigia/{} (+https://otherwisetech.com)", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn stars_body() {
        assert_eq!(parse_stars(r#"{"id":1,"full_name":"otherwisetech/vigia-rs","stargazers_count":12,"watchers_count":12}"#), Some(12));
        assert_eq!(parse_stars(r#"{"stargazers_count":0}"#), Some(0));
        assert_eq!(parse_stars(r#"{"message":"API rate limit exceeded"}"#), None);
        assert_eq!(parse_stars(r#"{"stargazers_count":-1}"#), None);
        assert_eq!(parse_stars("<html>"), None);
    }

    #[test]
    fn freshness() {
        let c = Cache { checked: 1_000, stars: Some(3) };
        assert!(c.fresh(1_000));
        assert!(c.fresh(1_000 + REFRESH_SECS - 1));
        assert!(!c.fresh(1_000 + REFRESH_SECS));
        assert!(!c.fresh(999));
    }

    #[test]
    fn cache_roundtrip() {
        for c in [Cache { checked: 1_790_000_000, stars: Some(42) }, Cache { checked: 5, stars: None }] {
            assert_eq!(Cache::parse(&c.to_json()), Some(c));
        }
        assert_eq!(Cache::parse("{}"), None);
        assert_eq!(Cache::parse("garbage"), None);
    }

    #[test]
    fn counts() {
        assert_eq!(short_count(0), "0");
        assert_eq!(short_count(999), "999");
        assert_eq!(short_count(1_000), "1k");
        assert_eq!(short_count(1_234), "1.2k");
        assert_eq!(short_count(12_345), "12k");
    }
}
