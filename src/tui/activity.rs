//! O feed de atividade (TUI-01): fases REAIS, texto literal do filho.
//!
//! Quem alimenta este feed é o MESMO canal que alimenta o log da
//! execução (`RunEvent` do [`crate::tui::action::runner`]) — os MESMOS
//! eventos que o `emit` do daemon imprime ao vivo desde o #151. O feed
//! não é cronograma nem relógio: linha que o filho não escreveu nunca
//! vira fase, e nada aqui anima sozinho.
//!
//! Dois fontes de fase existem hoje (os dois produtores reais):
//! - o SPAWN (o [`Action`] confirmado): `scan` → pesquisando,
//!   `triage` → pensando, `pr draft` → trabalhando;
//! - as linhas do filho: o resumo do `scan` (o MESMO formato que
//!   [`crate::scan::findings_count_in_summary`] lê da trilha) e a linha
//!   de tick do loop (`tick N: ingest: … · digest: … · publish: …` — os
//!   eventos que o `TickReport::line` produz). Skills/ferramentas
//!   (`chamando skill…`) chegam com o RESEARCH-01 — o parser é função
//!   pura, testada contra as linhas reais de hoje.
//!
//! Toda entrada passa por [`crate::redact::redact`]: o feed é tela, não
//! log — e segredo em tela é igual a segredo em log.

use crate::tui::action::{Action, RunEvent};
use crate::tui::duck::DuckState;

/// Teto de entradas do feed (o mais novo é o que importa; o resto não
/// vira scroll infinito no overlay).
pub const MAX_ENTRIES: usize = 12;

/// Uma fase observada: estado do pato + o texto REAL que a marcou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub state: DuckState,
    pub text: String,
}

/// O feed: estado atual do pato, fase corrente e o histórico curto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    /// Estado do mascote — o que o desenho mostra agora.
    pub state: DuckState,
    /// Fase corrente (o texto depois de `fase:` no overlay).
    pub current: Option<String>,
    /// Histórico curto (mais velho → mais novo), limitado a [`MAX_ENTRIES`].
    pub entries: Vec<Entry>,
}

impl Activity {
    /// Feed vazio: nenhum filho, pato em repouso.
    #[must_use]
    pub fn idle() -> Self {
        Self::default()
    }

    /// Estado inicial para o filho que acabou de ser confirmado. O spawn
    /// é o primeiro evento REAL: o comando existe e é dele que a fase
    /// nasce (nada de "preparando…" inventado antes dele).
    #[must_use]
    pub fn for_action(action: Action) -> Self {
        let mut feed = Self::idle();
        let (state, text) = started_phase(action);
        feed.record(state, text);
        feed
    }

    /// Registra uma fase: atualiza o estado e empilha a entrada. O texto
    /// passa por [`crate::redact::redact`] ANTES de virar tela (a linha do
    /// filho pode carregar qualquer coisa).
    fn record(&mut self, state: DuckState, text: String) {
        let text = crate::redact::redact(&text);
        self.state = state;
        self.current = Some(text.clone());
        self.entries.push(Entry { state, text });
        // Teto: o início é que sai (o último acontecimento fica).
        while self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }

