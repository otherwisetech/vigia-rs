//! The database: one SQLite file (by default `vigia.db` in the app's data folder, movable to a
//! synced folder). Plain rollback journal (no WAL) so the single file syncs cleanly; integrity
//! check and application id verified on open; schema migrated forward with `PRAGMA
//! user_version`. Attachments live in `vigia-files/` next to the file, referenced by relative
//! path.
use crate::date::{self, Date};
use crate::entity::{self, Entity, EntityId, EntityKind, Relation, Role};
use crate::model::*;
use crate::reminder::Reminder;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// `VGIA` in ASCII, in the SQLite header so other tools can tell what the file is.
pub const APPLICATION_ID: i32 = 0x5647_4941;
pub const EXTENSION: &str = "db";
pub const FILES_DIR: &str = "vigia-files";
/// `meta` keys of shared cards whose Vigía entity was deleted, to unlink on the next sync.
pub const UNLINK_PREFIX: &str = "shared_unlink:";

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
    // 2: what items belong to (people, vehicles, companies…) and how those relate; the old
    // free-text owners become entities in `backfill_entities`
    "CREATE TABLE entities (
       id INTEGER PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, ident TEXT NOT NULL DEFAULT '',
       detail TEXT NOT NULL DEFAULT '', year INTEGER, country TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '',
       created INTEGER NOT NULL DEFAULT 0, updated INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE relations (
       id INTEGER PRIMARY KEY, from_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
       to_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE, role TEXT NOT NULL DEFAULT 'owner',
       UNIQUE (from_id, to_id));
     ALTER TABLE items ADD COLUMN entity_id INTEGER REFERENCES entities(id) ON DELETE SET NULL;
     CREATE INDEX items_entity ON items(entity_id);",
    // 3: business-day rule per item; calendars (.ics files or URLs) shown read-only
    "ALTER TABLE items ADD COLUMN business_days TEXT NOT NULL DEFAULT '';
     CREATE TABLE calendars (
       id INTEGER PRIMARY KEY, name TEXT NOT NULL, source TEXT NOT NULL, body TEXT NOT NULL DEFAULT '',
       fetched INTEGER, enabled INTEGER NOT NULL DEFAULT 1);",
    // 4: link to the shared entity registry of the Otherwise apps (opt-in): the card's id and
    // which revision of the entity was last pushed or pulled
    "ALTER TABLE entities ADD COLUMN shared_id TEXT;
     ALTER TABLE entities ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
     ALTER TABLE entities ADD COLUMN shared_rev INTEGER NOT NULL DEFAULT 0;",
    // 5: entity kinds are sections; owners first taken for people take the kind all their
    // items show, in `backfill_entity_kinds`
    "CREATE INDEX entities_kind ON entities(kind);",
];

/// Version 2: each distinct owner (case-insensitively) becomes an entity, its kind inferred
/// from the name (plate → vehicle, legal suffix → company, domain → digital, else a person).
fn backfill_entities(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    let rows: Vec<(i64, String)> = {
        let mut st = tx.prepare("SELECT id, owner FROM items WHERE trim(owner) != '' ORDER BY id")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
    };
    let mut made: Vec<(String, i64)> = Vec::new();
    let now = now_secs();
    for (item, owner) in rows {
        let name = owner.trim().to_string();
        let key = name.to_lowercase();
        let id = match made.iter().find(|(k, _)| *k == key) {
            Some((_, id)) => *id,
            None => {
                let e = Entity::new(entity::infer(&name).unwrap_or(EntityKind::Person), name.clone());
                tx.execute("INSERT INTO entities (kind, name, ident, created, updated) VALUES (?1, ?2, ?3, ?4, ?4)", params![e.kind.id(), e.name, e.ident, now])?;
                let id = tx.last_insert_rowid();
                made.push((key, id));
                id
            }
        };
        tx.execute("UPDATE items SET entity_id = ?2, owner = ?3 WHERE id = ?1", params![item, id, name])?;
    }
    Ok(())
}

