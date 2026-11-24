//! The AI panel (otherwise-agent-panel), docked next to every view and toggled with ⌘J: the
//! user's own Claude Code, Codex or opencode with `vigia mcp --db <open database>` attached for
//! that session only (the user's agent configs are never written).
//!
//! The assistant reads. "Solo lectura" offers read tools only; the other modes start the server
//! with `--propose <folder>`, so `add_item` / `renew_item` only leave a checked proposal there
//! (`vigia_core::agent`). This window turns each into an Aplicar / Rechazar card and saves it,
//! parsed the same way, only on Aplicar.
use crate::app::{View, VigiaApp};
use crate::state::Data;
use crate::theme::Tokens;
use gpui::*;
use otherwise_agent_panel::ui::{AgentHost, AgentPanel, PanelStyle};
use otherwise_agent_panel::{McpServer, PendingChange, PermissionMode};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use vigia_core::agent::{self, Proposal};
use vigia_core::date;

const APP_ID: &str = "vigia"; // i18n-ok: app id

const INSTRUCTIONS: &str = "This is Vigía, an app that tracks things that expire: documents, vehicle papers, warranties, domains, TLS certificates, insurance, contracts, company filings. The MCP server \"vigia\" is the user's open database: summary (what needs attention), due, list_items, item (with its renewal history), checks (domains and certificates with their automatic RDAP / TLS check results), entities and entity (owners and their relations), entities_by_kind (the app's sections: vehicles, pets, people, companies, home, digital, each owner with its next deadline and the suggested reminders it lacks) and templates (suggested reminders per kind). Dates are YYYY-MM-DD; days_left is negative once overdue (\"vencido\"). Never invent dates; look them up. For \"recuérdame el SOAT del carro\" use propose_add_from_template with a template id; ask for the real date rather than guessing it (official dates such as tax calendars are never invented: tell the user to check them with their accountant). When add_item / renew_item / propose_add_from_template are offered they only propose: each becomes a card the user accepts or rejects in the app, so tell them to review the card instead of saying it is done. Answer in the user's language."; // i18n-ok: agent instructions (English)

const POLL: Duration = Duration::from_millis(800);

pub struct AgentState {
    pub panel: Option<WeakEntity<AgentPanel>>,
    pending: HashMap<String, Proposal>,
    seq: u32,
}

impl AgentState {
    pub fn new() -> AgentState {
        AgentState { panel: None, pending: HashMap::new(), seq: 0 }
    }
}

/// Where this process's sessions leave proposals.
pub fn proposals_dir() -> PathBuf {
    otherwise_link::paths::app_dir(APP_ID).join("assistant").join("live").join(std::process::id().to_string()).join("proposals")
}

/// The command line inside the bundle (`Contents/MacOS/vigia`), next to this binary.
pub fn bundled_cli() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let cli = exe.with_file_name(if cfg!(windows) { "vigia.exe" } else { "vigia" }); // i18n-ok: file name
    cli.is_file().then_some(cli)
}

/// `vigia mcp` on `db` for a session in `mode`.
pub fn mcp_server(cli: &Path, db: &Path, mode: PermissionMode, proposals: &Path, lang: &str) -> McpServer {
    let db = db.display().to_string();
    let dir = proposals.display().to_string();
    let mut args = vec!["mcp", "--db", db.as_str()]; // i18n-ok: command line
    if mode != PermissionMode::ReadOnly {
        args.extend(["--propose", dir.as_str()]); // i18n-ok: flag
    }
    let mut s = McpServer::new(APP_ID, cli, &args);
    s.env.push(("VIGIA_LANG".into(), lang.into())); // i18n-ok: env var
    s
}

/// The panel's strings follow the app's language.
pub fn sync_locale() {
    otherwise_agent_panel::i18n::set_locale(i18n::locale());
}

fn order() -> date::Order {
    date::order_for(i18n::locale())
}

