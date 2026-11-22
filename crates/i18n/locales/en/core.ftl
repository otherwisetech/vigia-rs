# The engine: types, statuses, reminders, database, checks.

# categories
core.cat.identity = Identity
core.cat.vehicle = Vehicle
core.cat.insurance = Insurance
core.cat.warranty = Warranties
core.cat.digital = Digital
core.cat.contract = Contracts
core.cat.company = Company
core.cat.membership = Memberships
core.cat.health = Health
core.cat.home = Home
core.cat.other = Other

# types
core.kind.passport = Passport
core.kind.national_id = National ID card
core.kind.driver_license = Driver's license
core.kind.visa = Visa
core.kind.residence_permit = Residence permit
core.kind.soat = SOAT (mandatory vehicle insurance)
core.kind.tecnomecanica = Vehicle inspection (tecnomecánica)
core.kind.vehicle_registration = Vehicle registration
core.kind.vehicle_insurance = Vehicle insurance
core.kind.insurance = Insurance policy
core.kind.warranty = Warranty
core.kind.domain = Domain
core.kind.certificate = SSL/TLS certificate
core.kind.subscription = Subscription / annual plan
core.kind.contract = Contract
core.kind.wyoming_report = Wyoming annual report
core.kind.camara_comercio = Chamber of Commerce renewal (Colombia)
core.kind.rut = RUT update (Colombian tax ID)
core.kind.membership = Membership
core.kind.vaccine = Vaccine
core.kind.checkup = Medical check-up
core.kind.gas_inspection = Gas inspection
core.kind.extinguisher = Fire extinguisher
core.kind.custom = Other

# recurrence
core.rec.none = Doesn't repeat
core.rec.days.one = Every day
core.rec.days.other = Every {n} days
core.rec.weeks.one = Every week
core.rec.weeks.other = Every {n} weeks
core.rec.months.one = Every month
core.rec.months.other = Every {n} months
core.rec.years.one = Every year
core.rec.years.other = Every {n} years
core.rec.wyoming = Yearly, on {month} 1st (formation month)
core.rec.yearly = Yearly, on {month} {day}

# months
core.month.1 = January
core.month.2 = February
core.month.3 = March
core.month.4 = April
core.month.5 = May
core.month.6 = June
core.month.7 = July
core.month.8 = August
core.month.9 = September
core.month.10 = October
core.month.11 = November
core.month.12 = December

# status
core.status.expired = Expired
core.status.soon = ≤ 30 days
core.status.upcoming = ≤ 90 days
core.status.ok = OK
core.status.undated = No date
core.item.title = {name} ({owner})

# reminders
core.remind.in.one = Expires tomorrow
core.remind.in.other = Expires in {n} days
core.remind.today = Expires today
core.remind.expired_ago.one = Expired yesterday
core.remind.expired_ago.other = Expired {n} days ago
core.remind.notice_in.one = Notice must be given by tomorrow
core.remind.notice_in.other = {n} days left to give notice
core.remind.notice_today = Today is the last day to give notice
core.remind.notice_passed.one = The notice deadline passed yesterday
core.remind.notice_passed.other = The notice deadline passed {n} days ago

# database
core.db.not_database = The file is not an SQLite database.
core.db.error = Database error: {detail}
core.db.io = Couldn't read or write the file: {detail}
core.db.not_vigia = It's a database, but not a Vigía one.
core.db.too_new = It was made by a newer Vigía (schema {version}). Update the app.
core.db.corrupt = The database is damaged: {detail}
core.db.not_found = There is no item {id}.

# calendar
core.ics.calendar_name = Vigía: deadlines
core.ics.expires = Expires: {title}
core.ics.notice = Last day to give notice: {title}

# checks
core.check.rdap_not_found = The registry doesn't know that domain.
core.check.http = The server answered {code}.
core.check.bad_domain = “{name}” doesn't look like a domain name.
core.check.no_rdap = No known RDAP server for {name}.
core.check.bad_answer = Couldn't read the answer.
core.check.bad_host = “{host}” isn't a valid host.
core.check.dns = Couldn't find {host}: {detail}
core.check.connect = Couldn't connect to {host}: {detail}
core.check.tls = TLS handshake failed: {detail}
core.check.no_cert = The server sent no certificate.
core.check.not_checkable = This type can't be checked automatically.
core.check.domain_ok = RDAP: expires {date}
core.check.cert_ok = Certificate valid until {date} · {issuer}
core.check.no_expiry = The answer has no expiry date.
core.check.failed = Check failed: {detail}

# ocr
core.ocr.models_missing = The OCR models aren't downloaded yet (about 12 MB, once).
core.ocr.unavailable = This build doesn't include OCR.
core.ocr.unsupported = Only images (PNG, JPG, WebP) and PDFs can be read.
core.ocr.failed = Couldn't read the document: {detail}

# demo
core.demo.passport = Passport
core.demo.soat = SOAT
core.demo.tecno = Vehicle inspection
core.demo.cert = Website certificate
core.demo.warranty = Laptop warranty
core.demo.lease = Apartment lease
core.demo.camara = Business registration (Colombia)
core.demo.sub = Cloud storage annual plan
core.demo.extinguisher = Fire extinguisher recharge
core.entity.person = Person
core.entity.vehicle = Vehicle
core.entity.company = Company
core.entity.property = Property
core.entity.pet = Pet
core.entity.digital = Digital
core.entity.other = Other
core.entity.ident.person = ID number
core.entity.ident.vehicle = Plate
core.entity.ident.company = Tax ID (EIN, NIT)
core.entity.ident.property = Address
core.entity.ident.digital = Website or account
core.entity.detail.vehicle = Make and model
core.entity.detail.pet = Species and breed
core.entity.detail.digital = Provider
core.entity.detail.other = Description
core.role.owner = owner
core.role.member = member
core.role.manager = manager
core.role.family = family
core.role.uses = uses
core.role.other = related
core.db.self_relation = An entity can't be related to itself.
core.ics.belongs_to = {kind}: {owner}
core.country.co = Colombia
core.country.us = United States (federal)
core.country.us_wy = Wyoming (state)
core.country.es = Spain
core.country.mx = Mexico
core.country.br = Brazil
core.country.ca = Canada (federal)
core.biz.weekends_only = weekends only
core.biz.previous = Previous business day ({holidays})
core.biz.next = Next business day ({holidays})
core.cal.not_ics = That isn't an iCalendar (.ics) file.

# shared registry of the Otherwise apps
core.sync.error = Shared registry: {detail}
core.cal.reminder = Reminder: {title}
