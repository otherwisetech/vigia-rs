// No console window behind the app on Windows release builds.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![recursion_limit = "512"]

mod app;
mod dialogs;
mod entities;
mod graph;
#[cfg(target_os = "macos")]
mod macos;
mod panel;
mod star;
mod state;
mod storage_ui;
mod system;
mod theme;
mod ui;
mod views;

use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;
use std::borrow::Cow;
use std::path::PathBuf;
use std::time::Duration;
use vigia_core::settings::Settings;

actions!(vigia, [Quit, OpenWebsite, OpenVcity]);

pub(crate) const APP_NAME: &str = "Vigía"; // i18n-ok: product name
pub(crate) const COMPANY: &str = "Otherwise Technologies LLC"; // i18n-ok: company name
pub(crate) const COMPANY_URL: &str = "https://otherwisetech.com";
/// The invitation to Vcity (About and the Help menu).
pub(crate) const VCITY_URL: &str = "https://vcity.io/?utm_source=vigia&utm_medium=app&utm_campaign=otherwise-apps";
pub(crate) const REPO_URL: &str = "https://github.com/otherwisetech/vigia-rs";
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The native menu bar holds plain strings, so it is rebuilt on every language switch.
pub(crate) fn set_app_menus(cx: &mut App) {
    use app::*;
    let app_menu = Menu {
        name: APP_NAME.into(),
        items: vec![
            MenuItem::action(i18n::t!("app.about.menu", app = APP_NAME), ShowAbout),
            MenuItem::action(i18n::t!("app.credit.menu", company = COMPANY), OpenWebsite),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.menu.settings"), OpenSettings),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.menu.quit", app = APP_NAME), Quit),
        ],
        disabled: false,
    };
    let file = Menu {
        name: i18n::t!("app.menu.file").into(),
        items: vec![
            MenuItem::action(i18n::t!("app.item.new"), NewItem),
            MenuItem::action(i18n::t!("app.quick.menu"), FocusQuickAdd),
            MenuItem::action(i18n::t!("app.ocr.menu"), ReadDocument),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.menu.import_csv"), ImportCsv),
            MenuItem::action(i18n::t!("app.menu.export_csv"), ExportCsv),
            MenuItem::action(i18n::t!("app.menu.export_ics"), ExportIcs),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.menu.open_db"), OpenDb),
            MenuItem::submenu(Menu { name: i18n::t!("app.menu.open_recent").into(), items: storage_ui::recent_menu(), disabled: false }),
            MenuItem::action(i18n::t!("app.menu.save_as"), SaveDbAs),
            MenuItem::action(i18n::t!("app.menu.move_db"), MoveDb),
            MenuItem::action(storage_ui::reveal_label(), RevealDb),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.menu.backup"), Backup),
            MenuItem::action(i18n::t!("app.menu.restore"), Restore),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.menu.close_window"), CloseWindow),
        ],
        disabled: false,
    };
    let view = Menu {
        name: i18n::t!("app.menu.view").into(),
        items: vec![
            MenuItem::action(i18n::t!("app.view.dashboard"), ShowDashboard),
            MenuItem::action(i18n::t!("app.view.timeline"), ShowTimeline),
            MenuItem::action(i18n::t!("app.view.calendar"), ShowCalendar),
            MenuItem::action(i18n::t!("app.view.list"), ShowList),
            MenuItem::action(i18n::t!("app.view.graph"), ShowGraph),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.search.menu"), FocusSearch),
        ],
        disabled: false,
    };
    let item = Menu {
        name: i18n::t!("app.menu.item").into(),
        items: vec![
            MenuItem::action(i18n::t!("app.item.renew"), RenewSelected),
            MenuItem::action(i18n::t!("app.item.edit"), EditSelected),
            MenuItem::action(i18n::t!("app.item.in_progress"), ToggleInProgress),
            MenuItem::action(i18n::t!("app.item.snooze_week"), SnoozeSelected),
            MenuItem::action(i18n::t!("app.item.check_now"), CheckSelected),
            MenuItem::separator(),
            MenuItem::action(i18n::t!("app.item.delete"), DeleteSelected),
        ],
        disabled: false,
    };
    let help = Menu { name: i18n::t!("app.menu.help").into(), items: vec![MenuItem::action(i18n::t!("app.vcity.menu"), OpenVcity)], disabled: false };
    cx.set_menus(vec![app_menu, file, view, item, help]);
}

