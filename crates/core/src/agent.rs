//! What the AI panel's assistant may change, as proposals. `vigia mcp --propose <folder>` turns
//! its write tools ([`WRITE_TOOLS`]) into a checked [`Proposal`] dropped in that folder; the app
//! shows it as a card and applies it, with the same parsing, only when the user accepts.
//!
//! Every write tool goes through [`plan`]: the arguments, read against the database as it is,
//! become a [`Plan`] (the card's title and lines, and the operations). [`apply`] runs the
//! operations through the [`Store`] and returns what [`undo`] needs to take them back; deleting
//! and merging can't be undone and their cards say so.
use crate::date::{self, Date, Order};
use crate::entity::{self, Entity};
use crate::model::{Item, Kind};
use crate::templates;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The write tools that become proposals.
pub const WRITE_TOOLS: &[&str] = &["add_item", "renew_item", "propose_add_from_template", "update_items", "delete_items", "convert_celebrations", "check_items", "create_entity", "update_entity", "archive_entity", "delete_entity", "merge_entities", "relate", "unrelate"];

fn str_arg<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty())
}

/// `add_item`'s arguments → the item it would add (not saved): `text` is a quick-add line;
/// kind, name, owner, expires, host and notes override it.
pub fn item_from_args(a: &Value, today: Date, order: Order) -> Result<Item, String> {
    let mut it = match (str_arg(a, "text"), str_arg(a, "kind")) {
        (Some(t), _) => crate::quick::parse(t, today, order),
        (None, Some(k)) => Item::new(Kind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = k))?, ""),
        (None, None) if str_arg(a, "name").is_some() => Item::new(Kind::Custom, ""),
        _ => return Err(i18n::t!("cli.err.add_usage")),
    };
    if let (Some(k), Some(_)) = (str_arg(a, "kind"), str_arg(a, "text")) {
        it.kind = Kind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = k))?;
    }
    if let Some(v) = str_arg(a, "name") {
        it.name = v.into();
    }
    if let Some(v) = str_arg(a, "owner") {
        it.owner = v.into();
        it.entity = None;
    }
    if let Some(v) = str_arg(a, "expires") {
        it.expires = Some(date::parse_loose(v, today, order).ok_or_else(|| i18n::t!("cli.err.bad_date", value = v))?);
    }
    if let Some(v) = str_arg(a, "host") {
        it.host = v.into();
    }
    if let Some(v) = str_arg(a, "notes") {
        it.notes = v.into();
    }
    if it.name.trim().is_empty() {
        it.name = it.kind.label();
    }
    Ok(it)
}

/// `propose_add_from_template`'s arguments → the item (not saved): `template` (an id from the
/// `templates` tool), `owner` (an entity's id, name, plate or tax id; a new entity of the
/// template's kind when nothing matches; left out, the only entity of that kind), and optional
/// `expires`, `name` and `notes`.
pub fn item_from_template_args(a: &Value, entities: &[Entity], today: Date, order: Order) -> Result<Item, String> {
    let id = str_arg(a, "template").ok_or_else(|| i18n::t!("cli.err.needs_template"))?;
    let tpl = templates::by_id(id).ok_or_else(|| i18n::t!("cli.err.bad_template", value = id))?;
    let typed = str_arg(a, "owner").map(str::to_string).or_else(|| a.get("owner").and_then(Value::as_i64).map(|n| n.to_string()));
    let owner = match typed.as_deref() {
        Some(o) => o.parse::<i64>().ok().and_then(|n| entities.iter().find(|e| e.id == n)).or_else(|| entity::find(entities, o)).map(Ok).unwrap_or(Err(o)),
        None => {
            let mut of = entities.iter().filter(|e| tpl.owners.contains(&e.kind));
            match (of.next(), of.next()) {
                (Some(e), None) => Ok(e),
                _ => return Err(i18n::t!("cli.err.needs_owner", kind = tpl.owners[0].label())),
            }
        }
    };
    let mut it = match owner {
        Ok(e) => tpl.item(Some(e)),
        // a domain's or subscription's holder is a person or company, not a digital entity
        Err(name) if tpl.category == crate::model::Category::Digital => Item { owner: name.to_string(), owner_kind: Some(entity::guess(name, None)), ..tpl.item(None) },
        Err(name) => Item { owner: name.to_string(), owner_kind: Some(entity::infer(name).filter(|k| tpl.owners.contains(k)).unwrap_or(tpl.owners[0])), ..tpl.item(None) },
    };
    if let Some(v) = str_arg(a, "expires") {
        it.expires = Some(date::parse_loose(v, today, order).ok_or_else(|| i18n::t!("cli.err.bad_date", value = v))?);
    }
    if let Some(v) = str_arg(a, "name") {
        it.name = v.into();
    }
    if let Some(v) = str_arg(a, "notes") {
        it.notes = v.into();
    }
    Ok(it)
}

/// A renewal of an item: the new expiry, cost and note.
#[derive(Debug, Clone, PartialEq)]
pub struct Renewal {
    pub id: i64,
    pub expires: Date,
    pub cost: Option<f64>,
    pub note: String,
}

/// `renew_item`'s arguments for `it`: the new expiry defaults to the suggested next one.
pub fn renewal_from_args(it: &Item, a: &Value, today: Date, order: Order) -> Result<Renewal, String> {
    let expires = match str_arg(a, "expires") {
        Some(v) => date::parse_loose(v, today, order).ok_or_else(|| i18n::t!("cli.err.bad_date", value = v))?,
        None => it.suggest_next(today),
    };
    Ok(Renewal { id: it.id, expires, cost: a.get("cost").and_then(Value::as_f64), note: str_arg(a, "note").unwrap_or_default().to_string() })
}

pub fn needs_id(a: &Value) -> Result<i64, String> {
    a.get("id").and_then(Value::as_i64).ok_or_else(|| i18n::t!("cli.err.needs_id"))
}

/// A write tool call waiting for the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub tool: String,
    pub args: Value,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// Drops `p` in `dir` (written beside it first, then renamed, so the app never reads half a file).
pub fn deliver(dir: &Path, p: &Proposal) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    let name = format!("{stamp:032}-{}.json", std::process::id());
    let tmp = dir.join(format!(".{name}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(p).map_err(std::io::Error::other)?)?;
    let path = dir.join(name);
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Takes every delivered proposal out of `dir`, oldest first; unreadable files are dropped.
pub fn collect(dir: &Path) -> Vec<Proposal> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json") && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))).collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            let v = std::fs::read(&p).ok();
            let _ = std::fs::remove_file(&p);
            serde_json::from_slice(&v?).ok()
        })
        .collect()
}

