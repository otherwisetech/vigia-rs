//! `vigia`: the headless command line on Vigía's engine, for scripts and AI agents. Every
//! command takes `--json` for machine-readable output and `--db PATH` to use another database.
mod mcp;

use serde_json::{Value, json};
use std::path::PathBuf;
use vigia_core::date::{self, Date};
use vigia_core::db::Store;
use vigia_core::entity::{Entity, EntityKind, Role};
use vigia_core::model::{Category, Item, Kind, Status};
use vigia_core::view::{self, Filter};

#[derive(Debug, Default, PartialEq)]
struct Args {
    db: Option<PathBuf>,
    json: bool,
    cmd: String,
    pos: Vec<String>,
    opts: Vec<(String, String)>,
    flags: Vec<String>,
}

/// Options that take a value; everything else starting with `--` is a flag.
const WITH_VALUE: &[&str] = &["db", "days", "status", "owner", "owner-kind", "business-days", "country", "year", "ident", "detail", "role", "category", "kind", "name", "expires", "issued", "host", "notes", "cost", "currency", "note", "provider", "reference", "lead", "recurrence", "notice", "months", "url", "tags"];

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = argv.iter();
    while let Some(s) = it.next() {
        if let Some(name) = s.strip_prefix("--") {
            let (name, inline) = match name.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (name, None),
            };
            if WITH_VALUE.contains(&name) {
                let v = match inline {
                    Some(v) => v,
                    None => it.next().cloned().ok_or_else(|| i18n::t!("cli.err.needs_value", option = name))?,
                };
                if name == "db" {
                    a.db = Some(PathBuf::from(v));
                } else {
                    a.opts.push((name.to_string(), v));
                }
            } else if name == "json" {
                a.json = true;
            } else {
                a.flags.push(name.to_string());
            }
        } else if s == "-h" {
            a.flags.push("help".into());
        } else if a.cmd.is_empty() {
            a.cmd = s.clone();
        } else {
            a.pos.push(s.clone());
        }
    }
    Ok(a)
}

impl Args {
    fn opt(&self, k: &str) -> Option<&str> {
        self.opts.iter().rev().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }
    fn flag(&self, k: &str) -> bool {
        self.flags.iter().any(|f| f == k)
    }
}

pub(crate) fn item_json(it: &Item, today: Date) -> Value {
    let d = |x: Option<Date>| x.map(date::iso);
    json!({
        "id": it.id,
        "kind": it.kind.id(),
        "name": it.name,
        "owner": it.owner,
        "owner_kind": it.owner_kind.filter(|_| !it.owner.is_empty()).map(|k| k.id()),
        "entity_id": it.entity,
        "category": it.category.id(),
        "issued": d(it.issued),
        "expires": d(it.end_date(today)),
        "due": d(it.due_date(today)),
        "days_left": it.days_left(today),
        "status": it.status(today).id(),
        "state": it.state.id(),
        "recurrence": it.recurrence.id(),
        "term_months": it.term_months,
        "notice_days": it.notice_days,
        "cost": it.cost,
        "currency": it.currency,
        "provider": it.provider,
        "reference": it.reference,
        "tags": it.tags,
        "renewal_url": it.renewal_url,
        "business_days": it.business_days.map(|b| b.id()),
        "auto_renews": it.auto_renews,
        "lead_days": it.leads(),
        "snoozed_until": d(it.snoozed_until),
        "host": it.host,
        "auto_check": it.auto_check,
        "last_checked": it.last_checked,
        "check_note": it.check_note,
        "notes": it.notes,
    })
}

pub(crate) fn entity_json(e: &Entity, items: &[Item]) -> Value {
    json!({
        "id": e.id,
        "kind": e.kind.id(),
        "name": e.name,
        "ident": e.ident,
        "detail": e.detail,
        "year": e.year,
        "country": e.country,
        "notes": e.notes,
        "shared_id": e.shared.as_deref().filter(|s| *s != vigia_core::sync::DETACHED),
        "items": items.iter().filter(|i| i.entity == Some(e.id)).count(),
    })
}

