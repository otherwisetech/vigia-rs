# O motor: tipos, estados, lembretes, banco de dados, verificações.

# categories
core.cat.identity = Identidade
core.cat.vehicle = Veículo
core.cat.insurance = Seguros
core.cat.warranty = Garantias
core.cat.digital = Digital
core.cat.contract = Contratos
core.cat.company = Empresa
core.cat.membership = Associações
core.cat.health = Saúde
core.cat.home = Casa
core.cat.other = Outros

# types
core.kind.passport = Passaporte
core.kind.national_id = Documento de identidade
core.kind.driver_license = Carteira de motorista
core.kind.visa = Visto
core.kind.residence_permit = Autorização de residência
core.kind.soat = SOAT (seguro obrigatório)
core.kind.tecnomecanica = Inspeção veicular
core.kind.vehicle_registration = Licenciamento do veículo
core.kind.vehicle_insurance = Seguro do veículo
core.kind.insurance = Apólice de seguro
core.kind.warranty = Garantia
core.kind.domain = Domínio
core.kind.certificate = Certificado SSL/TLS
core.kind.subscription = Assinatura / plano anual
core.kind.contract = Contrato
core.kind.wyoming_report = Relatório anual de Wyoming
core.kind.camara_comercio = Renovação da Câmara de Comércio
core.kind.rut = Atualização do RUT
core.kind.membership = Associação
core.kind.vaccine = Vacina
core.kind.checkup = Check-up médico
core.kind.gas_inspection = Inspeção de gás
core.kind.extinguisher = Extintor
core.kind.custom = Outro

# recurrence
core.rec.none = Não se repete
core.rec.days.one = Todo dia
core.rec.days.other = A cada {n} dias
core.rec.weeks.one = Toda semana
core.rec.weeks.other = A cada {n} semanas
core.rec.months.one = Todo mês
core.rec.months.other = A cada {n} meses
core.rec.years.one = Todo ano
core.rec.years.other = A cada {n} anos
core.rec.wyoming = Todo ano, em 1º de {month} (mês de constituição)
core.rec.yearly = Todo ano, em {day} de {month}

# months
core.month.1 = janeiro
core.month.2 = fevereiro
core.month.3 = março
core.month.4 = abril
core.month.5 = maio
core.month.6 = junho
core.month.7 = julho
core.month.8 = agosto
core.month.9 = setembro
core.month.10 = outubro
core.month.11 = novembro
core.month.12 = dezembro

# status
core.status.expired = Vencido
core.status.soon = ≤ 30 dias
core.status.upcoming = ≤ 90 dias
core.status.ok = Em dia
core.status.undated = Sem data
core.item.title = {name} ({owner})

# reminders
core.remind.in.one = Vence amanhã
core.remind.in.other = Vence em {n} dias
core.remind.today = Vence hoje
core.remind.expired_ago.one = Venceu ontem
core.remind.expired_ago.other = Venceu há {n} dias
core.remind.notice_in.one = O prazo para avisar é amanhã
core.remind.notice_in.other = Faltam {n} dias para dar o aviso
core.remind.notice_today = Hoje é o último dia para dar o aviso
core.remind.notice_passed.one = O prazo de aviso passou ontem
core.remind.notice_passed.other = O prazo de aviso passou há {n} dias

# database
core.db.not_database = O arquivo não é um banco de dados SQLite.
core.db.error = Erro do banco de dados: {detail}
core.db.io = Não foi possível ler ou gravar o arquivo: {detail}
core.db.not_vigia = É um banco de dados, mas não do Vigía.
core.db.too_new = Foi criado por uma versão mais nova do Vigía (esquema {version}). Atualize o aplicativo.
core.db.corrupt = O banco de dados está danificado: {detail}
core.db.not_found = Não existe o item {id}.

# calendar
core.ics.calendar_name = Vigía: vencimentos
core.ics.expires = Vence: {title}
core.ics.notice = Último dia para avisar: {title}

# checks
core.check.rdap_not_found = O registro não conhece esse domínio.
core.check.http = O servidor respondeu {code}.
core.check.bad_domain = “{name}” não parece um nome de domínio.
core.check.no_rdap = Não há servidor RDAP conhecido para {name}.
core.check.bad_answer = Não foi possível ler a resposta.
core.check.bad_host = “{host}” não é um servidor válido.
core.check.dns = Não foi possível encontrar {host}: {detail}
core.check.connect = Não foi possível conectar a {host}: {detail}
core.check.tls = Falha no handshake TLS: {detail}
core.check.no_cert = O servidor não enviou certificado.
core.check.not_checkable = Este tipo não pode ser verificado automaticamente.
core.check.domain_ok = RDAP: vence em {date}
core.check.cert_ok = Certificado válido até {date} · {issuer}
core.check.no_expiry = A resposta não tem data de vencimento.
core.check.failed = A verificação falhou: {detail}

