//! Where Vigía keeps its files: the data folder (default database, settings, caches) shared by
//! the app and the `vigia` CLI. The database itself can be moved anywhere (Settings → Database
//! location); the last one used is remembered in `settings.json`.
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

/// `<data dir>/vigia.db`.
pub fn default_db() -> Option<PathBuf> {
    data_dir().map(|d| d.join("vigia.db"))
}

/// The database to use: an explicit path (`--db`), else `VIGIA_DB`, else the last one opened,
/// else the default.
pub fn resolve_db(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("VIGIA_DB").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    crate::settings::Settings::load().db_path.map(PathBuf::from).or_else(default_db)
}

/// Where the OCR models are downloaded (once, on request).
pub fn ocr_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("ocr-models"))
}

/// The calendar file kept up to date for calendar apps to subscribe to.
pub fn default_ics() -> Option<PathBuf> {
    data_dir().map(|d| d.join("vigia.ics"))
}
