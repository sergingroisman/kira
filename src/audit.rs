//! `kira audit --weekly` (F2, PR19): o exame de rotina do ciclo.
//!
//! Uma leitura do store que responde, em uma tela, o que o dev precisa saber
//! para decidir se o KIRA está trabalhando: o que rodou, o que a fila de posts
//! está esperando, o que aguarda decisão humana, e quanto de orçamento foi
//! gasto. Nada de LLM, nada de rede, nada de escrita — é diagnóstico.
//!
//! Decisões que dão forma ao relatório:
//!
//! 1. **Janela explícita, nunca "semana passada" implícita.** O relatório diz o
//!    intervalo e o fuso em que foi calculado; um número sem janela é um número
//!    que ninguém consegue auditar.
//! 2. **Falhas aparecem primeiro.** Um relatório que esconde o que quebrou é
//!    vaidade: as sessões com `ok = 0` ganham linha própria com o comando.
//! 3. **Aprovações pendentes são ditas em voz alta** — é o que explica um ciclo
//!    que "não escreveu nada" (a guarda do PR18 barrou, não o KIRA).
//! 4. **Sem blog** (fora do escopo do PR19): o relatório é local e não publica.

use crate::error::Error;
use crate::store::{Store, now_rfc3339_utc, rfc3339_utc_from_unix_seconds};

/// Janela padrão do `--weekly`: 7 dias, como o nome promete.
pub const DEFAULT_WINDOW_DAYS: i64 = 7;

/// Uma linha de sessão agregada por comando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    pub command: String,
    pub total: usize,
    pub ok: usize,
}

/// O retrato do período. Tudo o que o `render` usa vem daqui — o relatório é
/// uma função pura do struct (testável sem banco).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyAudit {
    pub window_days: i64,
    /// Início da janela (RFC-3339 UTC) e a mesma data em horário local.
    pub since: String,
    pub since_local: String,
    pub commands: Vec<CommandLine>,
    pub posts: Vec<(String, usize)>,
    pub approvals_pending: usize,
    pub approvals_approved: usize,
    pub approvals_rejected: usize,
    pub llm_calls: usize,
    pub tokens_est: u64,
    pub pack_chars: u64,
    pub budget_chars: u64,
}

impl WeeklyAudit {
    /// Total de sessões na janela.
    #[must_use]
    pub fn sessions_total(&self) -> usize {
        self.commands.iter().map(|line| line.total).sum()
    }

    /// Sessões que falharam.
    #[must_use]
    pub fn sessions_failed(&self) -> usize {
        self.commands
            .iter()
            .map(|line| line.total.saturating_sub(line.ok))
            .sum()
    }

    /// Percentual do teto de contexto usado pelo packer (0..=100). Sem pack
    /// medido, `None` — "0%" mentiria dizendo que houve economia.
    #[must_use]
    pub fn pack_usage_pct(&self) -> Option<u64> {
        if self.budget_chars == 0 {
            return None;
        }
        Some((self.pack_chars.saturating_mul(100) / self.budget_chars).min(100))
    }
}

/// Coleta a janela a partir do store. `now_seconds` é injetável (teste).
pub fn collect(store: &Store, window_days: i64, now_seconds: i64) -> Result<WeeklyAudit, Error> {
    let window_days = window_days.max(1);
    let since_seconds = now_seconds - window_days * 86_400;
    let since = rfc3339_utc_from_unix_seconds(since_seconds);
    let since_local = store
        .local_display(&since)?
        .unwrap_or_else(|| since.clone());

    let commands = store
        .sessions_since(&since)?
        .into_iter()
        .map(|(command, total, ok, _duration)| CommandLine { command, total, ok })
        .collect();

    // Aprovações não têm consulta por janela: o volume é pequeno (uma decisão
    // humana por escrita) e filtrar em memória mantém o store simples.
    let mut approvals_pending = 0;
    let mut approvals_approved = 0;
    let mut approvals_rejected = 0;
    for row in store.list_approvals(false)? {
        // Só interessa o que foi criado OU decidido dentro da janela.
        let in_window = [Some(row.created_at.as_str()), row.decided_at.as_deref()]
            .into_iter()
            .flatten()
            .any(|stamp| stamp >= since.as_str());
        if !in_window {
            continue;
        }
        match row.state.as_str() {
            "approved" => approvals_approved += 1,
            "rejected" => approvals_rejected += 1,
            _ => approvals_pending += 1,
        }
    }

    let (tokens_est, _economy_calls, pack_chars, budget_chars) = store.economy_since(&since)?;

    let since_for_queries = since.clone();
    Ok(WeeklyAudit {
        window_days,
        since: since_for_queries,
        since_local,
        commands,
        posts: store.post_state_counts()?,
        approvals_pending,
        approvals_approved,
        approvals_rejected,
        llm_calls: store.llm_calls_since(&since)?,
        tokens_est,
        pack_chars,
        budget_chars,
    })
}

/// Coleta usando o relógio do sistema (o caminho do CLI).
pub fn collect_now(store: &Store, window_days: i64) -> Result<WeeklyAudit, Error> {
    let now =
        crate::scheduler::parse_unix_seconds(&now_rfc3339_utc()).ok_or(Error::ClockUnreadable)?;
    collect(store, window_days, now)
}

