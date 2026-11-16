//! Scheduler do ciclo (F2, PR16): cron interno, **sem daemon**.
//!
//! O dono declara cadências (`kira schedule add --action digest --every 2h`) e
//! o `kira loop --scheduled` roda SÓ as fases vencidas — a decisão "é hora?"
//! mora aqui, numa função pura de dois campos persistidos (último run +
//! cadência). Três regras dão a forma deste módulo:
//!
//! 1. **Próximo tick é derivado, nunca gravado.** Gravar `next_run_at` cria um
//!    segundo relógio que pode discordar de `last_run_at` + `every_seconds`
//!    depois de um clock skew ou de um update manual (`enabled`).
//! 2. **Nada devido = nada acontece.** Um ciclo agendado sem ação vencida
//!    devolve zero fases — é o que impede o agendamento de virar "roda tudo a
//!    cada tick" (o anti-PR-inócuo do PR15 continua valendo).
//! 3. **Piso de cadência.** 60s é o mínimo aceito: abaixo disso o agendamento
//!    vira laço quente na máquina do dono sem ganho de informação.
//!
//! Opt-in explícito: sem `--scheduled`, o `kira loop` ignora a tabela
//! `schedule` por completo — agendar não liga nada sozinho, e o `setup` avisa
//! que só roda com o KIRA vivo (sem serviço do Windows no MVP).

use crate::error::Error;
use crate::store::{ScheduleRow, Store};

/// Cadência mínima aceita (60s). Cadência menor é recusada na hora de gravar.
pub const MIN_EVERY_SECONDS: i64 = 60;

/// Uma fase do ciclo que pode ser agendada. O conjunto é fechado de propósito:
/// agendar é escolher QUANDO uma fase existente roda, não criar trabalho novo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleAction {
    Scan,
    Digest,
    Publish,
}

impl ScheduleAction {
    /// Todas as ações agendáveis, na ordem em que o ciclo as executa.
    pub const ALL: [ScheduleAction; 3] = [Self::Scan, Self::Digest, Self::Publish];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Digest => "digest",
            Self::Publish => "publish",
        }
    }

    /// Lê o nome da ação do CLI/banco. Erro acionável lista as válidas — a
    /// mensagem é a interface, não um detalhe de log.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "scan" => Ok(Self::Scan),
            "digest" => Ok(Self::Digest),
            "publish" => Ok(Self::Publish),
            _ => Err(Error::ScheduleActionInvalid {
                value: value.trim().to_string(),
            }),
        }
    }
}

/// Converte uma cadência legível (`30s`, `30m`, `2h`, `1d`, com espaço opcional
/// antes da unidade) em segundos. Sem unidade = segundos.
///
/// O piso é [`MIN_EVERY_SECONDS`]: recusar cedo é melhor que descobrir depois
/// que o ciclo foi agendado a cada 5 segundos.
pub fn parse_every(value: &str) -> Result<i64, Error> {
    let trimmed = value.trim();
    let invalid = || Error::ScheduleEveryInvalid {
        value: trimmed.to_string(),
    };
    if trimmed.is_empty() {
        return Err(invalid());
    }
    let (number, unit) = trimmed.split_at(
        trimmed
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(trimmed.len()),
    );
    let amount: i64 = number.parse().map_err(|_| invalid())?;
    if amount <= 0 {
        return Err(invalid());
    }
    let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return Err(invalid()),
    };
    let seconds = amount.checked_mul(multiplier).ok_or_else(invalid)?;
    if seconds < MIN_EVERY_SECONDS {
        return Err(invalid());
    }
    Ok(seconds)
}

/// Cadência em segundos de volta para texto humano (`120` ⇒ `2m`), usada no
/// `kira schedule list` e nas notas do ciclo. Sempre a maior unidade exata.
#[must_use]
pub fn format_every(seconds: i64) -> String {
    for (unit, size) in [("d", 86_400_i64), ("h", 3_600), ("m", 60)] {
        if seconds >= size && seconds % size == 0 {
            return format!("{}{unit}", seconds / size);
        }
    }
    format!("{seconds}s")
}

