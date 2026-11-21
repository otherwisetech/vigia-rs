//! Where the database lives, from the user's side: File → Save As… / Move database to… / Show in
//! Finder / Open another… / Open Recent, the "open on another computer" notice, the missing
//! recent file ("Locate…"), and the one-time question of 0.2.1 (move the database to the
//! Otherwise data folder). The file work itself is in `state.rs` + vigia-core `db.rs`, on
//! otherwise-link's verified copies.
use crate::app::VigiaApp;
use crate::dialogs::{Dialog, Parts};
use crate::theme::Tokens;
use gpui::*;
use std::path::{Path, PathBuf};
use vigia_core::paths::Migration;
use vigia_core::settings::Settings;

pub fn reveal_label() -> String {
    if cfg!(target_os = "macos") {
        i18n::t!("app.menu.reveal_macos")
    } else if cfg!(windows) {
        i18n::t!("app.menu.reveal_windows")
    } else {
        i18n::t!("app.menu.reveal_linux")
    }
}

/// `vigia.db — ~/Dropbox/Vigía`, or the missing marker.
pub fn recent_label(p: &Path) -> String {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
    if p.exists() { i18n::t!("app.recent.item", name = name, dir = dir) } else { i18n::t!("app.recent.missing", name = name, dir = dir) }
}

pub fn recent_menu() -> Vec<MenuItem> {
    use crate::app::*;
    let list = Settings::load().recent_dbs();
    let mut items: Vec<MenuItem> = list
        .iter()
        .take(vigia_core::paths::RECENT_MAX)
        .enumerate()
        .map(|(i, p)| {
            let name = recent_label(Path::new(p));
            match i {
                0 => MenuItem::action(name, OpenRecent0),
                1 => MenuItem::action(name, OpenRecent1),
                2 => MenuItem::action(name, OpenRecent2),
                3 => MenuItem::action(name, OpenRecent3),
                4 => MenuItem::action(name, OpenRecent4),
                5 => MenuItem::action(name, OpenRecent5),
                6 => MenuItem::action(name, OpenRecent6),
                _ => MenuItem::action(name, OpenRecent7),
            }
        })
        .collect();
    if items.is_empty() {
        items.push(MenuItem::Action { name: i18n::t!("app.recent.none").into(), action: Box::new(OpenDb), os_action: None, checked: false, disabled: true });
    } else {
        items.push(MenuItem::separator());
        items.push(MenuItem::action(i18n::t!("app.recent.clear"), ClearRecent));
    }
    items
}

fn set_recent(f: impl FnOnce(&mut Vec<String>)) {
    Settings::update(|s| {
        let mut r = s.recent_dbs();
        f(&mut r);
        s.recent_dbs = Some(r);
    });
}

type DialogAction = fn(&mut VigiaApp, &mut Window, &mut Context<VigiaApp>);

