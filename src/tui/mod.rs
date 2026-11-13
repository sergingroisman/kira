//! Casca TUI read-only do `kira dashboard` (W6-PR1, ADR-0014).
//!
//! O módulo é a "casca" prevista no ADR-0014, escrita do zero (nenhum trecho
//! do statui foi copiado — ver `third_party/statui/NOTICE.md`): `app` é o loop
//! de eventos (crossterm) + render (ratatui), `state` é o snapshot imutável
//! lido do disco no início e no `r` (nunca por frame), `keymap` traduz tecla →
//! ação, `theme` guarda a paleta com fallback mono e `widgets` os painéis.
//!
//! Contratos vividos aqui: **navegar nunca executa** (ADR-0033) — o
//! [`app::Outcome`] só ganha `ConfirmExecute` depois de `s`/`t`/`d` + folha de
//! alvo + popup, e quem spawna é `action::runner`, com `current_exe` e argv
//! allowlist (`action::Action::argv`), `stdin(null)` e saída sanitizada por
//! `action::log`. `src/tui` continua fora da allowlist do
//! `tests/llm_allowlist.rs` por construção: quem fala com modelo é o filho.
//! Honestidade de dado: ausente aparece como `—` com a causa, nunca inventado
//! (docs/kira-w6-dashboard.md, docs/kira-persona.md §7: denso, setas E vim).

/// Camada de ação (ADR-0033): allowlist de argv, confirmação e execução.
pub mod action;
pub mod app;
/// Chrome fixo: linha de status e abas de fonte (casca do statui).
pub mod chrome;
/// Glifos do dashboard: Unicode (padrão) e ASCII (`KIRA_GLYPHS=ascii`).
pub mod glyph;
/// Grid do Resumo (PR-UX2): colunas por largura e altura de cada cartão.
pub mod grid;
pub mod keymap;
/// Geometria pura da casca (faixas do chrome, 100% × 50–50 do inspetor).
pub mod layout;
/// Renderizador markdown mínimo do painel de wiki (W6-PR4).
pub mod markdown;
/// Chuva de fundo: decoração pura, desenha só célula em branco (ADR-0036).
pub mod rain;
/// Spinner de execução (PR-UX2): puro sobre o tick, só com filho rodando.
pub mod spinner;
pub mod state;
pub mod theme;
/// Viewport das listas: offset que mantém o cursor visível e rótulo `12/87`.
pub mod viewport;
pub mod widgets;
