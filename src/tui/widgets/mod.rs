//! Painéis do dashboard (casca do statui: painel, seção, rodapé, tabelas).
/// Moldura de cartão: borda + título + coluna de fatos (PR-UX2).
pub mod card;
/// Os seis cartões do Resumo e o grid que os posiciona (PR-UX2).
pub mod cards;
/// Chips de estado das sessões: `[ok]`, `[falhou]`, `[exec]` (PR-UX2).
pub mod chips;
pub mod findings;
pub mod footer;
/// Barra de proporção (uso do índice) com glifo escolhido pelo modo.
pub mod gauge;
/// Primitiva de painel: borda dupla + título `| x |`.
pub mod panel;
/// Caixa de correio: lista de posts + leitura do rascunho entregue (M4).
pub mod posts;
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
