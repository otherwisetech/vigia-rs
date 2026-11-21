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
              kinds, plate / legal-suffix inference, find by name / ident, relations), graph.rs
              (map layout + camera math, deterministic), holidays.rs (computed public holidays,
              business-day shift), calendars.rs (read-only .ics overlays)
crates/i18n   vigia-i18n: embedded translation catalogs, t!/tn!/tv! macros, i18n guard tests
crates/cli    `vigia`: list / due / show / add / edit / renew / delete / check / export /
              import / backup / kinds / entities / entity / relate / holidays / path, `--json`
              everywhere
crates/app    `vigia-app` (GPUI + gpui-component). main.rs: menus, Handles, background tick
              (reminders every 30 s, checks hourly), menu-bar pump; state.rs: `Data` entity held
              by the app (not the window) so reminders run with the window closed; app.rs:
              window, top bar + quick add, sidebar, footer, About, file actions, OCR flow;
              views.rs: dashboard / timeline / calendar / list (+ holidays / overlay menu);
              entities.rs: entity page + form; graph.rs: the map canvas; panel.rs: item detail + edit
              form; dialogs.rs: renew, delete, settings, OCR; macos.rs: NSStatusItem (from
              expense-rs); system.rs: notifications, launch at login; star.rs: GitHub star
              (from rama / nodo)
```

## Rules

- **The database is the user's.** Default `<data dir>/vigia.db`; Settings can move it (copy,
  verify, then remove the original) or open another; the last one is remembered in
  `settings.json` (`db_path`). `--db PATH` is session-only and never remembered; `--demo` only
  seeds a database given with `--db` that is empty. Never put demo data in the default database.
  Every change is written right away in small transactions; `journal_mode=DELETE` (never WAL) so
  it can sit in a synced folder. Schema changes are a new entry at the end of `db::MIGRATIONS`
  (never edit a shipped one) plus a test. Attachments are copied (never moved) into
  `vigia-files/<item id>/` and referenced by relative `/`-separated paths.
- **Network only for opt-in checks.** RDAP and TLS run for items with `auto_check`, daily while
  the app runs (or `vigia check`). A calendar URL is downloaded only when the user adds or
  refreshes it. The OCR models are downloaded only after the user confirms.
  The star count is the only other request (once a day, off switch). Tests never touch the
  network: parsers run on fixtures in `crates/core/tests/fixtures`, checks through the `Checker`
  trait's fakes.
- **Reminders are pure.** `reminder::plan(items, today, sent)` decides; the app records what it
  returns (`notify` + `silent`) in `reminders_sent`. Only the most urgent due reminder per item
  notifies; older ones are recorded silently (no bursts). Renewing changes the deadline, so a new
  cycle starts by itself. In-progress items only get the deadline-day reminder; archived and
  snoozed ones none.
- **i18n is mandatory.** No user-facing string literal in `app/src`, `core/src` or `cli/src`;
  use `i18n::t!` / `tn!` and add the key to **every** `crates/i18n/locales/<lang>/` catalog in the
  same change. Untranslatable literals end their line with `// i18n-ok`. The guard also fails on
  catalog keys no code uses.
- Every core change ships with a test on synthetic data. Tests must pass on macOS, Linux and
  Windows: no assumptions about installed tools, paths, locale or time zone (pass `today`).
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
