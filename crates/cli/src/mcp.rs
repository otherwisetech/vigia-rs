//! `vigia mcp`: a stdio MCP server (JSON-RPC 2.0, one message per line) over one Vigía
//! database, proxied by Otherwise Apps' `otherwise-hub` as `vigia.*` and started by the app's AI
//! panel for one session. Read-only unless started with `--allow-writes` (the hub passes it only
//! when the user enabled write tools) or `--propose <folder>` (the panel: the write tools only
//! check the change and leave it in that folder for the app to show as a card). Writes only add
//! items or renew them (the renewal history keeps the old date). The database is opened fresh
//! for every call, so the server sees what the app wrote.
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use vigia_core::date;
use vigia_core::db::Store;
use vigia_core::entity::EntityKind;
use vigia_core::model::{Category, Item, Kind, Status};
use vigia_core::view::{self, Filter};

pub const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub struct Server {
    pub db: PathBuf,
    pub writes: bool,
    /// The user's own database: writes refresh the Today feed.
    pub publish: bool,
    /// The app's proposals folder: write tools become cards there instead of writing.
    pub propose: Option<PathBuf>,
}

const PROPOSE_NOTE: &str = " In this session it is a proposal: the user sees it as a card in Vigía and it is saved only if they accept it, so tell them to review the card instead of saying it is done."; // i18n-ok: for agents

const HELP: &str = "Vigía tracks things that expire: documents, vehicle papers, warranties, domains, TLS certificates, insurance, contracts, company filings. Items belong to entities (people, vehicles, companies, properties, digital assets). Dates are YYYY-MM-DD; days_left is negative once overdue."; // i18n-ok: for agents

struct Tool {
    name: &'static str,
    description: &'static str,
    schema: fn() -> Value,
    write: bool,
}

