//! The database: one SQLite file (by default `vigia.db` in the app's data folder, movable to a
//! synced folder). Plain rollback journal (no WAL) so the single file syncs cleanly; integrity
//! check and application id verified on open; schema migrated forward with `PRAGMA
//! user_version`. Attachments live in `vigia-files/` next to the file, referenced by relative
//! path.
use crate::date::{self, Date};
use crate::model::*;
use crate::reminder::Reminder;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// `VGIA` in ASCII, in the SQLite header so other tools can tell what the file is.
pub const APPLICATION_ID: i32 = 0x5647_4941;
pub const EXTENSION: &str = "db";
pub const FILES_DIR: &str = "vigia-files";

/// Schema steps; `MIGRATIONS[i]` takes the file from version `i` to `i + 1`. Never edit a
/// shipped step: add a new one.
pub const MIGRATIONS: &[&str] = &[
    // 1: items, attachments, renewals, reminders sent, meta
    "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
     CREATE TABLE items (
       id INTEGER PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, owner TEXT NOT NULL DEFAULT '',
       category TEXT NOT NULL, issued TEXT, expires TEXT, recurrence TEXT NOT NULL DEFAULT 'none',
       term_months INTEGER, notice_days INTEGER, cost REAL, currency TEXT NOT NULL DEFAULT '',
       provider TEXT NOT NULL DEFAULT '', reference TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '',
       tags TEXT NOT NULL DEFAULT '[]', renewal_url TEXT NOT NULL DEFAULT '', auto_renews INTEGER NOT NULL DEFAULT 0,
       lead_days TEXT NOT NULL DEFAULT '[]', state TEXT NOT NULL DEFAULT 'active', snoozed_until TEXT,
       host TEXT NOT NULL DEFAULT '', auto_check INTEGER NOT NULL DEFAULT 0, last_checked INTEGER,
       check_note TEXT NOT NULL DEFAULT '', created INTEGER NOT NULL DEFAULT 0, updated INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE attachments (
       id INTEGER PRIMARY KEY, item INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
       rel_path TEXT NOT NULL, name TEXT NOT NULL, added INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE renewals (
       id INTEGER PRIMARY KEY, item INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
       on_date TEXT NOT NULL, old_expiry TEXT, new_expiry TEXT, cost REAL, note TEXT NOT NULL DEFAULT '');
     CREATE TABLE reminders_sent (
       item INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE, deadline TEXT NOT NULL,
       lead INTEGER NOT NULL, at INTEGER NOT NULL, PRIMARY KEY (item, deadline, lead));
     CREATE INDEX attachments_item ON attachments(item);
     CREATE INDEX renewals_item ON renewals(item, on_date);",
];

pub fn latest_version() -> i64 {
    MIGRATIONS.len() as i64
}

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    /// The file is a database, but not Vigía's.
    NotVigia,
    /// Made by a newer Vigía.
    TooNew(i64),
    /// `PRAGMA integrity_check` failed.
    Corrupt(String),
    NotFound(ItemId),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sqlite(rusqlite::Error::SqliteFailure(c, _)) if c.code == rusqlite::ErrorCode::NotADatabase => write!(f, "{}", i18n::t!("core.db.not_database")),
            StoreError::Sqlite(e) => write!(f, "{}", i18n::t!("core.db.error", detail = e)),
            StoreError::Io(e) => write!(f, "{}", i18n::t!("core.db.io", detail = e)),
            StoreError::NotVigia => write!(f, "{}", i18n::t!("core.db.not_vigia")),
            StoreError::TooNew(v) => write!(f, "{}", i18n::t!("core.db.too_new", version = v)),
            StoreError::Corrupt(d) => write!(f, "{}", i18n::t!("core.db.corrupt", detail = d)),
            StoreError::NotFound(id) => write!(f, "{}", i18n::t!("core.db.not_found", id = id)),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    conn: Connection,
    path: PathBuf,
}

fn d2s(d: Option<Date>) -> Option<String> {
    d.map(date::iso)
}

fn s2d(s: Option<String>) -> Option<Date> {
    s.as_deref().and_then(date::parse_iso)
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The attachments folder for a database file.
pub fn files_dir(db: &Path) -> PathBuf {
    db.parent().unwrap_or(Path::new(".")).join(FILES_DIR)
}

const ITEM_COLS: &str = "id, kind, name, owner, category, issued, expires, recurrence, term_months, notice_days, cost, currency, provider, reference, notes, tags, renewal_url, auto_renews, lead_days, state, snoozed_until, host, auto_check, last_checked, check_note, created, updated";

fn item_from_row(r: &Row) -> rusqlite::Result<Item> {
    let kind = Kind::from_id(&r.get::<_, String>(1)?).unwrap_or(Kind::Custom);
    Ok(Item {
        id: r.get(0)?,
        kind,
        name: r.get(2)?,
        owner: r.get(3)?,
        category: Category::from_id(&r.get::<_, String>(4)?).unwrap_or(kind.category()),
        issued: s2d(r.get(5)?),
        expires: s2d(r.get(6)?),
        recurrence: Recurrence::parse(&r.get::<_, String>(7)?).unwrap_or_default(),
        term_months: r.get(8)?,
        notice_days: r.get(9)?,
        cost: r.get(10)?,
        currency: r.get(11)?,
        provider: r.get(12)?,
        reference: r.get(13)?,
        notes: r.get(14)?,
        tags: serde_json::from_str(&r.get::<_, String>(15)?).unwrap_or_default(),
        renewal_url: r.get(16)?,
        auto_renews: r.get::<_, i64>(17)? != 0,
        lead_days: serde_json::from_str(&r.get::<_, String>(18)?).unwrap_or_default(),
        state: State::from_id(&r.get::<_, String>(19)?),
        snoozed_until: s2d(r.get(20)?),
        host: r.get(21)?,
        auto_check: r.get::<_, i64>(22)? != 0,
        last_checked: r.get(23)?,
        check_note: r.get(24)?,
        created: r.get(25)?,
        updated: r.get(26)?,
    })
}

impl Store {
    /// Opens the database at `path`, creating it (and its folder) when missing.
    pub fn open_or_create(path: &Path) -> Result<Store> {
        if path.exists() { Store::open(path) } else { Store::create(path) }
    }

    pub fn create(path: &Path) -> Result<Store> {
        if let Some(d) = path.parent()
            && !d.as_os_str().is_empty()
        {
            std::fs::create_dir_all(d)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "application_id", APPLICATION_ID)?;
        let mut s = Store { conn, path: path.to_path_buf() };
        s.configure()?;
        s.migrate()?;
        s.set_meta("created", &now_secs().to_string())?;
        Ok(s)
    }

    /// Integrity check, application id, migrations.
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        let check: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err(StoreError::Corrupt(check));
        }
        let app: i32 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if app != APPLICATION_ID {
            return Err(StoreError::NotVigia);
        }
        if version > latest_version() {
            return Err(StoreError::TooNew(version));
        }
        let mut s = Store { conn, path: path.to_path_buf() };
        s.configure()?;
        s.migrate()?;
        Ok(s)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn files_dir(&self) -> PathBuf {
        files_dir(&self.path)
    }

    fn configure(&mut self) -> Result<()> {
        // a single file (plus a short-lived -journal) plays well with Dropbox / iCloud / Syncthing
        let _: String = self.conn.query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))?;
        self.conn.pragma_update(None, "synchronous", "FULL")?;
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        self.conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(())
    }

    fn migrate(&mut self) -> Result<()> {
        let version: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, step) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(step)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn version(&self) -> Result<i64> {
        Ok(self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute("INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", [key, value])?;
        Ok(())
    }

    // ── items ────────────────────────────────────────────────────────────────────────────────

    pub fn items(&self) -> Result<Vec<Item>> {
        let mut st = self.conn.prepare(&format!("SELECT {ITEM_COLS} FROM items ORDER BY id"))?;
        let v = st.query_map([], item_from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn item(&self, id: ItemId) -> Result<Item> {
        self.conn.query_row(&format!("SELECT {ITEM_COLS} FROM items WHERE id = ?1"), [id], item_from_row).optional()?.ok_or(StoreError::NotFound(id))
    }

    /// Inserts `item` (its id is ignored) and returns the new id.
    pub fn insert(&self, item: &Item) -> Result<ItemId> {
        let now = now_secs();
        self.conn.execute(
            "INSERT INTO items (kind, name, owner, category, issued, expires, recurrence, term_months, notice_days, cost, currency, provider, reference, notes, tags, renewal_url, auto_renews, lead_days, state, snoozed_until, host, auto_check, last_checked, check_note, created, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?25)",
            params![
                item.kind.id(),
                item.name,
                item.owner,
                item.category.id(),
                d2s(item.issued),
                d2s(item.expires),
                item.recurrence.id(),
                item.term_months,
                item.notice_days,
                item.cost,
                item.currency,
                item.provider,
                item.reference,
                item.notes,
                serde_json::to_string(&item.tags).unwrap_or_else(|_| "[]".into()),
                item.renewal_url,
                item.auto_renews as i64,
                serde_json::to_string(&item.lead_days).unwrap_or_else(|_| "[]".into()),
                item.state.id(),
                d2s(item.snoozed_until),
                item.host,
                item.auto_check as i64,
                item.last_checked,
                item.check_note,
                if item.created > 0 { item.created } else { now },
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update(&self, item: &Item) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE items SET kind = ?2, name = ?3, owner = ?4, category = ?5, issued = ?6, expires = ?7, recurrence = ?8, term_months = ?9, notice_days = ?10, cost = ?11, currency = ?12, provider = ?13, reference = ?14, notes = ?15, tags = ?16, renewal_url = ?17, auto_renews = ?18, lead_days = ?19, state = ?20, snoozed_until = ?21, host = ?22, auto_check = ?23, last_checked = ?24, check_note = ?25, updated = ?26 WHERE id = ?1",
            params![
                item.id,
                item.kind.id(),
                item.name,
                item.owner,
                item.category.id(),
                d2s(item.issued),
                d2s(item.expires),
                item.recurrence.id(),
                item.term_months,
                item.notice_days,
                item.cost,
                item.currency,
                item.provider,
                item.reference,
                item.notes,
                serde_json::to_string(&item.tags).unwrap_or_else(|_| "[]".into()),
                item.renewal_url,
                item.auto_renews as i64,
                serde_json::to_string(&item.lead_days).unwrap_or_else(|_| "[]".into()),
                item.state.id(),
                d2s(item.snoozed_until),
                item.host,
                item.auto_check as i64,
                item.last_checked,
                item.check_note,
                now_secs(),
            ],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(item.id));
        }
        Ok(())
    }

    /// Deletes the item, its history and its attachment files.
    pub fn delete(&self, id: ItemId) -> Result<()> {
        for a in self.attachments(id)? {
            let _ = std::fs::remove_file(self.attachment_path(&a));
        }
        let _ = std::fs::remove_dir(self.files_dir().join(id.to_string()));
        self.conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Marks the item renewed: history row, new expiry, back to active, snooze cleared.
    pub fn renew(&mut self, id: ItemId, new_expiry: Date, on: Date, cost: Option<f64>, note: &str) -> Result<Item> {
        let mut item = self.item(id)?;
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO renewals (item, on_date, old_expiry, new_expiry, cost, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![id, date::iso(on), d2s(item.expires.or(item.end_date(on))), date::iso(new_expiry), cost, note])?;
        if item.issued.is_some() {
            item.issued = Some(on);
        }
        // a warranty's end comes from purchase + months; a renewal sets it explicitly
        item.expires = Some(new_expiry);
        item.state = State::Active;
        item.snoozed_until = None;
        if cost.is_some() {
            item.cost = cost;
        }
        tx.execute("UPDATE items SET issued = ?2, expires = ?3, state = 'active', snoozed_until = NULL, cost = ?4, updated = ?5 WHERE id = ?1", params![id, d2s(item.issued), date::iso(new_expiry), item.cost, now_secs()])?;
        tx.commit()?;
        Ok(item)
    }

    pub fn renewals(&self, id: ItemId) -> Result<Vec<Renewal>> {
        let mut st = self.conn.prepare("SELECT id, item, on_date, old_expiry, new_expiry, cost, note FROM renewals WHERE item = ?1 ORDER BY on_date DESC, id DESC")?;
        let v = st
            .query_map([id], |r| Ok(Renewal { id: r.get(0)?, item: r.get(1)?, on: s2d(Some(r.get(2)?)).unwrap_or_default(), old_expiry: s2d(r.get(3)?), new_expiry: s2d(r.get(4)?), cost: r.get(5)?, note: r.get(6)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    // ── reminders ────────────────────────────────────────────────────────────────────────────

    pub fn sent(&self) -> Result<HashSet<Reminder>> {
        let mut st = self.conn.prepare("SELECT item, deadline, lead FROM reminders_sent")?;
        let v = st
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, u32>(2)?)))?
            .filter_map(|r| r.ok())
            .filter_map(|(item, d, lead)| Some(Reminder { item, deadline: date::parse_iso(&d)?, lead }))
            .collect();
        Ok(v)
    }

    pub fn mark_sent(&mut self, reminders: &[Reminder]) -> Result<()> {
        let tx = self.conn.transaction()?;
        let now = now_secs();
        for r in reminders {
            tx.execute("INSERT OR IGNORE INTO reminders_sent (item, deadline, lead, at) VALUES (?1, ?2, ?3, ?4)", params![r.item, date::iso(r.deadline), r.lead, now])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── attachments ──────────────────────────────────────────────────────────────────────────

    pub fn attachments(&self, item: ItemId) -> Result<Vec<Attachment>> {
        let mut st = self.conn.prepare("SELECT id, item, rel_path, name, added FROM attachments WHERE item = ?1 ORDER BY id")?;
        let v = st.query_map([item], |r| Ok(Attachment { id: r.get(0)?, item: r.get(1)?, rel_path: r.get(2)?, name: r.get(3)?, added: r.get(4)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn attachment_counts(&self) -> Result<std::collections::HashMap<ItemId, usize>> {
        let mut st = self.conn.prepare("SELECT item, COUNT(*) FROM attachments GROUP BY item")?;
        let v = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as usize)))?.filter_map(|r| r.ok()).collect();
        Ok(v)
    }

    pub fn attachment_path(&self, a: &Attachment) -> PathBuf {
        let mut p = self.files_dir();
        for part in a.rel_path.split('/') {
            p.push(part);
        }
        p
    }

    /// Copies `src` into `vigia-files/<item>/` (never overwriting) and records it.
    pub fn attach(&self, item: ItemId, src: &Path) -> Result<Attachment> {
        let dir = self.files_dir().join(item.to_string());
        std::fs::create_dir_all(&dir)?;
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
        let file = unique_name(&dir, &sanitize(&name));
        std::fs::copy(src, dir.join(&file))?;
        let rel = format!("{item}/{file}");
        let added = now_secs();
        self.conn.execute("INSERT INTO attachments (item, rel_path, name, added) VALUES (?1, ?2, ?3, ?4)", params![item, rel, name, added])?;
        Ok(Attachment { id: self.conn.last_insert_rowid(), item, rel_path: rel, name, added })
    }

    pub fn detach(&self, id: i64) -> Result<()> {
        let a: Option<Attachment> = self.conn.query_row("SELECT id, item, rel_path, name, added FROM attachments WHERE id = ?1", [id], |r| Ok(Attachment { id: r.get(0)?, item: r.get(1)?, rel_path: r.get(2)?, name: r.get(3)?, added: r.get(4)? })).optional()?;
        if let Some(a) = a {
            let _ = std::fs::remove_file(self.attachment_path(&a));
            self.conn.execute("DELETE FROM attachments WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    /// A consistent copy of the database at `dest` (`VACUUM INTO`), for backups.
    pub fn snapshot(&self, dest: &Path) -> Result<()> {
        if dest.exists() {
            std::fs::remove_file(dest)?;
        }
        self.conn.execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
        Ok(())
    }
}

/// Keeps names portable: no path separators or characters Windows rejects.
pub fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() { "file".into() } else { s }
}

fn unique_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|i| format!("{stem} ({i}){ext}")).find(|n| !dir.join(n).exists()).unwrap_or_else(|| name.to_string())
}

/// Moves the database (and its attachments folder) to `dest`: copies, opens the copy to
/// verify it, then removes the originals. The store must be dropped by the caller first; the
/// returned store is the one at `dest`.
pub fn move_database(from: &Path, dest: &Path) -> Result<Store> {
    if dest.exists() {
        return Err(StoreError::Io(std::io::Error::new(std::io::ErrorKind::AlreadyExists, dest.display().to_string())));
    }
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::copy(from, dest)?;
    let (src_files, dst_files) = (files_dir(from), files_dir(dest));
    if src_files.is_dir() && src_files != dst_files {
        copy_dir(&src_files, &dst_files)?;
    }
    let store = match Store::open(dest) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(dest);
            return Err(e);
        }
    };
    let _ = std::fs::remove_file(from);
    if src_files.is_dir() && src_files != dst_files {
        let _ = std::fs::remove_dir_all(&src_files);
    }
    Ok(store)
}

pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()))?;
        } else {
            std::fs::copy(&p, to.join(e.file_name()))?;
        }
    }
    Ok(())
}

