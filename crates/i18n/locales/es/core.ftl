# El motor: tipos, estados, recordatorios, base de datos, revisiones.

# categories
core.cat.identity = Identidad
core.cat.vehicle = Vehículo
core.cat.insurance = Seguros
core.cat.warranty = Garantías
core.cat.digital = Digital
core.cat.contract = Contratos
core.cat.company = Empresa
core.cat.membership = Membresías
core.cat.health = Salud
core.cat.home = Hogar
core.cat.other = Otros

# types
core.kind.passport = Pasaporte
core.kind.national_id = Cédula / documento de identidad
core.kind.driver_license = Licencia de conducción
core.kind.visa = Visa
core.kind.residence_permit = Permiso de residencia
core.kind.soat = SOAT
core.kind.tecnomecanica = Revisión técnico-mecánica
core.kind.vehicle_registration = Matrícula / impuesto del vehículo
core.kind.vehicle_insurance = Seguro del vehículo
core.kind.insurance = Póliza de seguro
core.kind.warranty = Garantía
core.kind.domain = Dominio
core.kind.certificate = Certificado SSL/TLS
core.kind.subscription = Suscripción / plan anual
core.kind.contract = Contrato
core.kind.wyoming_report = Reporte anual de Wyoming
core.kind.camara_comercio = Renovación Cámara de Comercio
core.kind.rut = Actualización del RUT
core.kind.membership = Membresía
core.kind.vaccine = Vacuna
core.kind.checkup = Chequeo médico
core.kind.gas_inspection = Revisión de gas
core.kind.extinguisher = Extintor
core.kind.custom = Otro

# recurrence
core.rec.none = No se repite
core.rec.days.one = Cada día
core.rec.days.other = Cada {n} días
core.rec.weeks.one = Cada semana
core.rec.weeks.other = Cada {n} semanas
core.rec.months.one = Cada mes
core.rec.months.other = Cada {n} meses
core.rec.years.one = Cada año
core.rec.years.other = Cada {n} años
core.rec.wyoming = Cada año, el 1 de {month} (mes de constitución)
core.rec.yearly = Cada año, el {day} de {month}

# months
core.month.1 = enero
core.month.2 = febrero
core.month.3 = marzo
core.month.4 = abril
core.month.5 = mayo
core.month.6 = junio
core.month.7 = julio
core.month.8 = agosto
core.month.9 = septiembre
core.month.10 = octubre
core.month.11 = noviembre
core.month.12 = diciembre

# status
core.status.expired = Vencido
core.status.soon = ≤ 30 días
core.status.upcoming = ≤ 90 días
core.status.ok = Al día
core.status.undated = Sin fecha
core.item.title = {name} ({owner})

# reminders
core.remind.in.one = Vence mañana
core.remind.in.other = Vence en {n} días
core.remind.today = Vence hoy
core.remind.expired_ago.one = Venció ayer
core.remind.expired_ago.other = Venció hace {n} días
core.remind.notice_in.one = El plazo para avisar es mañana
core.remind.notice_in.other = Quedan {n} días para dar el aviso
core.remind.notice_today = Hoy es el último día para dar el aviso
core.remind.notice_passed.one = El plazo de aviso pasó ayer
core.remind.notice_passed.other = El plazo de aviso pasó hace {n} días

# database
core.db.not_database = El archivo no es una base de datos SQLite.
core.db.error = Error de la base de datos: {detail}
core.db.io = No se pudo leer o escribir el archivo: {detail}
core.db.not_vigia = Es una base de datos, pero no de Vigía.
core.db.too_new = La creó una versión más nueva de Vigía (esquema {version}). Actualiza la aplicación.
core.db.corrupt = La base de datos está dañada: {detail}
core.db.not_found = No existe el elemento {id}.

# calendar
core.ics.calendar_name = Vigía: vencimientos
core.ics.expires = Vence: {title}
core.ics.notice = Último día para avisar: {title}

# checks
core.check.rdap_not_found = El registro no conoce ese dominio.
core.check.http = El servidor respondió {code}.
core.check.bad_domain = «{name}» no parece un nombre de dominio.
core.check.no_rdap = No hay servidor RDAP conocido para {name}.
core.check.bad_answer = La respuesta no se pudo leer.
core.check.bad_host = «{host}» no es un servidor válido.
core.check.dns = No se encontró {host}: {detail}
core.check.connect = No se pudo conectar a {host}: {detail}
core.check.tls = Falló el saludo TLS: {detail}
core.check.no_cert = El servidor no envió certificado.
core.check.not_checkable = Este tipo no se puede revisar automáticamente.
core.check.domain_ok = RDAP: vence el {date}
core.check.cert_ok = Certificado válido hasta {date} · {issuer}
core.check.no_expiry = La respuesta no trae fecha de vencimiento.
core.check.failed = Falló la revisión: {detail}

# ocr
core.ocr.models_missing = Faltan los modelos de OCR (unos 12 MB, se descargan una vez).
core.ocr.unavailable = Esta compilación no incluye OCR.
core.ocr.unsupported = Solo se leen imágenes (PNG, JPG, WebP) y PDF.
core.ocr.failed = No se pudo leer el documento: {detail}

# demo
core.demo.passport = Pasaporte
core.demo.soat = SOAT
core.demo.tecno = Revisión técnico-mecánica
core.demo.cert = Certificado del sitio web
core.demo.warranty = Garantía del portátil
core.demo.lease = Contrato de arriendo
core.demo.camara = Matrícula mercantil
core.demo.sub = Plan anual de la nube
core.demo.extinguisher = Recarga del extintor