/// gpui-component's icon set plus our own mark.
struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let own: Option<&'static [u8]> = match path {
            LOGO_SVG => Some(include_bytes!("../assets/icons/vigia-logo.svg")),
            ui::CAR_SVG => Some(include_bytes!("../assets/icons/vigia-car.svg")),
            ui::HOUSE_SVG => Some(include_bytes!("../assets/icons/vigia-house.svg")),
            ui::PAW_SVG => Some(include_bytes!("../assets/icons/vigia-paw.svg")),
            ui::GRAPH_SVG => Some(include_bytes!("../assets/icons/vigia-graph.svg")),
            _ => None,
        };
        if let Some(b) = own {
            return Ok(Some(Cow::Borrowed(b)));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Assets.list(path)
    }
}

pub(crate) const LOGO_SVG: &str = "icons/vigia-logo.svg";
pub(crate) static ICON_PNG: &[u8] = include_bytes!("../assets/logo/vigia-icon-512.png");
#[cfg(target_os = "macos")]
static STATUS_PNG: &[u8] = include_bytes!("../assets/icons/vigia-status.png");
static FONTS: [&[u8]; 3] = [
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
];

/// Sets the Dock icon at runtime: covers `cargo run` (no bundle) and stale Dock icon caches.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2 macros check a `cargo-clippy` feature
fn set_dock_icon() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    static PNG: &[u8] = include_bytes!("../assets/logo/vigia-icon-1024.png");
    // SAFETY: plain AppKit calls on the main thread with a valid byte buffer that NSData copies.
    unsafe {
        let data: *mut Object = msg_send![class!(NSData), dataWithBytes: PNG.as_ptr() length: PNG.len()];
        let image: *mut Object = msg_send![class!(NSImage), alloc];
        let image: *mut Object = msg_send![image, initWithData: data];
        if image.is_null() {
            return;
        }
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setApplicationIconImage: image];
        let _: () = msg_send![image, release];
    }
}

/// The window and what it should do once it exists.
#[derive(Default)]
struct Handles {
    app: Option<WeakEntity<app::VigiaApp>>,
    window: Option<AnyWindowHandle>,
    pending: Vec<Pending>,
}

#[derive(Clone)]
enum Pending {
    Select(i64),
    About,
    QuickAdd,
    Link(otherwise_link::Link),
}

/// Links from the OS (macOS Apple events arrive without an `App` and maybe before the window).
static INBOUND_LINKS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Makes the OS send `otherwise-vigia://` links here. macOS: the bundle's Info.plist does it.
/// Windows: the per-user registry, written once per executable path (`reg` would flash a
/// console on every start). Linux: only once the menu entry is installed.
fn register_scheme() {
    use vigia_core::links::SCHEME;
    let Ok(exe) = std::env::current_exe() else { return };
    let exe_s = exe.display().to_string();
    if Settings::load().link_registered.as_deref() == Some(exe_s.as_str()) {
        return;
    }
    let done = if cfg!(windows) {
        otherwise_link::register::register_windows(SCHEME, &exe, APP_NAME).is_ok()
    } else if cfg!(target_os = "linux") {
        let installed = otherwise_link::paths::home().join(".local/share/applications").join(vigia_core::links::DESKTOP_FILE);
        installed.exists() && otherwise_link::register::register_linux(vigia_core::links::DESKTOP_FILE, &[SCHEME]).is_ok()
    } else {
        false
    };
    if done {
        Settings::update(|s| s.link_registered = Some(exe_s));
    }
}

impl Global for Handles {}

pub(crate) fn tray_enabled() -> bool {
    cfg!(target_os = "macos") && Settings::load().tray()
}

fn open_main_window(cx: &mut App) {
    if let Some(w) = cx.global::<Handles>().window
        && w.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        cx.activate(true);
        return;
    }
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1360.), px(860.)), cx)),
        titlebar: Some(TitlebarOptions { title: Some(APP_NAME.into()), ..Default::default() }),
        ..Default::default()
    };
    let handle = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| app::VigiaApp::new(window, cx));
        cx.global_mut::<Handles>().app = Some(view.downgrade());
        cx.new(|cx| Root::new(view, window, cx))
    });
    if let Ok(h) = handle {
        cx.global_mut::<Handles>().window = Some(h.into());
    }
    cx.activate(true);
    #[cfg(target_os = "macos")]
    macos::activate();
}