fn entity_line(e: &Entity, items: &[Item]) -> String {
    let n = items.iter().filter(|i| i.entity == Some(e.id)).count();
    format!("{:>4}  {:<10}  {}  {}  ({})", e.id, e.kind.id(), e.name, e.ident, i18n::tn!("cli.entity.items", n)) // i18n-ok: table row
}

fn entity_kind_opt(a: &Args, k: &str) -> Result<Option<EntityKind>, String> {
    a.opt(k).map(|v| EntityKind::from_id(v).ok_or_else(|| i18n::t!("cli.err.bad_entity_kind", value = v))).transpose()
}

/// An entity by id, else by name / identifier.
fn entity_arg(s: &Store, v: Option<&String>) -> Result<Entity, String> {
    let v = v.ok_or_else(|| i18n::t!("cli.err.needs_entity"))?;
    if let Ok(id) = v.parse::<i64>()
        && let Ok(e) = s.entity(id)
    {
        return Ok(e);
    }
    let all = s.entities().map_err(|e| e.to_string())?;
    vigia_core::entity::find(&all, v).cloned().ok_or_else(|| i18n::t!("cli.err.no_entity", value = v))
}

fn line(it: &Item, today: Date) -> String {
    let due = it.due_date(today).map(date::iso).unwrap_or_else(|| "—".into()); // i18n-ok: placeholder
    let left = match it.days_left(today) {
        None => String::new(),
        Some(n) if n < 0 => i18n::tn!("cli.days_ago", -n),
        Some(n) => i18n::tn!("cli.days_left", n),
    };
    format!("{:>4}  {due}  {:<12}  {}  ·  {}  {left}", it.id, it.status(today).label(), it.title(), it.kind.label()) // i18n-ok: table row
}

fn out(a: &Args, v: Value, text: impl FnOnce() -> String) {
    if a.json {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    } else {
        let t = text();
        if !t.is_empty() {
            println!("{t}");
        }
    }
}

fn parse_date(s: &str, today: Date) -> Result<Date, String> {
    date::parse_loose(s, today, date::order_for(i18n::locale())).ok_or_else(|| i18n::t!("cli.err.bad_date", value = s))
}

fn apply_opts(it: &mut Item, a: &Args, today: Date) -> Result<(), String> {
    if let Some(v) = a.opt("name") {
        it.name = v.to_string();
    }
    if let Some(v) = a.opt("owner") {
        if v.trim() != it.owner.trim() {
            it.entity = None;
            it.owner_kind = None;
        }
        it.owner = v.to_string();
    }
    if let Some(k) = entity_kind_opt(a, "owner-kind")? {
        it.entity = None;
        it.owner_kind = Some(k);
    }
    if let Some(v) = a.opt("expires") {
        it.expires = Some(parse_date(v, today)?);
    }
    if let Some(v) = a.opt("issued") {
        it.issued = Some(parse_date(v, today)?);
    }
    if let Some(v) = a.opt("host") {
        it.host = v.to_string();
    }
    if let Some(v) = a.opt("notes") {
        it.notes = v.to_string();
    }
    if let Some(v) = a.opt("provider") {
        it.provider = v.to_string();
    }
    if let Some(v) = a.opt("reference") {
        it.reference = v.to_string();
    }
    if let Some(v) = a.opt("url") {
        it.renewal_url = v.to_string();
    }
    if let Some(v) = a.opt("currency") {
        it.currency = v.to_string();
    }
    if let Some(v) = a.opt("tags") {
        it.tags = v.split(',').map(str::trim).filter(|t| !t.is_empty()).map(String::from).collect();
    }
    if let Some(v) = a.opt("cost") {
        it.cost = Some(v.replace(',', ".").parse().map_err(|_| i18n::t!("cli.err.bad_number", value = v))?);
    }
    if let Some(v) = a.opt("lead") {
        it.lead_days = vigia_core::model::parse_leads(v);
    }
    if let Some(v) = a.opt("notice") {
        it.notice_days = Some(v.parse().map_err(|_| i18n::t!("cli.err.bad_number", value = v))?);
    }
    if let Some(v) = a.opt("months") {
        it.term_months = Some(v.parse().map_err(|_| i18n::t!("cli.err.bad_number", value = v))?);
    }
    if let Some(v) = a.opt("recurrence") {
        it.recurrence = vigia_core::model::Recurrence::parse(v).ok_or_else(|| i18n::t!("cli.err.bad_recurrence", value = v))?;
    }
    if let Some(v) = a.opt("business-days") {
        it.business_days = match v.trim() {
            "" | "none" => None,
            v => Some(vigia_core::holidays::BusinessDays::parse(v).ok_or_else(|| i18n::t!("cli.err.bad_business_days", value = v))?),
        };
    }
    if a.flag("auto-renews") {
        it.auto_renews = true;
    }
    if a.flag("check") {
        it.auto_check = it.kind.checkable();
    }
    Ok(())
}

