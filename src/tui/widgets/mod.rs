//! Painéis do dashboard (casca do statui: painel, seção, rodapé, tabelas).
pub mod findings;
pub mod footer;
/// Barra de proporção (uso do índice) com glifo escolhido pelo modo.
pub mod gauge;
/// Primitiva de painel: borda dupla + título `| x |`.
pub mod panel;
/// Tela Resumo: faixa visual de risco + seções do inspetor do statui.
pub mod resumo;
/// Scrollbar vertical das listas (posição do viewport).
pub mod scrollbar;
/// Régua de seção com título (`═| Seção |════`).
pub mod section;
pub mod sessions;
/// Barras horizontais de severidade (`BarChart` nativo).
pub mod severity;
/// Sparkline de ritmo (barras de 7 níveis, técnica do statui).
pub mod sparkline;
/// Tendência de achados por scan (`Sparkline` nativo).
pub mod trend;
pub mod wiki;
