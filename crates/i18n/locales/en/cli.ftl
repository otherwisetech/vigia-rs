# The `vigia` command line.
cli.usage = Usage: vigia [--db PATH] [--json] <command>\n\n  list [text] [--status expired|soon|upcoming|ok|undated] [--owner NAME] [--owner-kind vehicle] [--category ID] [--all]\n  due [--days 30]              what expires in the next N days (and what already did)\n  show <id>                    details, renewal history and attachments\n  add "Passport @Ana 05/12/2031"   or  add --kind passport --name … --expires …\n  edit <id> [--name … --expires … --owner … --host … --lead "90,30,7" --business-days prev:co|next|none …]\n  renew <id> [--expires DATE] [--cost N] [--note TEXT]\n  delete <id>\n  check <id> | --all           check domains (RDAP) and certificates (TLS) over the network\n  export csv|ics [FILE]        import FILE.csv        backup FILE.zip\n  entities [--kind vehicle]    people, vehicles, companies, properties, pets, digital\n  entity <id|name> [--name … --kind … --ident PLATE/TAX-ID --detail … --year … --country …] [--delete]\n  relate <from> <to> [--role owner|member|manager|family|uses|other] [--remove]\n  holidays [--country co,us,us-wy,es,mx,br,ca] [--year 2026]   public holidays\n  templates [--kind vehicle]   suggested reminders for each kind of owner (SOAT, vaccines, filings…)\n  kinds                        types and their default reminders\n  today                        publish what's due to Otherwise Apps' Today view\n  sync                         sync people, vehicles, companies… with the shared registry (if on)\n  mcp [--allow-writes | --propose FOLDER]   MCP server on stdin/stdout (read-only; --propose leaves changes as proposals for the app)\n  path                         the database in use\n\nDates: 2031-05-12, 05/12/2031, May 12 2031, "in 3 months". The database is the app's (or VIGIA_DB, or --db).
cli.days_left.one = 1 day left
cli.days_left.other = {n} days left
cli.days_ago.one = 1 day ago
cli.days_ago.other = {n} days ago
cli.list.empty = No items.
cli.due.none.one = Nothing expires in the next day.
cli.due.none.other = Nothing expires in the next {n} days.
cli.show.renewed = Renewed on {on} → {to}
cli.add.done = Added: {line}
cli.renew.done = {name}: renewed until {date}
cli.delete.done = Deleted: {name}
cli.export.done = Wrote {file}
cli.import.done.one = Imported 1 item.
cli.import.done.other = Imported {n} items.
cli.backup.done.one = Backup written to {file} (1 attachment).
cli.backup.done.other = Backup written to {file} ({n} attachments).

# errors
cli.err.needs_value = --{option} needs a value.
cli.err.bad_date = Can't read the date “{value}”.
cli.err.bad_number = “{value}” isn't a number.
cli.err.bad_recurrence = Unknown recurrence “{value}” (none, every:1y, every:6m, wyoming:3, yearly:03-31).
cli.err.bad_kind = Unknown type “{value}”. See `vigia kinds`.
cli.err.bad_category = Unknown category “{value}”.
cli.err.bad_status = Unknown status “{value}” (expired, soon, upcoming, ok, undated).
cli.err.needs_id = Missing the item id (see `vigia list`).
cli.err.no_data_dir = Couldn't find the data folder; use --db.
cli.err.add_usage = Usage: vigia add "Passport @Ana 2031-05-12" or vigia add --kind passport --name … --expires …
cli.err.export_usage = Usage: vigia export csv|ics [FILE]
cli.err.import_usage = Usage: vigia import FILE.csv
cli.err.backup_usage = Usage: vigia backup FILE.zip
cli.err.unknown = Unknown command “{cmd}”. Try `vigia help`.
cli.err.bad_entity_kind = Unknown kind of owner "{value}" (person, vehicle, company, property, pet, digital, other).
cli.err.needs_entity = Which one? Give an id or a name.
cli.err.no_entity = Nothing is called "{value}".
cli.entities.empty = No people, vehicles or companies yet.
cli.relate.done = {from} → {to} ({role})
cli.relate.removed = {from} and {to} are no longer linked.
cli.entity.items.one = 1 item
cli.entity.items.other = {n} items
cli.err.bad_business_days = Unknown business-day rule "{value}" (prev, next, prev:co, next:us-wy, none).
cli.err.bad_country = Unknown holiday calendar "{value}" (co, us, us-wy, es, mx, br, ca).

# Otherwise Apps: Today view and shared registry
cli.today.done.one = 1 event published to the Today view of Otherwise Apps.
cli.today.done.other = {n} events published to the Today view of Otherwise Apps.
cli.today.off = The Today feed is off (Settings in Vigía).
cli.sync.off = The shared registry is off. Turn it on in Otherwise Apps.
cli.sync.done = Shared registry: {pushed} sent, {pulled} received, {created} new.

# background jobs (Otherwise Helper)
cli.bg.reminders_label = Deadline reminders
cli.bg.checks_label = Domain and certificate checks
cli.bg.unknown_job = Unknown background job
cli.bg.app_open = Vigía is open and does this itself
cli.bg.elsewhere = the database is open on {host}
cli.bg.failed = couldn't run: {error}
cli.bg.no_reminders = no reminders due
cli.bg.reminded.one = {n} reminder sent
cli.bg.reminded.other = {n} reminders sent
cli.bg.down = {item}: check failed
cli.bg.recovered = {item} is reachable again
cli.bg.renewed = {item} renewed until {date}
cli.bg.failing.one = {n} check failing
cli.bg.failing.other = {n} checks failing
cli.bg.all_ok = all checks OK
cli.err.needs_template = Missing the template (see `vigia templates`).
cli.err.bad_template = Unknown template “{value}”. See `vigia templates`.
cli.err.needs_owner = Which one? Give the owner ({kind}).