/// Deliver menu-bar clicks and queued requests to the window, opening it if needed.
fn pump(cx: &mut App) {
    #[cfg(target_os = "macos")]
    for e in macos::take_events() {
        match e {
            macos::Event::OpenWindow => open_main_window(cx),
            macos::Event::OpenItem(id) => cx.global_mut::<Handles>().pending.push(Pending::Select(id)),
            macos::Event::QuickAdd => cx.global_mut::<Handles>().pending.push(Pending::QuickAdd),
            macos::Event::About => cx.global_mut::<Handles>().pending.push(Pending::About),
            macos::Event::Quit => cx.quit(),
        }
    }
    let links = std::mem::take(&mut *INBOUND_LINKS.lock().unwrap_or_else(|e| e.into_inner()));
    for l in links.iter().filter_map(|u| otherwise_link::Link::parse(u).ok()) {
        cx.global_mut::<Handles>().pending.push(Pending::Link(l));
    }
    if cx.global::<Handles>().pending.is_empty() {
        return;
    }
    let alive = cx.global::<Handles>().app.as_ref().and_then(|w| w.upgrade());
    let Some(view) = alive else {
        open_main_window(cx);
        return;
    };
    let pending = std::mem::take(&mut cx.global_mut::<Handles>().pending);
    if let Some(w) = cx.global::<Handles>().window {
        let _ = w.update(cx, |_, window, cx| {
            window.activate_window();
            view.update(cx, |this, cx| {
                for p in pending {
                    match p {
                        Pending::Select(id) => this.select_and_show(id, cx),
                        Pending::About => this.show_about(cx),
                        Pending::QuickAdd => this.focus_quick(window, cx),
                        Pending::Link(l) => this.open_link(&l, window, cx),
                    }
                }
            });
        });
    }
    cx.activate(true);
}

/// The status item mirrors the next deadlines.
pub(crate) fn sync_tray(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        if !tray_enabled() {
            macos::hide();
            return;
        }
        let d = state::data(cx);
        let d = d.read(cx);
        let today = state::Data::today();
        let counts = vigia_core::view::counts(&d.items, today);
        let items = vigia_core::view::next(&d.items, today, 8).into_iter().map(|i| (i.id, format!("{} — {}", i.title(), views::days_text(i.days_left(today))))).collect(); // i18n-ok: separator
        let tray = macos::Tray {
            badge: counts.attention(),
            tooltip: if counts.attention() == 0 { i18n::t!("app.tray.tooltip_ok") } else { i18n::tn!("app.tray.tooltip", counts.attention()) },
            header: i18n::t!("app.tray.header"),
            items,
            empty: i18n::t!("app.tray.empty"),
            open: i18n::t!("app.tray.open", app = APP_NAME),
            add: i18n::t!("app.quick.menu"),
            about: i18n::t!("app.about.menu", app = APP_NAME),
            quit: i18n::t!("app.menu.quit", app = APP_NAME),
        };
        macos::show(STATUS_PNG, &tray);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
}

/// Every half minute (`n` counts ticks): reminders due today (after the chosen hour), the menu
/// bar; at start and then every hour, the daily network checks that are due.
fn tick(cx: &mut App, n: u64) {
    let d = state::data(cx);
    let hour: u32 = chrono::Timelike::hour(&chrono::Local::now());
    if hour >= Settings::load().remind_hour() {
        let notes = d.update(cx, |d, cx| d.tick_reminders(cx));
        let on = Settings::load().notify();
        for (_, title, body) in &notes {
            if on {
                system::notify(title, body);
            }
        }
        if !notes.is_empty()
            && let Some(view) = cx.global::<Handles>().app.as_ref().and_then(|w| w.upgrade())
        {
            let text = notes.iter().map(|(_, t, b)| format!("{t}: {b}")).collect::<Vec<_>>().join(" · "); // i18n-ok: separators
            view.update(cx, |this, cx| this.toast(text, cx));
        }
    }
    if n.is_multiple_of(120) {
        d.update(cx, |d, cx| d.run_due_checks(cx));
    }
    d.update(cx, |d, cx| d.heartbeat(cx));
    // the Today feed follows the date; the shared registry after edits and every 5 minutes
    d.update(cx, |d, cx| {
        d.publish_today();
        if d.sync_dirty || n.is_multiple_of(10) {
            d.sync_shared(cx);
        }
    });
    sync_tray(cx);
}

