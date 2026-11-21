# Vigía

<img src="crates/app/assets/logo/vigia-icon-512.png" width="128" alt="Vigía icon">

**Never miss a renewal.** Vigía keeps track of everything that expires — identity documents,
vehicle papers, insurance, warranties, domains, TLS certificates, subscriptions, contracts and
company filings — and reminds you in time, with the window closed too. *Vigía* is Spanish for
*lookout*.

Native desktop app in Rust with [GPUI](https://www.gpui.rs/) and
[gpui-component](https://github.com/longbridge/gpui-component). macOS, Windows and Linux. UI in
Spanish, English, Portuguese and French. Plus `vigia`, a headless command line on the same
engine, for scripts and AI agents.

## Features

- **Items with type templates**: passport, national ID (cédula), driver's license, visa,
  residence permit; vehicle SOAT, tecnomecánica, registration and insurance; insurance policies;
  warranties (purchase date + warranty months → expiry, receipt attached); **domains**
  (registrar, auto-renew); **SSL/TLS certificates** (host); subscriptions and annual plans;
  contracts (start / end / **notice period**); company filings — **Wyoming annual report** (due
  the 1st day of the formation anniversary month), **Cámara de Comercio** renewal (by March 31 in
  Colombia), RUT updates; memberships; vaccines and check-ups; gas inspection, fire extinguisher;
  or anything custom. Each type sets sensible reminder lead times (a passport starts 9 months
  ahead, because many countries want 6 months of validity), a recurrence and its category.
- Every item: name, **who or what it belongs to**, category, issue and expiry dates or a
  recurrence rule, cost and currency, provider, reference number, renewal link, auto-renews,
  tags, notes, **attachments** (scans, PDFs, photos), a **renewal history**, and optionally a
  **business-day rule** (a deadline on a weekend or public holiday moves to the business day
  before or after; Cámara de Comercio and RUT default to the Colombian business day before).
- **Belongs to: people, vehicles, companies, properties, pets, digital accounts.** Each owner is
  an entity with a kind and its own fields (plate, make and model, year; tax id and country;
  address). The kind is guessed — a plate (`ABC123`, `ABC12D`, `7ABC123`) means a vehicle, a
  legal suffix (LLC, Inc., S.A.S.) a company, a domain a digital asset — and templates suggest
  one (SOAT → vehicle, Wyoming report → company, passport → person); you can always change it.
  The sidebar groups them (People, Vehicles, Companies, Properties, Digital…), each with its
  count and a dot coloured by its most urgent item; an entity's page lists its items, coming
  deadlines, renewal history, files and **links** to other entities (Juan → owner of Otherwise
  SAS, member of Otherwise Technologies LLC, owner of the Mazda), and can keep a **live .ics
  file** of just its deadlines.
- **Map**: entities and their items on a pan / zoom canvas, links drawn between them, rings
  coloured by the worst status (expired, ≤ 30 days, OK). Layered (people → companies → assets →
  items) or around one entity; drag from one node onto another to link them, click to open.
- **Reminders**: per-item lead times (default 90 / 30 / 7 / 1 days) plus the day itself, as
  system notifications while Vigía runs. On macOS a **menu-bar item** shows the next deadlines
  and a badge of what needs attention, and keeps Vigía running (and reminding) when the window is
  closed; **open at login** starts it in the background. **Snooze** (a day, a week, a month),
  **mark renewed** (asks the new expiry, suggests one from the recurrence, keeps the history), and
  a **renewal in progress** state that pauses the early reminders. A first run, or a week away,
  never produces a burst of stale reminders: only the most recent one fires.
- **Automatic checks**, opt-in per item and the only network Vigía uses for your data: a
  domain's expiry over **RDAP** (`rdap.org`, falling back to the IANA bootstrap file) and a
  certificate's `notAfter` from a **TLS handshake** with the host (expired and self-signed
  certificates are read too). Daily while the app runs, with *last checked* shown.
- **Views**: a **dashboard** (counts by urgency, the next 5, the reminders coming in the next 30
  days, what was sent, the automatic checks), a **timeline** of the next 12 months grouped by
  month and coloured by urgency, a **calendar** month view, and a **list** with filters by
  owner (or kind of owner), category and status (expired / ≤ 30 days / ≤ 90 days / OK) plus search.
- **Quick add** with natural dates: `Pasaporte @Ana 12/05/2031`, `SOAT moto en 3 meses`,
  `example.com 2027-08-13`, `SSL api.example.com vence el 15 mar 2027`, `Passeport Luc dans 2
  ans`, `SOAT #ABC123 en 3 meses`, `RUT @"Otherwise SAS"`. The type is guessed from keywords in
  four languages; `@Ana`, `@"Mazda ABC123"` or `#ABC123` names the owner (matched to an
  existing one by name, plate or tax id, else created with a guessed kind), and a domain name
  becomes a domain item with its host filled in.
- **Public holidays and other calendars**: computed holidays for Colombia (Ley Emiliani and the
  Easter-based days), the US (federal), Wyoming, Spain, Mexico, Brazil and Canada, switched on
  per country, in the calendar and the timeline. Any **.ics file or URL** (a Google public
  holiday calendar, a team calendar) can be added as a read-only overlay and refreshed.
- **Read a document (OCR, optional, local)**: drop a photo, scan or PDF and Vigía reads the
  expiry date, issue date and document number to prefill the item — passports and ID cards from
  their machine-readable zone with check digits. Images are read with
  [ocrs](https://github.com/robertknight/ocrs) on your computer (its two small models, ~12 MB,
  are downloaded once when you first ask); PDFs from their text layer. Nothing goes to any cloud.
- **Import / export**: CSV both ways, an **.ics** export, and an .ics file kept up to date that
  your calendar app can subscribe to (Settings → Calendar).
- **Your data, one file**: a local **SQLite** database, by default in the app's data folder
  (`~/Library/Application Support/Vigia/vigia.db` on macOS, `%APPDATA%\Vigia` on Windows,
  `~/.local/share/vigia` on Linux). Plain rollback journal (no WAL), so it can live in Dropbox,
  iCloud or Syncthing: Settings → *Database location* moves it (with its attachments) or opens
  another `.db`; the last one is remembered. Attachments sit next to it in `vigia-files/`,
  referenced by relative path. Integrity check on open, forward-only migrations. **Back up**
  writes a zip of the database and the attachments; *Restore* unpacks one and opens it.

## Try it

```bash
# the app, on a throwaway database with sample items (your real one is untouched)
cargo run -p vigia-app -- --db /tmp/vigia-demo.db --demo

# a macOS .app (debug or release; UNIVERSAL=1 for Apple silicon + Intel)
script/bundle-macos.sh debug
```

`vigia-app --db PATH` uses that database for the session only, without remembering it;
`--background` starts in the menu bar without a window (what *open at login* uses).

## Command line

`vigia` works on the same database as the app (or `VIGIA_DB`, or `--db PATH`). Every command
takes `--json`.

```bash
vigia due --days 30 --json            # what expires in the next 30 days (and what already did)
vigia list --status soon --owner Ana  # filters: status, owner, category, free text
vigia add "Pasaporte @Ana 12/05/2031" # quick-add syntax, or --kind passport --name … --expires …
vigia renew 7 --expires 2036-05-12 --cost 420000 --note "consulado"
vigia check --all                     # RDAP / TLS checks of the items that opted in
vigia export ics vigia.ics            # or csv; import FILE.csv; backup FILE.zip
vigia kinds                           # types, categories and default reminders
vigia entities --kind vehicle         # people, vehicles, companies… with their item counts
vigia entity "Otherwise SAS"          # details, links and items; --name/--kind/--ident… to edit
vigia relate Juan "Otherwise SAS" --role owner
vigia holidays --country co --year 2026
```

JSON items carry `id`, `kind`, `name`, `owner`, `owner_kind`, `entity_id`, `category`, `issued`, `expires`, `due` (the day
to act: the expiry, or the notice deadline of a contract), `days_left`, `status`, `state`,
`recurrence`, `business_days`, cost, provider, reference, tags, lead days, host and check
results. CSV has `owner_kind` and `business_days` columns; .ics events name the owner and its
kind.

## Privacy

Nothing leaves your computer except: the opt-in RDAP / TLS checks of the items you mark, the
one-time OCR model download when you ask for it, and an unauthenticated, once-a-day request for
this repository's GitHub star count (Settings → *Show GitHub stars* turns it off).

## Build

Rust stable (see `rust-toolchain.toml`). The GPUI stack is pinned in `Cargo.lock` (zed
`eb5483528be607b497afdaad0a237bebeb191249`, gpui-component
`972a3ebfd01afca7da6d8b6f31c9a51288ea5565`). Linux needs the usual GPUI system libraries (see
`.github/workflows/release.yml`).

```bash
cargo test -p vigia-i18n -p vigia-core -p vigia-cli -p vigia-app
cargo run -p vigia-app
python3 script/make-icons.py   # redraw the icon (Pillow)
```

Pushing a `v*` tag builds macOS (universal), Linux and Windows downloads and attaches them to a
GitHub Release.

## License

MIT or Apache-2.0, at your option. Made by [Otherwise Technologies LLC](https://otherwisetech.com),
Wyoming, USA.
