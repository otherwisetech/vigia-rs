# Le moteur : types, états, rappels, base de données, vérifications.

# categories
core.cat.identity = Identité
core.cat.vehicle = Véhicule
core.cat.insurance = Assurances
core.cat.warranty = Garanties
core.cat.digital = Numérique
core.cat.contract = Contrats
core.cat.company = Entreprise
core.cat.membership = Adhésions
core.cat.health = Santé
core.cat.home = Maison
core.cat.other = Autres

# types
core.kind.passport = Passeport
core.kind.national_id = Carte d'identité
core.kind.driver_license = Permis de conduire
core.kind.visa = Visa
core.kind.residence_permit = Titre de séjour
core.kind.soat = SOAT (assurance obligatoire)
core.kind.tecnomecanica = Contrôle technique
core.kind.vehicle_registration = Carte grise / vignette
core.kind.vehicle_insurance = Assurance auto
core.kind.insurance = Contrat d'assurance
core.kind.warranty = Garantie
core.kind.domain = Nom de domaine
core.kind.certificate = Certificat SSL/TLS
core.kind.subscription = Abonnement / forfait annuel
core.kind.contract = Contrat
core.kind.wyoming_report = Rapport annuel du Wyoming
core.kind.camara_comercio = Renouvellement Chambre de commerce
core.kind.rut = Mise à jour du RUT
core.kind.membership = Adhésion
core.kind.vaccine = Vaccin
core.kind.checkup = Bilan de santé
core.kind.gas_inspection = Contrôle gaz
core.kind.extinguisher = Extincteur
core.kind.custom = Autre

# recurrence
core.rec.none = Ne se répète pas
core.rec.days.one = Chaque jour
core.rec.days.other = Tous les {n} jours
core.rec.weeks.one = Chaque semaine
core.rec.weeks.other = Toutes les {n} semaines
core.rec.months.one = Chaque mois
core.rec.months.other = Tous les {n} mois
core.rec.years.one = Chaque année
core.rec.years.other = Tous les {n} ans
core.rec.wyoming = Chaque année, le 1er {month} (mois de création)
core.rec.yearly = Chaque année, le {day} {month}

# months
core.month.1 = janvier
core.month.2 = février
core.month.3 = mars
core.month.4 = avril
core.month.5 = mai
core.month.6 = juin
core.month.7 = juillet
core.month.8 = août
core.month.9 = septembre
core.month.10 = octobre
core.month.11 = novembre
core.month.12 = décembre

# status
core.status.expired = Expiré
core.status.soon = ≤ 30 jours
core.status.upcoming = ≤ 90 jours
core.status.ok = À jour
core.status.undated = Sans date
core.item.title = {name} ({owner})

# reminders
core.remind.in.one = Expire demain
core.remind.in.other = Expire dans {n} jours
core.remind.today = Expire aujourd'hui
core.remind.expired_ago.one = A expiré hier
core.remind.expired_ago.other = A expiré il y a {n} jours
core.remind.notice_in.one = Le préavis doit être donné demain
core.remind.notice_in.other = Plus que {n} jours pour donner le préavis
core.remind.notice_today = Dernier jour pour donner le préavis
core.remind.notice_passed.one = Le délai de préavis est passé hier
core.remind.notice_passed.other = Le délai de préavis est passé il y a {n} jours

# database
core.db.not_database = Le fichier n'est pas une base SQLite.
core.db.error = Erreur de base de données : {detail}
core.db.io = Impossible de lire ou d'écrire le fichier : {detail}
core.db.not_vigia = C'est une base de données, mais pas de Vigía.
core.db.too_new = Créée par une version plus récente de Vigía (schéma {version}). Mettez l'application à jour.
core.db.corrupt = La base de données est endommagée : {detail}
core.db.not_found = L'élément {id} n'existe pas.
core.db.read_only = Impossible d’écrire dans cette base de données : le fichier ou son dossier est en lecture seule. Rien n’a été enregistré. Vérifiez les autorisations du fichier, ou que l’app de synchronisation ne le retient pas, puis rouvrez-le.

# calendar
core.ics.calendar_name = Vigía : échéances
core.ics.expires = Expire : {title}
core.ics.notice = Dernier jour de préavis : {title}

# checks
core.check.rdap_not_found = Le registre ne connaît pas ce domaine.
core.check.http = Le serveur a répondu {code}.
core.check.bad_domain = « {name} » ne ressemble pas à un nom de domaine.
core.check.no_rdap = Aucun serveur RDAP connu pour {name}.
core.check.bad_answer = Réponse illisible.
core.check.bad_host = « {host} » n'est pas un hôte valide.
core.check.dns = Impossible de trouver {host} : {detail}
core.check.connect = Connexion à {host} impossible : {detail}
core.check.tls = Échec de la négociation TLS : {detail}
core.check.no_cert = Le serveur n'a envoyé aucun certificat.
core.check.not_checkable = Ce type ne peut pas être vérifié automatiquement.
core.check.domain_ok = RDAP : expire le {date}
core.check.cert_ok = Certificat valable jusqu'au {date} · {issuer}
core.check.no_expiry = La réponse n'indique pas de date d'expiration.
core.check.failed = Échec de la vérification : {detail}