fn id_arg(a: &Args) -> Result<i64, String> {
    a.pos.first().and_then(|s| s.parse().ok()).ok_or_else(|| i18n::t!("cli.err.needs_id"))
}

/// The user's own database (no `--db`, no `VIGIA_DB`): the one the Today feed and the shared
/// registry follow.
fn is_users_db(a: &Args) -> bool {
    a.db.is_none() && std::env::var_os("VIGIA_DB").is_none_or(|v| v.is_empty())
}

/// Rewrites Vigía's Today feed from `s` (or removes it when the user turned it off).
pub(crate) fn publish_today(s: &Store) -> Option<usize> {
    let root = otherwise_link::Root::system();
    if !vigia_core::settings::Settings::load().today_feed() {
        let _ = vigia_core::feed::clear(&root);
        let _ = vigia_core::calendar_feed::clear(&root);
        return None;
    }
    let items = s.items().ok()?;
    let entities = s.entities().ok()?;
    let _ = vigia_core::calendar_feed::publish(&root, &items, &entities, date::today());
    vigia_core::feed::publish(&root, &items, &entities, date::today()).ok().map(|v| v.len())
}

fn open(a: &Args) -> Result<Store, String> {
    let path = vigia_core::paths::resolve_db(a.db.as_deref()).ok_or_else(|| i18n::t!("cli.err.no_data_dir"))?;
    Store::open_or_create(&path).map_err(|e| format!("{}: {e}", path.display())) // i18n-ok: path: error
}

fn run(a: &Args) -> Result<(), String> {
    run_command(a)?;
    if is_users_db(a)
        && matches!(a.cmd.as_str(), "add" | "edit" | "renew" | "delete" | "rm" | "check" | "import" | "entity")
        && let Ok(s) = open(a)
    {
        publish_today(&s);
    }
    Ok(())
}

