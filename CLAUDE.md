# vigia-rs — for agents and contributors

Same conventions as rama-rs, nodo-rs, genealogy-rs and diskmap.

## Layout

```
crates/core   vigia-core (lib `vigia_core`), no UI:
              date.rs (calendar math, month ends, leap years, Wyoming / yearly rules, loose and
              relative date parsing in es/en/pt/fr), model.rs (Item, Kind templates, Category,
              Recurrence, State, Status), reminder.rs (pure scheduler: plan / upcoming /
              message), db.rs (SQLite file: migrations, integrity check, application id,
              attachments in vigia-files/, renew history, reminders sent, move), view.rs
              (filters, counts, timeline months, calendar grid), quick.rs (quick-add parser),
              extract.rs (dates / numbers / MRZ from document text), ocr.rs (ocrs + PDF text,
              feature `ocr`), rdap.rs + cert.rs (parsers + thin network runners), checks.rs
              (Checker trait, apply outcomes), csv.rs, ics.rs (export, parse, RRULE
              occurrences), backup.rs (zip), settings.rs, paths.rs, demo.rs, entity.rs (owners:
              kinds, plate / legal-suffix inference, find by name / ident, relations, masking of
              people's document numbers, `suggest_kind` for migration 5), templates.rs
              (suggested reminders per entity kind; never a date), graph.rs
              (map layout + camera math, deterministic), holidays.rs (computed public holidays,
              business-day shift), calendars.rs (read-only .ics overlays); otherwise-link
              integration: links.rs (deep-link routes, `new?…` prefill), feed.rs (Today
              events), sync.rs (two-way sync with the shared entity registry), background.rs
              (Otherwise Helper's jobs: reminders, checks with down / recovered / renewed)
crates/i18n   vigia-i18n: embedded translation catalogs, t!/tn!/tv! macros, i18n guard tests
crates/cli    `vigia`: list / due / show / add / edit / renew / delete / check / export /
              import / backup / kinds / templates / entities / entity / relate / holidays / today / sync /
              path / mcp (mcp.rs: stdio MCP server, read-only unless --allow-writes or --propose), `--json`
              everywhere; background.rs: `vigia background manifest | run reminders|checks`
crates/app    `vigia-app` (GPUI + gpui-component). main.rs: menus, Handles, background tick
              (reminders every 30 s, checks hourly), menu-bar pump; state.rs: `Data` entity held
              by the app (not the window) so reminders run with the window closed; app.rs:
              window, top bar + quick add, sidebar, footer, About, file actions, OCR flow;
              storage_ui.rs: Save As / Move / Show in Finder / Open Recent + Locate, lock
              notice, migration question;
              views.rs: dashboard / timeline / calendar / list (+ holidays / overlay menu);
              entities.rs: entity page + form; kinds.rs: section pages (cards + template
              chips), the top bar's template picker, new-entity form; graph.rs: the map canvas; panel.rs: item detail + edit
              form; dialogs.rs: renew, delete, settings, OCR; macos.rs: NSStatusItem (from
              expense-rs); system.rs: notifications, launch at login; star.rs: GitHub star
              (from rama / nodo)
```

## Rules