// ── plans: every write tool, checked against the data, applied and undone ───────────────────

use crate::db::{self, Store};
use crate::entity::{EntityId, EntityKind, Relation, Role};
use crate::fields::Field;
use crate::model::{Category, ItemId, Recurrence, State};

/// Who an owner argument means: an entity that exists, or a new one to create on apply.
#[derive(Debug, Clone, PartialEq)]
pub enum OwnerRef {
    Id(EntityId),
    New(String, EntityKind),
}

/// How an item's owners change.
#[derive(Debug, Clone, PartialEq)]
pub enum Owners {
    Keep,
    Set(Vec<OwnerRef>),
    Add(Vec<OwnerRef>),
    Remove(Vec<EntityId>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    AddItem { item: Item, owners: Owners },
    UpdateItem { after: Item, owners: Owners },
    DeleteItem(ItemId),
    Renew(Renewal),
    CreateEntity { entity: Entity, owners: Vec<OwnerRef> },
    UpdateEntity { after: Entity, owners: Owners },
    DeleteEntity(EntityId),
    Merge { from: EntityId, into: EntityId },
    Relate { from: EntityId, to: EntityId, role: Role },
    Unrelate { from: EntityId, to: EntityId },
}

/// A checked write: what the card says and what accepting it does.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub title: String,
    pub lines: Vec<String>,
    pub ops: Vec<Op>,
    /// Items to check over the network (RDAP / TLS) once applied.
    pub check: Vec<ItemId>,
    pub undoable: bool,
}

/// What [`undo`] does to take an applied plan back, in reverse order.
#[derive(Debug, Clone, PartialEq)]
pub enum Undo {
    DeleteItem(ItemId),
    RestoreItem(Box<Item>),
    UndoRenewal { renewal: i64, before: Box<Item> },
    DeleteEntity(EntityId),
    RestoreEntity(Box<Entity>),
    Relate { from: EntityId, to: EntityId, role: Role },
    Unrelate { from: EntityId, to: EntityId },
}

/// The data a plan is checked against.
pub struct Snapshot<'a> {
    pub items: &'a [Item],
    pub entities: &'a [Entity],
    pub relations: &'a [Relation],
}

fn ids_arg(a: &Value) -> Result<Vec<i64>, String> {
    let mut v: Vec<i64> = match a.get("ids") {
        Some(Value::Array(xs)) => xs.iter().filter_map(|x| x.as_i64().or_else(|| x.as_str()?.trim().parse().ok())).collect(),
        _ => a.get("id").and_then(|x| x.as_i64().or_else(|| x.as_str()?.trim().parse().ok())).into_iter().collect(),
    };
    v.dedup();
    if v.is_empty() { Err(i18n::t!("cli.err.needs_id")) } else { Ok(v) }
}

fn items_by_id<'a>(snap: &Snapshot<'a>, ids: &[i64]) -> Result<Vec<&'a Item>, String> {
    ids.iter().map(|id| snap.items.iter().find(|i| i.id == *id).ok_or_else(|| i18n::t!("core.db.not_found", id = id))).collect()
}

/// An entity by id, name, plate or tax id.
fn entity_ref<'a>(snap: &Snapshot<'a>, v: Option<&Value>) -> Result<&'a Entity, String> {
    let v = v.ok_or_else(|| i18n::t!("cli.err.needs_entity"))?;
    let text = v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).ok_or_else(|| i18n::t!("cli.err.needs_entity"))?;
    text.parse::<i64>().ok().and_then(|n| snap.entities.iter().find(|e| e.id == n)).or_else(|| entity::find(snap.entities, &text)).ok_or_else(|| i18n::t!("cli.err.no_entity", value = text))
}

fn kind_arg(a: &Value, key: &str) -> Result<Option<EntityKind>, String> {
    str_arg(a, key).map(|k| EntityKind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_entity_kind", value = k))).transpose()
}

/// Owner arguments (ids, names, plates, tax ids; one or a list): unknown names become new
/// entities of `new_kind`, else of the kind their name says (a legal suffix is a company),
/// else people.
fn owner_refs(snap: &Snapshot, v: &Value, new_kind: Option<EntityKind>) -> Vec<OwnerRef> {
    let list: Vec<Value> = match v {
        Value::Array(xs) => xs.clone(),
        Value::Null => Vec::new(),
        x => vec![x.clone()],
    };
    let mut out: Vec<OwnerRef> = Vec::new();
    for x in list {
        let Some(text) = x.as_i64().map(|n| n.to_string()).or_else(|| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) else { continue };
        let r = match text.parse::<i64>().ok().and_then(|n| snap.entities.iter().find(|e| e.id == n)).or_else(|| entity::find(snap.entities, &text)) {
            Some(e) => OwnerRef::Id(e.id),
            None => OwnerRef::New(text.clone(), new_kind.unwrap_or_else(|| entity::guess(&text, None))),
        };
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

fn ref_name(snap: &Snapshot, r: &OwnerRef) -> String {
    match r {
        OwnerRef::Id(id) => snap.entities.iter().find(|e| e.id == *id).map(|e| e.name.clone()).unwrap_or_default(),
        OwnerRef::New(n, k) => i18n::t!("core.agent.new_owner", name = n, kind = k.label()),
    }
}

fn names(snap: &Snapshot, refs: &[OwnerRef]) -> String {
    refs.iter().map(|r| ref_name(snap, r)).collect::<Vec<_>>().join(", ")
}

fn owners_arg(snap: &Snapshot, a: &Value) -> Result<Owners, String> {
    let new_kind = kind_arg(a, "owner_kind")?;
    Ok(if let Some(v) = a.get("owners").or_else(|| a.get("owner")) {
        Owners::Set(owner_refs(snap, v, new_kind))
    } else if let Some(v) = a.get("add_owners") {
        Owners::Add(owner_refs(snap, v, new_kind))
    } else if let Some(v) = a.get("remove_owners") {
        Owners::Remove(owner_refs(snap, v, None).into_iter().filter_map(|r| if let OwnerRef::Id(id) = r { Some(id) } else { None }).collect())
    } else {
        Owners::Keep
    })
}

fn owners_line(snap: &Snapshot, o: &Owners) -> Option<String> {
    match o {
        Owners::Keep => None,
        Owners::Set(v) if v.is_empty() => Some(i18n::t!("core.agent.no_owner")),
        Owners::Set(v) => Some(i18n::t!("core.agent.owners", names = names(snap, v))),
        Owners::Add(v) => Some(i18n::t!("core.agent.add_owners", names = names(snap, v))),
        Owners::Remove(v) => Some(i18n::t!("core.agent.remove_owners", names = names(snap, &v.iter().map(|i| OwnerRef::Id(*i)).collect::<Vec<_>>()))),
    }
}

fn date_or_clear(v: &Value, today: Date, order: Order) -> Result<Option<Date>, String> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) if s.trim().is_empty() => Ok(None),
        Value::String(s) => date::parse_loose(s, today, order).map(Some).ok_or_else(|| i18n::t!("cli.err.bad_date", value = s)),
        x => Err(i18n::t!("cli.err.bad_date", value = x)),
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.trim().to_string(),
        x => x.to_string(),
    }
}