struct Opts {
    db: Option<PathBuf>,
    demo: bool,
    background: bool,
    /// Linux / Windows hand a deep link over as an argument.
    link: Option<otherwise_link::Link>,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { db: None, demo: false, background: false, link: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => o.db = Some(PathBuf::from(it.next().ok_or_else(|| i18n::t!("app.cli.usage"))?)),
            "--demo" => o.demo = true,
            "--background" => o.background = true,
            s if s.starts_with("-psn_") => {} // old macOS Finder launch argument
            s if s.contains("://") => o.link = Some(otherwise_link::Link::parse(s).map_err(|_| i18n::t!("app.cli.usage"))?),
            _ => return Err(i18n::t!("app.cli.usage")),
        }
    }
    Ok(o)
}

fn main() {
    let settings = Settings::load();
    i18n::set_locale(app::initial_language(&settings));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", i18n::t!("app.cli.usage"));
        return;
    }
    let opts = match parse_opts(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let temporary = opts.db.is_some();
    let Some(db_path) = vigia_core::paths::resolve_db(opts.db.as_deref()) else {
        eprintln!("{}", i18n::t!("app.db.no_dir"));
        std::process::exit(1);
    };
    let mut data = state::Data::open(db_path, temporary);
    // 0.2.1: the default moved to the Otherwise data folder; ask once (never for --db / VIGIA_DB)
    if !temporary && std::env::var_os("VIGIA_DB").is_none_or(|v| v.is_empty()) {
        data.migration = vigia_core::paths::pending_migration(&settings, vigia_core::paths::legacy_default_db().as_deref(), vigia_core::paths::default_db().as_deref());
    }
    // demo items only into a database given with --db, and only when it's empty
    if opts.demo && temporary && data.items.is_empty()
        && let Some(s) = &data.store
    {
        if let Err(e) = vigia_core::demo::seed(s, state::Data::today()) {
            eprintln!("demo: {e}");
        }
        data.reload();
    }
    let background = opts.background && tray_enabled() && opts.link.is_none();
    let first_link = opts.link.clone();
    std::thread::spawn(register_scheme);
    let application = gpui_platform::application().with_assets(AppAssets);
    application.on_open_urls(|urls| INBOUND_LINKS.lock().unwrap_or_else(|e| e.into_inner()).extend(urls));
    application.on_reopen(open_main_window);
    application.run(move |cx| {
        if let Err(e) = cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()) {
            eprintln!("fonts: {e}");
        }
        #[cfg(target_os = "macos")]
        set_dock_icon();
        gpui_component::init(cx);
        app::sync_theme(None, cx);
        app::bind_keys(cx);
        let data = cx.new(|_| data);
        cx.observe(&data, |_, cx| sync_tray(cx)).detach();
        cx.set_global(state::Shared(data));
        cx.set_global(Handles { pending: first_link.into_iter().map(Pending::Link).collect(), ..Default::default() });
        cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &OpenWebsite, cx| cx.open_url(COMPANY_URL));
        cx.on_action(|_: &OpenVcity, cx| cx.open_url(VCITY_URL));
        cx.on_action(|_: &app::NewItem, cx| open_main_window(cx));
        gpui_component::set_locale(i18n::locale());
        set_app_menus(cx);
        // with the menu-bar item, closing the window keeps Vigía (and its reminders) running
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() && !tray_enabled() {
                cx.quit();
            }
        })
        .detach();
        if !background {
            open_main_window(cx);
        }
        sync_tray(cx);
        cx.spawn(async move |cx| {
            // menu-bar clicks every 250 ms, a tick every 30 s (120 × 250 ms)
            let mut n: u64 = 0;
            loop {
                if n.is_multiple_of(120) {
                    cx.update(|cx| tick(cx, n / 120));
                }
                cx.background_executor().timer(Duration::from_millis(250)).await;
                cx.update(pump);
                n += 1;
            }
        })
        .detach();
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn options() {
        let p = |a: &[&str]| super::parse_opts(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let o = p(&["--db", "/tmp/demo.db", "--demo"]).unwrap();
        assert_eq!(o.db.as_deref(), Some(std::path::Path::new("/tmp/demo.db")));
        assert!(o.demo && !o.background);
        assert!(p(&["--background"]).unwrap().background);
        assert!(p(&["--db"]).is_err());
        assert!(p(&["--nope"]).is_err());
        let o = p(&["otherwise-vigia://item/42"]).unwrap();
        assert_eq!(o.link.map(|l| l.to_url()), Some("otherwise://vigia/item/42".to_string()));
        assert!(p(&["otherwise://"]).is_err());
    }
}
