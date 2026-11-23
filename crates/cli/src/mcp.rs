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
    Tool { name: "entity", description: "One owner (person, vehicle, company, property, pet, digital asset) by id, name or plate / tax id: its items, most urgent first, and its relations to other entities.", schema: || json!({"type": "object", "properties": {"owner": {"type": "string"}}, "required": ["owner"]}), write: false }, // i18n-ok: for agents
    Tool { name: "list_items", description: "Items, most urgent first. Filters: query (text), status (expired|soon|upcoming|ok|undated), category, owner (entity name, plate or id), include_archived.", schema: || json!({"type": "object", "properties": {"query": {"type": "string"}, "status": {"type": "string", "enum": ["expired", "soon", "upcoming", "ok", "undated"]}, "category": {"type": "string"}, "owner": {"type": "string"}, "include_archived": {"type": "boolean"}}}), write: false }, // i18n-ok: for agents
    Tool { name: "due", description: "Items overdue or due within `days` (default 30), most urgent first.", schema: || json!({"type": "object", "properties": {"days": {"type": "integer", "minimum": 0}}}), write: false }, // i18n-ok: for agents
    Tool { name: "entities", description: "Who or what items belong to (person, vehicle, company, property, pet, digital, other), with item counts and their shared registry id when synced.", schema: || json!({"type": "object", "properties": {"kind": {"type": "string"}}}), write: false }, // i18n-ok: for agents
    Tool { name: "item", description: "One item with its renewal history and attachments.", schema: || json!({"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]}), write: false }, // i18n-ok: for agents
    Tool { name: "add_item", description: "Add an item: `text` is a quick-add line (\"Pasaporte @Ana 12/05/2031\"); kind, name, owner, expires (YYYY-MM-DD), host, notes override it.", schema: || json!({"type": "object", "properties": {"text": {"type": "string"}, "kind": {"type": "string"}, "name": {"type": "string"}, "owner": {"type": "string"}, "expires": {"type": "string"}, "host": {"type": "string"}, "notes": {"type": "string"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}}), write: true }, // i18n-ok: for agents
    Tool { name: "renew_item", description: "Record a renewal: the new expiry (default: the suggested next one), optional cost and note. The old date stays in the history.", schema: || json!({"type": "object", "properties": {"id": {"type": "integer"}, "expires": {"type": "string"}, "cost": {"type": "number"}, "note": {"type": "string"}, "reason": {"type": "string", "description": "proposals: one short sentence for the user"}}, "required": ["id"]}), write: true }, // i18n-ok: for agents
];

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
        let list = |v: Vec<&Item>| json!({"count": v.len(), "items": v.iter().map(|i| crate::item_json(i, today)).collect::<Vec<_>>()});
        let order = date::order_for(i18n::locale());
        if !self.writes
            && let Some(dir) = &self.propose
            && vigia_core::agent::WRITE_TOOLS.contains(&name)
        {
            let preview = match name {
                "add_item" => crate::item_json(&vigia_core::agent::item_from_args(a, today, order)?, today),
                _ => {
                    let it = s.item(vigia_core::agent::needs_id(a)?).map_err(|e| e.to_string())?;
                    let r = vigia_core::agent::renewal_from_args(&it, a, today, order)?;
                    json!({"item": crate::item_json(&it, today), "new_expiry": date::iso(r.expires), "cost": r.cost, "note": r.note})
                }
            };
            let p = vigia_core::agent::Proposal { tool: name.to_string(), args: a.clone(), reason: str_arg("reason").unwrap_or_default().to_string() };
            vigia_core::agent::deliver(dir, &p).map_err(|e| e.to_string())?;
            return Ok(json!({"status": "awaiting_approval", "preview": preview, "message": "Shown to the user as a card in Vigía; it is saved only if they accept it."})); // i18n-ok: for agents
        }
        match name {
            "summary" => {
                let c = view::counts(&items, today);
                let failing: Vec<Value> = items.iter().filter(|i| i.auto_check && check_failed(i)).map(|i| crate::item_json(i, today)).collect();
                Ok(json!({"today": date::iso(today), "expired": c.expired, "soon": c.soon, "upcoming": c.upcoming, "ok": c.ok, "undated": c.undated, "in_progress": c.in_progress, "total": c.total, "next": view::next(&items, today, 8).iter().map(|i| crate::item_json(i, today)).collect::<Vec<_>>(), "failing_checks": failing}))
            }
            "checks" => {
                let only = a.get("failing_only").and_then(Value::as_bool).unwrap_or(false);
                let v: Vec<Value> = view::sorted(items.iter().filter(|i| matches!(i.kind, Kind::Domain | Kind::Certificate) || !i.host.is_empty()), today)
                    .into_iter()
                    .filter(|i| !only || check_failed(i))
                    .map(|i| {
                        let mut j = crate::item_json(i, today);
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
                let mut v = crate::entity_json(e, &items);
                v["items"] = view::sorted(items.iter().filter(|i| i.entity == Some(e.id)), today).iter().map(|i| crate::item_json(i, today)).collect();
                let name_of = |id: i64| entities.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default();
                v["relations"] = s.relations().map_err(|e| e.to_string())?.iter().filter(|r| r.from == e.id || r.to == e.id).map(|r| json!({"from": r.from, "from_name": name_of(r.from), "to": r.to, "to_name": name_of(r.to), "role": r.role.id()})).collect();
                Ok(v)
            }
            "list_items" => {
                let entities = s.entities().map_err(|e| e.to_string())?;
                let f = Filter {
                    query: str_arg("query").unwrap_or_default().to_string(),
                    entity: str_arg("owner").map(|o| o.parse::<i64>().ok().filter(|id| entities.iter().any(|e| e.id == *id)).or_else(|| vigia_core::entity::find(&entities, o).map(|e| e.id)).unwrap_or(-1)),
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
                let v: Vec<Value> = s.entities().map_err(|e| e.to_string())?.iter().filter(|e| kind.is_none_or(|k| e.kind == k)).map(|e| crate::entity_json(e, &items)).collect();
                Ok(json!({"count": v.len(), "entities": v}))
            }
            "item" => {
                let id = a.get("id").and_then(Value::as_i64).ok_or_else(|| i18n::t!("cli.err.needs_id"))?;
                let it = s.item(id).map_err(|e| e.to_string())?;
                let mut v = crate::item_json(&it, today);
                v["renewals"] = s.renewals(id).map_err(|e| e.to_string())?.iter().map(|r| json!({"on": date::iso(r.on), "old_expiry": r.old_expiry.map(date::iso), "new_expiry": r.new_expiry.map(date::iso), "cost": r.cost, "note": r.note})).collect();
                v["attachments"] = s.attachments(id).map_err(|e| e.to_string())?.iter().map(|f| json!({"name": f.name, "path": s.attachment_path(f).display().to_string()})).collect();
                v["link"] = json!(vigia_core::links::item_link(id).to_url());
                Ok(v)
            }
            "add_item" => {
                let it = vigia_core::agent::item_from_args(a, today, order)?;
                let id = s.insert(&it).map_err(|e| e.to_string())?;
                self.republish(&s);
                Ok(crate::item_json(&s.item(id).map_err(|e| e.to_string())?, today))
            }
            "renew_item" => {
                let mut s = s;
                let it = s.item(vigia_core::agent::needs_id(a)?).map_err(|e| e.to_string())?;
                let r = vigia_core::agent::renewal_from_args(&it, a, today, order)?;
                let it = s.renew(r.id, r.expires, today, r.cost, &r.note).map_err(|e| e.to_string())?;
                self.republish(&s);
                Ok(crate::item_json(&it, today))
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
        assert_eq!(names, vec!["summary", "checks", "entity", "list_items", "due", "entities", "item"]);
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
}