# ocr
core.ocr.models_missing = Les modèles OCR ne sont pas encore téléchargés (environ 12 Mo, une seule fois).
core.ocr.unavailable = Cette version n'inclut pas l'OCR.
core.ocr.unsupported = Seuls les images (PNG, JPG, WebP) et les PDF sont lus.
core.ocr.failed = Impossible de lire le document : {detail}

# demo
core.demo.passport = Passeport
core.demo.soat = SOAT
core.demo.tecno = Contrôle technique
core.demo.cert = Certificat du site web
core.demo.warranty = Garantie de l'ordinateur portable
core.demo.lease = Bail de l'appartement
core.demo.camara = Immatriculation commerciale
core.demo.sub = Forfait cloud annuel
core.demo.extinguisher = Recharge de l'extincteur
core.entity.person = Personne
core.entity.vehicle = Véhicule
core.entity.company = Entreprise
core.entity.property = Bien immobilier
core.entity.pet = Animal
core.entity.digital = Numérique
core.entity.other = Autre
core.entity.ident.person = Numéro de pièce
core.entity.ident.vehicle = Plaque
core.entity.ident.company = Identifiant fiscal (SIREN, EIN)
core.entity.ident.property = Adresse
core.entity.ident.digital = Site ou compte
core.entity.detail.vehicle = Marque et modèle
core.entity.detail.pet = Espèce et race
core.entity.detail.digital = Fournisseur
core.entity.detail.other = Description
core.role.owner = propriétaire
core.role.member = associé
core.role.manager = gérant
core.role.family = famille
core.role.uses = utilise
core.role.other = lié
core.db.self_relation = Une entité ne peut pas être liée à elle-même.
core.ics.belongs_to = {kind} : {owner}
core.country.co = Colombie
core.country.us = États-Unis (fédéral)
core.country.us_wy = Wyoming (État)
core.country.es = Espagne
core.country.mx = Mexique
core.country.br = Brésil
core.country.ca = Canada (fédéral)
core.biz.weekends_only = week-ends seulement
core.biz.previous = Jour ouvré précédent ({holidays})
core.biz.next = Jour ouvré suivant ({holidays})
core.cal.not_ics = Ce n'est pas un fichier iCalendar (.ics).

# shared registry of the Otherwise apps
core.sync.error = Registre partagé : {detail}
core.cal.reminder = Rappel : {title}
core.tpl.soat = SOAT (assurance obligatoire)
core.tpl.tecnomecanica = Contrôle technique (técnico-mecánica)
core.tpl.vehicle_insurance = Assurance tous risques
core.tpl.vehicle_tax = Taxe sur le véhicule
core.tpl.oil_change = Vidange
core.tpl.brakes = Contrôle des freins
core.tpl.vehicle_registration = Carte grise / immatriculation
core.tpl.pet_rabies = Vaccin antirabique
core.tpl.pet_multiple = Vaccin polyvalent
core.tpl.pet_deworming = Vermifuge
core.tpl.pet_fleas = Antipuces
core.tpl.pet_checkup = Visite chez le vétérinaire
core.tpl.pet_grooming = Toilettage
core.tpl.birthday = Anniversaire
core.tpl.national_id = Carte d'identité
core.tpl.passport = Passeport
core.tpl.visa = Visa
core.tpl.driver_license = Permis de conduire
core.tpl.medical = Rendez-vous médical
core.tpl.dentist = Dentiste
core.tpl.lab_tests = Examens médicaux
core.tpl.health_plan = Mutuelle / assurance santé
core.tpl.camara_comercio = Renouvellement du registre du commerce
core.tpl.rut = Mise à jour du registre fiscal (RUT)
core.tpl.income_tax = Déclaration d'impôt sur le revenu
core.tpl.vat = Déclaration de TVA
core.tpl.withholding = Retenue à la source
core.tpl.annual_report = Rapport annuel de la LLC
core.tpl.policy = Police d'assurance
core.tpl.contract = Contrat
core.tpl.domain = Nom de domaine
core.tpl.certificate = Certificat SSL/TLS
core.tpl.software_license = Licence logicielle
core.tpl.property_tax = Taxe foncière
core.tpl.hoa_fee = Charges de copropriété
core.tpl.utilities = Factures (eau, gaz, électricité)
core.tpl.home_insurance = Assurance habitation
core.tpl.gas_inspection = Contrôle du gaz
core.tpl.water_heater = Entretien du chauffe-eau
core.tpl.air_conditioning = Entretien de la climatisation
core.tpl.extinguisher = Recharge de l'extincteur
core.tpl.appliance_warranty = Garantie d'appareil
core.tpl.subscription = Abonnement
core.tplnote.accountant = Vérifiez les dates avec votre comptable : elles dépendent de votre numéro fiscal et du calendrier de l'année.
core.tplnote.check = Avec l'hôte renseigné, Vigía peut vérifier la date automatiquement (RDAP / TLS).
core.tplnote.real_date = Indiquez la date inscrite sur le document.
core.tplnote.runt = Vérifiez dans le registre officiel quand le vôtre arrive à échéance.
core.tplnote.local_tax = Votre commune ou région fixe les dates et réductions chaque année.
core.tplnote.km = Ou au kilométrage : notez le km de la prochaine.
core.tplnote.vet = Confirmez le calendrier avec votre vétérinaire.
core.tplnote.private = Le numéro reste sur cet ordinateur ; seuls les 4 derniers s'affichent.
