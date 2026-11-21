//! `settings.json` in the data folder, shared by the app and the CLI. Unknown keys are kept
//! out; missing ones take their defaults.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: Option<String>,
    /// The database last opened (absent: the default one in the data folder).
    pub db_path: Option<String>,
    /// `false`: don't fetch the repo's star count from api.github.com.
    pub github_stars: Option<bool>,
    /// `false`: reminders stay in the app, no system notification.
    pub notify: Option<bool>,
    /// `false`: no menu-bar item; closing the window quits.
    pub tray: Option<bool>,
    /// `false`: no daily automatic checks (RDAP / TLS) even for items that opted in.
    pub auto_checks: Option<bool>,
    /// Keep an .ics file up to date for calendar subscriptions.
    pub ics_sync: Option<bool>,
    pub ics_path: Option<String>,
    /// dashboard / timeline / calendar / list
    pub view: Option<String>,
    /// Hour of the day (0–23) after which the daily reminders are sent.
    pub remind_hour: Option<u32>,
    /// Public holidays shown in the calendar and timeline (`co`, `us`, `us-wy`, …).
    pub holidays: Option<Vec<String>>,
    /// `false`: don't publish deadlines to Otherwise Apps' Today view.
    pub today_feed: Option<bool>,
    /// The executable `otherwise-vigia://` was last registered for (Windows / Linux).
    pub link_registered: Option<String>,
    /// The answer to "move the database to the Otherwise data folder?" (0.2.1): `moved`, `kept`
    /// or `use-new`. Absent: not asked yet.
    pub storage_migration: Option<String>,
    /// Databases opened recently, most recent first.
    pub recent_dbs: Option<Vec<String>>,
}

impl Settings {
    fn file() -> Option<PathBuf> {
        crate::paths::data_dir().map(|d| d.join("settings.json"))
    }

    pub fn load() -> Settings {
        Self::file().and_then(|f| std::fs::read_to_string(f).ok()).map(|s| Settings::parse(&s)).unwrap_or_default()
    }

    pub fn parse(s: &str) -> Settings {
        serde_json::from_str(s).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into()) + "\n"
    }

    pub fn save(&self) {
        let Some(f) = Self::file() else { return };
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, self.to_json());
    }

    /// Load, change, save: another process (the CLI) may have saved in between.
    pub fn update(f: impl FnOnce(&mut Settings)) -> Settings {
        let mut s = Settings::load();
        f(&mut s);
        s.save();
        s
    }

    pub fn github_stars(&self) -> bool {
        self.github_stars != Some(false)
    }
    pub fn notify(&self) -> bool {
        self.notify != Some(false)
    }
    pub fn tray(&self) -> bool {
        self.tray != Some(false)
    }
    pub fn auto_checks(&self) -> bool {
        self.auto_checks != Some(false)
    }
    pub fn today_feed(&self) -> bool {
        self.today_feed != Some(false)
    }
    pub fn ics_sync(&self) -> bool {
        self.ics_sync == Some(true)
    }
    pub fn holiday_countries(&self) -> Vec<crate::holidays::Country> {
        self.holidays.iter().flatten().filter_map(|c| crate::holidays::Country::from_id(c)).collect()
    }

    pub fn recent_dbs(&self) -> Vec<String> {
        self.recent_dbs.clone().unwrap_or_default()
    }

    pub fn remind_hour(&self) -> u32 {
        self.remind_hour.unwrap_or(9).min(23)
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn roundtrip_and_defaults() {
        let s = Settings { language: Some("pt".into()), db_path: Some("/x/vigia.db".into()), github_stars: Some(false), notify: Some(false), tray: Some(false), auto_checks: Some(false), ics_sync: Some(true), ics_path: Some("/x/v.ics".into()), view: Some("timeline".into()), remind_hour: Some(30), holidays: Some(vec!["co".into(), "mars".into(), "us-wy".into()]), today_feed: Some(false), link_registered: Some("/x/vigia-app".into()), storage_migration: Some("moved".into()), recent_dbs: Some(vec!["/x/a.db".into()]) };
        assert_eq!(Settings::parse(&s.to_json()), s);
        assert!(!s.github_stars() && !s.notify() && !s.tray() && !s.auto_checks() && s.ics_sync() && !s.today_feed());
        assert_eq!(s.remind_hour(), 23);
        assert_eq!(s.holiday_countries(), vec![crate::holidays::Country::Co, crate::holidays::Country::UsWy]);
        assert!(Settings::default().holiday_countries().is_empty());
        let d = Settings::default();
        assert!(d.github_stars() && d.notify() && d.tray() && d.auto_checks() && !d.ics_sync() && d.today_feed());
        assert_eq!(d.remind_hour(), 9);
        assert_eq!(Settings::parse("garbage"), d);
        assert_eq!(Settings::parse(r#"{"language":"fr","unknown":1}"#).language.as_deref(), Some("fr"));
    }
}