impl VigiaApp {
    fn pick_new_path(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>, then: impl FnOnce(&mut Self, PathBuf, &mut Window, &mut Context<Self>) + 'static) {
        let dir = self.data.read(cx).path.parent().map(Path::to_path_buf).filter(|d| d.is_dir()).unwrap_or_else(crate::app::documents_dir);
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(p))) = rx.await {
                this.update_in(cx, |this, window, cx| then(this, p, window, cx)).ok();
            }
        })
        .detach();
    }

    pub fn save_db_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.data.read(cx).store.is_none() {
            return self.toast(i18n::t!("app.db.none"), cx);
        }
        let stem = self.data.read(cx).path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "vigia".into());
        self.pick_new_path(i18n::t!("app.db.copy_name", name = stem), window, cx, |this, p, window, cx| {
            let p = if p.extension().is_none() { p.with_extension(vigia_core::db::EXTENSION) } else { p };
            if p.exists() {
                return this.toast(i18n::t!("app.db.exists"), cx);
            }
            match this.data.update(cx, |d, cx| d.save_as(p.clone(), cx)) {
                Ok(()) => {
                    this.toast(i18n::t!("app.db.saved_as", path = p.display()), cx);
                    let t = crate::app::window_title(this.data.read(cx));
                    window.set_window_title(&t);
                    crate::set_app_menus(cx);
                }
                Err(e) => this.toast(e, cx),
            }
        });
    }

    pub fn reveal_db(&mut self, cx: &mut Context<Self>) {
        let p = self.data.read(cx).path.clone();
        if p.exists() {
            cx.reveal_path(&p);
        } else if let Some(d) = p.parent() {
            cx.reveal_path(d);
        }
    }

    pub fn clear_recent(&mut self, cx: &mut Context<Self>) {
        set_recent(|r| r.clear());
        crate::set_app_menus(cx);
    }

    pub fn open_recent(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = Settings::load().recent_dbs().get(i).map(PathBuf::from) else { return };
        if p.exists() {
            self.switch_db(p, window, cx);
        } else {
            self.dialog = Some(Dialog::Missing { path: p });
            cx.notify();
        }
    }

    fn locate_recent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Missing { path: old }) = self.dialog.take() else { return };
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(i18n::t!("app.recent.locate").into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(p) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| {
                    let o = old.display().to_string();
                    set_recent(|r| r.retain(|x| *x != o));
                    this.switch_db(p, window, cx);
                    crate::set_app_menus(cx);
                })
                .ok();
            }
        })
        .detach();
    }

    fn forget_recent(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Missing { path }) = self.dialog.take() else { return };
        let p = path.display().to_string();
        set_recent(|r| r.retain(|x| *x != p));
        crate::set_app_menus(cx);
        cx.notify();
    }

    fn migrate_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Migrate(m)) = self.dialog.take() else { return };
        self.data.update(cx, |d, _| d.migration = None);
        if m.both {
            Settings::update(|s| s.storage_migration = Some("use-new".into()));
            self.switch_db(m.to, window, cx);
            return;
        }
        let r = self.data.update(cx, |d, cx| if d.path == m.from { d.move_to(m.to.clone(), cx) } else { Err(i18n::t!("app.db.none")) });
        match r {
            Ok(()) => {
                Settings::update(|s| s.storage_migration = Some("moved".into()));
                self.toast(i18n::t!("app.db.moved", path = m.to.display()), cx);
                let t = crate::app::window_title(self.data.read(cx));
                window.set_window_title(&t);
                crate::set_app_menus(cx);
            }
            // nothing changed: ask again next time
            Err(e) => self.toast(e, cx),
        }
        cx.notify();
    }

    fn migrate_keep(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Migrate(m)) = self.dialog.take() else { return };
        self.data.update(cx, |d, _| d.migration = None);
        let from = m.from.display().to_string();
        Settings::update(|s| {
            s.storage_migration = Some("kept".into());
            s.db_path = Some(from);
        });
        cx.notify();
    }

    fn take_over_locked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Locked { path, .. }) = self.dialog.take() else { return };
        self.switch_db_forced(path, true, window, cx);
    }

    /// Title, body, OK; for the dialogs of this file.
    pub(crate) fn storage_dialog(&self, d: &Dialog, t: &Tokens) -> Option<Parts> {
        let text = |s: String| div().text_size(px(13.)).line_height(px(19.)).text_color(t.ink_muted).child(s);
        let mono = |s: String| div().p(px(8.)).rounded(px(6.)).bg(t.canvas).text_size(px(12.)).child(s);
        Some(match d {
            Dialog::Migrate(Migration { from, to, both }) => {
                let b = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(text(if *both { i18n::t!("app.migrate.body_both") } else { i18n::t!("app.migrate.body") }))
                    .child(text(i18n::t!("app.migrate.from")))
                    .child(mono(from.display().to_string()))
                    .child(text(i18n::t!("app.migrate.to")))
                    .child(mono(to.display().to_string()))
                    .child(text(if *both { i18n::t!("app.migrate.hint_both") } else { i18n::t!("app.migrate.hint") }));
                let ok = if *both { i18n::t!("app.migrate.use_new") } else { i18n::t!("app.migrate.move") };
                (i18n::t!("app.migrate.title"), b.into_any_element(), ok, false, |this, window, cx| this.migrate_accept(window, cx))
            }
            Dialog::Locked { path, info } => {
                let b = div().flex().flex_col().gap(px(8.)).child(mono(path.display().to_string())).child(text(crate::state::locked_message(info))).child(text(i18n::t!("app.db.locked_hint")));
                (i18n::t!("app.db.locked_title"), b.into_any_element(), i18n::t!("app.db.take_over"), true, |this, window, cx| this.take_over_locked(window, cx))
            }
            Dialog::Missing { path } => {
                let b = div().flex().flex_col().gap(px(8.)).child(mono(path.display().to_string())).child(text(i18n::t!("app.recent.missing_body")));
                (i18n::t!("app.recent.missing_title"), b.into_any_element(), i18n::t!("app.recent.locate"), false, |this, window, cx| this.locate_recent(window, cx))
            }
            _ => return None,
        })
    }

    /// A second choice next to OK.
    pub(crate) fn storage_dialog_alt(d: &Dialog) -> Option<(String, DialogAction)> {
        match d {
            Dialog::Migrate(m) => Some((if m.both { i18n::t!("app.migrate.keep_both") } else { i18n::t!("app.migrate.keep") }, |this, window, cx| this.migrate_keep(window, cx))),
            Dialog::Missing { .. } => Some((i18n::t!("app.recent.forget"), |this, _, cx| this.forget_recent(cx))),
            _ => None,
        }
    }

    /// One line about where the file sits: the cloud provider and conflicted copies.
    pub(crate) fn storage_note(&self, cx: &App) -> Option<(String, bool)> {
        let d = self.data.read(cx);
        let st = d.storage.as_ref()?;
        if !st.conflicts.is_empty() {
            return Some((i18n::tn!("app.db.conflicts", st.conflicts.len()), true));
        }
        if let Some(c) = &st.cloud {
            return Some((i18n::t!("app.db.in_cloud", provider = c.provider.name()), false));
        }
        let default = vigia_core::paths::default_db();
        (default.as_deref() == Some(d.path.as_path())).then(|| (i18n::t!("app.db.in_data_folder"), false))
    }
}
