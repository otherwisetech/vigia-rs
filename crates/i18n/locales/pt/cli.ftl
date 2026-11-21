# A linha de comando `vigia`.
cli.usage = Uso: vigia [--db CAMINHO] [--json] <comando>\n\n  list [texto] [--status expired|soon|upcoming|ok|undated] [--owner NOME] [--category ID] [--all]\n  due [--days 30]              o que vence nos próximos N dias (e o que já venceu)\n  show <id>                    detalhes, histórico de renovações e anexos\n  add "Passaporte @Ana 12/05/2031"   ou  add --kind passport --name … --expires …\n  edit <id> [--name … --expires … --owner … --host … --lead "90,30,7" …]\n  renew <id> [--expires DATA] [--cost N] [--note TEXTO]\n  delete <id>\n  check <id> | --all           verifica domínios (RDAP) e certificados (TLS) pela rede\n  export csv|ics [ARQUIVO]     import ARQUIVO.csv     backup ARQUIVO.zip\n  kinds                        tipos e seus lembretes padrão\n  path                         o banco de dados em uso\n\nDatas: 2031-05-12, 12/05/2031, 12 mai 2031, "em 3 meses". O banco de dados é o do aplicativo (ou VIGIA_DB, ou --db).
cli.days_left.one = falta 1 dia
cli.days_left.other = faltam {n} dias
cli.days_ago.one = venceu há 1 dia
cli.days_ago.other = venceu há {n} dias
cli.list.empty = Nenhum item.
cli.due.none.one = Nada vence no próximo dia.
cli.due.none.other = Nada vence nos próximos {n} dias.
cli.show.renewed = Renovado em {on} → {to}
cli.add.done = Adicionado: {line}
cli.renew.done = {name}: renovado até {date}
cli.delete.done = Excluído: {name}
cli.export.done = Gravado {file}
cli.import.done.one = Importado 1 item.
cli.import.done.other = Importados {n} itens.
cli.backup.done.one = Backup gravado em {file} (1 anexo).
cli.backup.done.other = Backup gravado em {file} ({n} anexos).

# errors
cli.err.needs_value = --{option} precisa de um valor.
cli.err.bad_date = Não entendi a data “{value}”.
cli.err.bad_number = “{value}” não é um número.
cli.err.bad_recurrence = Recorrência desconhecida “{value}” (none, every:1y, every:6m, wyoming:3, yearly:03-31).
cli.err.bad_kind = Tipo desconhecido “{value}”. Veja `vigia kinds`.
cli.err.bad_category = Categoria desconhecida “{value}”.
cli.err.bad_status = Estado desconhecido “{value}” (expired, soon, upcoming, ok, undated).
cli.err.needs_id = Falta o id do item (veja `vigia list`).
cli.err.no_data_dir = Pasta de dados não encontrada; use --db.
cli.err.add_usage = Uso: vigia add "Passaporte @Ana 12/05/2031" ou vigia add --kind passport --name … --expires …
cli.err.export_usage = Uso: vigia export csv|ics [ARQUIVO]
cli.err.import_usage = Uso: vigia import ARQUIVO.csv
cli.err.backup_usage = Uso: vigia backup ARQUIVO.zip
cli.err.unknown = Comando desconhecido “{cmd}”. Tente `vigia help`.