fn run_command(a: &Args) -> Result<(), String> {
    let today = date::today();
    match a.cmd.as_str() {
        "list" | "ls" => {
            let s = open(a)?;
            let items = s.items().map_err(|e| e.to_string())?;
            let f = Filter {
                query: a.pos.join(" "),
                // an owner that matches nothing shows nothing
                entity: a.opt("owner").map(|o| entity_arg(&s, Some(&o.to_string())).map(|e| e.id).unwrap_or(-1)),
                entity_kind: entity_kind_opt(a, "owner-kind")?,
                category: a.opt("category").map(|c| Category::from_id(c).ok_or_else(|| i18n::t!("cli.err.bad_category", value = c))).transpose()?,
                status: a.opt("status").map(|c| Status::from_id(c).ok_or_else(|| i18n::t!("cli.err.bad_status", value = c))).transpose()?,
                show_archived: a.flag("all"),
            };
            let v = view::filtered(&items, &f, today);
            out(a, Value::Array(v.iter().map(|i| item_json(i, today)).collect()), || if v.is_empty() { i18n::t!("cli.list.empty") } else { v.iter().map(|i| line(i, today)).collect::<Vec<_>>().join("\n") });
        }
        "due" => {
            let days: i64 = a.opt("days").map(|d| d.parse().map_err(|_| i18n::t!("cli.err.bad_number", value = d))).transpose()?.unwrap_or(30);
            let s = open(a)?;
            let items = s.items().map_err(|e| e.to_string())?;
            let v: Vec<&Item> = view::next(&items, today, usize::MAX).into_iter().filter(|i| i.days_left(today).is_some_and(|d| d <= days)).collect();
            out(a, Value::Array(v.iter().map(|i| item_json(i, today)).collect()), || if v.is_empty() { i18n::tn!("cli.due.none", days) } else { v.iter().map(|i| line(i, today)).collect::<Vec<_>>().join("\n") });
        }
        "show" => {
            let s = open(a)?;
            let it = s.item(id_arg(a)?).map_err(|e| e.to_string())?;
            let hist = s.renewals(it.id).map_err(|e| e.to_string())?;
            let files = s.attachments(it.id).map_err(|e| e.to_string())?;
            let mut v = item_json(&it, today);
            v["renewals"] = hist.iter().map(|r| json!({"on": date::iso(r.on), "old_expiry": r.old_expiry.map(date::iso), "new_expiry": r.new_expiry.map(date::iso), "cost": r.cost, "note": r.note})).collect();
            v["attachments"] = files.iter().map(|f| json!({"name": f.name, "path": s.attachment_path(f).display().to_string()})).collect();
            out(a, v, || {
                let mut t = vec![line(&it, today)];
                if !it.notes.is_empty() {
                    t.push(it.notes.clone());
                }
                for r in &hist {
                    t.push(i18n::t!("cli.show.renewed", on = date::iso(r.on), to = r.new_expiry.map(date::iso).unwrap_or_default()));
                }
                for f in &files {
                    t.push(format!("📎 {}", s.attachment_path(f).display())); // i18n-ok: icon + path
                }
                t.join("\n")
            });
        }
        "add" => {
            let s = open(a)?;
            let mut it = if let Some(k) = a.opt("kind") {
                let kind = Kind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = k))?;
                Item::new(kind, a.pos.join(" "))
            } else if !a.pos.is_empty() {
                vigia_core::quick::parse(&a.pos.join(" "), today, date::order_for(i18n::locale()))
            } else if a.opt("name").is_some() {
                Item::new(Kind::Custom, "")
            } else {
                return Err(i18n::t!("cli.err.add_usage"));
            };
            apply_opts(&mut it, a, today)?;
            if it.name.trim().is_empty() {
                it.name = it.kind.label();
            }
            it.id = s.insert(&it).map_err(|e| e.to_string())?;
            let it = s.item(it.id).map_err(|e| e.to_string())?;
            out(a, item_json(&it, today), || i18n::t!("cli.add.done", line = line(&it, today)));
        }
        "edit" => {
            let s = open(a)?;
            let mut it = s.item(id_arg(a)?).map_err(|e| e.to_string())?;
            if let Some(k) = a.opt("kind") {
                it.kind = Kind::from_id(k).ok_or_else(|| i18n::t!("cli.err.bad_kind", value = k))?;
            }
            apply_opts(&mut it, a, today)?;
            s.update(&it).map_err(|e| e.to_string())?;
            out(a, item_json(&it, today), || line(&it, today));
        }
        "renew" => {
            let mut s = open(a)?;
            let it = s.item(id_arg(a)?).map_err(|e| e.to_string())?;
            let new = match a.opt("expires") {
                Some(v) => parse_date(v, today)?,
                None => it.suggest_next(today),
            };
            let cost = a.opt("cost").map(|v| v.replace(',', ".").parse::<f64>().map_err(|_| i18n::t!("cli.err.bad_number", value = v))).transpose()?;
            let it = s.renew(it.id, new, today, cost, a.opt("note").unwrap_or_default()).map_err(|e| e.to_string())?;
            out(a, item_json(&it, today), || i18n::t!("cli.renew.done", name = it.title(), date = date::iso(new)));
        }
        "delete" | "rm" => {
            let s = open(a)?;
            let id = id_arg(a)?;
            let it = s.item(id).map_err(|e| e.to_string())?;
            s.delete(id).map_err(|e| e.to_string())?;
            out(a, json!({"deleted": id}), || i18n::t!("cli.delete.done", name = it.title()));
        }
        "check" => {
            let s = open(a)?;
            let mut items = s.items().map_err(|e| e.to_string())?;
            let ids: Vec<i64> = if a.flag("all") { items.iter().filter(|i| i.kind.checkable() && i.auto_check).map(|i| i.id).collect() } else { vec![id_arg(a)?] };
            let mut results = Vec::new();
            for it in items.iter_mut().filter(|i| ids.contains(&i.id)) {
                let o = vigia_core::checks::run(it, &vigia_core::checks::Net);
                vigia_core::checks::apply(it, &o, vigia_core::db::now_secs());
                s.update(it).map_err(|e| e.to_string())?;
                results.push(it.clone());
            }
            out(a, Value::Array(results.iter().map(|i| item_json(i, today)).collect()), || results.iter().map(|i| format!("{}\n      {}", line(i, today), i.check_note)).collect::<Vec<_>>().join("\n")); // i18n-ok: layout
        }
        "export" => {
            let s = open(a)?;
            let items = s.items().map_err(|e| e.to_string())?;
            let text = match a.pos.first().map(String::as_str) {
                Some("csv") => vigia_core::csv::export(&items),
                Some("ics") => vigia_core::ics::export(&items, today, vigia_core::db::now_secs()),
                _ => return Err(i18n::t!("cli.err.export_usage")),
            };
            match a.pos.get(1) {
                Some(f) => {
                    std::fs::write(f, text).map_err(|e| e.to_string())?;
                    out(a, json!({"written": f}), || i18n::t!("cli.export.done", file = f));
                }
                None => print!("{text}"),
            }
        }
        "import" => {
            let s = open(a)?;
            let f = a.pos.first().ok_or_else(|| i18n::t!("cli.err.import_usage"))?;
            let text = std::fs::read_to_string(f).map_err(|e| e.to_string())?;
            let r = vigia_core::csv::import(&text, today);
            for it in &r.items {
                s.insert(it).map_err(|e| e.to_string())?;
            }
            out(a, json!({"imported": r.items.len(), "skipped_lines": r.skipped}), || i18n::tn!("cli.import.done", r.items.len()));
        }
        "backup" => {
            let s = open(a)?;
            let f = a.pos.first().ok_or_else(|| i18n::t!("cli.err.backup_usage"))?;
            let n = vigia_core::backup::create(&s, std::path::Path::new(f)).map_err(|e| e.to_string())?;
            out(a, json!({"written": f, "attachments": n}), || i18n::tn!("cli.backup.done", n, file = f));
        }
        "entities" => {
            let s = open(a)?;
            let items = s.items().map_err(|e| e.to_string())?;
            let kind = entity_kind_opt(a, "kind")?;
            let v: Vec<Entity> = s.entities().map_err(|e| e.to_string())?.into_iter().filter(|e| kind.is_none_or(|k| e.kind == k)).collect();
            out(a, Value::Array(v.iter().map(|e| entity_json(e, &items)).collect()), || if v.is_empty() { i18n::t!("cli.entities.empty") } else { v.iter().map(|e| entity_line(e, &items)).collect::<Vec<_>>().join("\n") });
        }
        "entity" => {
            let s = open(a)?;
            let mut e = entity_arg(&s, a.pos.first())?;
            let before = e.clone();
            if let Some(k) = entity_kind_opt(a, "kind")? {
                e.kind = k;
            }
            for (k, field) in [("name", &mut e.name), ("ident", &mut e.ident), ("detail", &mut e.detail), ("country", &mut e.country), ("notes", &mut e.notes)] {
                if let Some(v) = a.opt(k) {
                    *field = v.to_string();
                }
            }
            if let Some(v) = a.opt("year") {
                e.year = if v.is_empty() { None } else { Some(v.parse().map_err(|_| i18n::t!("cli.err.bad_number", value = v))?) };
            }
            if a.flag("delete") {
                s.delete_entity(e.id).map_err(|x| x.to_string())?;
                out(a, json!({"deleted_entity": e.id}), || i18n::t!("cli.delete.done", name = e.name));
                return Ok(());
            }
            if e != before {
                s.update_entity(&e).map_err(|x| x.to_string())?;
            }
            let items = s.items().map_err(|x| x.to_string())?;
            let rels = s.relations().map_err(|x| x.to_string())?;
            let all = s.entities().map_err(|x| x.to_string())?;
            let name = |id: i64| all.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default();
            let mine: Vec<&Item> = view::sorted(items.iter().filter(|i| i.entity == Some(e.id)), today);
            let links: Vec<_> = rels.iter().filter(|r| r.from == e.id || r.to == e.id).collect();
            let mut v = entity_json(&e, &items);
            v["item_list"] = mine.iter().map(|i| item_json(i, today)).collect();
            v["relations"] = links.iter().map(|r| json!({"id": r.id, "from": r.from, "from_name": name(r.from), "to": r.to, "to_name": name(r.to), "role": r.role.id()})).collect();
            out(a, v, || {
                let mut t = vec![entity_line(&e, &items)];
                for r in &links {
                    t.push(format!("      {} → {} ({})", name(r.from), name(r.to), r.role.label())); // i18n-ok: layout
                }
                t.extend(mine.iter().map(|i| line(i, today)));
                t.join("\n")
            });
        }
        "relate" => {
            let s = open(a)?;
            let from = entity_arg(&s, a.pos.first())?;
            let to = entity_arg(&s, a.pos.get(1))?;
            if a.flag("remove") {
                let rels = s.relations().map_err(|e| e.to_string())?;
                for r in rels.iter().filter(|r| r.from == from.id && r.to == to.id) {
                    s.unrelate(r.id).map_err(|e| e.to_string())?;
                }
                out(a, json!({"removed": true}), || i18n::t!("cli.relate.removed", from = from.name, to = to.name));
            } else {
                let role = a.opt("role").map(Role::from_id).unwrap_or_default();
                let id = s.relate(from.id, to.id, role).map_err(|e| e.to_string())?;
                out(a, json!({"id": id, "from": from.id, "to": to.id, "role": role.id()}), || i18n::t!("cli.relate.done", from = from.name, to = to.name, role = role.label()));
            }
        }
        "holidays" => {
            let year: i32 = a.opt("year").map(|y| y.parse().map_err(|_| i18n::t!("cli.err.bad_number", value = y))).transpose()?.unwrap_or_else(|| chrono::Datelike::year(&today));
            let countries: Vec<vigia_core::holidays::Country> = match a.opt("country") {
                Some(c) => c.split(',').map(|x| vigia_core::holidays::Country::from_id(x).ok_or_else(|| i18n::t!("cli.err.bad_country", value = x))).collect::<Result<_, _>>()?,
                None => vigia_core::holidays::Country::ALL.to_vec(),
            };
            let (from, to) = (date::ymd(year, 1, 1).unwrap_or(today), date::ymd(year, 12, 31).unwrap_or(today));
            let v = vigia_core::holidays::between(&countries, from, to);
            out(a, Value::Array(v.iter().map(|h| json!({"date": date::iso(h.date), "name": h.name, "country": h.country.id()})).collect()), || v.iter().map(|h| format!("{}  {:<6} {}", date::iso(h.date), h.country.id(), h.name)).collect::<Vec<_>>().join("\n")); // i18n-ok: table row
        }
        "kinds" => {
            out(a, Value::Array(Kind::ALL.iter().map(|k| json!({"id": k.id(), "label": k.label(), "category": k.category().id(), "lead_days": k.lead_days(), "recurrence": k.recurrence().id()})).collect()), || Kind::ALL.iter().map(|k| format!("{:<22} {}", k.id(), k.label())).collect::<Vec<_>>().join("\n")); // i18n-ok: table row
        }
        "path" => {
            let p = vigia_core::paths::resolve_db(a.db.as_deref()).ok_or_else(|| i18n::t!("cli.err.no_data_dir"))?;
            out(a, json!({"db": p.display().to_string(), "files": vigia_core::db::files_dir(&p).display().to_string()}), || p.display().to_string());
        }
        "today" => {
            let s = open(a)?;
            match publish_today(&s) {
                Some(n) => out(a, json!({"published": n}), || i18n::tn!("cli.today.done", n)),
                None => out(a, json!({"published": null}), || i18n::t!("cli.today.off")),
            }
        }
        "sync" => {
            let s = open(a)?;
            match vigia_core::sync::sync_shared(&s, &otherwise_link::Root::system()).map_err(|e| e.to_string())? {
                Some(r) => out(a, json!({"pushed": r.pushed, "pulled": r.pulled, "created": r.created, "errors": r.errors}), || i18n::t!("cli.sync.done", pushed = r.pushed, pulled = r.pulled, created = r.created)),
                None => out(a, json!({"enabled": false}), || i18n::t!("cli.sync.off")),
            }
        }
        "mcp" => {
            let db = vigia_core::paths::resolve_db(a.db.as_deref()).ok_or_else(|| i18n::t!("cli.err.no_data_dir"))?;
            let mut srv = mcp::Server { db, writes: a.flag("allow-writes"), publish: is_users_db(a) };
            srv.serve(std::io::stdin().lock(), std::io::stdout().lock()).map_err(|e| e.to_string())?;
        }
        "" | "help" => println!("{}", i18n::t!("cli.usage")),
        other => return Err(i18n::t!("cli.err.unknown", cmd = other)),
    }
    Ok(())
}