/// Whether a file looks like a Vigía database (for the "open another" picker).
pub fn is_vigia(path: &Path) -> bool {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok().and_then(|c| c.query_row("PRAGMA application_id", [], |r| r.get::<_, i32>(0)).ok()).is_some_and(|id| id == APPLICATION_ID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::ymd;

    fn sample() -> Item {
        Item {
            owner: "Ana".into(),
            issued: ymd(2021, 5, 12),
            expires: ymd(2031, 5, 12),
            cost: Some(420_000.0),
            currency: "COP".into(),
            provider: "Cancillería".into(),
            reference: "AX123456".into(),
            notes: "renew at the consulate\nline 2".into(),
            tags: vec!["family".into(), "travel".into()],
            renewal_url: "https://example.org/renew".into(),
            snoozed_until: ymd(2027, 1, 1),
            ..Item::new(Kind::Passport, "Pasaporte")
        }
    }

    #[test]
    fn create_insert_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/vigia.db");
        let s = Store::create(&path).unwrap();
        assert_eq!(s.version().unwrap(), latest_version());
        let mut it = sample();
        it.id = s.insert(&it).unwrap();
        let back = s.item(it.id).unwrap();
        assert_eq!(Item { created: 0, updated: 0, ..back.clone() }, Item { created: 0, updated: 0, ..it.clone() });
        let mut changed = back.clone();
        changed.name = "Pasaporte CO".into();
        changed.recurrence = Recurrence::Wyoming { month: 4 };
        s.update(&changed).unwrap();
        drop(s);
        let s = Store::open(&path).unwrap();
        assert_eq!(s.items().unwrap().len(), 1);
        assert_eq!(s.item(it.id).unwrap().recurrence, Recurrence::Wyoming { month: 4 });
        let mode: String = s.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        assert_eq!(mode, "delete");
        assert!(matches!(s.item(999), Err(StoreError::NotFound(999))));
        assert!(is_vigia(&path));
    }

    #[test]
    fn rejects_other_databases_and_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("other.db");
        Connection::open(&other).unwrap().execute_batch("CREATE TABLE t (x)").unwrap();
        assert!(matches!(Store::open(&other), Err(StoreError::NotVigia)));
        assert!(!is_vigia(&other));
        let junk = dir.path().join("junk.db");
        std::fs::write(&junk, b"this is not a database at all, not even close....").unwrap();
        assert!(Store::open(&junk).is_err());
        let newer = dir.path().join("newer.db");
        {
            let s = Store::create(&newer).unwrap();
            s.conn.pragma_update(None, "user_version", latest_version() + 1).unwrap();
        }
        assert!(matches!(Store::open(&newer), Err(StoreError::TooNew(_))));
    }

    #[test]
    fn migrations_from_every_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        {
            let c = Connection::open(&path).unwrap();
            c.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.version().unwrap(), latest_version());
        // every table the code uses exists
        for t in ["meta", "items", "attachments", "renewals", "reminders_sent"] {
            let n: i64 = s.conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [t], |r| r.get(0)).unwrap();
            assert_eq!(n, 1, "{t}");
        }
    }

    #[test]
    fn renew_keeps_history_and_reminders_persist() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::create(&dir.path().join("v.db")).unwrap();
        let mut it = sample();
        it.state = State::InProgress;
        let id = s.insert(&it).unwrap();
        let today = ymd(2031, 4, 1).unwrap();
        let after = s.renew(id, ymd(2041, 5, 12).unwrap(), today, Some(500_000.0), "consulado").unwrap();
        assert_eq!(after.expires, ymd(2041, 5, 12));
        assert_eq!(after.state, State::Active);
        assert_eq!(after.snoozed_until, None);
        assert_eq!(s.item(id).unwrap(), Item { updated: s.item(id).unwrap().updated, ..after });
        let h = s.renewals(id).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].old_expiry, h[0].new_expiry, h[0].cost, h[0].note.as_str()), (ymd(2031, 5, 12), ymd(2041, 5, 12), Some(500_000.0), "consulado"));
        let r = Reminder { item: id, deadline: ymd(2041, 5, 12).unwrap(), lead: 30 };
        s.mark_sent(&[r, r]).unwrap();
        assert_eq!(s.sent().unwrap(), HashSet::from([r]));
        s.delete(id).unwrap();
        assert!(s.sent().unwrap().is_empty());
        assert!(s.renewals(id).unwrap().is_empty());
    }

    #[test]
    fn attachments_relative_and_move() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/vigia.db");
        let s = Store::create(&path).unwrap();
        let id = s.insert(&sample()).unwrap();
        let src = dir.path().join("scan.png");
        std::fs::write(&src, b"png").unwrap();
        let a1 = s.attach(id, &src).unwrap();
        let a2 = s.attach(id, &src).unwrap();
        assert_eq!(a1.rel_path, format!("{id}/scan.png"));
        assert_eq!(a2.rel_path, format!("{id}/scan (2).png"));
        assert!(s.attachment_path(&a1).is_file());
        assert_eq!(s.attachment_counts().unwrap().get(&id), Some(&2));
        s.detach(a2.id).unwrap();
        assert_eq!(s.attachments(id).unwrap().len(), 1);
        drop(s);
        let dest = dir.path().join("synced/Vigia/vigia.db");
        let s = move_database(&path, &dest).unwrap();
        assert!(!path.exists() && dest.exists());
        let a = &s.attachments(id).unwrap()[0];
        assert_eq!(std::fs::read(s.attachment_path(a)).unwrap(), b"png");
        assert!(!files_dir(&path).exists());
        assert!(move_database(&dest, &dest).is_err());
        assert_eq!(sanitize("../a:b?.pdf"), "_a_b_.pdf");
        assert_eq!(sanitize(".."), "file");
    }
}
