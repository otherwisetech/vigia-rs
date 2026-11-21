# La ligne de commande `vigia`.
cli.usage = Usage : vigia [--db CHEMIN] [--json] <commande>\n\n  list [texte] [--status expired|soon|upcoming|ok|undated] [--owner NOM] [--category ID] [--all]\n  due [--days 30]              ce qui expire dans les N prochains jours (et ce qui a expiré)\n  show <id>                    détails, historique des renouvellements et pièces jointes\n  add "Passeport @Ana 12/05/2031"   ou  add --kind passport --name … --expires …\n  edit <id> [--name … --expires … --owner … --host … --lead "90,30,7" …]\n  renew <id> [--expires DATE] [--cost N] [--note TEXTE]\n  delete <id>\n  check <id> | --all           vérifie les domaines (RDAP) et certificats (TLS) par le réseau\n  export csv|ics [FICHIER]     import FICHIER.csv     backup FICHIER.zip\n  kinds                        types et leurs rappels par défaut\n  path                         la base de données utilisée\n\nDates : 2031-05-12, 12/05/2031, 12 mai 2031, « dans 3 mois ». La base est celle de l'application (ou VIGIA_DB, ou --db).
cli.days_left.one = 1 jour restant
cli.days_left.other = {n} jours restants
cli.days_ago.one = il y a 1 jour
cli.days_ago.other = il y a {n} jours
cli.list.empty = Aucun élément.
cli.due.none.one = Rien n'expire dans le prochain jour.
cli.due.none.other = Rien n'expire dans les {n} prochains jours.
cli.show.renewed = Renouvelé le {on} → {to}
cli.add.done = Ajouté : {line}
cli.renew.done = {name} : renouvelé jusqu'au {date}
cli.delete.done = Supprimé : {name}
cli.export.done = Écrit {file}
cli.import.done.one = 1 élément importé.
cli.import.done.other = {n} éléments importés.
cli.backup.done.one = Sauvegarde écrite dans {file} (1 pièce jointe).
cli.backup.done.other = Sauvegarde écrite dans {file} ({n} pièces jointes).

# errors
cli.err.needs_value = --{option} attend une valeur.
cli.err.bad_date = Date illisible : « {value} ».
cli.err.bad_number = « {value} » n'est pas un nombre.
cli.err.bad_recurrence = Récurrence inconnue « {value} » (none, every:1y, every:6m, wyoming:3, yearly:03-31).
cli.err.bad_kind = Type inconnu « {value} ». Voir `vigia kinds`.
cli.err.bad_category = Catégorie inconnue « {value} ».
cli.err.bad_status = État inconnu « {value} » (expired, soon, upcoming, ok, undated).
cli.err.needs_id = Il manque l'id de l'élément (voir `vigia list`).
cli.err.no_data_dir = Dossier de données introuvable ; utilisez --db.
cli.err.add_usage = Usage : vigia add "Passeport @Ana 12/05/2031" ou vigia add --kind passport --name … --expires …
cli.err.export_usage = Usage : vigia export csv|ics [FICHIER]
cli.err.import_usage = Usage : vigia import FICHIER.csv
cli.err.backup_usage = Usage : vigia backup FICHIER.zip
cli.err.unknown = Commande inconnue « {cmd} ». Essayez `vigia help`.