# ocr
core.ocr.models_missing = Os modelos de OCR ainda não foram baixados (cerca de 12 MB, uma vez).
core.ocr.unavailable = Esta compilação não inclui OCR.
core.ocr.unsupported = Só imagens (PNG, JPG, WebP) e PDF podem ser lidos.
core.ocr.failed = Não foi possível ler o documento: {detail}

# demo
core.demo.passport = Passaporte
core.demo.soat = SOAT
core.demo.tecno = Inspeção veicular
core.demo.cert = Certificado do site
core.demo.warranty = Garantia do notebook
core.demo.lease = Contrato de aluguel
core.demo.camara = Registro mercantil
core.demo.sub = Plano anual da nuvem
core.demo.extinguisher = Recarga do extintor
core.entity.person = Pessoa
core.entity.vehicle = Veículo
core.entity.company = Empresa
core.entity.property = Imóvel
core.entity.pet = Animal de estimação
core.entity.digital = Digital
core.entity.other = Outro
core.entity.ident.person = Número do documento
core.entity.ident.vehicle = Placa
core.entity.ident.company = CNPJ / identificação fiscal
core.entity.ident.property = Endereço
core.entity.ident.digital = Site ou conta
core.entity.detail.vehicle = Marca e modelo
core.entity.detail.pet = Espécie e raça
core.entity.detail.digital = Provedor
core.entity.detail.other = Descrição
core.role.owner = dono
core.role.member = sócio
core.role.manager = administrador
core.role.family = família
core.role.uses = usa
core.role.other = relacionado
core.db.self_relation = Uma entidade não pode se relacionar consigo mesma.
core.ics.belongs_to = {kind}: {owner}
core.country.co = Colômbia
core.country.us = Estados Unidos (federal)
core.country.us_wy = Wyoming (estadual)
core.country.es = Espanha
core.country.mx = México
core.country.br = Brasil
core.country.ca = Canadá (federal)
core.biz.weekends_only = só fins de semana
core.biz.previous = Dia útil anterior ({holidays})
core.biz.next = Próximo dia útil ({holidays})
core.cal.not_ics = Isso não é um arquivo iCalendar (.ics).

# shared registry of the Otherwise apps
core.sync.error = Registro compartilhado: {detail}
core.cal.reminder = Lembrete: {title}
core.tpl.soat = SOAT (seguro obrigatório)
core.tpl.tecnomecanica = Inspeção veicular (técnico-mecânica)
core.tpl.vehicle_insurance = Seguro total do veículo
core.tpl.vehicle_tax = Imposto do veículo
core.tpl.oil_change = Troca de óleo
core.tpl.brakes = Revisão dos freios
core.tpl.vehicle_registration = Licenciamento / registro do veículo
core.tpl.pet_rabies = Vacina antirrábica
core.tpl.pet_multiple = Vacina múltipla (V8/V10)
core.tpl.pet_deworming = Vermifugação
core.tpl.pet_fleas = Antipulgas
core.tpl.pet_checkup = Consulta veterinária
core.tpl.pet_grooming = Banho e tosa
core.tpl.birthday = Aniversário
core.tpl.national_id = Documento de identidade
core.tpl.passport = Passaporte
core.tpl.visa = Visto
core.tpl.driver_license = Carteira de motorista
core.tpl.medical = Consulta médica
core.tpl.dentist = Dentista
core.tpl.lab_tests = Exames médicos
core.tpl.health_plan = Plano de saúde
core.tpl.camara_comercio = Renovação do registro comercial (Junta Comercial)
core.tpl.rut = Atualização do cadastro fiscal (RUT)
core.tpl.income_tax = Declaração de imposto de renda
core.tpl.vat = Declaração de impostos sobre vendas
core.tpl.withholding = Retenção na fonte
core.tpl.annual_report = Relatório anual da LLC
core.tpl.policy = Apólice de seguro
core.tpl.contract = Contrato
core.tpl.domain = Domínio
core.tpl.certificate = Certificado SSL/TLS
core.tpl.software_license = Licença de software
core.tpl.property_tax = IPTU (imposto predial)
core.tpl.hoa_fee = Condomínio
core.tpl.utilities = Contas de consumo
core.tpl.home_insurance = Seguro residencial
core.tpl.gas_inspection = Inspeção de gás
core.tpl.water_heater = Manutenção do aquecedor
core.tpl.air_conditioning = Manutenção do ar-condicionado
core.tpl.extinguisher = Recarga do extintor
core.tpl.appliance_warranty = Garantia de eletrodoméstico
core.tpl.subscription = Assinatura
core.tplnote.accountant = Confirme as datas com seu contador: dependem do seu cadastro e do calendário do ano.
core.tplnote.check = Com o host preenchido, o Vigía pode verificar a data automaticamente (RDAP / TLS).
core.tplnote.real_date = Use a data que consta no documento.
core.tplnote.runt = Consulte no registro oficial quando vence a sua.
core.tplnote.local_tax = Seu município ou estado define as datas e descontos todo ano.
core.tplnote.km = Ou por quilometragem: anote o km da próxima.
core.tplnote.vet = Confirme o calendário com seu veterinário.
core.tplnote.private = O número fica só neste computador; aparecem só os últimos 4.