    /// A fase corrente, para a linha do chip de execução.
    pub fn phase(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Consome o MESMO evento que o `ExecState::apply` consome.
    pub fn on_event(&mut self, event: &RunEvent) {
        match event {
            // A fase do spawn já foi registrada por [`Activity::for_action`]
            // (o app a grava no momento do `ConfirmExecute`).
            RunEvent::Started { .. } => {}
            // Nota é texto da PRÓPRIA casca (cancelamento, teto) — não é
            // fase do trabalho.
            RunEvent::Note(_) => {}
            RunEvent::Line { text, .. } => on_line(self, text),
            RunEvent::Exited { code, .. } => match code {
                Some(0) => {
                    // Concluído FECHA o comando, não o alerta: um scan que
                    // acabou de reportar critical continua pedindo atenção
                    // (visto no hardware do dono — o "concluído (exit 0)"
                    // apagava o alerta do resumo com 2 critical).
                    let state = if self.state == DuckState::Alerta {
                        DuckState::Alerta
                    } else {
                        DuckState::Idle
                    };
                    self.record(state, "concluído (exit 0)".to_string());
                }
                Some(code) => {
                    self.record(DuckState::Alerta, format!("falhou (exit {code})"));
                }
                None => self.record(
                    DuckState::Alerta,
                    "encerrado sem código de saída".to_string(),
                ),
            },
        }
    }
}

/// A fase que o SPAWN de cada ação abre. Os três filhos da allowlist
/// (ADR-0033) e o que cada um realmente faz:
/// - `scan` pergunta às ferramentas o estado da stack — pesquisando;
/// - `triage` fala com o modelo local sobre UM work item — pensando;
/// - `pr draft` monta rascunho do store — trabalhando.
fn started_phase(action: Action) -> (DuckState, String) {
    match action {
        Action::Scan => (DuckState::Pesquisando, "pesquisando stack…".to_string()),
        Action::Triage { work_item } => (
            DuckState::Pensando,
            format!("pensando no work item {work_item}…"),
        ),
        Action::PrDraft { work_item } => (
            DuckState::Trabalhando,
            format!("montando o rascunho do PR do item {work_item}…"),
        ),
    }
}

/// Uma linha REAL do filho muda a fase só quando casa com um produtor
/// conhecido. Linha desconhecida não vira fase (o log dela fica no
/// overlay — o feed é fases, não transcript).
fn on_line(feed: &mut Activity, text: &str) {
    // Resumo do `scan` (a primeira linha da saída): `… | achados: 7
    // (critical 1, high 6) | arquivo: …`. critical > 0 É o alerta real —
    // o pato aponta o que pede atenção, com o número que veio.
    if let Some(total) = crate::scan::findings_count_in_summary(text) {
        let critical = critical_in_summary(text).unwrap_or(0);
        let state = if critical > 0 {
            DuckState::Alerta
        } else if total > 0 {
            DuckState::Trabalhando
        } else {
            feed.record(DuckState::Idle, "nenhum achado no scan".to_string());
            return;
        };
        feed.record(state, text.trim().to_string());
        return;
    }
    // Linha de tick do loop (`tick 3: ingest: … · digest: … · publish:
    // …`) — os MESMOS eventos que o emit do daemon imprime ao vivo. O
    // texto vai literal: evidência, não parafraseia.
    if text.trim_start().starts_with("tick ") && text.contains("ingest:") {
        feed.record(DuckState::Trabalhando, text.trim().to_string());
    }
}

/// `critical N` do resumo do scan (o parens da contagem). `None` quando
/// a linha não é o resumo — o caller trata `None` como 0.
fn critical_in_summary(line: &str) -> Option<usize> {
    let start = line.find("critical ")?;
    let after = &line[start + "critical ".len()..];
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::action::Stream;

    fn line(text: &str) -> RunEvent {
        RunEvent::Line {
            stream: Stream::Out,
            text: text.to_string(),
        }
    }

    #[test]
    fn the_spawn_opens_the_real_phase_of_each_action() {
        let scan = Activity::for_action(Action::Scan);
        assert_eq!(scan.state, DuckState::Pesquisando);
        assert_eq!(scan.phase(), Some("pesquisando stack…"));

        let triage = Activity::for_action(Action::Triage { work_item: 4821 });
        assert_eq!(triage.state, DuckState::Pensando);
        assert_eq!(
            triage.phase(),
            Some("pensando no work item 4821…"),
            "o id é o alvo real, não decoração"
        );

        let draft = Activity::for_action(Action::PrDraft { work_item: 900 });
        assert_eq!(draft.state, DuckState::Trabalhando);
        assert!(draft.phase().unwrap_or_default().contains("rascunho"));
    }

    /// O resumo REAL do `kira scan` (formato da trilha, testado no scan):
    /// critical > 0 vira alerta; a linha vai literal.
    #[test]
    fn scan_summary_line_is_taken_literally() {
        let mut feed = Activity::for_action(Action::Scan);
        feed.on_event(&line(
            "kira scan em 'C:\\proj' — stacks: dotnet | achados: 7 (critical 1, high 6) | arquivo: C:\\proj\\.kira\\findings.json",
        ));
        assert_eq!(feed.state, DuckState::Alerta, "critical pede atenção");
        let last = feed.entries.last().unwrap_or_else(|| panic!("entrada"));
        assert!(
            last.text.contains("achados: 7 (critical 1, high 6)"),
            "{last:?}"
        );
        assert!(last.text.contains("kira scan em"), "evidência literal");
    }

    #[test]
    fn scan_summary_without_critical_stays_working() {
        let mut feed = Activity::for_action(Action::Scan);
        feed.on_event(&line(
            "kira scan em '/' — stacks: npm | achados: 3 (high 3) | arquivo: /f",
        ));
        assert_eq!(feed.state, DuckState::Trabalhando);
    }

    #[test]
    fn scan_summary_without_findings_goes_idle() {
        let mut feed = Activity::for_action(Action::Scan);
        feed.on_event(&line(
            "kira scan em '/' — stacks: npm | achados: 0 (nenhum) | arquivo: /f",
        ));
        assert_eq!(feed.state, DuckState::Idle);
        assert!(
            feed.phase().unwrap_or_default().contains("nenhum achado"),
            "{:?}",
            feed.current
        );
    }

    /// A linha de tick do loop entra literal (os eventos do emit do daemon).
    #[test]
    fn tick_line_is_taken_literally() {
        let mut feed = Activity::for_action(Action::Scan);
        feed.on_event(&line(
            "tick 3: ingest: facts iguais · digest: 1 novo(s), 0 cache, 0 falha(s) · publish: 1 publicado(s), 0 igual(is)",
        ));
        assert_eq!(feed.state, DuckState::Trabalhando);
        let last = feed.entries.last().unwrap_or_else(|| panic!("entrada"));
        assert!(last.text.starts_with("tick 3: ingest:"), "{last:?}");
    }

    /// Linha que não casa com produtor conhecido não vira fase.
    #[test]
    fn unknown_lines_do_not_become_phases() {
        let mut feed = Activity::for_action(Action::Scan);
        let before = feed.entries.len();
        feed.on_event(&line("achados por severidade (contagem por ferramenta):"));
        feed.on_event(&line("stacks: dotnet, npm"));
        feed.on_event(&line("linha aleatória do filho"));
        assert_eq!(feed.entries.len(), before);
        assert_eq!(feed.state, DuckState::Pesquisando, "estado intacto");
    }

    #[test]
    fn the_exit_closes_the_phase_honestly() {
        let mut ok = Activity::for_action(Action::Scan);
        ok.on_event(&RunEvent::Exited {
            code: Some(0),
            duration_ms: 1_200,
        });
        assert_eq!(ok.state, DuckState::Idle);
        assert!(ok.phase().unwrap_or_default().contains("concluído"));

        let mut bad = Activity::for_action(Action::Scan);
        bad.on_event(&RunEvent::Exited {
            code: Some(7),
            duration_ms: 1_200,
        });
        assert_eq!(bad.state, DuckState::Alerta);
        assert!(bad.phase().unwrap_or_default().contains("exit 7"));

        let mut killed = Activity::for_action(Action::Scan);
        killed.on_event(&RunEvent::Exited {
            code: None,
            duration_ms: 1_200,
        });
        assert_eq!(killed.state, DuckState::Alerta);
    }

    /// O "concluído" fecha o COMANDO, não o alerta: critical que o filho
    /// acabou de reportar continua pedindo atenção (hardware do dono).
    #[test]
    fn a_clean_exit_does_not_erase_a_critical_alert() {
        let mut feed = Activity::for_action(Action::Scan);
        feed.on_event(&line(
            "kira scan em 'C:\\proj' — stacks: pnpm | achados: 38 (critical 2, high 13) | arquivo: C:\\proj\\.kira\\findings.json",
        ));
        assert_eq!(feed.state, DuckState::Alerta);
        feed.on_event(&RunEvent::Exited {
            code: Some(0),
            duration_ms: 2_000,
        });
        assert_eq!(feed.state, DuckState::Alerta, "o critical continua lá");
        assert!(
            feed.phase().unwrap_or_default().contains("concluído"),
            "o fechamento segue na trilha do feed: {:?}",
            feed.current
        );
        // …mas um scan LIMPO (nenhum achado) volta ao repouso no exit 0.
        let mut clean = Activity::for_action(Action::Scan);
        clean.on_event(&line(
            "kira scan em '/' — stacks: npm | achados: 0 (nenhum) | arquivo: /f",
        ));
        clean.on_event(&RunEvent::Exited {
            code: Some(0),
            duration_ms: 500,
        });
        assert_eq!(clean.state, DuckState::Idle);
    }

    /// Segredo na linha do filho nunca chega à tela do feed.
    #[test]
    fn feed_text_is_redacted() {
        let mut feed = Activity::for_action(Action::Scan);
        feed.on_event(&line(
            "kira scan em '/' | achados: 1 (critical 1) | pat = Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT",
        ));
        let last = feed.entries.last().unwrap_or_else(|| panic!("entrada"));
        assert!(
            !last.text.contains("Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT"),
            "{last:?}"
        );
        assert!(last.text.contains("[REDACTED]"), "{last:?}");
    }

    #[test]
    fn the_feed_keeps_only_the_last_entries() {
        let mut feed = Activity::for_action(Action::Scan);
        for index in 0..(MAX_ENTRIES + 5) {
            feed.on_event(&line(&format!(
                "tick {index}: ingest: x · digest: 0 novo(s), 0 cache, 0 falha(s) · publish: 0 publicado(s), 0 igual(is)"
            )));
        }
        assert_eq!(feed.entries.len(), MAX_ENTRIES);
        assert!(
            feed.entries[0].text.contains("tick 5:"),
            "o início saiu: {:?}",
            feed.entries[0]
        );
    }

    #[test]
    fn started_events_and_notes_are_ignored() {
        let mut feed = Activity::for_action(Action::Scan);
        let before = feed.entries.len();
        feed.on_event(&RunEvent::Started {
            command: "kira scan".to_string(),
        });
        feed.on_event(&RunEvent::Note("nota da casca".to_string()));
        assert_eq!(feed.entries.len(), before);
    }

    #[test]
    fn critical_parser_reads_the_real_parens() {
        assert_eq!(
            critical_in_summary("… | achados: 7 (critical 1, high 6) | …"),
            Some(1)
        );
        assert_eq!(
            critical_in_summary("… | achados: 3 (high 3) | …"),
            None,
            "sem critical, None"
        );
        assert_eq!(critical_in_summary("nada aqui"), None);
    }
}