- **The database is the user's.** Default `<Otherwise data root>/vigia/vigia.db`
  (`paths::default_db`, otherwise-link `app_dir`; remembered as "no choice" so it follows the
  root); before 0.2.1 it was `<data dir>/vigia.db`, and the first window asks once
  (`paths::pending_migration`, `Dialog::Migrate`; answer in `settings.storage_migration`).
  File → Save As (`db::copy_database`) / Move To (`db::move_database`: verified copy with
  otherwise-link `storage`, originals removed only afterwards) / Show in Finder / Open another /
  Open Recent (`recent_dbs`, Locate… for missing ones); a custom choice is remembered in
  `settings.json` (`db_path`). While open the file holds an otherwise-link lock
  (`<db>.lock.json`, heartbeat in the tick); open elsewhere → the "open on <host>" notice,
  never written without "Use here anyway". `--db PATH` is session-only and never remembered; `--demo` only
  seeds a database given with `--db` that is empty. Never put demo data in the default database.
  Every change is written right away in small transactions, through `Store::write` (reopens when
  a sync client renamed a new file over the open one, which SQLite reports as a read-only
  database; `StoreError::ReadOnly` when the file really can't be written, and the forms disable Save); `journal_mode=DELETE` (never WAL) so
  it can sit in a synced folder. Schema changes are a new entry at the end of `db::MIGRATIONS`
  (never edit a shipped one) plus a test. Attachments are copied (never moved) into
  `vigia-files/<item id>/` and referenced by relative `/`-separated paths.
- **Network only for opt-in checks.** RDAP and TLS run for items with `auto_check`, daily while
  the app runs, from Otherwise Helper's `checks` job while it is closed (or `vigia check`). A calendar URL is downloaded only when the user adds or
  refreshes it. The OCR models are downloaded only after the user confirms.
  The star count is the only other request (once a day, off switch). Tests never touch the
  network: parsers run on fixtures in `crates/core/tests/fixtures`, checks through the `Checker`
  trait's fakes.
- **Background jobs (Otherwise Helper)**: declared in the bundle
  (`Contents/Resources/otherwise-background.json`, written by the bundle script from `vigia
  background manifest`), run as `vigia background run reminders|checks` only while the app is
  closed (`whileOpen: false`; the app does both itself). They take the database's lock for the
  run and leave it alone when another process here or another computer holds it; only the
  user's own database. Checks keep `bg.check.<id>` meta (ok / fail) to notify once per outage.
- **Reminders are pure.** `reminder::plan(items, today, sent)` decides; the app records what it
  returns (`notify` + `silent`) in `reminders_sent`. Only the most urgent due reminder per item
  notifies; older ones are recorded silently (no bursts). Renewing changes the deadline, so a new
  cycle starts by itself. In-progress items only get the deadline-day reminder; archived and
  snoozed ones none.
- **i18n is mandatory.** No user-facing string literal in `app/src`, `core/src` or `cli/src`;
  use `i18n::t!` / `tn!` and add the key to **every** `crates/i18n/locales/<lang>/` catalog in the
  same change. Untranslatable literals end their line with `// i18n-ok`. The guard also fails on
  catalog keys no code uses.
- **Otherwise Apps integration is opt-in or read-only.** The Today feed and the registry
  follow only the user's own database (never `--db`); the registry is touched only when
  `hub.json` has `entities: true` (`Registry::open_shared`); `vigia mcp` writes only with
  `--allow-writes`. Tests use `Root::at(tempdir)`, never the real shared folder. The
  otherwise-link dependency is a git tag; bumping it must only change its own Cargo.lock entry.
- Every core change ships with a test on synthetic data. Tests must pass on macOS, Linux and
  Windows: no assumptions about installed tools, paths, locale or time zone (pass `today`).
- **The assistant is the shared AI panel** (`otherwise-agent-panel`, `app/src/assistant.rs`,
  ⌘J): the user's Claude Code / Codex / opencode with `vigia mcp --db <open db>` for that
  session only (never write the user's agent configs). The panel never gets `--allow-writes`:
  outside "Solo lectura" it starts `--propose <app dir>/assistant/live/<pid>/proposals`, where
  `add_item` / `renew_item` / `propose_add_from_template` only validate and drop a `vigia_core::agent::Proposal`; the window
  polls it, shows cards and applies them with the same parsing (`agent::item_from_args`,
  `renewal_from_args`) through `Data::save` / `Data::renew` on Aplicar. A database open on
  another computer gets read tools only.
- **Templates never carry a date** and never fix a calendar rule (`Wyoming` / `Yearly`): the
  user enters legally set dates; tax filings get the accountant note. A new template needs its
  label (and note) in every catalog; the catalogue test checks kind, interval and lead.
- **People's document numbers stay local**: screens and `vigia mcp` show `entity::masked`
  (last 4); only the edit form and the local CLI show them whole.
- In app test modules don't `use super::*` (gpui's glob exports its own `test` attribute).
- Comments: sparse, only non-obvious why.
- GPUI pins: `Cargo.lock` pins zed `eb5483528be607b497afdaad0a237bebeb191249` and gpui-component
  `972a3ebfd01afca7da6d8b6f31c9a51288ea5565`, as in rama-rs and nodo-rs. Never `cargo update`
  them.

## Validate

```
cargo test -p vigia-i18n -p vigia-core -p vigia-cli -p vigia-app
cargo clippy --workspace --all-targets
```