/// Version 5: see [`entity::suggest_kind`]. Kinds this version doesn't know are left alone.
fn backfill_entity_kinds(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    let ents: Vec<Entity> = {
        let mut st = tx.prepare(&format!("SELECT {ENTITY_COLS} FROM entities WHERE kind = 'person' ORDER BY id"))?;
        st.query_map([], entity_from_row)?.collect::<rusqlite::Result<_>>()?
    };
    for e in ents {
        let kinds: Vec<String> = {
            let mut st = tx.prepare("SELECT kind FROM items WHERE entity_id = ?1 AND state != 'archived' ORDER BY id")?;
            st.query_map([e.id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        let items: Vec<Item> = kinds.iter().map(|k| Item::new(Kind::from_id(k).unwrap_or(Kind::Custom), "")).collect();
        let refs: Vec<&Item> = items.iter().collect();
        if let Some(k) = entity::suggest_kind(&e, &refs) {
            tx.execute("UPDATE entities SET kind = ?2 WHERE id = ?1", params![e.id, k.id()])?;
        }
    }
    Ok(())
}

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
    /// An entity can't be related to itself.
    SelfRelation,
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
            StoreError::SelfRelation => write!(f, "{}", i18n::t!("core.db.self_relation")),
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

fn known_kinds() -> String {
    serde_json::to_string(&EntityKind::ALL.iter().map(|k| k.id()).collect::<Vec<_>>()).unwrap_or_default()
}

const ITEM_COLS: &str = "i.id, i.kind, i.name, COALESCE(e.name, i.owner), i.category, i.issued, i.expires, i.recurrence, i.term_months, i.notice_days, i.cost, i.currency, i.provider, i.reference, i.notes, i.tags, i.renewal_url, i.auto_renews, i.lead_days, i.state, i.snoozed_until, i.host, i.auto_check, i.last_checked, i.check_note, i.created, i.updated, i.entity_id, e.kind, i.business_days";
const ITEM_FROM: &str = "items i LEFT JOIN entities e ON e.id = i.entity_id";
const ENTITY_COLS: &str = "id, kind, name, ident, detail, year, country, notes, created, updated, shared_id";

fn entity_from_row(r: &Row) -> rusqlite::Result<Entity> {
    Ok(Entity {
        id: r.get(0)?,
        kind: EntityKind::from_id(&r.get::<_, String>(1)?).unwrap_or(EntityKind::Other),
        name: r.get(2)?,
        ident: r.get(3)?,
        detail: r.get(4)?,
        year: r.get(5)?,
        country: r.get(6)?,
        notes: r.get(7)?,
        created: r.get(8)?,
        updated: r.get(9)?,
        shared: r.get(10)?,
    })
}

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
        entity: r.get(27)?,
        owner_kind: r.get::<_, Option<String>>(28)?.and_then(|k| EntityKind::from_id(&k)),
        business_days: crate::holidays::BusinessDays::parse(&r.get::<_, String>(29)?),
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
            if i + 1 == 2 {
                backfill_entities(&tx)?;
            }
            if i + 1 == 5 {
                backfill_entity_kinds(&tx)?;
            }
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
        let mut st = self.conn.prepare(&format!("SELECT {ITEM_COLS} FROM {ITEM_FROM} ORDER BY i.id"))?;
        let v = st.query_map([], item_from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn item(&self, id: ItemId) -> Result<Item> {
        self.conn.query_row(&format!("SELECT {ITEM_COLS} FROM {ITEM_FROM} WHERE i.id = ?1"), [id], item_from_row).optional()?.ok_or(StoreError::NotFound(id))
    }

    /// Inserts `item` (its id is ignored) and returns the new id.
    pub fn insert(&self, item: &Item) -> Result<ItemId> {
        let now = now_secs();
        let (entity, owner) = self.resolve_owner(item)?;
        self.conn.execute(
            "INSERT INTO items (kind, name, owner, category, issued, expires, recurrence, term_months, notice_days, cost, currency, provider, reference, notes, tags, renewal_url, auto_renews, lead_days, state, snoozed_until, host, auto_check, last_checked, check_note, created, updated, entity_id, business_days)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?25, ?26, ?27)",
            params![
                item.kind.id(),
                item.name,
                owner,
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
                entity,
                item.business_days.map(|b| b.id()).unwrap_or_default(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update(&self, item: &Item) -> Result<()> {
        let (entity, owner) = self.resolve_owner(item)?;
        let n = self.conn.execute(
            "UPDATE items SET kind = ?2, name = ?3, owner = ?4, category = ?5, issued = ?6, expires = ?7, recurrence = ?8, term_months = ?9, notice_days = ?10, cost = ?11, currency = ?12, provider = ?13, reference = ?14, notes = ?15, tags = ?16, renewal_url = ?17, auto_renews = ?18, lead_days = ?19, state = ?20, snoozed_until = ?21, host = ?22, auto_check = ?23, last_checked = ?24, check_note = ?25, updated = ?26, entity_id = ?27, business_days = ?28 WHERE id = ?1",
            params![
                item.id,
                item.kind.id(),
                item.name,
                owner,
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
                entity,
                item.business_days.map(|b| b.id()).unwrap_or_default(),
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

    /// The entity an item's `owner` names: its loaded entity while the name still matches, else
    /// an existing one found by name / identifier, else a new one (`owner_kind`, or guessed).
    fn resolve_owner(&self, item: &Item) -> Result<(Option<EntityId>, String)> {
        let name = item.owner.trim();
        if name.is_empty() {
            return Ok((None, String::new()));
        }
        if let Some(id) = item.entity
            && let Ok(e) = self.entity(id)
            && e.name.trim().to_lowercase() == name.to_lowercase()
        {
            return Ok((Some(id), e.name));
        }
        let all = self.entities()?;
        if let Some(e) = entity::find(&all, name) {
            return Ok((Some(e.id), e.name.clone()));
        }
        // a stale kind belongs to the entity the item had, not to this new name
        let kind = item.owner_kind.filter(|_| item.entity.is_none()).unwrap_or_else(|| entity::guess(name, None));
        let id = self.insert_entity(&Entity::new(kind, name))?;
        Ok((Some(id), name.to_string()))
    }

    // ── entities ─────────────────────────────────────────────────────────────────────────────

    pub fn entities(&self) -> Result<Vec<Entity>> {
        let mut st = self.conn.prepare(&format!("SELECT {ENTITY_COLS} FROM entities ORDER BY name COLLATE NOCASE, id"))?;
        let v = st.query_map([], entity_from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn entity(&self, id: EntityId) -> Result<Entity> {
        self.conn.query_row(&format!("SELECT {ENTITY_COLS} FROM entities WHERE id = ?1"), [id], entity_from_row).optional()?.ok_or(StoreError::NotFound(id))
    }

    pub fn insert_entity(&self, e: &Entity) -> Result<EntityId> {
        let now = now_secs();
        self.conn.execute(
            "INSERT INTO entities (kind, name, ident, detail, year, country, notes, created, updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![e.kind.id(), e.name.trim(), e.ident, e.detail, e.year, e.country, e.notes, if e.created > 0 { e.created } else { now }],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Saves the entity; its items show the new name.
    pub fn update_entity(&self, e: &Entity) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let n = tx.execute(
            // a kind written by a newer version reads as "other": saving keeps it unless changed
            "UPDATE entities SET kind = CASE WHEN ?2 = 'other' AND NOT EXISTS (SELECT 1 FROM json_each(?10) WHERE value = entities.kind) THEN kind ELSE ?2 END,
               name = ?3, ident = ?4, detail = ?5, year = ?6, country = ?7, notes = ?8, updated = ?9, rev = rev + 1 WHERE id = ?1",
            params![e.id, e.kind.id(), e.name.trim(), e.ident, e.detail, e.year, e.country, e.notes, now_secs(), known_kinds()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(e.id));
        }
        tx.execute("UPDATE items SET owner = ?2 WHERE entity_id = ?1", params![e.id, e.name.trim()])?;
        tx.commit()?;
        Ok(())
    }

    /// Deletes the entity and its relations; its items stay, with no owner. A shared one is
    /// queued for unlinking from the registry (its card stays for the other apps).
    pub fn delete_entity(&self, id: EntityId) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let shared: Option<String> = tx.query_row("SELECT shared_id FROM entities WHERE id = ?1", [id], |r| r.get(0)).optional()?.flatten();
        if let Some(sid) = shared.filter(|s| uuid::Uuid::parse_str(s).is_ok()) {
            tx.execute("INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![format!("{UNLINK_PREFIX}{sid}"), id.to_string()])?;
        }
        tx.execute("UPDATE items SET owner = '', entity_id = NULL WHERE entity_id = ?1", [id])?;
        tx.execute("DELETE FROM entities WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    // ── shared registry link ─────────────────────────────────────────────────────────────────

    /// `(id, rev, shared_rev)` of every entity: `rev` grows on each edit, `shared_rev` is the
    /// revision last pushed to / pulled from the registry.
    pub fn entity_revs(&self) -> Result<Vec<(EntityId, i64, i64)>> {
        let mut st = self.conn.prepare("SELECT id, rev, shared_rev FROM entities ORDER BY id")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Links the entity to shared card `shared` (or unlinks it) and marks its current revision
    /// as in sync.
    pub fn set_shared(&self, id: EntityId, shared: Option<&str>) -> Result<()> {
        self.conn.execute("UPDATE entities SET shared_id = ?2, shared_rev = rev WHERE id = ?1", params![id, shared])?;
        Ok(())
    }

    /// Repoints every entity linked to `from` (a card merged into `to`).
    pub fn repoint_shared(&self, from: &str, to: &str) -> Result<usize> {
        Ok(self.conn.execute("UPDATE entities SET shared_id = ?2 WHERE shared_id = ?1", params![from, to])?)
    }

    pub fn entity_by_shared(&self, shared: &str) -> Result<Option<Entity>> {
        Ok(self.conn.query_row(&format!("SELECT {ENTITY_COLS} FROM entities WHERE shared_id = ?1 ORDER BY id LIMIT 1"), [shared], entity_from_row).optional()?)
    }

    // ── relations ────────────────────────────────────────────────────────────────────────────

    pub fn relations(&self) -> Result<Vec<Relation>> {
        let mut st = self.conn.prepare("SELECT id, from_id, to_id, role FROM relations ORDER BY id")?;
        let v = st.query_map([], |r| Ok(Relation { id: r.get(0)?, from: r.get(1)?, to: r.get(2)?, role: Role::from_id(&r.get::<_, String>(3)?) }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Links `from` → `to` (one link per pair and direction; linking again changes the role).
    pub fn relate(&self, from: EntityId, to: EntityId, role: Role) -> Result<i64> {
        if from == to {
            return Err(StoreError::SelfRelation);
        }
        self.conn.execute("INSERT INTO relations (from_id, to_id, role) VALUES (?1, ?2, ?3) ON CONFLICT(from_id, to_id) DO UPDATE SET role = excluded.role", params![from, to, role.id()])?;
        Ok(self.conn.query_row("SELECT id FROM relations WHERE from_id = ?1 AND to_id = ?2", [from, to], |r| r.get(0))?)
    }

    pub fn unrelate(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM relations WHERE id = ?1", [id])?;
        Ok(())
    }

    // ── calendars ────────────────────────────────────────────────────────────────────────────

    pub fn calendars(&self) -> Result<Vec<crate::calendars::Calendar>> {
        let mut st = self.conn.prepare("SELECT id, name, source, body, fetched, enabled FROM calendars ORDER BY id")?;
        let v = st
            .query_map([], |r| Ok(crate::calendars::Calendar { id: r.get(0)?, name: r.get(1)?, source: r.get(2)?, body: r.get(3)?, fetched: r.get(4)?, enabled: r.get::<_, i64>(5)? != 0 }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Inserts (id 0) or updates a calendar.
    pub fn save_calendar(&self, c: &crate::calendars::Calendar) -> Result<i64> {
        if c.id == 0 {
            self.conn.execute("INSERT INTO calendars (name, source, body, fetched, enabled) VALUES (?1, ?2, ?3, ?4, ?5)", params![c.name, c.source, c.body, c.fetched, c.enabled as i64])?;
            return Ok(self.conn.last_insert_rowid());
        }
        self.conn.execute("UPDATE calendars SET name = ?2, source = ?3, body = ?4, fetched = ?5, enabled = ?6 WHERE id = ?1", params![c.id, c.name, c.source, c.body, c.fetched, c.enabled as i64])?;
        Ok(c.id)
    }

    pub fn delete_calendar(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM calendars WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Meta entries whose key starts with `prefix` (key, value).
    pub fn metas(&self, prefix: &str) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT key, value FROM meta WHERE substr(key, 1, length(?1)) = ?1 ORDER BY key")?;
        let v = st.query_map([prefix], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn delete_meta(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM meta WHERE key = ?1", [key])?;
        Ok(())
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

/// Copies the database and its attachments folder to `dest` and opens the copy ("Save As"). The
/// copy is verified (otherwise-link `storage::copy_sqlite`: snapshot, integrity check, same
/// content; files byte-compared) and nothing at the destination is overwritten; on failure
/// whatever was copied is removed again.
pub fn copy_database(from: &Path, dest: &Path) -> Result<Store> {
    if dest.exists() {
        return Err(StoreError::Io(std::io::Error::new(std::io::ErrorKind::AlreadyExists, dest.display().to_string())));
    }
    otherwise_link::storage::copy_sqlite(from, dest)?;
    let (src_files, dst_files) = (files_dir(from), files_dir(dest));
    let mut copied = Vec::new();
    if src_files.is_dir() && src_files != dst_files {
        match otherwise_link::storage::copy_tree(&src_files, &dst_files) {
            Ok(c) => copied = c,
            Err(e) => {
                let _ = otherwise_link::storage::remove_sqlite(dest);
                return Err(e.into());
            }
        }
    }
    match Store::open(dest) {
        Ok(s) => Ok(s),
        Err(e) => {
            let _ = otherwise_link::storage::remove_sqlite(dest);
            for f in copied {
                let _ = std::fs::remove_file(f);
            }
            Err(e)
        }
    }
}

/// Moves the database (and its attachments folder) to `dest`: [`copy_database`], then removes
/// the originals, only once the copy verified and opened. The caller must drop its store (and
/// release its lock) first; the returned store is the one at `dest`.
pub fn move_database(from: &Path, dest: &Path) -> Result<Store> {
    let store = copy_database(from, dest)?;
    otherwise_link::storage::remove_sqlite(from)?;
    let (src_files, dst_files) = (files_dir(from), files_dir(dest));
    if src_files.is_dir() && src_files != dst_files {
        let _ = std::fs::remove_dir_all(&src_files);
    }
    Ok(store)
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
        let ana = s.entities().unwrap();
        assert_eq!((ana.len(), ana[0].kind, ana[0].name.as_str()), (1, EntityKind::Person, "Ana"));
        assert_eq!(Item { created: 0, updated: 0, ..back.clone() }, Item { created: 0, updated: 0, entity: Some(ana[0].id), owner_kind: Some(EntityKind::Person), ..it.clone() });
        let mut changed = back.clone();
        changed.name = "Pasaporte CO".into();
        changed.recurrence = Recurrence::Wyoming { month: 4 };
        changed.business_days = crate::holidays::BusinessDays::parse("next:co");
        s.update(&changed).unwrap();
        drop(s);
        let s = Store::open(&path).unwrap();
        assert_eq!(s.items().unwrap().len(), 1);
        assert_eq!(s.item(it.id).unwrap().recurrence, Recurrence::Wyoming { month: 4 });
        assert_eq!(s.item(it.id).unwrap().business_days.map(|b| b.id()), Some("next:co".into()));
        let cal = crate::calendars::Calendar { name: "Festivos".into(), source: "/x/festivos.ics".into(), body: "BEGIN:VCALENDAR".into(), ..Default::default() };
        let cid = s.save_calendar(&cal).unwrap();
        s.save_calendar(&crate::calendars::Calendar { id: cid, enabled: false, ..cal.clone() }).unwrap();
        assert_eq!(s.calendars().unwrap()[0], crate::calendars::Calendar { id: cid, enabled: false, ..cal });
        s.delete_calendar(cid).unwrap();
        assert!(s.calendars().unwrap().is_empty());
        s.set_meta("live_ics:3", "/x/a.ics").unwrap();
        s.set_meta("live_icsX", "no").unwrap();
        assert_eq!(s.metas("live_ics:").unwrap(), vec![("live_ics:3".to_string(), "/x/a.ics".to_string())]);
        s.delete_meta("live_ics:3").unwrap();
        assert!(s.metas("live_ics:").unwrap().is_empty());
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
        for t in ["meta", "items", "attachments", "renewals", "reminders_sent", "entities", "relations", "calendars"] {
            let n: i64 = s.conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [t], |r| r.get(0)).unwrap();
            assert_eq!(n, 1, "{t}");
        }
        // 4: the registry link columns
        let _: Option<(Option<String>, i64, i64)> = s.conn.query_row("SELECT shared_id, rev, shared_rev FROM entities", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().unwrap();
    }

    /// A version-1 file with free-text owners: each becomes one entity of a guessed kind.
    #[test]
    fn owners_become_entities() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        {
            let c = Connection::open(&path).unwrap();
            c.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
            c.execute_batch(MIGRATIONS[0]).unwrap();
            c.pragma_update(None, "user_version", 1).unwrap();
            for (kind, name, owner) in [("passport", "Pasaporte", "Ana"), ("soat", "SOAT", "Mazda ABC123"), ("tecnomecanica", "Tecno", "mazda abc123 "), ("wyoming_report", "Annual report", "Otherwise Technologies LLC"), ("camara_comercio", "Cámara", "Otherwise S.A.S."), ("soat", "SOAT moto", "XYZ12A"), ("custom", "Netflix", "")] {
                c.execute("INSERT INTO items (kind, name, owner, category) VALUES (?1, ?2, ?3, 'other')", [kind, name, owner]).unwrap();
            }
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.version().unwrap(), latest_version());
        let es = s.entities().unwrap();
        let kinds: Vec<(&str, EntityKind, &str)> = es.iter().map(|e| (e.name.as_str(), e.kind, e.ident.as_str())).collect();
        assert_eq!(kinds, vec![("Ana", EntityKind::Person, ""), ("Mazda ABC123", EntityKind::Vehicle, "ABC123"), ("Otherwise S.A.S.", EntityKind::Company, ""), ("Otherwise Technologies LLC", EntityKind::Company, ""), ("XYZ12A", EntityKind::Vehicle, "XYZ12A")]);
        let items = s.items().unwrap();
        let mazda = es.iter().find(|e| e.kind == EntityKind::Vehicle && e.name.starts_with("Mazda")).unwrap().id;
        assert_eq!(items.iter().filter(|i| i.entity == Some(mazda)).count(), 2);
        assert!(items.iter().filter(|i| i.entity == Some(mazda)).all(|i| i.owner == "Mazda ABC123" && i.owner_kind == Some(EntityKind::Vehicle)));
        assert_eq!(items[6].entity, None);
    }

    #[test]
    fn kinds_migrate_and_unknown_kinds_survive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v4.db");
        {
            let mut c = Connection::open(&path).unwrap();
            c.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
            let tx = c.transaction().unwrap();
            for (i, step) in MIGRATIONS[..4].iter().enumerate() {
                tx.execute_batch(step).unwrap();
                if i + 1 == 2 {
                    backfill_entities(&tx).unwrap();
                }
            }
            tx.pragma_update(None, "user_version", 4).unwrap();
            tx.commit().unwrap();
            for (id, kind, name, ident, shared) in [(1, "person", "Mazda", "", None), (2, "person", "Ana", "", None), (3, "person", "Luna", "", None), (4, "person", "Renault", "", Some("u-1")), (5, "starship", "Enterprise", "NCC1701", None), (6, "person", "Kia", "", None)] {
                c.execute("INSERT INTO entities (id, kind, name, ident, shared_id) VALUES (?1, ?2, ?3, ?4, ?5)", params![id, kind, name, ident, shared]).unwrap();
            }
            for (kind, ent, state) in [("soat", 1, "active"), ("tecnomecanica", 1, "active"), ("passport", 2, "active"), ("vaccine", 3, "active"), ("soat", 4, "active"), ("soat", 6, "active"), ("passport", 6, "archived")] {
                c.execute("INSERT INTO items (kind, name, category, entity_id, state) VALUES (?1, 'x', 'other', ?2, ?3)", params![kind, ent, state]).unwrap();
            }
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.version().unwrap(), latest_version());
        let kind = |id| s.entity(id).unwrap().kind;
        assert_eq!(kind(1), EntityKind::Vehicle, "only vehicle papers");
        assert_eq!(kind(2), EntityKind::Person);
        assert_eq!(kind(3), EntityKind::Person, "a vaccine could be anyone's");
        assert_eq!(kind(4), EntityKind::Person, "shared entities are left alone");
        assert_eq!(kind(6), EntityKind::Vehicle, "archived items don't count");
        // a kind from a newer version reads as other and is kept on save
        let mut e = s.entity(5).unwrap();
        assert_eq!(e.kind, EntityKind::Other);
        e.notes = "edited".into();
        s.update_entity(&e).unwrap();
        let raw: String = s.conn.query_row("SELECT kind FROM entities WHERE id = 5", [], |r| r.get(0)).unwrap();
        assert_eq!(raw, "starship");
        assert_eq!(s.entity(5).unwrap().notes, "edited");
        e.kind = EntityKind::Vehicle;
        s.update_entity(&e).unwrap();
        assert_eq!(kind(5), EntityKind::Vehicle, "changing it is the user's call");
        // and a known kind can still be set to other
        let mut ana = s.entity(2).unwrap();
        ana.kind = EntityKind::Other;
        s.update_entity(&ana).unwrap();
        assert_eq!(kind(2), EntityKind::Other);
    }

    #[test]
    fn entities_resolve_rename_delete_and_relate() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::create(&dir.path().join("e.db")).unwrap();
        let car = s.insert_entity(&Entity { detail: "Mazda 3".into(), year: Some(2019), ..Entity::new(EntityKind::Vehicle, "Mazda ABC123") }).unwrap();
        assert_eq!(s.entity(car).unwrap().ident, "ABC123");
        // a plate, or a word of the name, finds the existing entity
        let soat = s.insert(&Item { owner: "abc-123".into(), ..Item::new(Kind::Soat, "SOAT") }).unwrap();
        let tecno = s.insert(&Item { owner: "Mazda".into(), ..Item::new(Kind::Tecnomecanica, "Tecno") }).unwrap();
        assert_eq!(s.item(soat).unwrap().entity, Some(car));
        assert_eq!(s.item(tecno).unwrap().owner, "Mazda ABC123");
        // a new name with an explicit kind
        let dom = s.insert(&Item { owner: "Otherwise".into(), owner_kind: Some(EntityKind::Company), ..Item::new(Kind::Domain, "otherwisetech.com") }).unwrap();
        let co = s.item(dom).unwrap();
        assert_eq!(co.owner_kind, Some(EntityKind::Company));
        // changing the owner text moves the item; the stale kind is not reused
        let mut moved = s.item(soat).unwrap();
        moved.owner = "Ana".into();
        s.update(&moved).unwrap();
        let moved = s.item(soat).unwrap();
        assert_eq!((moved.owner.as_str(), moved.owner_kind), ("Ana", Some(EntityKind::Person)));
        let mut e = s.entity(car).unwrap();
        e.name = "Mazda 3 ABC123".into();
        s.update_entity(&e).unwrap();
        assert_eq!(s.item(tecno).unwrap().owner, "Mazda 3 ABC123");
        let ana = moved.entity.unwrap();
        let company = co.entity.unwrap();
        let r = s.relate(ana, company, Role::Member).unwrap();
        assert_eq!(s.relate(ana, company, Role::Owner).unwrap(), r);
        s.relate(ana, car, Role::Owner).unwrap();
        assert!(matches!(s.relate(ana, ana, Role::Owner), Err(StoreError::SelfRelation)));
        let rels = s.relations().unwrap();
        assert_eq!(rels.iter().map(|r| (r.from, r.to, r.role)).collect::<Vec<_>>(), vec![(ana, company, Role::Owner), (ana, car, Role::Owner)]);
        s.unrelate(r).unwrap();
        assert_eq!(s.relations().unwrap().len(), 1);
        s.delete_entity(car).unwrap();
        assert!(s.relations().unwrap().is_empty());
        let t = s.item(tecno).unwrap();
        assert_eq!((t.owner.as_str(), t.entity), ("", None));
        assert!(matches!(s.entity(car), Err(StoreError::NotFound(_))));
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

    #[test]
    fn save_as_and_migration_from_the_old_default() {
        let dir = tempfile::tempdir().unwrap();
        // the pre-0.2.1 default with an attachment
        let legacy = dir.path().join("Application Support/Vigia/vigia.db");
        let s = Store::create(&legacy).unwrap();
        let id = s.insert(&sample()).unwrap();
        let src = dir.path().join("soat.pdf");
        std::fs::write(&src, b"%PDF").unwrap();
        s.attach(id, &src).unwrap();
        // Save As: a verified copy, the original untouched
        let copy = dir.path().join("Dropbox/vigia copy.db");
        let c = copy_database(&legacy, &copy).unwrap();
        assert_eq!(c.items().unwrap().len(), 1);
        assert!(legacy.exists() && files_dir(&legacy).join(id.to_string()).join("soat.pdf").exists());
        assert!(copy_database(&legacy, &copy).is_err(), "never overwrites");
        drop((s, c));
        // not a Vigía file: refused, nothing left behind
        let other = dir.path().join("other.db");
        rusqlite::Connection::open(&other).unwrap().execute_batch("CREATE TABLE t(x)").unwrap();
        let bad = dir.path().join("out/other.db");
        assert!(matches!(copy_database(&other, &bad), Err(StoreError::NotVigia)));
        assert!(!bad.exists());
        // the migration: everything arrives, the old place is emptied only afterwards
        let new = dir.path().join(".otherwise/vigia/vigia.db");
        let s = move_database(&legacy, &new).unwrap();
        let a = &s.attachments(id).unwrap()[0];
        assert_eq!(std::fs::read(s.attachment_path(a)).unwrap(), b"%PDF");
        assert_eq!(s.items().unwrap()[0].name, sample().name);
        assert!(!legacy.exists() && !files_dir(&legacy).exists());
        let mode: String = s.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        assert_eq!(mode, "delete");
    }
}
