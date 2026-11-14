//! Ledger de economia de uma sessão (PR34, onda TE): o que o pack de contexto
//! consumiu vs o teto, quantas chamadas de LLM a sessão custou e qual rota a
//! produziram (hoje só `llm`; o PR35 acrescenta `cache-wiki`).
//!
//! É MEDIÇÃO, não enforcement: o teto de chamadas continua na tabela
//! `llm_calls` (v9). Aqui fica o retrato por sessão — o `rtk gain` da casa.
//! Determinístico não consome budget (ADR-0016): uma sessão sem LLM grava
//! `llm_calls = 0` e continua sendo dado do ledger, nunca chamada.

/// One row of `session_economy`: what one recorded session spent.
///
/// All numbers are ledger numbers (observability), never billing; the token
/// estimate is the same conservative `chars / 3` the packer already uses
/// (PT-BR, ADR-0028: 3 chars/token fica, `bytes/4` do RTK recusado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EconomyRow {
    /// Characters the context pack occupied in the prompt (0 = pack vazio).
    pub pack_chars: i64,
    /// The budget the pack was allowed to use (`KIRA_NUM_CTX` × 3).
    pub budget_chars: i64,
    /// `pack_chars / 3`, floored — same conversion as the packer.
    pub tokens_est: i64,
    /// LLM judgment calls this session made (deterministic work = 0).
    pub llm_calls: i64,
    /// Route that produced the answer: `llm` hoje, `cache-wiki` no PR35.
    pub route: String,
    /// Orçamento desperdiçado: pack abaixo do limiar do D9 (< 25% do teto)
    /// — suspeita de furo de coleta, visível no ledger, nunca contado como
    /// chamada.
    pub pack_idle: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O ledger é dado puro: campos na ordem do schema v13, sem inventar nada.
    #[test]
    fn row_carries_the_seven_ledger_fields() {
        let row = EconomyRow {
            pack_chars: 4096,
            budget_chars: 12_288,
            tokens_est: 4096 / 3,
            llm_calls: 1,
            route: "llm".to_string(),
            pack_idle: false,
        };
        assert_eq!(row.pack_chars, 4096);
        assert_eq!(row.budget_chars, 12_288);
        assert_eq!(row.tokens_est, 1365);
        assert_eq!(row.llm_calls, 1);
        assert_eq!(row.route, "llm");
        assert!(!row.pack_idle);
    }
}
