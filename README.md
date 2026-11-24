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
- **Sections by kind, with one-click reminders.** The sidebar always shows *Vehículos*,
  *Mascotas*, *Personas*, *Empresas*, *Hogar* and *Digital* (plus *Otros* when used). A section
  lists its entities as cards — the next deadline, its status (overdue, soon, OK) and the
  number of items — with *Agregar vehículo / mascota / …* to add one. Each kind has
  **suggested reminders** as chips; one click opens the new-item form already filled with the
  name, category, a typical interval and lead times, so you only set the date. Colombian-first:
  SOAT, técnico-mecánica, seguro todo riesgo, impuesto vehicular, cambio de aceite, frenos,
  licencia de tránsito; vacunas (rabia, múltiple), desparasitación, antipulgas, control
  veterinario, baño; cumpleaños, cédula, pasaporte, visa, licencia de conducción, citas médicas,
  odontología, exámenes, EPS; matrícula mercantil, RUT, renta, IVA, retención en la fuente,
  pólizas, contratos, dominios and certificates (with the automatic checks), licencias de
  software; predial, administración, servicios, seguro de hogar, gas, calentador, aire
  acondicionado, extintor, garantías. Templates never set a date: legal dates (SOAT expiry, tax
  calendars) are entered by you, and filings carry a *verifica las fechas con tu contador* note.
  The top bar's **Sugerido** menu reaches the same from anywhere (kind → who → what);
  `vigia templates` lists the catalogue. A person's document number shows only its last 4
  characters on screen and to the assistant.
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
- **Your data, one file**: a local **SQLite** database, by default in the Otherwise data folder
  shared by all Otherwise apps (`~/.otherwise/vigia/vigia.db`; move that folder, e.g. to
  OneDrive, from Otherwise Apps → Settings). Versions before 0.2.1 used
  `~/Library/Application Support/Vigia/vigia.db` (`%APPDATA%\Vigia`, `~/.local/share/vigia`):
  the first launch offers to move it, verified, with its attachments. Plain rollback journal (no
  WAL), so it can live in Dropbox, iCloud, OneDrive or Syncthing; a lock file shows when it is
  open on another computer and conflicted copies are flagged. File → *Save As…*, *Move Database
  To…*, *Show in Finder*, *Open Another Database…* and *Open Recent* (with *Locate…* for missing
  ones); a custom location is remembered. Attachments sit next to it in `vigia-files/`,
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
vigia templates --kind pet            # suggested reminders per kind (SOAT, vaccines, filings…)
vigia entity "Otherwise SAS"          # details, links and items; --name/--kind/--ident… to edit
vigia relate Juan "Otherwise SAS" --role owner
vigia holidays --country co --year 2026
vigia today                           # publish what's due to Otherwise Apps' Today view
vigia sync                            # sync entities with the shared registry (when turned on)
vigia mcp                             # MCP server on stdio (read-only; --allow-writes adds writes)
```

JSON items carry `id`, `kind`, `name`, `owner`, `owner_kind`, `entity_id`, `category`, `issued`, `expires`, `due` (the day
to act: the expiry, or the notice deadline of a contract), `days_left`, `status`, `state`,
`recurrence`, `business_days`, cost, provider, reference, tags, lead days, host and check
results. CSV has `owner_kind` and `business_days` columns; .ics events name the owner and its
kind.

## Otherwise Apps

Vigía plugs into [otherwise-link](https://github.com/otherwisetech/otherwise-link), the shared
layer of the Otherwise Technologies apps. Everything stays on the computer.

- **Deep links.** The app registers `otherwise-vigia://` (and answers `otherwise://vigia/…`
  through Otherwise Apps): macOS through the bundle's `Info.plist`, Linux through
  `packaging/linux/io.github.otherwisetech.vigia-app.desktop` (`MimeType=x-scheme-handler/…`,
  `Exec=… %u`), Windows through `HKCU\Software\Classes` on first start.

  | Link | Opens |
  |------|-------|
  | `otherwise://vigia` | the window |
  | `otherwise://vigia/item/<id>` | an item |
  | `otherwise://vigia/owner/<id>` | an entity's page (Vigía's id) |
  | `otherwise://vigia/entity/<uuid>` | the entity linked to a shared card |
  | `otherwise://vigia/new?kind=certificate&name=…&owner=…&expires=YYYY-MM-DD&host=…` | the new-item form, prefilled (also `text` = a quick-add line, `owner_kind`, `entity`, `issued`, `months`, `provider`, `reference`, `url`, `notes`, `tags`, `check=1`); nothing is saved until you do |

- **Calendar.** Every live item's deadline (expiry, or a contract's notice deadline) and its
  next reminder day, from 60 days back to 18 months ahead, go to `<data folder>/vigia/events.json`
  for Otherwise Apps' Calendar (otherwise-link SPEC §9), each linking to `otherwise://vigia/item/<id>`;
  health items are private there ("Busy" in exports and for assistants). The same Settings switch
  as Today turns it off.
- **Today.** What is overdue or due in the next 30 days goes to `today/vigia.json`, which
  Otherwise Apps shows in its Today view, each with a link to the item. Rewritten when it
  changes; Settings → *Show what’s due in Today and Calendar* turns it off (and removes the file).
  `vigia today` publishes it from the command line.
- **Shared entities** (only when turned on in Otherwise Apps). People, companies, vehicles,
  properties and digital assets are synced both ways with the shared registry: matched by card
  id, by Vigía's ref, by tax id / plate / domain, else by name, so nothing is duplicated. Edits
  in other apps land here; merges made in Otherwise Apps are followed; a card deleted there
  leaves the entity in Vigía (detached), and deleting an entity here only unlinks it. Pets and
  "other" stay local. `vigia sync` runs it from the command line; the app syncs after edits
  and every 5 minutes.
- **MCP.** `vigia mcp` is a stdio MCP server, read-only: `summary`, `checks`, `entity`,
  `list_items`, `due`, `entities`, `item`, `entities_by_kind` (the sections: each owner's next
  deadline, status and missing suggestions) and `templates`. With `--allow-writes` (which
  Otherwise Apps' hub passes only when you allow write tools) it adds `add_item`, `renew_item`
  and `propose_add_from_template` (*recuérdame el SOAT del carro*); with `--propose <folder>`
  (the app's assistant) those only leave a proposal for the app. People's document numbers
  (and identity documents' reference numbers) reach it masked to their last 4 characters.

  ```bash
  claude mcp add vigia -- vigia mcp            # alone, or through Otherwise Apps' hub
  ```

## AI assistant (⌘J)

The **Asistente** button (or ⌘J) docks the shared Otherwise AI panel
([otherwise-agent-panel](https://github.com/otherwisetech/otherwise-agent-panel)) next to every
view, as a bar, at the bottom, on the right or full window, in Chat or Terminal mode. It runs
**your own** Claude Code, Codex or opencode, or a model of your own **Ollama** on this computer,
with `vigia mcp` on the open database for that session only; your agent's configuration is never changed. Try *¿Qué está vencido y desde
cuándo?*, *¿Qué certificados y dominios vencen pronto?*, *Resume lo que vence este mes* or
*¿Qué revisiones automáticas están fallando?*.

It reads items, deadlines, renewal history, domain / certificate check results and owners. In
*Preguntar antes de cambiar* or *Automático* it may propose adding an item or recording a
renewal: each proposal is a card, saved only when you press *Aplicar*. What you ask and what
the assistant reads goes to the provider you picked, with your own account (with Ollama it stays
on this computer); the panel says so under the input.

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
