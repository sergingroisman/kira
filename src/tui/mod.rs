//! Casca TUI read-only do `kira dashboard` (W6-PR1, ADR-0014).
//!
//! O módulo é a "casca" prevista no ADR-0014, escrita do zero (nenhum trecho
//! do statui foi copiado — ver `third_party/statui/NOTICE.md`): `app` é o loop
//! de eventos (crossterm) + render (ratatui), `state` é o snapshot imutável
//! lido do disco no início e no `r` (nunca por frame), `keymap` traduz tecla →
//! ação, `theme` guarda a paleta com fallback mono e `widgets` os painéis.
//!
//! Contratos vividos aqui: read-only total (nenhuma escrita em disco, nada de
//! rede, nada de LLM — `src/tui` está fora da allowlist do
//! `tests/llm_allowlist.rs` por construção) e honestidade de dado: ausente
//! aparece como `—` com a causa, nunca inventado (docs/kira-w6-dashboard.md,
//! docs/kira-persona.md §7: denso, setas E vim, nenhuma tecla escreve).

pub mod app;
pub mod keymap;
pub mod state;
pub mod theme;
pub mod widgets;