const TOOLS: &[Tool] = &[
    Tool { name: "summary", description: "What needs attention: counts per status (expired, soon = within 30 days, upcoming, ok, undated), the next deadlines, and automatic domain / certificate checks that are failing.", schema: || json!({"type": "object", "properties": {}}), write: false }, // i18n-ok: for agents
    Tool { name: "checks", description: "Domains and TLS certificates: host, expiry, days left, whether the automatic check (RDAP / TLS) is on, when it last ran and its last result (check_note). failing_only lists those whose last check failed.", schema: || json!({"type": "object", "properties": {"failing_only": {"type": "boolean"}}}), write: false }, // i18n-ok: for agents
    Tool { name: "entity", description: "One owner (person, vehicle, company, property, pet, digital asset) by id, name or plate / tax id: its items (also those it has through what it owns, with via), owned_by, owns, and its relations.", schema: || json!({"type": "object", "properties": {"owner": {"type": "string"}}, "required": ["owner"]}), write: false }, // i18n-ok: for agents
    Tool { name: "list_items", description: "Items, most urgent first. Filters: query (text), status (expired|soon|upcoming|ok|undated), category, owner (entity name, plate or id; includes what it owns through companies it owns), include_archived.", schema: || json!({"type": "object", "properties": {"query": {"type": "string"}, "status": {"type": "string", "enum": ["expired", "soon", "upcoming", "ok", "undated"]}, "category": {"type": "string"}, "owner": {"type": "string"}, "include_archived": {"type": "boolean"}}}), write: false }, // i18n-ok: for agents
    Tool { name: "due", description: "Items overdue or due within `days` (default 30), most urgent first.", schema: || json!({"type": "object", "properties": {"days": {"type": "integer", "minimum": 0}}}), write: false }, // i18n-ok: for agents
    Tool { name: "entities", description: "Who or what items belong to (person, vehicle, company, property, pet, digital, other), with item counts and their shared registry id when synced.", schema: || json!({"type": "object", "properties": {"kind": {"type": "string"}}}), write: false }, // i18n-ok: for agents
    Tool { name: "item", description: "One item with its renewal history and attachments.", schema: || json!({"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]}), write: false }, // i18n-ok: for agents
    Tool { name: "entities_by_kind", description: "The app's sections: owners grouped by kind (vehicle, pet, person, company, property, digital, other), each with its next deadline, its status (worst among its items) and the ids of suggested templates it doesn't have yet; items lists the section's items no owner of that kind holds (a company's domains under digital). People's document numbers show only their last 4 characters.", schema: || json!({"type": "object", "properties": {"kind": {"type": "string", "enum": ["person", "vehicle", "company", "property", "pet", "digital", "other"]}}}), write: false }, // i18n-ok: for agents
    Tool { name: "templates", description: "Suggested reminders per kind of owner (SOAT, técnico-mecánica, pet vaccines, passport, Cámara de Comercio, predial…): id, label, the kinds it is for, item kind, category, suggested recurrence and lead days. Templates never carry a date: ask the user for the real one. Use an id with propose_add_from_template.", schema: || json!({"type": "object", "properties": {"kind": {"type": "string", "enum": ["person", "vehicle", "company", "property", "pet", "digital", "other"]}}}), write: false }, // i18n-ok: for agents
    Tool { name: "add_item", description: "Add an item: `text` is a quick-add line (\"Pasaporte @Ana 12/05/2031\"); kind, name, owner, expires (YYYY-MM-DD), host, notes override it.", schema: || json!({"type": "object", "properties": {"text": {"type": "string"}, "kind": {"type": "string"}, "name": {"type": "string"}, "owner": {"type": "string"}, "expires": {"type": "string"}, "host": {"type": "string"}, "notes": {"type": "string"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}}), write: true }, // i18n-ok: for agents
    Tool { name: "renew_item", description: "Record a renewal: the new expiry (default: the suggested next one), optional cost and note. The old date stays in the history.", schema: || json!({"type": "object", "properties": {"id": {"type": "integer"}, "expires": {"type": "string"}, "cost": {"type": "number"}, "note": {"type": "string"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["id"]}), write: true }, // i18n-ok: for agents
    Tool { name: "propose_add_from_template", description: "Add a reminder from a template (see the templates tool), e.g. \"recuérdame el SOAT del carro\" → template soat. owner: an entity's id, name, plate or tax id (left out: the only entity of the template's kind; an unknown name becomes a new entity of that kind). expires (YYYY-MM-DD) only when the user said the real date; never guess official dates.", schema: || json!({"type": "object", "properties": {"template": {"type": "string"}, "owner": {"type": "string"}, "expires": {"type": "string"}, "name": {"type": "string"}, "notes": {"type": "string"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["template"]}), write: true }, // i18n-ok: for agents
    Tool { name: "update_items", description: "Change one or more items (ids); only the fields given change. owners replaces who they belong to (names or ids; an unknown name becomes a new entity, kind from owner_kind or its name), add_owners / remove_owners adjust it. Also name, kind, category, issued / expires (YYYY-MM-DD, null clears), recurrence (none, every:1y, every:6m, yearly:03-31), lead_days, cost, currency, provider, reference, renewal_url, tags, auto_renews, notes, host (domain or TLS host), auto_check, state (active | in_progress | archived), snoozed_until, business_days.", schema: || json!({"type": "object", "properties": {"ids": {"type": "array", "items": {"type": "integer"}}, "owners": {"type": ["array", "string"], "items": {"type": ["string", "integer"]}}, "add_owners": {"type": ["array", "string"], "items": {"type": ["string", "integer"]}}, "remove_owners": {"type": ["array", "string"], "items": {"type": ["string", "integer"]}}, "owner_kind": {"type": "string", "enum": ["person", "vehicle", "company", "property", "pet", "digital", "other"]}, "name": {"type": "string"}, "kind": {"type": "string"}, "category": {"type": "string"}, "issued": {"type": ["string", "null"]}, "expires": {"type": ["string", "null"]}, "recurrence": {"type": "string"}, "lead_days": {"type": "array", "items": {"type": "integer"}}, "cost": {"type": ["number", "null"]}, "currency": {"type": "string"}, "provider": {"type": "string"}, "reference": {"type": "string"}, "renewal_url": {"type": "string"}, "tags": {"type": "array", "items": {"type": "string"}}, "auto_renews": {"type": "boolean"}, "notes": {"type": "string"}, "host": {"type": "string"}, "auto_check": {"type": "boolean"}, "state": {"type": "string", "enum": ["active", "in_progress", "archived"]}, "snoozed_until": {"type": ["string", "null"]}, "business_days": {"type": "string"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["ids"]}), write: true }, // i18n-ok: for agents
    Tool { name: "delete_items", description: "Delete items (ids) with their history and files; can't be undone. Prefer update_items with state archived.", schema: || json!({"type": "object", "properties": {"ids": {"type": "array", "items": {"type": "integer"}}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["ids"]}), write: true }, // i18n-ok: for agents
    Tool { name: "convert_celebrations", description: "Make items (ids) yearly birthdays or anniversaries (kind birthday | anniversary, else guessed from the name): their date becomes the day each year; a past year is kept to count the age.", schema: || json!({"type": "object", "properties": {"ids": {"type": "array", "items": {"type": "integer"}}, "kind": {"type": "string", "enum": ["birthday", "anniversary"]}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["ids"]}), write: true }, // i18n-ok: for agents
    Tool { name: "check_items", description: "Look up domains (public RDAP) or certificates (TLS) now (ids) and save the expiry found.", schema: || json!({"type": "object", "properties": {"ids": {"type": "array", "items": {"type": "integer"}}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["ids"]}), write: true }, // i18n-ok: for agents
    Tool { name: "create_entity", description: "Add a person, company, vehicle, pet, property (home), digital asset or other: name, kind (guessed: a legal suffix is a company), ident (plate, tax id, document number), detail, year, country, notes, owners (who owns it: names or ids).", schema: || json!({"type": "object", "properties": {"name": {"type": "string"}, "kind": {"type": "string", "enum": ["person", "vehicle", "company", "property", "pet", "digital", "other"]}, "ident": {"type": "string"}, "detail": {"type": "string"}, "year": {"type": "integer"}, "country": {"type": "string"}, "notes": {"type": "string"}, "owners": {"type": ["array", "string"], "items": {"type": ["string", "integer"]}}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["name"]}), write: true }, // i18n-ok: for agents
    Tool { name: "update_entity", description: "Change an entity: name, kind, ident, detail, year, country, notes, archived, and who owns it (owners replaces, add_owners / remove_owners adjust).", schema: || json!({"type": "object", "properties": {"entity": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "name": {"type": "string"}, "kind": {"type": "string", "enum": ["person", "vehicle", "company", "property", "pet", "digital", "other"]}, "ident": {"type": "string"}, "detail": {"type": "string"}, "year": {"type": "integer"}, "country": {"type": "string"}, "notes": {"type": "string"}, "archived": {"type": "boolean"}, "owners": {"type": ["array", "string"], "items": {"type": ["string", "integer"]}}, "add_owners": {"type": ["array", "string"], "items": {"type": ["string", "integer"]}}, "remove_owners": {"type": ["array", "string"], "items": {"type": ["string", "integer"]}}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["entity"]}), write: true }, // i18n-ok: for agents
    Tool { name: "archive_entity", description: "Archive an entity (archived false restores it): it leaves the sidebar and sections; its items and links stay.", schema: || json!({"type": "object", "properties": {"entity": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "archived": {"type": "boolean"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["entity"]}), write: true }, // i18n-ok: for agents
    Tool { name: "delete_entity", description: "Delete an entity and its links; its items stay, with their other owners or none. Can't be undone.", schema: || json!({"type": "object", "properties": {"entity": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["entity"]}), write: true }, // i18n-ok: for agents
    Tool { name: "merge_entities", description: "Merge a duplicate entity (from) into another (into): its items, co-ownerships and links move, then it is deleted.", schema: || json!({"type": "object", "properties": {"from": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "into": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["from", "into"]}), write: true }, // i18n-ok: for agents
    Tool { name: "relate", description: "Link two entities: from is role of to (Juan owner of Otherwise SAS; a company owner of a car). Linking again changes the role.", schema: || json!({"type": "object", "properties": {"from": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "to": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "role": {"type": "string", "enum": ["owner", "member", "manager", "family", "uses", "other"]}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["from", "to"]}), write: true }, // i18n-ok: for agents
    Tool { name: "unrelate", description: "Remove the link between two entities.", schema: || json!({"type": "object", "properties": {"from": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "to": {"type": ["string", "integer"], "description": "id, name, plate or tax id"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["from", "to"]}), write: true }, // i18n-ok: for agents
];

/// An item for the agent: identity documents show only the last 4 characters of their number.
fn item_out(it: &Item, today: date::Date) -> Value {
    let mut j = crate::item_json(it, today);
    if it.category == Category::Identity && !it.reference.trim().is_empty() {
        j["reference"] = json!(vigia_core::entity::masked(&it.reference));
    }
    j
}

/// An entity for the agent: a person's document number masked.
fn entity_out(e: &vigia_core::entity::Entity, items: &[Item]) -> Value {
    let mut j = crate::entity_json(e, items);
    j["ident"] = json!(e.shown_ident());
    j
}

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

impl Server {
    fn offers(&self, t: &Tool) -> bool {
        !t.write || self.writes || self.propose.is_some()
    }

    pub fn tools(&self) -> Vec<Value> {
        let proposing = self.propose.is_some() && !self.writes;
        TOOLS
            .iter()
            .filter(|t| self.offers(t))
            .map(|t| {
                let description = if t.write && proposing { format!("{}{PROPOSE_NOTE}", t.description) } else { t.description.to_string() };
                json!({"name": t.name, "description": description, "inputSchema": (t.schema)(), "annotations": {"readOnlyHint": !t.write, "destructiveHint": false}})
            })
            .collect()
    }

    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned()?;
        if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Some(rpc_error(id, -32600, "invalid request"));
        }
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        Some(match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
                let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
                ok(id, json!({"protocolVersion": version, "capabilities": {"tools": {"listChanged": false}}, "serverInfo": {"name": "vigia", "version": env!("CARGO_PKG_VERSION")}, "instructions": HELP}))
            }
            "ping" => ok(id, json!({})),
            "tools/list" => ok(id, json!({"tools": self.tools()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                if !TOOLS.iter().any(|t| t.name == name && self.offers(t)) {
                    return Some(rpc_error(id, -32602, &format!("unknown tool {name:?}"))); // i18n-ok: protocol
                }
                match self.call(name, &args) {
                    Ok(v) => ok(id, json!({"content": [{"type": "text", "text": v.to_string()}], "structuredContent": if v.is_object() { v.clone() } else { json!({"result": v}) }, "isError": false})),
                    Err(e) => ok(id, json!({"content": [{"type": "text", "text": e}], "isError": true})),
                }
            }
            _ => rpc_error(id, -32601, &format!("method not found: {method}")), // i18n-ok: protocol
        })
    }

    fn call(&self, name: &str, a: &Value) -> Result<Value, String> {
        let s = Store::open(&self.db).map_err(|e| format!("{}: {e}", self.db.display()))?; // i18n-ok: path: error
        let today = date::today();
        let str_arg = |k: &str| a.get(k).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
        let items = s.items().map_err(|e| e.to_string())?;
        let list = |v: Vec<&Item>| json!({"count": v.len(), "items": v.iter().map(|i| item_out(i, today)).collect::<Vec<_>>()});
        let order = date::order_for(i18n::locale());
        if vigia_core::agent::WRITE_TOOLS.contains(&name) {
            let entities = s.entities().map_err(|e| e.to_string())?;
            let relations = s.relations().map_err(|e| e.to_string())?;
            let snap = vigia_core::agent::Snapshot { items: &items, entities: &entities, relations: &relations };
            let plan = vigia_core::agent::plan(name, a, &snap, today, order)?;
            if !self.writes && let Some(dir) = &self.propose {
                let p = vigia_core::agent::Proposal { tool: name.to_string(), args: a.clone(), reason: str_arg("reason").unwrap_or_default().to_string() };
                vigia_core::agent::deliver(dir, &p).map_err(|e| e.to_string())?;
                let preview = match plan.ops.first() {
                    Some(vigia_core::agent::Op::AddItem { item, .. }) => item_out(item, today),
                    Some(vigia_core::agent::Op::Renew(r)) => json!({"item": items.iter().find(|i| i.id == r.id).map(|i| item_out(i, today)), "new_expiry": date::iso(r.expires), "cost": r.cost, "note": r.note}),
                    _ => Value::Null,
                };
                return Ok(json!({"status": "awaiting_approval", "title": plan.title, "changes": plan.lines, "undoable": plan.undoable, "preview": preview, "message": "Shown to the user as a card in Vigía; it is saved only if they accept it."})); // i18n-ok: for agents
            }
            let mut s = s;
            let done = vigia_core::agent::apply(&mut s, &plan, today).map_err(|e| e.to_string())?;
            for id in &plan.check {
                if let Ok(mut it) = s.item(*id) {
                    let o = vigia_core::checks::run(&it, &vigia_core::checks::Net);
                    vigia_core::checks::apply(&mut it, &o, vigia_core::db::now_secs());
                    s.update(&it).map_err(|e| e.to_string())?;
                }
            }
            self.republish(&s);
            let items = s.items().map_err(|e| e.to_string())?;
            let touched: Vec<Value> = items.iter().filter(|i| done.items.contains(&i.id)).map(|i| item_out(i, today)).collect();
            let entities = s.entities().map_err(|e| e.to_string())?;
            let ents: Vec<Value> = entities.iter().filter(|e| done.entities.contains(&e.id)).map(|e| entity_out(e, &items)).collect();
            // the single item an add / renew made, as before
            if matches!(name, "add_item" | "renew_item" | "propose_add_from_template") && touched.len() == 1 {
                return Ok(touched[0].clone());
            }
            return Ok(json!({"status": "saved", "title": plan.title, "items": touched, "entities": ents}));
        }
        match name {
            "summary" => {
                let c = view::counts(&items, today);
                let failing: Vec<Value> = items.iter().filter(|i| i.auto_check && check_failed(i)).map(|i| item_out(i, today)).collect();
                Ok(json!({"today": date::iso(today), "expired": c.expired, "soon": c.soon, "upcoming": c.upcoming, "ok": c.ok, "undated": c.undated, "in_progress": c.in_progress, "total": c.total, "next": view::next(&items, today, 8).iter().map(|i| item_out(i, today)).collect::<Vec<_>>(), "failing_checks": failing}))
            }
            "checks" => {
                let only = a.get("failing_only").and_then(Value::as_bool).unwrap_or(false);
                let v: Vec<Value> = view::sorted(items.iter().filter(|i| matches!(i.kind, Kind::Domain | Kind::Certificate) || !i.host.is_empty()), today)
                    .into_iter()
                    .filter(|i| !only || check_failed(i))
                    .map(|i| {
                        let mut j = item_out(i, today);
                        j["last_checked_at"] = json!(i.last_checked.and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|t| t.to_rfc3339()));
                        j["check_failed"] = json!(check_failed(i));
                        j
                    })
                    .collect();
                Ok(json!({"count": v.len(), "items": v}))
            }
            "entity" => {
                let entities = s.entities().map_err(|e| e.to_string())?;
                let o = str_arg("owner").ok_or_else(|| i18n::t!("cli.err.needs_id"))?;
                let e = o.parse::<i64>().ok().and_then(|id| entities.iter().find(|e| e.id == id)).or_else(|| vigia_core::entity::find(&entities, o)).ok_or_else(|| format!("no entity {o:?}; see the entities tool"))?; // i18n-ok: for agents
                let mut v = entity_out(e, &items);
                let rels = s.relations().map_err(|e| e.to_string())?;
                let f = Filter::for_entity(e.id, &rels);
                let name_of_e = |id: i64| entities.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default();
                // its own items, then those it has through what it owns ("via")
                v["items"] = view::filtered(&items, &Filter { show_archived: true, ..f.clone() }, today)
                    .iter()
                    .map(|i| {
                        let mut j = item_out(i, today);
                        if let Some(Some(via)) = view::via(i, e.id, &f.through) {
                            j["via"] = json!({"id": via, "name": name_of_e(via)});
                        }
                        j
                    })
                    .collect();
                v["owned_by"] = vigia_core::entity::owners_of(e.id, &rels).into_iter().map(|o| json!({"id": o, "name": name_of_e(o)})).collect();
                v["owns"] = rels.iter().filter(|r| r.from == e.id && r.role == vigia_core::entity::Role::Owner).map(|r| json!({"id": r.to, "name": name_of_e(r.to)})).collect();
                let name_of = |id: i64| entities.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default();
                v["relations"] = s.relations().map_err(|e| e.to_string())?.iter().filter(|r| r.from == e.id || r.to == e.id).map(|r| json!({"from": r.from, "from_name": name_of(r.from), "to": r.to, "to_name": name_of(r.to), "role": r.role.id()})).collect();
                Ok(v)
            }
            "list_items" => {
                let entities = s.entities().map_err(|e| e.to_string())?;
                let owner = str_arg("owner").map(|o| o.parse::<i64>().ok().filter(|id| entities.iter().any(|e| e.id == *id)).or_else(|| vigia_core::entity::find(&entities, o).map(|e| e.id)).unwrap_or(-1));
                let f = Filter {
                    query: str_arg("query").unwrap_or_default().to_string(),
                    through: owner.map(|o| vigia_core::entity::owned_through(o, &s.relations().unwrap_or_default())).unwrap_or_default(),
                    entity: owner,
                    entity_kind: None,
                    category: str_arg("category").map(|c| Category::from_id(c).ok_or_else(|| i18n::t!("cli.err.bad_category", value = c))).transpose()?,
                    status: str_arg("status").map(|c| Status::from_id(c).ok_or_else(|| i18n::t!("cli.err.bad_status", value = c))).transpose()?,
                    show_archived: a.get("include_archived").and_then(Value::as_bool).unwrap_or(false),
                };
                Ok(list(view::filtered(&items, &f, today)))
            }
            "due" => {
                let days = a.get("days").and_then(Value::as_i64).unwrap_or(30);
                Ok(list(view::next(&items, today, usize::MAX).into_iter().filter(|i| i.days_left(today).is_some_and(|d| d <= days)).collect()))
            }
            "entities" => {
                let kind = str_arg("kind").map(|k| EntityKind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_entity_kind", value = k))).transpose()?;
                let v: Vec<Value> = s.entities().map_err(|e| e.to_string())?.iter().filter(|e| kind.is_none_or(|k| e.kind == k)).map(|e| entity_out(e, &items)).collect();
                Ok(json!({"count": v.len(), "entities": v}))
            }
            "entities_by_kind" => {
                let entities = s.entities().map_err(|e| e.to_string())?;
                let only = str_arg("kind").map(|k| EntityKind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_entity_kind", value = k))).transpose()?;
                let sections: Vec<Value> = EntityKind::ALL
                    .into_iter()
                    .filter(|k| only.is_none_or(|o| o == *k))
                    .map(|k| {
                        let cards: Vec<Value> = view::cards(&entities, &items, k, today)
                            .iter()
                            .map(|c| {
                                let own: Vec<&Item> = items.iter().filter(|i| i.owned_by(c.entity.id) && i.state != vigia_core::model::State::Archived).collect();
                                json!({"id": c.entity.id, "name": c.entity.name, "ident": c.entity.shown_ident(), "detail": c.entity.detail, "items": c.items, "status": c.status.map(|s| s.id()), "next": c.next.map(|i| item_out(i, today)), "missing_templates": vigia_core::templates::missing(k, &own).iter().map(|t| t.id).collect::<Vec<_>>()})
                            })
                            .collect();
                        let loose: Vec<Value> = view::loose(&items, &entities, k, today).into_iter().map(|i| item_out(i, today)).collect();
                        json!({"kind": k.id(), "label": k.label(), "count": cards.len(), "entities": cards, "items": loose})
                    })
                    .collect();
                Ok(json!({"sections": sections}))
            }
            "templates" => {
                let kind = str_arg("kind").map(|k| EntityKind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_entity_kind", value = k))).transpose()?;
                let v: Vec<Value> = match kind {
                    Some(k) => vigia_core::templates::for_kind(k),
                    None => vigia_core::templates::ALL.iter().collect(),
                }
                .into_iter()
                .map(crate::template_json)
                .collect();
                Ok(json!({"count": v.len(), "templates": v}))
            }
            "item" => {
                let id = a.get("id").and_then(Value::as_i64).ok_or_else(|| i18n::t!("cli.err.needs_id"))?;
                let it = s.item(id).map_err(|e| e.to_string())?;
                let mut v = item_out(&it, today);
                v["renewals"] = s.renewals(id).map_err(|e| e.to_string())?.iter().map(|r| json!({"on": date::iso(r.on), "old_expiry": r.old_expiry.map(date::iso), "new_expiry": r.new_expiry.map(date::iso), "cost": r.cost, "note": r.note})).collect();
                v["attachments"] = s.attachments(id).map_err(|e| e.to_string())?.iter().map(|f| json!({"name": f.name, "path": s.attachment_path(f).display().to_string()})).collect();
                v["link"] = json!(vigia_core::links::item_link(id).to_url());
                Ok(v)
            }
            _ => Err(format!("unknown tool {name:?}")), // i18n-ok: protocol
        }
    }

    fn republish(&self, s: &Store) {
        if self.publish {
            crate::publish_today(s);
        }
    }

    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(Value::Array(batch)) => {
                    let r: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                    (!r.is_empty()).then_some(Value::Array(r))
                }
                Ok(m) => self.handle(&m),
                Err(_) => Some(rpc_error(Value::Null, -32700, "parse error")),
            };
            if let Some(r) = reply {
                writeln!(output, "{r}")?;
                output.flush()?;
            }
        }
        Ok(())
    }
}

/// The last automatic check ended in an error (RDAP / TLS failed or the host didn't answer).
fn check_failed(i: &Item) -> bool {
    i.last_checked.is_some() && vigia_core::checks::note_is_failure(&i.check_note)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("v.db");
        let s = Store::create(&p).unwrap();
        let soon = date::add_days(date::today(), 5);
        s.insert(&Item { owner: "Mazda ABC123".into(), expires: Some(soon), ..Item::new(Kind::Soat, "SOAT") }).unwrap();
        s.insert(&Item { expires: Some(date::add_days(date::today(), 400)), ..Item::new(Kind::Domain, "otherwisetech.com") }).unwrap();
        (dir, p)
    }

    fn run(srv: &mut Server, lines: &[Value]) -> Vec<Value> {
        let input = lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        let mut out = Vec::new();
        srv.serve(std::io::Cursor::new(input), &mut out).unwrap();
        String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    #[test]
    fn read_only_by_default() {
        let (_d, p) = db();
        let mut srv = Server { db: p, writes: false, publish: false, propose: None };
        let r = run(
            &mut srv,
            &[
                json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
                json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
                json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "due", "arguments": {}}}),
                json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "list_items", "arguments": {"owner": "ABC123"}}}),
                json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "entities", "arguments": {"kind": "vehicle"}}}),
                json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "item", "arguments": {"id": 2}}}),
                json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "add_item", "arguments": {"text": "x"}}}),
                json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "item", "arguments": {"id": 99}}}),
            ],
        );
        assert_eq!(r.len(), 8, "the notification gets no answer");
        assert_eq!(r[0]["result"]["protocolVersion"], "2025-06-18");
        let names: Vec<&str> = r[1]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["summary", "checks", "entity", "list_items", "due", "entities", "item", "entities_by_kind", "templates"]);
        assert!(r[1]["result"]["tools"].as_array().unwrap().iter().all(|t| t["annotations"]["readOnlyHint"] == true));
        assert_eq!(r[2]["result"]["structuredContent"]["count"], 1);
        assert_eq!(r[3]["result"]["structuredContent"]["items"][0]["kind"], "soat");
        assert_eq!(r[4]["result"]["structuredContent"]["count"], 1);
        assert_eq!(r[5]["result"]["structuredContent"]["link"], "otherwise://vigia/item/2");
        assert_eq!(r[6]["error"]["code"], -32602, "write tools are hidden and refused");
        assert_eq!(r[7]["result"]["isError"], true);
    }

    #[test]
    fn writes_only_with_the_flag() {
        let (_d, p) = db();
        let mut srv = Server { db: p.clone(), writes: true, publish: false, propose: None };
        let list = srv.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        let tools = list["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), TOOLS.len());
        assert!(tools.iter().any(|t| t["name"] == "add_item" && t["annotations"]["readOnlyHint"] == false));
        let r = srv.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "add_item", "arguments": {"kind": "certificate", "name": "SSL otherwisetech.com", "expires": "2027-01-15", "host": "otherwisetech.com"}}})).unwrap();
        assert_eq!(r["result"]["structuredContent"]["kind"], "certificate");
        let r = srv.handle(&json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "renew_item", "arguments": {"id": 1, "expires": "2030-01-01"}}})).unwrap();
        assert_eq!(r["result"]["structuredContent"]["expires"], "2030-01-01");
        let s = Store::open(&p).unwrap();
        assert_eq!((s.items().unwrap().len(), s.renewals(1).unwrap().len()), (3, 1));
    }

    #[test]
    fn propose_mode_leaves_cards_and_never_writes() {
        let (d, p) = db();
        let dir = d.path().join("proposals");
        let mut srv = Server { db: p.clone(), writes: false, publish: false, propose: Some(dir.clone()) };
        let list = srv.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        let add = list["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "add_item").expect("offered as a proposal");
        assert!(add["description"].as_str().unwrap().contains("proposal"));
        let call = |srv: &mut Server, name: &str, args: Value| srv.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap();
        let r = call(&mut srv, "add_item", json!({"kind": "certificate", "name": "TLS example.org", "expires": "2027-01-15", "host": "example.org", "reason": "found in the mail"}));
        assert_eq!(r["result"]["structuredContent"]["status"], "awaiting_approval");
        assert_eq!(r["result"]["structuredContent"]["preview"]["kind"], "certificate");
        let r = call(&mut srv, "renew_item", json!({"id": 1, "expires": "2030-01-01"}));
        assert_eq!(r["result"]["structuredContent"]["preview"]["new_expiry"], "2030-01-01");
        assert_eq!(call(&mut srv, "renew_item", json!({"id": 99}))["result"]["isError"], true);
        assert_eq!(call(&mut srv, "add_item", json!({"kind": "nope"}))["result"]["isError"], true);
        let s = Store::open(&p).unwrap();
        assert_eq!((s.items().unwrap().len(), s.renewals(1).unwrap().len()), (2, 0), "nothing written");
        let got = vigia_core::agent::collect(&dir);
        assert_eq!(got.iter().map(|p| p.tool.as_str()).collect::<Vec<_>>(), ["add_item", "renew_item"]);
        assert_eq!(got[0].reason, "found in the mail");
    }

    #[test]
    fn template_tools_schemas_and_cards() {
        let (d, p) = db();
        let s = Store::open(&p).unwrap();
        s.insert_entity(&vigia_core::entity::Entity { kind: EntityKind::Person, name: "Ana".into(), ident: "1020345678".into(), ..Default::default() }).unwrap();
        let dir = d.path().join("proposals");
        let mut srv = Server { db: p.clone(), writes: false, publish: false, propose: Some(dir.clone()) };
        let list = srv.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        let tools = list["result"]["tools"].as_array().unwrap().clone();
        for t in &tools {
            let schema = &t["inputSchema"];
            assert_eq!(schema["type"], "object", "{}", t["name"]);
            let props = schema["properties"].as_object().unwrap();
            for r in schema["required"].as_array().into_iter().flatten() {
                assert!(props.contains_key(r.as_str().unwrap()), "{}: required {r} is a property", t["name"]);
            }
        }
        let find = |n: &str| tools.iter().find(|t| t["name"] == n).unwrap_or_else(|| panic!("{n}")).clone();
        assert_eq!(find("templates")["annotations"]["readOnlyHint"], true);
        assert_eq!(find("entities_by_kind")["annotations"]["readOnlyHint"], true);
        let ebk = find("entities_by_kind");
        let kinds: Vec<&str> = ebk["inputSchema"]["properties"]["kind"]["enum"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(kinds, EntityKind::ALL.iter().map(|k| k.id()).collect::<Vec<_>>());
        let add = find("propose_add_from_template");
        assert_eq!((add["annotations"]["readOnlyHint"].clone(), add["inputSchema"]["required"].clone()), (json!(false), json!(["template"])));
        let call = |srv: &mut Server, name: &str, args: Value| srv.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap()["result"].clone();
        let t = call(&mut srv, "templates", json!({"kind": "vehicle"}));
        assert_eq!(t["structuredContent"]["templates"][0]["id"], "soat");
        assert!(t["structuredContent"]["templates"].as_array().unwrap().iter().all(|t| t["lead_days"].as_array().is_some_and(|l| !l.is_empty())));
        assert_eq!(call(&mut srv, "templates", json!({"kind": "spaceship"}))["isError"], true);
        let sec = call(&mut srv, "entities_by_kind", json!({}))["structuredContent"].clone();
        let by = |k: &str| sec["sections"].as_array().unwrap().iter().find(|s| s["kind"] == k).unwrap().clone();
        let car = by("vehicle")["entities"][0].clone();
        assert_eq!((car["name"].clone(), car["status"].clone(), car["next"]["kind"].clone()), (json!("Mazda ABC123"), json!("soon"), json!("soat")));
        assert!(!car["missing_templates"].as_array().unwrap().contains(&json!("soat")));
        assert!(car["missing_templates"].as_array().unwrap().contains(&json!("tecnomecanica")));
        assert_eq!(by("person")["entities"][0]["ident"], "•••• 5678", "documents stay masked");
        assert_eq!(call(&mut srv, "entity", json!({"owner": "Ana"}))["structuredContent"]["ident"], "•••• 5678");
        // "recuérdame la técnico-mecánica del carro": a card, nothing written
        let r = call(&mut srv, "propose_add_from_template", json!({"template": "tecnomecanica", "reason": "asked"}));
        assert_eq!(r["structuredContent"]["status"], "awaiting_approval");
        assert_eq!(r["structuredContent"]["preview"]["owner"], "Mazda ABC123");
        assert_eq!(call(&mut srv, "propose_add_from_template", json!({"template": "nope"}))["isError"], true);
        assert_eq!(Store::open(&p).unwrap().items().unwrap().len(), 2);
        let got = vigia_core::agent::collect(&dir);
        assert_eq!(got.iter().map(|p| p.tool.as_str()).collect::<Vec<_>>(), ["propose_add_from_template"]);
        // with writes it saves
        let mut srv = Server { db: p.clone(), writes: true, publish: false, propose: None };
        let r = call(&mut srv, "propose_add_from_template", json!({"template": "pet_rabies", "owner": "Luna"}));
        assert_eq!((r["structuredContent"]["kind"].clone(), r["structuredContent"]["owner_kind"].clone()), (json!("vaccine"), json!("pet")));
    }

    #[test]
    fn summary_checks_and_entity() {
        let (_d, p) = db();
        let s = Store::open(&p).unwrap();
        let mut dom = s.item(2).unwrap();
        dom.host = "example.org".into();
        dom.auto_check = true;
        vigia_core::checks::apply(&mut dom, &vigia_core::checks::Outcome::Failed("timeout".into()), 1_700_000_000);
        s.update(&dom).unwrap();
        let mut srv = Server { db: p, writes: false, publish: false, propose: None };
        let call = |srv: &mut Server, name: &str, args: Value| srv.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap()["result"]["structuredContent"].clone();
        let sum = call(&mut srv, "summary", json!({}));
        assert_eq!(sum["soon"], 1);
        assert_eq!(sum["failing_checks"][0]["id"], 2);
        let checks = call(&mut srv, "checks", json!({"failing_only": true}));
        assert_eq!(checks["count"], 1);
        assert_eq!(checks["items"][0]["check_failed"], true);
        assert!(checks["items"][0]["last_checked_at"].as_str().is_some());
        let e = call(&mut srv, "entity", json!({"owner": "ABC123"}));
        assert_eq!(e["items"][0]["kind"], "soat");
    }

    #[test]
    fn entity_and_item_tools_propose_or_write() {
        let (d, p) = db();
        let dir = d.path().join("proposals");
        let call = |srv: &mut Server, name: &str, args: Value| srv.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap()["result"].clone();
        let mut srv = Server { db: p.clone(), writes: false, publish: false, propose: Some(dir.clone()) };
        let list = srv.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        let names: Vec<String> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        for t in vigia_core::agent::WRITE_TOOLS {
            assert!(names.iter().any(|n| n == t), "{t} offered");
        }
        assert!(list["result"]["tools"].as_array().unwrap().iter().all(|t| t["description"].as_str().unwrap().len() < 700), "short descriptions");
        let r = call(&mut srv, "create_entity", json!({"name": "Otherwise SAS"}));
        assert_eq!(r["structuredContent"]["status"], "awaiting_approval");
        let r = call(&mut srv, "update_items", json!({"ids": [2], "owners": ["Otherwise SAS"]}));
        assert!(r["structuredContent"]["changes"][0].as_str().unwrap().contains("Otherwise SAS"));
        assert_eq!(call(&mut srv, "update_items", json!({"ids": [42], "cost": 1}))["isError"], true);
        assert_eq!(call(&mut srv, "delete_items", json!({"ids": [1]}))["structuredContent"]["undoable"], false);
        let s = Store::open(&p).unwrap();
        assert_eq!((s.entities().unwrap().len(), s.items().unwrap().len()), (1, 2), "nothing written");
        assert_eq!(vigia_core::agent::collect(&dir).len(), 3);
        let mut srv = Server { db: p.clone(), writes: true, publish: false, propose: None };
        assert_eq!(call(&mut srv, "create_entity", json!({"name": "Otherwise SAS"}))["structuredContent"]["entities"][0]["kind"], "company");
        let r = call(&mut srv, "update_items", json!({"ids": [2], "owners": ["Otherwise SAS"], "provider": "Namecheap"}));
        assert_eq!(r["structuredContent"]["items"][0]["owner"], "Otherwise SAS");
        assert_eq!(r["structuredContent"]["items"][0]["provider"], "Namecheap");
        let e = call(&mut srv, "entity", json!({"owner": "Otherwise SAS"}));
        assert_eq!(e["structuredContent"]["items"][0]["name"], "otherwisetech.com");
    }
}

