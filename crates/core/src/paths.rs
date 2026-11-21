//! Where Vigía keeps its files. Machine-local things (settings, OCR models, caches, the .ics
//! file) live in the data folder (`~/Library/Application Support/Vigia`…), shared by the app
//! and the `vigia` CLI. The database defaults to the Otherwise data root
//! (`<root>/vigia/vigia.db`, `~/.otherwise/vigia/vigia.db` unless the user moved the root in
//! Otherwise Apps) and can be moved anywhere; a custom choice is remembered in `settings.json`.
//! Before 0.2.1 the default was `<data folder>/vigia.db` ([`legacy_default_db`]); see
//! [`pending_migration`].
use std::path::{Path, PathBuf};

pub fn data_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("VIGIA_DATA_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(d));
    }
    if cfg!(windows) {
        return std::env::var_os("APPDATA").filter(|v| !v.is_empty()).map(|d| PathBuf::from(d).join("Vigia"));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/Vigia"));
    }
    std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| home.map(|h| h.join(".local/share"))).map(|d| d.join("vigia"))
}

pub const DB_FILE: &str = "vigia.db";

/// `<Otherwise data root>/vigia/vigia.db` (with `VIGIA_DATA_DIR` set: `<that>/vigia.db`, for
/// tests and portable setups).
pub fn default_db() -> Option<PathBuf> {
    if std::env::var_os("VIGIA_DATA_DIR").is_some_and(|v| !v.is_empty()) {
        return data_dir().map(|d| d.join(DB_FILE));
    }
    Some(otherwise_link::paths::app_dir(crate::sync::APP).join(DB_FILE))
}

/// The default before 0.2.1: `<data dir>/vigia.db`.
pub fn legacy_default_db() -> Option<PathBuf> {
    data_dir().map(|d| d.join(DB_FILE))
}

/// The user's database is the default one (nothing remembered, or the default itself).
pub fn uses_default(s: &crate::settings::Settings, default: Option<&Path>) -> bool {
    match s.db_path.as_deref() {
        None => true,
        Some(p) => default.is_some_and(|d| Path::new(p) == d),
    }
}

/// The question to ask on the first launch of 0.2.1+.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// The old default, which exists.
    pub from: PathBuf,
    /// The new default.
    pub to: PathBuf,
    /// The new default exists too (another computer put it in a shared data root): offer to use
    /// either, never to move one over the other.
    pub both: bool,
}

/// Whether to offer moving the database from the old default to the new one: only when the user
/// never chose a place (or chose the old default itself), hasn't answered yet, and the old file
/// exists.
pub fn pending_migration(s: &crate::settings::Settings, legacy: Option<&Path>, new_default: Option<&Path>) -> Option<Migration> {
    let (legacy, to) = (legacy?, new_default?);
    if s.storage_migration.is_some() || legacy == to || !legacy.is_file() {
        return None;
    }
    let default_choice = match s.db_path.as_deref() {
        None => true,
        Some(p) => Path::new(p) == legacy || Path::new(p) == to,
    };
    default_choice.then(|| Migration { from: legacy.to_path_buf(), to: to.to_path_buf(), both: to.exists() })
}

/// Most recent first, at most [`RECENT_MAX`], without duplicates.
pub const RECENT_MAX: usize = 8;

pub fn push_recent(list: &mut Vec<String>, path: &Path) {
    let p = path.display().to_string();
    list.retain(|x| *x != p);
    list.insert(0, p);
    list.truncate(RECENT_MAX);
}

/// The database to use: an explicit path (`--db`), else `VIGIA_DB`, else the last one opened,
/// else the default (a migration still waiting for the user's answer keeps the old default).
pub fn resolve_db(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("VIGIA_DB").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let s = crate::settings::Settings::load();
    let (legacy, new) = (legacy_default_db(), default_db());
    if let Some(m) = pending_migration(&s, legacy.as_deref(), new.as_deref()) {
        return Some(m.from);
    }
    s.db_path.map(PathBuf::from).or(new)
}

/// Where the OCR models are downloaded (once, on request).
pub fn ocr_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("ocr-models"))
}

/// The calendar file kept up to date for calendar apps to subscribe to.
pub fn default_ics() -> Option<PathBuf> {
    data_dir().map(|d| d.join("vigia.ics"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn migration_is_offered_only_for_the_old_default() {
        let t = tempfile::tempdir().unwrap();
        let legacy = t.path().join("Vigia").join(DB_FILE);
        let new = t.path().join(".otherwise").join("vigia").join(DB_FILE);
        let s = Settings::default();
        // nothing at the old place: nothing to ask
        assert_eq!(pending_migration(&s, Some(&legacy), Some(&new)), None);
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, b"db").unwrap();
        assert_eq!(pending_migration(&s, Some(&legacy), Some(&new)), Some(Migration { from: legacy.clone(), to: new.clone(), both: false }));
        // remembered as the old default itself: still the default
        let s2 = Settings { db_path: Some(legacy.display().to_string()), ..Default::default() };
        assert!(pending_migration(&s2, Some(&legacy), Some(&new)).is_some());
        // a custom place, or already answered: never
        let custom = Settings { db_path: Some("/Users/x/Dropbox/v.db".into()), ..Default::default() };
        assert_eq!(pending_migration(&custom, Some(&legacy), Some(&new)), None);
        for a in ["moved", "kept"] {
            let s = Settings { storage_migration: Some(a.into()), ..Default::default() };
            assert_eq!(pending_migration(&s, Some(&legacy), Some(&new)), None);
        }
        // both exist: offered as a choice, not a move
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        std::fs::write(&new, b"db2").unwrap();
        assert!(pending_migration(&s, Some(&legacy), Some(&new)).unwrap().both);
        assert!(uses_default(&s, Some(&new)));
        assert!(uses_default(&Settings { db_path: Some(new.display().to_string()), ..Default::default() }, Some(&new)));
        assert!(!uses_default(&custom, Some(&new)));
    }

    #[test]
    fn recent_list() {
        let mut l = Vec::new();
        for i in 0..10 {
            push_recent(&mut l, Path::new(&format!("/d/{i}.db")));
        }
        push_recent(&mut l, Path::new("/d/5.db"));
        assert_eq!(l.len(), RECENT_MAX);
        assert_eq!(l[0], "/d/5.db");
        assert_eq!(l.iter().filter(|x| *x == "/d/5.db").count(), 1);
    }
}