/// The card's title and detail lines; `Err` when the proposal no longer makes sense.
pub fn card_text(d: &Data, p: &Proposal) -> Result<(String, Vec<String>), String> {
    let today = Data::today();
    let mut lines = Vec::new();
    let title = match p.tool.as_str() {
        "add_item" | "propose_add_from_template" => {
            let it = proposed_item(d, p)?;
            lines.push(it.kind.label());
            if !it.owner.is_empty() {
                lines.push(i18n::t!("app.assistant.card_owner", owner = it.owner));
            }
            match it.expires {
                Some(e) => lines.push(i18n::t!("app.assistant.card_expires", date = date::iso(e))),
                None if p.tool != "add_item" => lines.push(i18n::t!("app.assistant.card_no_date")),
                None => {}
            }
            if !it.host.is_empty() {
                lines.push(it.host.clone());
            }
            i18n::t!("app.assistant.card_add", name = it.name)
        }
        "renew_item" => {
            let id = agent::needs_id(&p.args)?;
            let it = d.item(id).ok_or_else(|| i18n::t!("app.assistant.card_gone"))?;
            let r = agent::renewal_from_args(it, &p.args, today, order())?;
            let before = it.end_date(today).map(date::iso).unwrap_or_else(|| i18n::t!("app.assistant.no_date"));
            lines.push(i18n::t!("app.assistant.card_renew_dates", before = before, after = date::iso(r.expires)));
            if let Some(c) = r.cost {
                lines.push(i18n::t!("app.assistant.card_cost", cost = c));
            }
            if !r.note.is_empty() {
                lines.push(r.note.clone());
            }
            i18n::t!("app.assistant.card_renew", name = it.name)
        }
        other => return Err(other.to_string()),
    };
    if !p.reason.is_empty() {
        lines.push(i18n::t!("app.assistant.card_reason", reason = p.reason));
    }
    Ok((title, lines))
}

/// The item an add proposal stands for, parsed as `vigia mcp` did.
fn proposed_item(d: &Data, p: &Proposal) -> Result<vigia_core::model::Item, String> {
    match p.tool.as_str() {
        "propose_add_from_template" => agent::item_from_template_args(&p.args, &d.entities, Data::today(), order()),
        _ => agent::item_from_args(&p.args, Data::today(), order()),
    }
}

/// Saves an accepted proposal.
fn apply(data: &Entity<Data>, p: &Proposal, cx: &mut App) -> Result<String, String> {
    let today = Data::today();
    match p.tool.as_str() {
        "add_item" | "propose_add_from_template" => {
            let it = proposed_item(data.read(cx), p)?;
            let name = it.name.clone();
            data.update(cx, |d, cx| d.save(it, cx))?;
            Ok(name)
        }
        "renew_item" => {
            let id = agent::needs_id(&p.args)?;
            let (r, name) = {
                let d = data.read(cx);
                let it = d.item(id).ok_or_else(|| i18n::t!("app.assistant.card_gone"))?;
                (agent::renewal_from_args(it, &p.args, today, order())?, it.name.clone())
            };
            data.update(cx, |d, cx| d.renew(r.id, r.expires, r.cost, &r.note, cx))?;
            Ok(name)
        }
        _ => Err(i18n::t!("app.assistant.card_gone")),
    }
}

pub struct Host {
    pub app: WeakEntity<VigiaApp>,
}

impl AgentHost for Host {
    fn app_id(&self) -> String {
        APP_ID.into()
    }

    fn app_name(&self) -> String {
        crate::APP_NAME.into()
    }

    fn mcp_servers(&self, mode: PermissionMode, cx: &App) -> Vec<McpServer> {
        let Some(app) = self.app.upgrade() else { return Vec::new() };
        let Some(cli) = bundled_cli() else { return Vec::new() };
        let d = app.read(cx).data.read(cx);
        // another computer has the database: it can be read, but nothing would be saved
        let mode = if d.blocked.is_some() || d.store.is_none() { PermissionMode::ReadOnly } else { mode };
        vec![mcp_server(&cli, &d.path, mode, &proposals_dir(), i18n::locale())]
    }

    fn context(&self, cx: &App) -> Option<String> {
        Some(self.app.upgrade()?.read(cx).agent_context(cx))
    }

    fn instructions(&self) -> Option<String> {
        Some(INSTRUCTIONS.into())
    }

    fn suggestions(&self) -> Vec<String> {
        vec![i18n::t!("app.assistant.suggest_overdue"), i18n::t!("app.assistant.suggest_certs"), i18n::t!("app.assistant.suggest_month"), i18n::t!("app.assistant.suggest_checks")]
    }

    fn resolve_change(&self, id: &str, approve: bool, _window: &mut Window, cx: &mut App) -> Result<(), String> {
        self.app.update(cx, |a, cx| a.resolve_proposal(id, approve, cx)).map_err(|e| e.to_string())?
    }
}

/// The panel in Vigía's colours.
pub fn style(t: &Tokens) -> PanelStyle {
    PanelStyle {
        background: t.canvas,
        surface: t.surface,
        raised: t.surface,
        border: t.border,
        ink: t.ink,
        muted: t.ink_muted,
        accent: t.accent,
        accent_soft: t.accent_soft,
        danger: t.danger,
        danger_soft: t.danger.opacity(0.12),
        ok: t.ok,
        note: t.warn,
        note_soft: t.warn.opacity(0.14),
        terminal_bg: if t.dark { rgb(0x161514).into() } else { rgb(0xFFFFFF).into() },
        terminal_fg: t.ink,
        mono_font: "Menlo".into(), // i18n-ok: font name
        dark: t.dark,
    }
}