fn shown(d: Option<Date>) -> String {
    d.map(date::iso).unwrap_or_else(|| i18n::t!("core.agent.none"))
}

/// Applies `update_items`' field arguments to `it`; returns the card lines (once per field).
fn patch_item(it: &mut Item, a: &Value, today: Date, order: Order) -> Result<Vec<String>, String> {
    let spec = Kind::Custom.form();
    let mut lines = Vec::new();
    let mut line = |field: String, value: String| lines.push(i18n::t!("core.agent.set", field = field, value = value));
    if let Some(v) = str_arg(a, "kind") {
        let k = Kind::from_id(v).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = v))?;
        if k != it.kind {
            it.kind = k;
            it.category = k.category();
            if k.checkable() {
                it.auto_check = true;
            }
        }
        line(i18n::t!("core.agent.kind"), k.label());
    }
    if let Some(v) = str_arg(a, "name") {
        it.name = v.to_string();
        line(i18n::t!("core.agent.name"), v.to_string());
    }
    if let Some(v) = str_arg(a, "category") {
        it.category = Category::from_id(v).ok_or_else(|| i18n::t!("cli.err.bad_category", value = v))?;
        line(i18n::t!("core.agent.category"), it.category.label());
    }
    if let Some(v) = a.get("issued") {
        it.issued = date_or_clear(v, today, order)?;
        line(spec.label(Field::Issued), shown(it.issued));
    }
    if let Some(v) = a.get("expires") {
        it.expires = date_or_clear(v, today, order)?;
        line(spec.label(Field::Expires), shown(it.expires));
    }
    if let Some(v) = a.get("recurrence") {
        let t = text(v);
        it.recurrence = Recurrence::parse(&t).ok_or_else(|| i18n::t!("cli.err.bad_recurrence", value = t))?;
        line(spec.label(Field::Recurrence), it.recurrence.label());
    }
    if let Some(v) = a.get("lead_days") {
        it.lead_days = match v {
            Value::Array(xs) => crate::model::parse_leads(&xs.iter().map(text).collect::<Vec<_>>().join(",")),
            x => crate::model::parse_leads(&text(x)),
        };
        line(spec.label(Field::Leads), crate::model::leads_text(&it.leads()));
    }
    if let Some(v) = a.get("cost") {
        it.cost = match v {
            Value::Null => None,
            Value::Number(n) => n.as_f64(),
            x => {
                let t = text(x);
                if t.is_empty() { None } else { Some(t.replace(',', ".").parse().map_err(|_| i18n::t!("cli.err.bad_number", value = t))?) }
            }
        };
        line(spec.label(Field::Cost), it.cost.map(|c| c.to_string()).unwrap_or_else(|| i18n::t!("core.agent.none")));
    }
    if let Some(v) = a.get("currency") {
        it.currency = text(v).to_uppercase();
        line(i18n::t!("core.agent.currency"), it.currency.clone());
    }
    for (key, field) in [("provider", Field::Provider), ("reference", Field::Reference), ("renewal_url", Field::RenewalUrl), ("host", Field::Host)] {
        if let Some(v) = a.get(key) {
            let t = text(v);
            match field {
                Field::Provider => it.provider = t.clone(),
                Field::Reference => it.reference = t.clone(),
                Field::RenewalUrl => it.renewal_url = t.clone(),
                _ => it.host = t.clone(),
            }
            line(spec.label(field), if t.is_empty() { i18n::t!("core.agent.none") } else { t });
        }
    }
    if let Some(v) = a.get("notes") {
        it.notes = text(v);
        line(spec.label(Field::Notes), it.notes.clone());
    }
    if let Some(v) = a.get("tags") {
        it.tags = match v {
            Value::Array(xs) => xs.iter().map(text).filter(|s| !s.is_empty()).collect(),
            x => text(x).split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect(),
        };
        line(spec.label(Field::Tags), it.tags.join(", "));
    }
    for (key, field) in [("auto_renews", Field::AutoRenews), ("auto_check", Field::AutoCheck)] {
        if let Some(b) = a.get(key).and_then(Value::as_bool) {
            if field == Field::AutoRenews { it.auto_renews = b } else { it.auto_check = b && it.kind.checkable() }
            line(spec.label(field), if b { i18n::t!("core.agent.yes") } else { i18n::t!("core.agent.no") });
        }
    }
    if let Some(v) = a.get("business_days") {
        let t = text(v);
        it.business_days = if t.is_empty() || t == "none" { None } else { Some(crate::holidays::BusinessDays::parse(&t).ok_or_else(|| i18n::t!("cli.err.bad_business_days", value = t))?) };
        line(spec.label(Field::BusinessDays), it.business_days.map(|b| b.label()).unwrap_or_else(|| i18n::t!("core.agent.none")));
    }
    if let Some(v) = a.get("term_months") {
        it.term_months = v.as_u64().map(|n| n as u32);
        line(spec.label(Field::Term), text(v));
    }
    if let Some(v) = a.get("notice_days") {
        it.notice_days = v.as_u64().map(|n| n as u32);
        line(spec.label(Field::Notice), text(v));
    }
    if let Some(v) = str_arg(a, "state") {
        it.state = match v {
            "active" | "in_progress" | "archived" => State::from_id(v),
            _ => return Err(i18n::t!("core.agent.bad_state", value = v)),
        };
        line(i18n::t!("core.agent.state"), state_label(it.state));
    }
    if let Some(v) = a.get("snoozed_until") {
        it.snoozed_until = date_or_clear(v, today, order)?;
        line(i18n::t!("core.agent.snoozed"), shown(it.snoozed_until));
    }
    Ok(lines)
}

fn state_label(s: State) -> String {
    match s {
        State::Active => i18n::t!("core.agent.state_active"),
        State::InProgress => i18n::t!("core.agent.state_in_progress"),
        State::Archived => i18n::t!("core.agent.state_archived"),
    }
}

