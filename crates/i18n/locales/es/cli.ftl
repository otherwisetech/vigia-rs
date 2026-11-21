# La línea de comandos `vigia`.
cli.usage = Uso: vigia [--db RUTA] [--json] <comando>\n\n  list [texto] [--status expired|soon|upcoming|ok|undated] [--owner NOMBRE] [--category ID] [--all]\n  due [--days 30]              lo que vence en los próximos N días (y lo vencido)\n  show <id>                    detalle, historial de renovaciones y adjuntos\n  add "Pasaporte @Ana 12/05/2031"   o  add --kind passport --name … --expires …\n  edit <id> [--name … --expires … --owner … --host … --lead "90,30,7" …]\n  renew <id> [--expires FECHA] [--cost N] [--note TEXTO]\n  delete <id>\n  check <id> | --all           revisa dominios (RDAP) y certificados (TLS) por la red\n  export csv|ics [ARCHIVO]     import ARCHIVO.csv     backup ARCHIVO.zip\n  kinds                        tipos y sus recordatorios por defecto\n  path                         la base de datos en uso\n\nFechas: 2031-05-12, 12/05/2031, 12 may 2031, «en 3 meses». La base de datos es la misma de la app (o VIGIA_DB, o --db).
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