impl VigiaApp {
    fn agent_panel(&self) -> Option<Entity<AgentPanel>> {
        self.agent.panel.as_ref().and_then(|p| p.upgrade())
    }

    pub fn toggle_agent_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.agent_panel() {
            p.update(cx, |p, cx| p.toggle(window, cx));
        }
    }

    /// "Lo que estoy viendo" for the agent (English).
    pub fn agent_context(&self, cx: &App) -> String {
        let d = self.data.read(cx);
        let view = match self.view {
            View::Dashboard => "Dashboard (what needs attention)".to_string(),
            View::Timeline => "Timeline (deadlines by month)".to_string(),
            View::Calendar => format!("Calendar ({})", date::iso(self.cal_month)),
            View::List => "List of items".to_string(),
            View::Graph => "Map (owners and how they relate)".to_string(),
            View::Kind(k) => format!("Section: {} (entities of kind {})", k.label(), k.id()),
            View::Entity(id) => format!("Owner page: {}", d.entity(id).map(|e| format!("{} (entity id {id})", e.name)).unwrap_or_default()),
        }; // i18n-ok: context for the agent (English)
        let mut v = serde_json::json!({ "view": view, "today": date::iso(Data::today()) });
        if let Some(it) = self.selected.and_then(|id| d.item(id)) {
            v["selected_item"] = serde_json::json!({ "id": it.id, "name": it.name, "kind": it.kind.id() });
        }
        if !self.filter.query.trim().is_empty() {
            v["search"] = Value::from(self.filter.query.clone());
        }
        if d.blocked.is_some() {
            v["read_only"] = Value::Bool(true);
        }
        v.to_string()
    }

    /// Per render: the panel's colours.
    pub fn sync_agent_style(t: &Tokens, cx: &mut App) {
        if cx.try_global::<PanelStyle>().is_none_or(|s| s.dark != t.dark) {
            cx.set_global(style(t));
        }
    }

    /// Turns delivered proposals into cards while the window lives.
    pub fn start_agent_pump(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |this, cx| this.agent_tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn agent_tick(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else { return };
        let dir = proposals_dir();
        if !dir.is_dir() {
            return;
        }
        for p in agent::collect(&dir) {
            let Ok((title, lines)) = card_text(self.data.read(cx), &p) else { continue };
            self.agent.seq += 1;
            let id = format!("p{}", self.agent.seq); // i18n-ok: card id
            self.agent.pending.insert(id.clone(), p);
            let change = PendingChange::new(id, title, lines.join("\n"));
            panel.update(cx, |p, cx| p.report_pending_change(change, cx));
        }
    }

    /// Aplicar / Rechazar on a card.
    pub fn resolve_proposal(&mut self, id: &str, approve: bool, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(p) = self.agent.pending.remove(id) else {
            return if approve { Err(i18n::t!("app.assistant.card_gone")) } else { Ok(()) };
        };
        if !approve {
            return Ok(());
        }
        let name = apply(&self.data.clone(), &p, cx)?;
        self.toast(i18n::t!("app.assistant.applied", name = name), cx);
        Ok(())
    }
}

/// The proposals folder of a process that is gone stays small and harmless, but a quit cleans
/// up after itself.
pub fn remove_live_dir() {
    if let Some(live) = proposals_dir().parent() {
        let _ = std::fs::remove_dir_all(live);
    }
}

#[cfg(test)]
mod tests {
    // not `super::*`: gpui's glob brings its own `test` attribute
    use super::{PermissionMode, mcp_server};
    use std::path::Path;

    #[test]
    fn modes_start_the_database_server_read_only_or_proposing() {
        let cli = Path::new("/Applications/Vigía.app/Contents/MacOS/vigia");
        let db = Path::new("/data/vigia/vigia.db");
        let dir = Path::new("/data/vigia/assistant/live/7/proposals");
        let ro = mcp_server(cli, db, PermissionMode::ReadOnly, dir, "es");
        assert_eq!(ro.name, "vigia");
        assert_eq!(ro.args, ["mcp", "--db", "/data/vigia/vigia.db"]);
        assert_eq!(ro.env, [("VIGIA_LANG".to_string(), "es".to_string())]);
        for mode in [PermissionMode::Ask, PermissionMode::Auto] {
            assert_eq!(mcp_server(cli, db, mode, dir, "en").args, ["mcp", "--db", "/data/vigia/vigia.db", "--propose", "/data/vigia/assistant/live/7/proposals"], "{mode:?}: proposals, never --allow-writes");
        }
    }
}
