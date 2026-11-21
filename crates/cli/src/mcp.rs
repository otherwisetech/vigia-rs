//! `vigia mcp`: a stdio MCP server (JSON-RPC 2.0, one message per line) over one Vigía
//! database, proxied by Otherwise Apps' `otherwise-hub` as `vigia.*`. Read-only unless started
//! with `--allow-writes` (the hub passes it only when the user enabled write tools); writes only
//! add items or renew them (the renewal history keeps the old date). The database is opened
//! fresh for every call, so the server sees what the app wrote.
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
}

const HELP: &str = "Vigía tracks things that expire: documents, vehicle papers, warranties, domains, TLS certificates, insurance, contracts, company filings. Items belong to entities (people, vehicles, companies, properties, digital assets). Dates are YYYY-MM-DD; days_left is negative once overdue."; // i18n-ok: for agents

struct Tool {
    name: &'static str,
    description: &'static str,
    schema: fn() -> Value,
    write: bool,
}

const TOOLS: &[Tool] = &[
    Tool { name: "list_items", description: "Items, most urgent first. Filters: query (text), status (expired|soon|upcoming|ok|undated), category, owner (entity name, plate or id), include_archived.", schema: || json!({"type": "object", "properties": {"query": {"type": "string"}, "status": {"type": "string", "enum": ["expired", "soon", "upcoming", "ok", "undated"]}, "category": {"type": "string"}, "owner": {"type": "string"}, "include_archived": {"type": "boolean"}}}), write: false }, // i18n-ok: for agents
    Tool { name: "due", description: "Items overdue or due within `days` (default 30), most urgent first.", schema: || json!({"type": "object", "properties": {"days": {"type": "integer", "minimum": 0}}}), write: false }, // i18n-ok: for agents
    Tool { name: "entities", description: "Who or what items belong to (person, vehicle, company, property, pet, digital, other), with item counts and their shared registry id when synced.", schema: || json!({"type": "object", "properties": {"kind": {"type": "string"}}}), write: false }, // i18n-ok: for agents
    Tool { name: "item", description: "One item with its renewal history and attachments.", schema: || json!({"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]}), write: false }, // i18n-ok: for agents
    Tool { name: "add_item", description: "Add an item: `text` is a quick-add line (\"Pasaporte @Ana 12/05/2031\"); kind, name, owner, expires (YYYY-MM-DD), host, notes override it.", schema: || json!({"type": "object", "properties": {"text": {"type": "string"}, "kind": {"type": "string"}, "name": {"type": "string"}, "owner": {"type": "string"}, "expires": {"type": "string"}, "host": {"type": "string"}, "notes": {"type": "string"}}}), write: true }, // i18n-ok: for agents
    Tool { name: "renew_item", description: "Record a renewal: the new expiry (default: the suggested next one), optional cost and note. The old date stays in the history.", schema: || json!({"type": "object", "properties": {"id": {"type": "integer"}, "expires": {"type": "string"}, "cost": {"type": "number"}, "note": {"type": "string"}}, "required": ["id"]}), write: true }, // i18n-ok: for agents
];

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

impl Server {
    pub fn tools(&self) -> Vec<Value> {
        TOOLS.iter().filter(|t| self.writes || !t.write).map(|t| json!({"name": t.name, "description": t.description, "inputSchema": (t.schema)(), "annotations": {"readOnlyHint": !t.write, "destructiveHint": false}})).collect()
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
                if !TOOLS.iter().any(|t| t.name == name && (self.writes || !t.write)) {
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
        match name {
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
                let order = date::order_for(i18n::locale());
                let mut it = match (str_arg("text"), str_arg("kind")) {
                    (Some(t), _) => vigia_core::quick::parse(t, today, order),
                    (None, Some(k)) => Item::new(Kind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = k))?, ""),
                    (None, None) if str_arg("name").is_some() => Item::new(Kind::Custom, ""),
                    _ => return Err(i18n::t!("cli.err.add_usage")),
                };
                if let (Some(k), Some(_)) = (str_arg("kind"), str_arg("text")) {
                    it.kind = Kind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = k))?;
                }
                if let Some(v) = str_arg("name") {
                    it.name = v.into();
                }
                if let Some(v) = str_arg("owner") {
                    it.owner = v.into();
                    it.entity = None;
                }
                if let Some(v) = str_arg("expires") {
                    it.expires = Some(date::parse_loose(v, today, order).ok_or_else(|| i18n::t!("cli.err.bad_date", value = v))?);
                }
                if let Some(v) = str_arg("host") {
                    it.host = v.into();
                }
                if let Some(v) = str_arg("notes") {
                    it.notes = v.into();
                }
                if it.name.trim().is_empty() {
                    it.name = it.kind.label();
                }
                let id = s.insert(&it).map_err(|e| e.to_string())?;
                self.republish(&s);
                Ok(crate::item_json(&s.item(id).map_err(|e| e.to_string())?, today))
            }
            "renew_item" => {
                let id = a.get("id").and_then(Value::as_i64).ok_or_else(|| i18n::t!("cli.err.needs_id"))?;
                let mut s = s;
                let it = s.item(id).map_err(|e| e.to_string())?;
                let new = match str_arg("expires") {
                    Some(v) => date::parse_loose(v, today, date::order_for(i18n::locale())).ok_or_else(|| i18n::t!("cli.err.bad_date", value = v))?,
                    None => it.suggest_next(today),
                };
                let it = s.renew(id, new, today, a.get("cost").and_then(Value::as_f64), str_arg("note").unwrap_or_default()).map_err(|e| e.to_string())?;
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
        let mut srv = Server { db: p, writes: false, publish: false };
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
        assert_eq!(names, vec!["list_items", "due", "entities", "item"]);
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
        let mut srv = Server { db: p.clone(), writes: true, publish: false };
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
}