/// Item names for a card: up to five, then "+N more".
fn item_names(items: &[&Item]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().take(5).map(|i| if i.owner.is_empty() { i.name.clone() } else { i.title() }).collect();
    if items.len() > 5 {
        v.push(i18n::tn!("core.agent.more", items.len() - 5));
    }
    v
}

fn titled(items: &[&Item], one: &str, many: &str) -> String {
    match items {
        [i] => i18n::tr(one, &[("name", &i.name as &dyn std::fmt::Display)]),
        _ => i18n::tr_n(many, items.len() as i64, &[]),
    }
}

fn entity_fields(e: &mut Entity, a: &Value) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    if let Some(k) = kind_arg(a, "kind")? {
        e.kind = k;
        lines.push(i18n::t!("core.agent.set", field = i18n::t!("core.agent.kind"), value = k.label()));
    }
    if let Some(v) = str_arg(a, "name") {
        e.name = v.to_string();
    }
    for key in ["ident", "detail", "country", "notes"] {
        if let Some(v) = a.get(key) {
            let t = text(v);
            match key {
                "ident" => e.ident = t.clone(),
                "detail" => e.detail = t.clone(),
                "country" => e.country = t.clone(),
                _ => e.notes = t.clone(),
            }
            let shown = if key == "ident" && e.kind == EntityKind::Person { entity::masked(&t) } else { t };
            let field = match key {
                "ident" => e.kind.ident_label().unwrap_or_else(|| i18n::t!("core.agent.ident")),
                "detail" => e.kind.detail_label().unwrap_or_else(|| i18n::t!("core.agent.detail")),
                "country" => i18n::t!("core.agent.country"),
                _ => i18n::t!("core.field.notes"),
            };
            lines.push(i18n::t!("core.agent.set", field = field, value = shown));
        }
    }
    if let Some(v) = a.get("year") {
        e.year = v.as_i64().map(|y| y as i32).or_else(|| text(v).parse().ok());
        lines.push(i18n::t!("core.agent.set", field = i18n::t!("core.agent.year"), value = text(v)));
    }
    Ok(lines)
}