/// Texto do relatório. Ordem: janela → falhas → trabalho → fila → aprovações →
/// orçamento. Falha vem antes de volume porque é o que muda decisão.
#[must_use]
pub fn render(audit: &WeeklyAudit) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "auditoria do KIRA — últimos {} dia(s) (desde {})\n",
        audit.window_days, audit.since_local
    ));

    let total = audit.sessions_total();
    let failed = audit.sessions_failed();
    if total == 0 {
        out.push_str("  sessões: nenhuma no período — o ciclo não rodou\n");
    } else {
        out.push_str(&format!(
            "  sessões: {total} ({} ok, {failed} falha(s))\n",
            total - failed
        ));
        for line in audit.commands.iter().take(5) {
            let falhas = line.total - line.ok;
            let sufixo = if falhas > 0 {
                format!(" · {falhas} falha(s)")
            } else {
                String::new()
            };
            out.push_str(&format!("    {}: {}{sufixo}\n", line.command, line.total));
        }
    }

    if audit.posts.is_empty() {
        out.push_str("  posts: fila vazia\n");
    } else {
        let pares: Vec<String> = audit
            .posts
            .iter()
            .map(|(state, count)| format!("{count} {state}"))
            .collect();
        out.push_str(&format!("  posts: {}\n", pares.join(" · ")));
    }

    let aprovacoes = format!(
        "{} pendente(s) · {} aprovada(s) · {} recusada(s)",
        audit.approvals_pending, audit.approvals_approved, audit.approvals_rejected
    );
    out.push_str(&format!("  aprovações: {aprovacoes}\n"));
    if audit.approvals_pending > 0 {
        out.push_str("    → nada escreve no mundo até `kira approve <id>`\n");
    }

    let pct = audit
        .pack_usage_pct()
        .map(|pct| format!(" · pack a {pct}% do teto"))
        .unwrap_or_default();
    out.push_str(&format!(
        "  LLM: {} chamada(s) · {} token(s) estimados{pct}\n",
        audit.llm_calls, audit.tokens_est
    ));

    if failed > 0 {
        out.push_str(&format!(
            "  próximo passo: {failed} sessão(ões) falharam — veja `kira history`\n"
        ));
    } else if total == 0 {
        out.push_str("  próximo passo: `kira loop --once` para um ciclo agora\n");
    } else {
        out.push_str("  próximo passo: nada pendente\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> WeeklyAudit {
        WeeklyAudit {
            window_days: 7,
            since: "2026-11-09T00:00:00Z".to_string(),
            since_local: "2026-11-08 21:00:00".to_string(),
            commands: vec![
                CommandLine {
                    command: "triage".to_string(),
                    total: 3,
                    ok: 2,
                },
                CommandLine {
                    command: "scan".to_string(),
                    total: 1,
                    ok: 1,
                },
            ],
            posts: vec![("delivered".to_string(), 2), ("sealed".to_string(), 1)],
            approvals_pending: 1,
            approvals_approved: 0,
            approvals_rejected: 1,
            llm_calls: 4,
            tokens_est: 1_500,
            pack_chars: 400,
            budget_chars: 800,
        }
    }

    #[test]
    fn report_opens_with_the_window_and_names_the_failures_first() {
        let texto = render(&sample());
        assert!(
            texto.starts_with("auditoria do KIRA — últimos 7 dia(s)"),
            "{texto}"
        );
        assert!(texto.contains("desde 2026-11-08 21:00:00"), "{texto}");
        assert!(texto.contains("sessões: 4 (3 ok, 1 falha(s))"), "{texto}");
        // O comando que falhou carrega o sufixo — falha não some no agregado.
        assert!(texto.contains("triage: 3 · 1 falha(s)"), "{texto}");
        assert!(texto.contains("posts: 2 delivered · 1 sealed"), "{texto}");
        assert!(
            texto.contains("1 pendente(s) · 0 aprovada(s) · 1 recusada(s)"),
            "{texto}"
        );
        assert!(
            texto.contains("nada escreve no mundo até `kira approve <id>`"),
            "{texto}"
        );
        assert!(texto.contains("pack a 50% do teto"), "{texto}");
        assert!(texto.contains("veja `kira history`"), "{texto}");
    }

    #[test]
    fn empty_window_is_reported_as_such_and_suggests_a_tick() {
        let vazio = WeeklyAudit {
            commands: Vec::new(),
            posts: Vec::new(),
            approvals_pending: 0,
            approvals_approved: 0,
            approvals_rejected: 0,
            llm_calls: 0,
            tokens_est: 0,
            pack_chars: 0,
            budget_chars: 0,
            ..sample()
        };
        let texto = render(&vazio);
        assert!(texto.contains("sessões: nenhuma no período"), "{texto}");
        assert!(texto.contains("fila vazia"), "{texto}");
        assert!(texto.contains("kira loop --once"), "{texto}");
        // Sem pack medido não inventa percentual.
        assert!(!texto.contains("pack a"), "{texto}");
    }

    #[test]
    fn pack_usage_is_clamped_and_absent_without_budget() {
        assert_eq!(sample().pack_usage_pct(), Some(50));
        let sem_teto = WeeklyAudit {
            budget_chars: 0,
            ..sample()
        };
        assert_eq!(sem_teto.pack_usage_pct(), None);
    }
}