/// Lê o carimbo que o store grava (`2026-11-16T13:18:02Z`) em segundos Unix.
///
/// Inverso de [`crate::store::rfc3339_utc_from_unix_seconds`], sem dependência
/// de crate de data: o formato é fixo e gerado por nós. Fuso diferente de `Z`
/// (ou carimbo malformado) devolve `None` — quem chama trata como "não sei",
/// e "não sei" nunca vira "vencido" por acidente.
#[must_use]
pub fn parse_unix_seconds(stamp: &str) -> Option<i64> {
    let bytes = stamp.as_bytes();
    if bytes.len() != 20 || !stamp.ends_with('Z') {
        return None;
    }
    let parts: Vec<&str> = stamp[..19].split(['-', 'T', ':']).collect();
    if parts.len() != 6 {
        return None;
    }
    let mut fields = [0_i64; 6];
    for (index, part) in parts.iter().enumerate() {
        fields[index] = part.parse().ok()?;
    }
    let [year, month, day, hour, minute, second] = fields;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Dias desde 1970-01-01 (algoritmo civil-from-days de Howard Hinnant).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// O agendamento está vencido em `now_seconds`?
///
/// Nunca rodou ⇒ vencido (o dono criou e espera que aconteça). Desligado ⇒
/// nunca vencido. Carimbo ilegível ⇒ vencido por segurança? **Não**: um
/// agendamento que não sabe quando rodou pode disparar em rajada; aqui a
/// escolha é conservadora (não dispara) e o `list` mostra o carimbo cru.
#[must_use]
pub fn is_due(row: &ScheduleRow, now_seconds: i64) -> bool {
    if !row.enabled {
        return false;
    }
    let Some(last) = row.last_run_at.as_deref() else {
        return true;
    };
    let Some(last_seconds) = parse_unix_seconds(last) else {
        return false;
    };
    now_seconds.saturating_sub(last_seconds) >= row.every_seconds
}

/// Texto do próximo tick para `kira schedule list` — derivado de
/// `last_run_at` + `every_seconds`, a mesma conta de [`is_due`]. Nunca lido de
/// um campo: se fosse gravado, um relógio corrigido deixaria o texto mentindo.
#[must_use]
pub fn next_run_text(row: &ScheduleRow, now_seconds: i64) -> String {
    if !row.enabled {
        return "desligado".to_string();
    }
    let Some(last) = row.last_run_at.as_deref() else {
        return "vencido (nunca rodou)".to_string();
    };
    let Some(last_seconds) = parse_unix_seconds(last) else {
        return "carimbo ilegível".to_string();
    };
    let elapsed = now_seconds.saturating_sub(last_seconds);
    if elapsed >= row.every_seconds {
        "vencido".to_string()
    } else {
        format!("em {}", format_every(row.every_seconds - elapsed))
    }
}

/// O conjunto de ações vencidas NESTE instante, com os ids para marcar o run.
///
/// Carregado uma vez por tick: o ciclo decide as três fases com a mesma
/// fotografia do relógio (um tick que atravessa a virada de minuto não pode
/// ver "due" no começo e "não due" no fim).
#[derive(Debug, Clone, Default)]
pub struct SchedulePlan {
    due: Vec<(ScheduleAction, i64)>,
    /// Ações que o dono agendou (ligadas ou não). Sem cadência declarada a
    /// fase roda no default do ciclo — agendar `publish` não pode desligar o
    /// `digest` por acidente; declarar é o que restringe.
    declared: Vec<ScheduleAction>,
    total: usize,
}

impl SchedulePlan {
    /// Lê a tabela `schedule` e calcula o que venceu em `now_seconds`.
    pub fn load(store: &Store, now_seconds: i64) -> Result<Self, Error> {
        let rows = store.list_schedules()?;
        Self::from_rows(&rows, now_seconds)
    }

    /// Versão pura (testável sem banco): mesma decisão, mesmos ids.
    pub fn from_rows(rows: &[ScheduleRow], now_seconds: i64) -> Result<Self, Error> {
        let mut plan = Self {
            due: Vec::new(),
            declared: Vec::new(),
            total: 0,
        };
        for row in rows {
            // Ação desconhecida no banco (linha escrita por versão futura/à mão)
            // não pode derrubar o ciclo: é ignorada, e o `list` a mostra crua.
            let Ok(action) = ScheduleAction::parse(&row.action) else {
                continue;
            };
            plan.total += 1;
            if !plan.declared.contains(&action) {
                plan.declared.push(action);
            }
            if is_due(row, now_seconds) {
                plan.due.push((action, row.id));
            }
        }
        Ok(plan)
    }

    /// Há algum agendamento declarado (ligado ou não)?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Número de agendamentos declarados.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    /// Esta fase deve rodar neste tick?
    ///
    /// Fase SEM cadência declarada roda (default do ciclo); fase declarada roda
    /// só quando venceu — e um agendamento desligado a impede de rodar, que é
    /// o opt-out explícito.
    #[must_use]
    pub fn is_due(&self, action: ScheduleAction) -> bool {
        if !self.declared.contains(&action) {
            return true;
        }
        self.due.iter().any(|(due_action, _)| *due_action == action)
    }

    /// Marca o run de `action` em `at` (RFC-3339 UTC) — chamado DEPOIS de a
    /// fase ter dado certo: fase que falhou não pode parecer "feita".
    pub fn mark_run(&self, store: &Store, action: ScheduleAction, at: &str) -> Result<(), Error> {
        for (due_action, id) in &self.due {
            if *due_action == action {
                store.mark_schedule_run(*id, at)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(action: &str, every: i64, last: Option<&str>, enabled: bool) -> ScheduleRow {
        ScheduleRow {
            id: 1,
            action: action.to_string(),
            every_seconds: every,
            enabled,
            last_run_at: last.map(ToString::to_string),
            created_at: "2026-11-16T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn parse_every_accepts_units_and_space() {
        assert_eq!(parse_every("60s").unwrap(), 60);
        assert_eq!(parse_every("30m").unwrap(), 1_800);
        assert_eq!(parse_every("2h").unwrap(), 7_200);
        assert_eq!(parse_every("1d").unwrap(), 86_400);
        assert_eq!(parse_every("30 m").unwrap(), 1_800);
        assert_eq!(parse_every("90").unwrap(), 90);
    }

    #[test]
    fn parse_every_rejects_below_floor_and_garbage() {
        for bad in ["", "0", "59s", "5", "abc", "30x", "-5m", "m"] {
            let err = parse_every(bad).unwrap_err().to_string();
            assert!(err.contains("cadência"), "{bad}: {err}");
            assert!(err.contains("30m"), "{bad}: {err}");
        }
    }

    #[test]
    fn format_every_uses_the_largest_exact_unit() {
        assert_eq!(format_every(60), "1m");
        assert_eq!(format_every(120), "2m");
        assert_eq!(format_every(3_600), "1h");
        assert_eq!(format_every(86_400), "1d");
        assert_eq!(format_every(90), "90s");
    }

    #[test]
    fn parse_unix_seconds_round_trips_the_store_format() {
        assert_eq!(parse_unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        // Inverso exato do formatador do store: ida e volta devolve o mesmo segundo.
        let stamp = crate::store::rfc3339_utc_from_unix_seconds(1_000_000_000);
        assert_eq!(parse_unix_seconds(&stamp), Some(1_000_000_000));
        // Formato estranho nunca vira número.
        assert_eq!(parse_unix_seconds("2026-11-16T13:18:02+00:00"), None);
        assert_eq!(parse_unix_seconds("ontem"), None);
    }

    #[test]
    fn due_needs_cadence_to_have_elapsed() {
        let never = row("digest", 1_800, None, true);
        assert!(is_due(&never, 0), "nunca rodou ⇒ vence");
        let just_ran = row("digest", 1_800, Some("2026-11-16T13:00:00Z"), true);
        let ran_at = parse_unix_seconds("2026-11-16T13:00:00Z").unwrap();
        assert!(!is_due(&just_ran, ran_at + 1_799));
        assert!(is_due(&just_ran, ran_at + 1_800));
        let disabled = row("digest", 1_800, None, false);
        assert!(!is_due(&disabled, 10_000_000));
    }

    #[test]
    fn plan_exposes_only_due_actions_and_ignores_unknown_rows() {
        let rows = [
            row("digest", 1_800, Some("2026-11-16T13:00:00Z"), true),
            ScheduleRow {
                id: 2,
                action: "inventada".to_string(),
                every_seconds: 60,
                enabled: true,
                last_run_at: None,
                created_at: "2026-11-16T00:00:00Z".to_string(),
            },
            ScheduleRow {
                id: 3,
                ..row("publish", 60, None, true)
            },
        ];
        let now = parse_unix_seconds("2026-11-16T13:10:00Z").unwrap();
        let plan = SchedulePlan::from_rows(&rows, now).unwrap();
        // `digest` ainda não venceu (10 min < 30 min); `publish` nunca rodou.
        assert!(!plan.is_due(ScheduleAction::Digest));
        assert!(plan.is_due(ScheduleAction::Publish));
        // `scan` não tem cadência declarada ⇒ roda no default do ciclo:
        // agendar uma fase não pode desligar as outras.
        assert!(plan.is_due(ScheduleAction::Scan));
        // A linha de ação desconhecida não conta nem derruba o plano.
        assert_eq!(plan.total(), 2);
        // Plano vazio é distinguível de "nada vencido".
        assert!(!plan.is_empty());
    }
}
