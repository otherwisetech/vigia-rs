//! What the AI panel's assistant may change, as proposals. `vigia mcp --propose <folder>` turns
//! its write tools (`add_item`, `renew_item`) into a checked [`Proposal`] dropped in that folder;
//! the app shows it as a card and applies it, with the same parsing, only when the user accepts.
use crate::date::{self, Date, Order};
use crate::model::{Item, Kind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The write tools that become proposals.
pub const WRITE_TOOLS: &[&str] = &["add_item", "renew_item"];

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
}
