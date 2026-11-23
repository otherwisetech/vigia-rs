# La línea de comandos `vigia`.
cli.usage = Uso: vigia [--db RUTA] [--json] <comando>\n\n  list [texto] [--status expired|soon|upcoming|ok|undated] [--owner NOMBRE] [--owner-kind vehicle] [--category ID] [--all]\n  due [--days 30]              lo que vence en los próximos N días (y lo vencido)\n  show <id>                    detalle, historial de renovaciones y adjuntos\n  add "Pasaporte @Ana 12/05/2031"   o  add --kind passport --name … --expires …\n  edit <id> [--name … --expires … --owner … --host … --lead "90,30,7" --business-days prev:co|next|none …]\n  renew <id> [--expires FECHA] [--cost N] [--note TEXTO]\n  delete <id>\n  check <id> | --all           revisa dominios (RDAP) y certificados (TLS) por la red\n  export csv|ics [ARCHIVO]     import ARCHIVO.csv     backup ARCHIVO.zip\n  entities [--kind vehicle]    personas, vehículos, empresas, inmuebles, mascotas, digital\n  entity <id|nombre> [--name … --kind … --ident PLACA/NIT --detail … --year … --country …] [--delete]\n  relate <de> <a> [--role owner|member|manager|family|uses|other] [--remove]\n  holidays [--country co,us,us-wy,es,mx,br,ca] [--year 2026]   días festivos\n  kinds                        tipos y sus recordatorios por defecto\n  today                        publica lo que vence en la vista Hoy de Otherwise Apps\n  sync                         sincroniza personas, vehículos, empresas… con el registro compartido (si está activo)\n  mcp [--allow-writes]         servidor MCP por stdin/stdout (solo lectura salvo --allow-writes)\n  path                         la base de datos en uso\n\nFechas: 2031-05-12, 12/05/2031, 12 may 2031, «en 3 meses». La base de datos es la misma de la app (o VIGIA_DB, o --db).
cli.days_left.one = falta 1 día
cli.days_left.other = faltan {n} días
cli.days_ago.one = venció hace 1 día
cli.days_ago.other = venció hace {n} días
cli.list.empty = No hay elementos.
cli.due.none.one = Nada vence en el próximo día.
cli.due.none.other = Nada vence en los próximos {n} días.
cli.show.renewed = Renovado el {on} → {to}
cli.add.done = Añadido: {line}
cli.renew.done = {name}: renovado hasta {date}
cli.delete.done = Eliminado: {name}
cli.export.done = Escrito {file}
cli.import.done.one = Importado 1 elemento.
cli.import.done.other = Importados {n} elementos.
cli.backup.done.one = Copia de seguridad en {file} (1 adjunto).
cli.backup.done.other = Copia de seguridad en {file} ({n} adjuntos).

# errors
cli.err.needs_value = --{option} necesita un valor.
cli.err.bad_date = No entiendo la fecha «{value}».
cli.err.bad_number = «{value}» no es un número.
cli.err.bad_recurrence = Recurrencia desconocida «{value}» (none, every:1y, every:6m, wyoming:3, yearly:03-31).
cli.err.bad_kind = Tipo desconocido «{value}». Mira `vigia kinds`.
cli.err.bad_category = Categoría desconocida «{value}».
cli.err.bad_status = Estado desconocido «{value}» (expired, soon, upcoming, ok, undated).
cli.err.needs_id = Falta el id del elemento (mira `vigia list`).
cli.err.no_data_dir = No se encontró la carpeta de datos; usa --db.
cli.err.add_usage = Uso: vigia add "Pasaporte @Ana 12/05/2031" o vigia add --kind passport --name … --expires …
cli.err.export_usage = Uso: vigia export csv|ics [ARCHIVO]
cli.err.import_usage = Uso: vigia import ARCHIVO.csv
cli.err.backup_usage = Uso: vigia backup ARCHIVO.zip
cli.err.unknown = Comando desconocido «{cmd}». Prueba `vigia help`.
cli.err.bad_entity_kind = Tipo de dueño desconocido «{value}» (person, vehicle, company, property, pet, digital, other).
cli.err.needs_entity = ¿Cuál? Indica un id o un nombre.
cli.err.no_entity = No hay nada llamado «{value}».
cli.entities.empty = Aún no hay personas, vehículos ni empresas.
cli.relate.done = {from} → {to} ({role})
cli.relate.removed = {from} y {to} ya no están relacionados.
cli.entity.items.one = 1 elemento
cli.entity.items.other = {n} elementos
cli.err.bad_business_days = Regla de días hábiles desconocida «{value}» (prev, next, prev:co, next:us-wy, none).
cli.err.bad_country = Calendario de festivos desconocido «{value}» (co, us, us-wy, es, mx, br, ca).

# Otherwise Apps: Today view and shared registry
cli.today.done.one = 1 evento publicado en la vista Hoy de Otherwise Apps.
cli.today.done.other = {n} eventos publicados en la vista Hoy de Otherwise Apps.
cli.today.off = La vista Hoy está desactivada (Ajustes de Vigía).
cli.sync.off = El registro compartido está desactivado. Actívalo en Otherwise Apps.
cli.sync.done = Registro compartido: {pushed} enviados, {pulled} recibidos, {created} nuevos.

# trabajos en segundo plano (Otherwise Helper)
cli.bg.reminders_label = Recordatorios de vencimientos
cli.bg.checks_label = Revisión de dominios y certificados
cli.bg.unknown_job = Trabajo en segundo plano desconocido
cli.bg.app_open = Vigía está abierto y lo hace él mismo
cli.bg.elsewhere = la base de datos está abierta en {host}
cli.bg.failed = no se pudo ejecutar: {error}
cli.bg.no_reminders = sin recordatorios pendientes
cli.bg.reminded.one = {n} recordatorio enviado
cli.bg.reminded.other = {n} recordatorios enviados
cli.bg.down = {item}: la revisión falló
cli.bg.recovered = {item} vuelve a responder
cli.bg.renewed = {item} renovado hasta {date}
cli.bg.failing.one = {n} revisión fallando
cli.bg.failing.other = {n} revisiones fallando
cli.bg.all_ok = todas las revisiones bien