fn main() {
    let s = vigia_core::settings::Settings::load();
    let lang = std::env::var("VIGIA_LANG").ok().or(s.language).or_else(|| std::env::var("LANG").ok()).and_then(|l| i18n::match_system(&l)).unwrap_or(i18n::DEFAULT);
    i18n::set_locale(lang);
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let a = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if a.flag("help") {
        println!("{}", i18n::t!("cli.usage"));
        return;
    }
    if let Err(e) = run(&a) {
        if a.json {
            println!("{}", json!({"error": e}));
        } else {
            eprintln!("{e}");
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Args {
        parse_args(&s.iter().map(|x| x.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn arguments() {
        let a = args(&["--db", "/tmp/x.db", "due", "--days=7", "--json"]);
        assert_eq!((a.db.as_deref(), a.cmd.as_str(), a.opt("days"), a.json), (Some(std::path::Path::new("/tmp/x.db")), "due", Some("7"), true));
        let a = args(&["add", "Pasaporte", "@Ana", "12/05/2031", "--auto-renews"]);
        assert_eq!(a.pos, vec!["Pasaporte", "@Ana", "12/05/2031"]);
        assert!(a.flag("auto-renews"));
        assert!(parse_args(&["due".into(), "--days".into()]).is_err());
    }

    #[test]
    fn add_renew_due_on_a_temp_db() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("v.db");
        let d = db.to_str().unwrap();
        let today = date::today();
        let soon = date::iso(date::add_days(today, 10));
        run(&args(&["--db", d, "add", "SOAT", "moto", &soon, "--owner", "Ana", "--owner-kind", "person", "--cost", "350000"])).unwrap();
        run(&args(&["--db", d, "add", "--kind", "domain", "example.com", "--host", "example.com", "--expires", "2099-01-01"])).unwrap();
        let s = Store::open(&db).unwrap();
        let items = s.items().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!((items[0].kind, items[0].owner.as_str(), items[0].cost), (Kind::Soat, "Ana", Some(350000.0)));
        assert_eq!(item_json(&items[0], today)["days_left"], json!(10));
        assert_eq!(item_json(&items[0], today)["status"], json!("soon"));
        assert_eq!(item_json(&items[0], today)["owner_kind"], json!("person"));
        assert_eq!(item_json(&items[1], today)["owner_kind"], Value::Null);
        drop(s);
        run(&args(&["--db", d, "renew", &items[0].id.to_string()])).unwrap();
        let s = Store::open(&db).unwrap();
        assert_eq!(s.item(items[0].id).unwrap().expires, Some(date::add_years(date::add_days(today, 10), 1)));
        assert_eq!(s.renewals(items[0].id).unwrap().len(), 1);
        assert!(run(&args(&["--db", d, "renew", "999"])).is_err());
        assert!(run(&args(&["--db", d, "nope"])).is_err());
        assert!(run(&args(&["--db", d, "list", "--status", "weird"])).is_err());
        run(&args(&["--db", d, "export", "csv", dir.path().join("x.csv").to_str().unwrap()])).unwrap();
        run(&args(&["--db", d, "import", dir.path().join("x.csv").to_str().unwrap()])).unwrap();
        assert_eq!(Store::open(&db).unwrap().items().unwrap().len(), 4);
    }

    #[test]
    fn entities_and_relations() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("e.db");
        let d = db.to_str().unwrap();
        run(&args(&["--db", d, "add", "Tecno", "#ABC123", "2099-01-01"])).unwrap();
        run(&args(&["--db", d, "add", "RUT", "@Otherwise_SAS", "2029-03-31", "--business-days", "prev:co"])).unwrap();
        assert!(run(&args(&["--db", d, "add", "x", "--business-days", "sideways"])).is_err());
        run(&args(&["--db", d, "add", "Pasaporte", "@Juan"])).unwrap();
        run(&args(&["--db", d, "entity", "ABC123", "--name", "Mazda ABC123", "--detail", "Mazda 3", "--year", "2019"])).unwrap();
        run(&args(&["--db", d, "add", "SOAT", "@Mazda", "2099-02-01"])).unwrap();
        run(&args(&["--db", d, "relate", "Juan", "Otherwise SAS", "--role", "owner"])).unwrap();
        run(&args(&["--db", d, "relate", "Juan", "abc-123"])).unwrap();
        assert!(run(&args(&["--db", d, "relate", "Juan", "Juan"])).is_err());
        assert!(run(&args(&["--db", d, "entity", "Nobody"])).is_err());
        assert!(run(&args(&["--db", d, "entities", "--kind", "spaceship"])).is_err());
        let s = Store::open(&db).unwrap();
        let es = s.entities().unwrap();
        let mut got: Vec<(&str, EntityKind)> = es.iter().map(|e| (e.name.as_str(), e.kind)).collect();
        got.sort();
        assert_eq!(got, vec![("Juan", EntityKind::Person), ("Mazda ABC123", EntityKind::Vehicle), ("Otherwise SAS", EntityKind::Company)]);
        let car = es.iter().find(|e| e.kind == EntityKind::Vehicle).unwrap();
        assert_eq!((car.ident.as_str(), car.year), ("ABC123", Some(2019)));
        let items = s.items().unwrap();
        assert_eq!(items.iter().filter(|i| i.entity == Some(car.id)).count(), 2);
        assert_eq!(s.relations().unwrap().len(), 2);
        let rut = items.iter().find(|i| i.kind == Kind::Rut).unwrap();
        assert_eq!(item_json(rut, date::today())["due"], json!("2029-03-28"));
        assert!(run(&args(&["--db", d, "holidays", "--country", "co", "--year", "2026"])).is_ok());
        assert!(run(&args(&["--db", d, "holidays", "--country", "atlantis"])).is_err());
        let j = entity_json(car, &items);
        assert_eq!((j["kind"].clone(), j["items"].clone()), (json!("vehicle"), json!(2)));
        drop(s);
        run(&args(&["--db", d, "relate", "Juan", "Mazda ABC123", "--remove"])).unwrap();
        run(&args(&["--db", d, "list", "--owner", "ABC123", "--owner-kind", "vehicle"])).unwrap();
        assert_eq!(Store::open(&db).unwrap().relations().unwrap().len(), 1);
    }
}