/// Checks a write tool's arguments against `snap` and says what accepting it would do.
pub fn plan(tool: &str, a: &Value, snap: &Snapshot, today: Date, order: Order) -> Result<Plan, String> {
    let mut lines: Vec<String> = Vec::new();
    let p = |title: String, lines: Vec<String>, ops: Vec<Op>| Plan { title, lines, ops, check: Vec::new(), undoable: true };
    Ok(match tool {
        "add_item" | "propose_add_from_template" => {
            let mut it = if tool == "add_item" { item_from_args(a, today, order)? } else { item_from_template_args(a, snap.entities, today, order)? };
            it.tidy(today);
            // a typed owner the data knows is that entity
            let owners = match (it.entity, it.owner.trim()) {
                (Some(id), _) => Owners::Set(vec![OwnerRef::Id(id)]),
                (None, "") => Owners::Keep,
                (None, name) => Owners::Set(owner_refs(snap, &Value::from(name), it.owner_kind.or_else(|| Some(entity::guess(name, Some(it.kind)))))),
            };
            lines.push(it.kind.label());
            lines.extend(owners_line(snap, &owners));
            match it.end_date(today) {
                Some(e) => lines.push(i18n::t!("core.agent.date", date = date::iso(e))),
                None if !it.kind.checkable() => lines.push(i18n::t!("core.agent.no_date")),
                None => lines.push(i18n::t!("core.agent.will_check", source = crate::checks::source(&it))),
            }
            if !it.host.is_empty() {
                lines.push(it.host.clone());
            }
            let title = i18n::t!("core.agent.add", name = it.name);
            p(title, lines, vec![Op::AddItem { item: it, owners }])
        }
        "renew_item" => {
            let it = items_by_id(snap, &[needs_id(a)?])?[0];
            let r = renewal_from_args(it, a, today, order)?;
            lines.push(i18n::t!("core.agent.renew_dates", before = shown(it.end_date(today)), after = date::iso(r.expires)));
            if let Some(c) = r.cost {
                lines.push(i18n::t!("core.agent.set", field = Kind::Custom.form().label(Field::Cost), value = c));
            }
            if !r.note.is_empty() {
                lines.push(r.note.clone());
            }
            p(i18n::t!("core.agent.renew", name = it.name), lines, vec![Op::Renew(r)])
        }
        "update_items" => {
            let items = items_by_id(snap, &ids_arg(a)?)?;
            if items.len() > 1 && str_arg(a, "name").is_some() {
                return Err(i18n::t!("core.agent.one_name"));
            }
            let owners = owners_arg(snap, a)?;
            let mut ops = Vec::new();
            for it in &items {
                let mut after = (*it).clone();
                lines = patch_item(&mut after, a, today, order)?;
                after.tidy(today);
                ops.push(Op::UpdateItem { after, owners: owners.clone() });
            }
            let mut out: Vec<String> = owners_line(snap, &owners).into_iter().collect();
            out.extend(lines);
            if out.is_empty() {
                return Err(i18n::t!("core.agent.nothing"));
            }
            if items.len() > 1 {
                out.extend(item_names(&items));
            }
            p(titled(&items, "core.agent.update_one", "core.agent.update_many"), out, ops)
        }
        "delete_items" => {
            let items = items_by_id(snap, &ids_arg(a)?)?;
            let mut out = item_names(&items);
            out.push(i18n::t!("core.agent.permanent"));
            Plan { undoable: false, ..p(titled(&items, "core.agent.delete_one", "core.agent.delete_many"), out, items.iter().map(|i| Op::DeleteItem(i.id)).collect()) }
        }
        "convert_celebrations" => {
            let items = items_by_id(snap, &ids_arg(a)?)?;
            let forced = str_arg(a, "kind").map(|k| Kind::from_id(k).filter(|k| k.is_celebration()).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = k))).transpose()?;
            let mut ops = Vec::new();
            for it in &items {
                let mut after = (*it).clone();
                after.kind = forced.or_else(|| Kind::celebration_in(&it.name)).unwrap_or(Kind::Birthday);
                after.normalize_celebration(today);
                if after.celebration_day().is_none() || after.recurrence == it.recurrence && after.kind == it.kind {
                    return Err(i18n::t!("core.agent.no_day", name = it.name));
                }
                let (m, d) = after.celebration_day().unwrap_or((1, 1));
                let when = match after.issued {
                    Some(i) => date::iso(i),
                    None => format!("{d:02}/{m:02}"), // i18n-ok: day/month
                };
                lines.push(i18n::t!("core.agent.convert_line", name = it.name, kind = after.kind.label(), date = when));
                ops.push(Op::UpdateItem { after, owners: Owners::Keep });
            }
            p(titled(&items, "core.agent.convert_one", "core.agent.convert_many"), lines, ops)
        }
        "check_items" => {
            let items = items_by_id(snap, &ids_arg(a)?)?;
            if let Some(i) = items.iter().find(|i| !i.kind.checkable()) {
                return Err(i18n::t!("core.check.not_checkable") + " (" + &i.name + ")"); // i18n-ok: name in parentheses
            }
            let mut plan = p(titled(&items, "core.agent.check_one", "core.agent.check_many"), item_names(&items), Vec::new());
            plan.check = items.iter().map(|i| i.id).collect();
            plan.undoable = false;
            plan
        }
        "create_entity" => {
            let name = str_arg(a, "name").ok_or_else(|| i18n::t!("cli.err.needs_entity"))?;
            if let Some(e) = snap.entities.iter().find(|e| e.name.trim().eq_ignore_ascii_case(name)) {
                return Err(i18n::t!("core.agent.exists", name = e.name, id = e.id));
            }
            let kind = kind_arg(a, "kind")?.unwrap_or_else(|| entity::guess(name, None));
            let mut e = Entity::new(kind, name);
            let mut out = vec![kind.label()];
            out.extend(entity_fields(&mut e, a)?);
            e.kind = kind;
            let owners = owner_refs(snap, a.get("owners").or_else(|| a.get("owned_by")).unwrap_or(&Value::Null), None);
            if !owners.is_empty() {
                out.push(i18n::t!("core.agent.owned_by", names = names(snap, &owners)));
            }
            p(i18n::t!("core.agent.create_entity", kind = kind.label(), name = e.name), out, vec![Op::CreateEntity { entity: e, owners }])
        }
        "update_entity" | "archive_entity" => {
            let before = entity_ref(snap, a.get("entity").or_else(|| a.get("id")))?;
            let mut after = before.clone();
            let mut out = Vec::new();
            if tool == "archive_entity" {
                after.archived = a.get("archived").and_then(Value::as_bool).unwrap_or(true);
                let n = snap.items.iter().filter(|i| i.owned_by(before.id) && i.state != State::Archived).count();
                out.push(i18n::tn!("core.agent.archive_keeps", n));
                let title = if after.archived { i18n::t!("core.agent.archive_entity", name = before.name) } else { i18n::t!("core.agent.unarchive_entity", name = before.name) };
                return Ok(p(title, out, vec![Op::UpdateEntity { after, owners: Owners::Keep }]));
            }
            out.extend(entity_fields(&mut after, a)?);
            if after.name != before.name {
                out.insert(0, i18n::t!("core.agent.rename", before = before.name, after = after.name));
            }
            if let Some(b) = a.get("archived").and_then(Value::as_bool) {
                after.archived = b;
                out.push(i18n::t!("core.agent.set", field = i18n::t!("core.agent.state"), value = if b { i18n::t!("core.agent.state_archived") } else { i18n::t!("core.agent.state_active") }));
            }
            let owners = owners_arg(snap, a)?;
            if let Some(l) = owners_line(snap, &owners) {
                out.push(i18n::t!("core.agent.owned_by", names = l));
            }
            if out.is_empty() {
                return Err(i18n::t!("core.agent.nothing"));
            }
            p(i18n::t!("core.agent.update_entity", name = before.name), out, vec![Op::UpdateEntity { after, owners }])
        }
        "delete_entity" => {
            let e = entity_ref(snap, a.get("entity").or_else(|| a.get("id")))?;
            let n = snap.items.iter().filter(|i| i.owned_by(e.id)).count();
            Plan { undoable: false, ..p(i18n::t!("core.agent.delete_entity", name = e.name), vec![i18n::tn!("core.agent.delete_entity_items", n), i18n::t!("core.agent.permanent")], vec![Op::DeleteEntity(e.id)]) }
        }
        "merge_entities" => {
            let from = entity_ref(snap, a.get("from"))?;
            let into = entity_ref(snap, a.get("into"))?;
            if from.id == into.id {
                return Err(i18n::t!("core.db.self_relation"));
            }
            let n = snap.items.iter().filter(|i| i.owned_by(from.id)).count();
            Plan { undoable: false, ..p(i18n::t!("core.agent.merge", from = from.name, into = into.name), vec![i18n::tn!("core.agent.merge_items", n), i18n::t!("core.agent.permanent")], vec![Op::Merge { from: from.id, into: into.id }]) }
        }
        "relate" => {
            let from = entity_ref(snap, a.get("from"))?;
            let to = entity_ref(snap, a.get("to"))?;
            if from.id == to.id {
                return Err(i18n::t!("core.db.self_relation"));
            }
            let role = Role::from_id(str_arg(a, "role").unwrap_or("owner"));
            p(i18n::t!("core.agent.relate", from = from.name, role = role.label(), to = to.name), Vec::new(), vec![Op::Relate { from: from.id, to: to.id, role }])
        }
        "unrelate" => {
            let from = entity_ref(snap, a.get("from"))?;
            let to = entity_ref(snap, a.get("to"))?;
            let r = snap.relations.iter().find(|r| (r.from, r.to) == (from.id, to.id) || (r.from, r.to) == (to.id, from.id)).ok_or_else(|| i18n::t!("core.agent.no_link", from = from.name, to = to.name))?;
            p(i18n::t!("core.agent.unrelate", from = from.name, to = to.name), vec![r.role.label()], vec![Op::Unrelate { from: r.from, to: r.to }])
        }
        other => return Err(format!("unknown tool {other:?}")), // i18n-ok: protocol
    })
}

