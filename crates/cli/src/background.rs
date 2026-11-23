//! `vigia background manifest | run <job>`: Otherwise Helper's jobs (logic in
//! `vigia_core::background`). One report line on stdout; notifications go to
//! `<data root>/vigia/outbox/`. Only the user's own database (never `--db` / `VIGIA_DB`).
use otherwise_link::background::{Job, Label, Manifest, Notification, Report};
use otherwise_link::today::Severity;
use otherwise_link::Root;
use vigia_core::background::{self as bg, Access, Change};

fn label(key: &str) -> Label {
    let mut l = Label::en(key, &i18n::tr_in("en", key));
    for lang in ["es", "pt", "fr"] {
        l = l.with(lang, &i18n::tr_in(lang, key));
    }
    l
}

/// What the bundle ships as `Contents/Resources/otherwise-background.json`.
pub fn manifest() -> Manifest {
    let mut reminders = Job::every(bg::REMINDERS, label("cli.bg.reminders_label"), "15m", &["background", "run", bg::REMINDERS]); // i18n-ok: arguments
    reminders.exe = Some("vigia".into()); // i18n-ok: binary name
    reminders.while_open = false;
    reminders.timeout_secs = Some(60);
    reminders.memory_mb = Some(200);
    let mut checks = Job::every(bg::CHECKS, label("cli.bg.checks_label"), "1h", &["background", "run", bg::CHECKS]); // i18n-ok: arguments
    checks.exe = Some("vigia".into()); // i18n-ok: binary name
    checks.while_open = false;
    checks.network = true;
    checks.timeout_secs = Some(300);
    checks.memory_mb = Some(300);
    Manifest { schema: 1, app: vigia_core::sync::APP.into(), jobs: vec![reminders, checks] }
}

fn notify(root: &Root, n: &Notification) {
    let _ = otherwise_link::background::notify(root, vigia_core::sync::APP, n);
}

pub fn run(args: &[String]) -> i32 {
    if let Some(l) = std::env::var(otherwise_link::background::ENV_LANG).ok().filter(|l| !l.is_empty()) {
        i18n::set_locale(&l);
    }
    let (report, code) = match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        (Some("manifest"), _) => {
            print!("{}", manifest().to_json());
            return 0;
        }
        (Some("run"), Some(job @ (bg::REMINDERS | bg::CHECKS))) => job_run(job),
        _ => (Report::status(i18n::t!("cli.bg.unknown_job")), 2),
    };
    println!("{}", report.line());
    code
}

fn job_run(job: &str) -> (Report, i32) {
    let Some(db) = vigia_core::paths::resolve_db(None) else { return (Report::idle(), 0) };
    if !db.is_file() {
        return (Report::idle(), 0);
    }
    let _lock = match bg::access(&db) {
        Access::Mine(l) => l,
        Access::AppOpen => return (Report::status(i18n::t!("cli.bg.app_open")), 0),
        Access::Elsewhere(host) => return (Report::status(i18n::t!("cli.bg.elsewhere", host = host)), 0),
    };
    let mut store = match vigia_core::db::Store::open(&db) {
        Ok(s) => s,
        Err(e) => return (Report::status(i18n::t!("cli.bg.failed", error = e)), 1),
    };
    let root = Root::system();
    let res = if job == bg::REMINDERS { reminders(&mut store, &root) } else { checks(&store, &root) };
    if let Ok(r) = &res
        && !r.idle
    {
        crate::publish_today(&store);
    }
    match res {
        Ok(r) => (r, 0),
        Err(e) => (Report::status(i18n::t!("cli.bg.failed", error = e)), 1),
    }
}

fn reminders(store: &mut vigia_core::db::Store, root: &Root) -> Result<Report, vigia_core::db::StoreError> {
    if store.items()?.is_empty() {
        return Ok(Report::idle());
    }
    let notes = bg::reminders(store, vigia_core::date::today())?;
    for n in &notes {
        notify(root, &Notification::new(n.id.clone(), n.title.clone()).body(n.body.clone()).link(&vigia_core::links::item_link(n.item)));
    }
    Ok(Report::status(if notes.is_empty() { i18n::t!("cli.bg.no_reminders") } else { i18n::tn!("cli.bg.reminded", notes.len()) }))
}

fn checks(store: &vigia_core::db::Store, root: &Root) -> Result<Report, vigia_core::db::StoreError> {
    let items = store.items()?;
    if !items.iter().any(|i| i.auto_check && i.kind.checkable()) {
        return Ok(Report::idle());
    }
    let now = vigia_core::db::now_secs();
    let (_, changes) = bg::checks(store, &vigia_core::checks::Net, now)?;
    for c in &changes {
        let n = match c {
            Change::Down { item, title, why } => Notification::new(format!("down-{item}-{now}"), i18n::t!("cli.bg.down", item = title)).body(why.clone()).severity(Severity::High).link(&vigia_core::links::item_link(*item)),
            Change::Recovered { item, title } => Notification::new(format!("up-{item}-{now}"), i18n::t!("cli.bg.recovered", item = title)).link(&vigia_core::links::item_link(*item)),
            Change::Renewed { item, title, expires } => Notification::new(format!("renewed-{item}-{}", vigia_core::date::iso(*expires)), i18n::t!("cli.bg.renewed", item = title, date = vigia_core::date::iso(*expires))).link(&vigia_core::links::item_link(*item)),
        };
        notify(root, &n);
    }
    let failing = store.metas("bg.check.")?.iter().filter(|(_, v)| v == "fail").count();
    Ok(Report::status(if failing > 0 { i18n::tn!("cli.bg.failing", failing) } else { i18n::t!("cli.bg.all_ok") }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_is_valid_and_translated() {
        let m = super::manifest();
        let back = otherwise_link::background::Manifest::parse(&m.to_json(), "vigia").unwrap();
        assert_eq!(back.jobs.len(), 2);
        assert!(back.jobs.iter().all(|j| j.exe.as_deref() == Some("vigia") && !j.while_open));
        assert!(back.jobs[1].network);
        assert_ne!(back.jobs[0].label.get("es"), back.jobs[0].label.get("en"));
    }
}
