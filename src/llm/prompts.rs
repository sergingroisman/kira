//! Versioned prompts.
//!
//! The system prompts live here (and travel in the repository) instead of
//! being inlined in command code: there is no orchestration framework —
//! KIRA owns every prompt as a plain, reviewed constant.
//!
//! The triage prompt is read-only by contract: it never asks the model to
//! write, edit or fix code, and never mentions writes to Azure DevOps.

/// Fixed system prompt for `kira triage`.
///
/// Demands exactly the three fixed sections and forbids any code writing or
/// Azure DevOps mutation, so the command stays read-only end to end.
pub const TRIAGE_SYSTEM_PROMPT: &str = "Você é o assistente de triage do KIRA. Você analisa work items do Azure DevOps apenas para leitura e diagnóstico: nunca sugira escrever, editar ou corrigir código; nunca proponha ou execute alterações no Azure DevOps. Responda em português, exatamente com três seções, cada título em sua própria linha, com o conteúdo logo abaixo:\nCausa raiz provável:\nArquivos prováveis:\nConfiança:";

/// Section headers the triage prompt must demand, in order.
pub const TRIAGE_SECTIONS: [&str; 3] = ["Causa raiz provável", "Arquivos prováveis", "Confiança"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_is_read_only_and_demands_three_sections() {
        assert!(
            TRIAGE_SYSTEM_PROMPT.contains("nunca sugira escrever, editar ou corrigir código"),
            "prompt do sistema deve proibir escrita de código: {TRIAGE_SYSTEM_PROMPT}"
        );
        assert!(
            TRIAGE_SYSTEM_PROMPT.contains("nunca proponha ou execute alterações no Azure DevOps"),
            "prompt do sistema deve proibir escrita no Azure: {TRIAGE_SYSTEM_PROMPT}"
        );
        let mut cursor = TRIAGE_SYSTEM_PROMPT;
        for name in TRIAGE_SECTIONS {
            let header = format!("{name}:");
            let Some(position) = cursor.find(&header) else {
                panic!("prompt do sistema deve exigir a seção {header}: {TRIAGE_SYSTEM_PROMPT}");
            };
            // Headers must appear in the fixed order, each on its own line.
            let rest = &cursor[position + header.len()..];
            assert!(
                rest.starts_with('\n') || rest.is_empty(),
                "título {header} deve terminar em quebra de linha: {TRIAGE_SYSTEM_PROMPT}"
            );
            cursor = rest;
        }
    }

    #[test]
    fn system_prompt_mentions_only_reading_as_its_purpose() {
        assert!(
            TRIAGE_SYSTEM_PROMPT.contains("apenas para leitura e diagnóstico"),
            "prompt deve restringir o escopo à leitura: {TRIAGE_SYSTEM_PROMPT}"
        );
        assert!(
            TRIAGE_SYSTEM_PROMPT.contains("Responda em português"),
            "prompt deve fixar o idioma da resposta: {TRIAGE_SYSTEM_PROMPT}"
        );
    }
}