fn resolve(s: &Store, refs: &[OwnerRef], undo: &mut Vec<Undo>) -> db::Result<Vec<EntityId>> {
    let mut out = Vec::new();
    for r in refs {
        let id = match r {
            OwnerRef::Id(id) => *id,
            OwnerRef::New(name, kind) => {
                let (id, made) = s.find_or_create_entity(name, Some(*kind))?;
                if made {
                    undo.push(Undo::DeleteEntity(id));
                }
                id
            }
        };
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// Sets `it`'s owners to `ids` (the first is the main one).
fn set_owners(s: &Store, it: &mut Item, ids: &[EntityId]) -> db::Result<()> {
    match ids.first() {
        Some(first) => {
            let e = s.entity(*first)?;
            it.entity = Some(e.id);
            it.owner = e.name;
            it.owner_kind = Some(e.kind);
            it.co_owners = ids[1..].to_vec();
        }
        None => {
            it.entity = None;
            it.owner.clear();
            it.co_owners.clear();
        }
    }
    Ok(())
}

fn apply_owners(s: &Store, it: &mut Item, o: &Owners, undo: &mut Vec<Undo>) -> db::Result<()> {
    let ids = match o {
        Owners::Keep => return Ok(()),
        Owners::Set(v) => resolve(s, v, undo)?,
        Owners::Add(v) => {
            let mut ids = it.owner_ids();
            for id in resolve(s, v, undo)? {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            ids
        }
        Owners::Remove(v) => it.owner_ids().into_iter().filter(|i| !v.contains(i)).collect(),
    };
    set_owners(s, it, &ids)
}

/// Entity ownership links into `e`: who owns it.
fn apply_entity_owners(s: &Store, e: EntityId, o: &Owners, undo: &mut Vec<Undo>) -> db::Result<()> {
    let rels = s.relations()?;
    let current: Vec<EntityId> = entity::owners_of(e, &rels);
    let want: Vec<EntityId> = match o {
        Owners::Keep => return Ok(()),
        Owners::Set(v) => resolve(s, v, undo)?,
        Owners::Add(v) => current.iter().copied().chain(resolve(s, v, undo)?).collect(),
        Owners::Remove(v) => current.iter().copied().filter(|i| !v.contains(i)).collect(),
    };
    for from in want.iter().filter(|f| !current.contains(f) && **f != e) {
        let prev = rels.iter().find(|r| r.from == *from && r.to == e).map(|r| r.role);
        s.relate(*from, e, Role::Owner)?;
        undo.push(match prev {
            Some(role) => Undo::Relate { from: *from, to: e, role },
            None => Undo::Unrelate { from: *from, to: e },
        });
    }
    for from in current.iter().filter(|f| !want.contains(f)) {
        if let Some(r) = rels.iter().find(|r| r.from == *from && r.to == e) {
            s.unrelate(r.id)?;
            undo.push(Undo::Relate { from: *from, to: e, role: Role::Owner });
        }
    }
    Ok(())
}

/// What an applied plan did: how to take it back, and the items it touched.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Applied {
    pub undo: Vec<Undo>,
    pub items: Vec<ItemId>,
    pub entities: Vec<EntityId>,
}

/// Runs `plan` on the database (inside [`Store::write`] in the app).
pub fn apply(s: &mut Store, plan: &Plan, today: Date) -> db::Result<Applied> {
    let mut out = Applied::default();
    for op in &plan.ops {
        match op {
            Op::AddItem { item, owners } => {
                let mut it = item.clone();
                apply_owners(s, &mut it, owners, &mut out.undo)?;
                let id = s.insert(&it)?;
                out.undo.push(Undo::DeleteItem(id));
                out.items.push(id);
            }
            Op::UpdateItem { after, owners } => {
                let before = s.item(after.id)?;
                let mut it = after.clone();
                apply_owners(s, &mut it, owners, &mut out.undo)?;
                s.update(&it)?;
                out.undo.push(Undo::RestoreItem(Box::new(before)));
                out.items.push(it.id);
            }
            Op::DeleteItem(id) => s.delete(*id)?,
            Op::Renew(r) => {
                let before = s.item(r.id)?;
                s.renew(r.id, r.expires, today, r.cost, &r.note)?;
                let rid = s.renewals(r.id)?.first().map(|x| x.id).unwrap_or_default();
                out.undo.push(Undo::UndoRenewal { renewal: rid, before: Box::new(before) });
                out.items.push(r.id);
            }
            Op::CreateEntity { entity, owners } => {
                let id = s.insert_entity(entity)?;
                out.undo.push(Undo::DeleteEntity(id));
                for from in resolve(s, owners, &mut out.undo)? {
                    s.relate(from, id, Role::Owner)?;
                }
                out.entities.push(id);
            }
            Op::UpdateEntity { after, owners } => {
                let before = s.entity(after.id)?;
                s.update_entity(after)?;
                out.undo.push(Undo::RestoreEntity(Box::new(before)));
                apply_entity_owners(s, after.id, owners, &mut out.undo)?;
                out.entities.push(after.id);
            }
            Op::DeleteEntity(id) => s.delete_entity(*id)?,
            Op::Merge { from, into } => s.merge_entity(*from, *into)?,
            Op::Relate { from, to, role } => {
                let prev = s.relations()?.into_iter().find(|r| r.from == *from && r.to == *to).map(|r| r.role);
                s.relate(*from, *to, *role)?;
                out.undo.push(match prev {
                    Some(role) => Undo::Relate { from: *from, to: *to, role },
                    None => Undo::Unrelate { from: *from, to: *to },
                });
            }
            Op::Unrelate { from, to } => {
                if let Some(r) = s.relations()?.into_iter().find(|r| r.from == *from && r.to == *to) {
                    s.unrelate(r.id)?;
                    out.undo.push(Undo::Relate { from: *from, to: *to, role: r.role });
                }
            }
        }
    }
    out.items.extend(plan.check.iter().copied());
    Ok(out)
}

/// Takes an applied plan back, newest step first.
pub fn undo(s: &Store, steps: &[Undo]) -> db::Result<()> {
    for u in steps.iter().rev() {
        match u {
            Undo::DeleteItem(id) => s.delete(*id)?,
            Undo::RestoreItem(it) => s.update(it)?,
            Undo::UndoRenewal { renewal, before } => s.undo_renewal(*renewal, before)?,
            Undo::DeleteEntity(id) => s.delete_entity(*id)?,
            Undo::RestoreEntity(e) => s.update_entity(e)?,
            Undo::Relate { from, to, role } => {
                s.relate(*from, *to, *role)?;
            }
            Undo::Unrelate { from, to } => {
                if let Some(r) = s.relations()?.into_iter().find(|r| r.from == *from && r.to == *to) {
                    s.unrelate(r.id)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn day(s: &str) -> Date {
        date::parse_iso(s).unwrap()
    }

    #[test]
    fn add_arguments_build_the_item_without_saving() {
        let today = day("2026-11-23");
        let it = item_from_args(&json!({"kind": "certificate", "name": "TLS example.org", "expires": "2027-01-15", "host": "example.org"}), today, Order::DayFirst).unwrap();
        assert_eq!((it.kind, it.name.as_str(), it.host.as_str()), (Kind::Certificate, "TLS example.org", "example.org"));
        assert_eq!(it.expires, Some(day("2027-01-15")));
        assert_eq!(it.id, 0, "not saved");
        assert!(item_from_args(&json!({}), today, Order::DayFirst).is_err());
        assert!(item_from_args(&json!({"kind": "nope"}), today, Order::DayFirst).is_err());
        assert!(item_from_args(&json!({"name": "x", "expires": "someday"}), today, Order::DayFirst).is_err());
    }

    #[test]
    fn template_arguments_find_the_owner_or_make_one() {
        use crate::entity::EntityKind;
        let today = day("2026-11-23");
        let car = Entity { id: 2, kind: EntityKind::Vehicle, name: "Mazda".into(), ident: "ABC123".into(), ..Default::default() };
        let ana = Entity { id: 1, kind: EntityKind::Person, name: "Ana".into(), ..Default::default() };
        let es = vec![ana.clone(), car.clone()];
        // "recuérdame el SOAT del carro": the only vehicle
        let it = item_from_template_args(&json!({"template": "soat"}), &es, today, Order::DayFirst).unwrap();
        assert_eq!((it.kind, it.entity, it.owner.as_str(), it.expires), (Kind::Soat, Some(2), "Mazda", None));
        let it = item_from_template_args(&json!({"template": "soat", "owner": "abc-123", "expires": "2027-03-01"}), &es, today, Order::DayFirst).unwrap();
        assert_eq!((it.entity, it.expires), (Some(2), Some(day("2027-03-01"))));
        let it = item_from_template_args(&json!({"template": "pet_rabies", "owner": "Luna"}), &es, today, Order::DayFirst).unwrap();
        assert_eq!((it.entity, it.owner.as_str(), it.owner_kind), (None, "Luna", Some(EntityKind::Pet)), "a new pet");
        for owner in [json!(1), json!("1"), json!("ana")] {
            assert_eq!(item_from_template_args(&json!({"template": "passport", "owner": owner}), &es, today, Order::DayFirst).unwrap().entity, Some(1));
        }
        assert!(item_from_template_args(&json!({"template": "pet_rabies"}), &es, today, Order::DayFirst).is_err(), "no pet to pick");
        assert!(item_from_template_args(&json!({"template": "nope", "owner": "Ana"}), &es, today, Order::DayFirst).is_err());
        assert!(item_from_template_args(&json!({"owner": "Ana"}), &es, today, Order::DayFirst).is_err());
        assert!(item_from_template_args(&json!({"template": "soat", "expires": "someday"}), &es, today, Order::DayFirst).is_err());
    }

    #[test]
    fn renewals_default_to_the_suggested_next_expiry() {
        let today = day("2026-11-23");
        let it = Item { id: 4, expires: Some(day("2026-12-01")), ..Item::new(Kind::Domain, "example.org") };
        let r = renewal_from_args(&it, &json!({}), today, Order::DayFirst).unwrap();
        assert_eq!((r.id, r.expires), (4, it.suggest_next(today)));
        let r = renewal_from_args(&it, &json!({"expires": "2028-12-01", "cost": 12.5, "note": "2 years"}), today, Order::DayFirst).unwrap();
        assert_eq!((r.expires, r.cost, r.note.as_str()), (day("2028-12-01"), Some(12.5), "2 years"));
    }

    #[test]
    fn proposals_are_collected_once_in_order() {
        let d = tempfile::tempdir().unwrap();
        let a = Proposal { tool: "add_item".into(), args: json!({"text": "Pasaporte @Ana 12/05/2031"}), reason: String::new() };
        let b = Proposal { tool: "renew_item".into(), args: json!({"id": 1}), reason: "paid".into() };
        deliver(d.path(), &a).unwrap();
        deliver(d.path(), &b).unwrap();
        std::fs::write(d.path().join(".half.json.tmp"), "{").unwrap();
        assert_eq!(collect(d.path()), vec![a, b]);
        assert!(collect(d.path()).is_empty());
        assert!(collect(&d.path().join("missing")).is_empty());
    }

    /// The user's request: "they all belong to Otherwise SAS, create it; vcity.io belongs to
    /// Otherwise Technologies LLC, create it too".
    #[test]
    fn companies_are_created_and_domains_reassigned_then_undone() {
        use crate::db::Store;
        use crate::entity::EntityKind;
        let today = day("2026-11-24");
        let d = tempfile::tempdir().unwrap();
        let mut s = Store::create(&d.path().join("a.db")).unwrap();
        let juan = s.insert_entity(&Entity::new(EntityKind::Person, "Juan")).unwrap();
        let ids: Vec<i64> = ["otherwise.co", "otherwisetech.com", "vcity.io"].iter().map(|n| s.insert(&Item { owner: "Juan".into(), entity: Some(juan), ..Item::new(Kind::Domain, *n) }).unwrap()).collect();
        let snap = |s: &Store| (s.items().unwrap(), s.entities().unwrap(), s.relations().unwrap());
        let run = |s: &mut Store, tool: &str, a: Value| {
            let (i, e, r) = snap(s);
            let p = plan(tool, &a, &Snapshot { items: &i, entities: &e, relations: &r }, today, Order::DayFirst).unwrap();
            (p.clone(), apply(s, &p, today).unwrap())
        };
        let (p, _) = run(&mut s, "create_entity", json!({"name": "Otherwise SAS", "owners": ["Juan"]}));
        assert!(p.title.contains("Otherwise SAS") && p.undoable);
        let sas = s.entities().unwrap().into_iter().find(|e| e.name == "Otherwise SAS").unwrap();
        assert_eq!(sas.kind, EntityKind::Company, "the legal suffix says so");
        assert_eq!(crate::entity::owners_of(sas.id, &s.relations().unwrap()), vec![juan]);
        let (i, e, r) = snap(&s);
        assert!(plan("create_entity", &json!({"name": "otherwise sas"}), &Snapshot { items: &i, entities: &e, relations: &r }, today, Order::DayFirst).is_err(), "no duplicates");
        let (p, done) = run(&mut s, "update_items", json!({"ids": ids[..2], "owners": ["Otherwise SAS"]}));
        assert_eq!(p.ops.len(), 2);
        assert!(s.items().unwrap().iter().filter(|i| ids[..2].contains(&i.id)).all(|i| i.owner_ids() == vec![sas.id]));
        // a company that doesn't exist yet is created by the change, and undone with it
        let (p, done2) = run(&mut s, "update_items", json!({"ids": [ids[2]], "owners": ["Otherwise Technologies LLC"], "auto_renews": true}));
        assert!(p.lines.iter().any(|l| l.contains("Otherwise Technologies LLC")), "{:?}", p.lines);
        let llc = s.entities().unwrap().into_iter().find(|e| e.name == "Otherwise Technologies LLC").unwrap();
        assert_eq!(llc.kind, EntityKind::Company);
        let v = s.item(ids[2]).unwrap();
        assert_eq!((v.entity, v.auto_renews, v.host.as_str()), (Some(llc.id), true, "vcity.io"));
        undo(&s, &done2.undo).unwrap();
        let v = s.item(ids[2]).unwrap();
        assert_eq!((v.entity, v.auto_renews), (Some(juan), false));
        assert!(s.entities().unwrap().iter().all(|e| e.name != "Otherwise Technologies LLC"), "the company made for it goes too");
        undo(&s, &done.undo).unwrap();
        assert!(s.items().unwrap().iter().all(|i| i.entity == Some(juan)));
        // several owners at once, then one more, then one less
        let ana = s.insert_entity(&Entity::new(EntityKind::Person, "Ana")).unwrap();
        run(&mut s, "update_items", json!({"ids": [ids[0]], "owners": ["Juan", "Ana"]}));
        assert_eq!(s.item(ids[0]).unwrap().owner_ids(), vec![juan, ana]);
        run(&mut s, "update_items", json!({"ids": [ids[0]], "add_owners": [sas.id]}));
        assert_eq!(s.item(ids[0]).unwrap().owner_ids(), vec![juan, ana, sas.id]);
        run(&mut s, "update_items", json!({"ids": [ids[0]], "remove_owners": ["Juan"]}));
        assert_eq!(s.item(ids[0]).unwrap().owner_ids(), vec![ana, sas.id]);
        // the company owns a car; archive it and restore it
        run(&mut s, "create_entity", json!({"name": "Mazda ABC123", "owners": [sas.id, "Ana"]}));
        let car = s.entities().unwrap().into_iter().find(|e| e.name == "Mazda ABC123").unwrap();
        assert_eq!((car.kind, car.ident.as_str()), (EntityKind::Vehicle, "ABC123"));
        assert_eq!(crate::entity::owned_through(juan, &s.relations().unwrap()), vec![sas.id, car.id]);
        let (_, a) = run(&mut s, "archive_entity", json!({"entity": "ABC123"}));
        assert!(s.entity(car.id).unwrap().archived);
        undo(&s, &a.undo).unwrap();
        assert!(!s.entity(car.id).unwrap().archived);
        let (_, a) = run(&mut s, "update_entity", json!({"entity": car.id, "remove_owners": ["Ana"], "detail": "Mazda 3"}));
        assert_eq!(crate::entity::owners_of(car.id, &s.relations().unwrap()), vec![sas.id]);
        undo(&s, &a.undo).unwrap();
        assert_eq!(crate::entity::owners_of(car.id, &s.relations().unwrap()).len(), 2);
        assert_eq!(s.entity(car.id).unwrap().detail, "");
        // field edits, archive, delete
        run(&mut s, "update_items", json!({"ids": [ids[1]], "expires": "13/08/2027", "cost": 12.5, "provider": "Namecheap", "lead_days": [30, 7], "tags": ["web"], "recurrence": "every:2y", "state": "archived"}));
        let it = s.item(ids[1]).unwrap();
        assert_eq!((it.expires, it.cost, it.provider.as_str(), it.lead_days.clone(), it.state), (Some(day("2027-08-13")), Some(12.5), "Namecheap", vec![30, 7], State::Archived));
        let (i, e, r) = snap(&s);
        let sn = Snapshot { items: &i, entities: &e, relations: &r };
        for bad in [json!({"ids": [999], "cost": 1}), json!({"ids": [ids[0]]}), json!({"ids": [ids[0]], "expires": "someday"}), json!({"ids": ids, "name": "x"}), json!({"ids": [ids[0]], "state": "gone"})] {
            assert!(plan("update_items", &bad, &sn, today, Order::DayFirst).is_err(), "{bad}");
        }
        let del = plan("delete_items", &json!({"ids": [ids[1]]}), &sn, today, Order::DayFirst).unwrap();
        assert!(!del.undoable);
        apply(&mut s, &del, today).unwrap();
        assert!(s.item(ids[1]).is_err());
        let chk = plan("check_items", &json!({"ids": [ids[0]]}), &sn, today, Order::DayFirst).unwrap();
        assert_eq!(chk.check, vec![ids[0]]);
        assert!(plan("relate", &json!({"from": "Juan", "to": "Juan"}), &sn, today, Order::DayFirst).is_err());
        let (_, rel) = run(&mut s, "relate", json!({"from": "Ana", "to": "Otherwise SAS", "role": "manager"}));
        undo(&s, &rel.undo).unwrap();
        assert!(!s.relations().unwrap().iter().any(|r| r.from == ana && r.to == sas.id));
    }

    #[test]
    fn birthdays_convert_with_their_day_and_year() {
        use crate::db::Store;
        let today = day("2026-11-24");
        let d = tempfile::tempdir().unwrap();
        let mut s = Store::create(&d.path().join("b.db")).unwrap();
        let a = s.insert(&Item { expires: Some(day("1990-12-03")), owner: "Juan José".into(), ..Item::new(Kind::Custom, "Cumpleaños Juan José") }).unwrap();
        let b = s.insert(&Item { expires: Some(day("2027-03-10")), ..Item::new(Kind::Custom, "Aniversario de bodas") }).unwrap();
        let c = s.insert(&Item::new(Kind::Custom, "Cumpleaños sin fecha")).unwrap();
        let (i, e, r) = (s.items().unwrap(), s.entities().unwrap(), s.relations().unwrap());
        let sn = Snapshot { items: &i, entities: &e, relations: &r };
        assert!(plan("convert_celebrations", &json!({"ids": [c]}), &sn, today, Order::DayFirst).is_err(), "no date to take the day from");
        let p = plan("convert_celebrations", &json!({"ids": [a, b]}), &sn, today, Order::DayFirst).unwrap();
        apply(&mut s, &p, today).unwrap();
        let x = s.item(a).unwrap();
        assert_eq!((x.kind, x.recurrence, x.issued, x.expires), (Kind::Birthday, Recurrence::Yearly { month: 12, day: 3 }, Some(day("1990-12-03")), None));
        assert_eq!(x.due_date(today), Some(day("2026-12-03")));
        assert!(x.celebration_text(day("2026-12-03")).unwrap().contains("36"));
        let y = s.item(b).unwrap();
        assert_eq!((y.kind, y.issued, y.due_date(today)), (Kind::Anniversary, None, Some(day("2027-03-10"))), "a future date only gives the day");
        assert_eq!(Kind::celebration_in("Cumpleaños de Ana"), Some(Kind::Birthday));
        assert_eq!(Kind::celebration_in("Anniversaire de Luc"), Some(Kind::Birthday));
        assert_eq!(Kind::celebration_in("Anniversaire de mariage"), Some(Kind::Anniversary));
        assert_eq!(Kind::celebration_in("SOAT"), None);
    }
}

