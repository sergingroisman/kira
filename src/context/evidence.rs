//! Estados da evidência (PR36, onda TE): o que o packer entrega ao modelo é
//! rastreado por arquivo — `CONFIRMADA` (a âncora existe no repositório),
//! `INFERIDA` (citada sem âncora verificável no disco) e `AUSENTE`
//! ("não identificado", o placeholder do prompt).
//!
//! Determinismo primeiro: a tag é calculada em Rust ANTES do prompt — o
//! packer marca cada trecho com [`state_for_anchor`] e o prompt v3 manda o
//! modelo PRESERVAR a tag recebida. A resposta nunca é parseada para
//! classificar; se uma verificação pós-resposta for necessária, esta MESMA
//! função compara a âncora contra o arquivo real (existe/não existe), sem
//! LLM, sem rede (CLI-first, ADR-0016).

use std::path::Path;

use crate::llm::prompts::TRIAGE_FILES_UNKNOWN;

/// Placeholder de seção vazia — o mesmo contrato do `triage`.
const SECTION_EMPTY: &str = "—";

/// Estado da evidência de um arquivo citado.
///
/// Serde em minúsculas para o `kira context --json` (mesma convenção de
/// `RepoSource`) e `#[serde(default)]` nos payloads antigos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceState {
    /// O arquivo da âncora existe no repositório AGORA (verificado no disco).
    Confirmed,
    /// A âncora não pôde ser verificada (arquivo saiu do disco, caminho
    /// inválido ou fora da raiz). Default construtivo: um trecho só vira
    /// CONFIRMADA com o arquivo real na frente — nunca de graça.
    #[default]
    Inferred,
    /// Não há arquivo apontado ("não identificado", seção vazia).
    Absent,
}

impl EvidenceState {
    /// Rótulo impresso no pack e exigido de volta pelo prompt v3.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "CONFIRMADA",
            Self::Inferred => "INFERIDA",
            Self::Absent => "AUSENTE",
        }
    }

    /// Todos os rótulos, na ordem do enum. Consumido pelo
    /// `eval::file_basenames` (a tag não é basename) e pelo teste do prompt.
    pub const LABELS: [&'static str; 3] = ["CONFIRMADA", "INFERIDA", "AUSENTE"];
}

/// Estado da evidência de uma âncora `caminho:linha` contra o repositório.
///
/// Verificação de EXISTÊNCIA de arquivo (o que o cron do item pede:
/// "existe/não existe, sem LLM") — a linha em si não é relida: o trecho do
/// pack já veio do disco ou do índice derivado, e a tag carimba exatamente
/// isso. Caminho vazio, `—` e `não identificado` são [`EvidenceState::Absent`].
#[must_use]
pub fn state_for_anchor(root: &Path, anchor: &str) -> EvidenceState {
    let trimmed = anchor.trim();
    if trimmed.is_empty() || trimmed == SECTION_EMPTY || trimmed == TRIAGE_FILES_UNKNOWN {
        return EvidenceState::Absent;
    }
    let Some(path) = strip_line_suffix(trimmed) else {
        // `:42` puro não é caminho citável.
        return EvidenceState::Inferred;
    };
    // O mesmo contrato do grep: caminho absoluto e traversal (`..`) saem —
    // e, como não são verificáveis contra a raiz, ficam INFERIDA.
    match super::repo::normalize_rel(&path) {
        Some(rel) if root.join(&rel).is_file() => EvidenceState::Confirmed,
        _ => EvidenceState::Inferred,
    }
}

/// `src/a.rs:42` → `src/a.rs`; âncora sem `:linha` volta como está; um
/// `:42` sem caminho devolve `None` (não há o que verificar).
fn strip_line_suffix(anchor: &str) -> Option<String> {
    match anchor.rsplit_once(':') {
        None => Some(anchor.to_string()),
        Some((base, suffix))
            if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            (!base.is_empty()).then(|| base.to_string())
        }
        // Outro uso de dois-pontos (drive Windows, `a:b`) não é âncora de
        // linha: o texto inteiro segue como caminho candidato.
        Some(_) => Some(anchor.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-evidence-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    /// Aceite do PR36: âncora cujo arquivo existe = CONFIRMADA.
    #[test]
    fn existing_anchor_is_confirmed() {
        let root = temp_root("confirmed");
        fs::create_dir_all(root.join("src/auth"))
            .unwrap_or_else(|err| panic!("criar src/auth: {err}"));
        fs::write(root.join("src/auth/session.rs"), "let ttl = 5;\n")
            .unwrap_or_else(|err| panic!("escrever: {err}"));
        assert_eq!(
            state_for_anchor(&root, "src/auth/session.rs:42"),
            EvidenceState::Confirmed,
            "arquivo existe no disco ⇒ CONFIRMADA"
        );
        // Sem `:linha` também é verificável: o caminho é o que existe.
        assert_eq!(
            state_for_anchor(&root, "src/auth/session.rs"),
            EvidenceState::Confirmed
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// Aceite do PR36: âncora sem arquivo no disco = INFERIDA (nunca erro,
    /// nunca CONFIRMADA de graça).
    #[test]
    fn missing_anchor_is_inferred() {
        let root = temp_root("inferred");
        assert_eq!(
            state_for_anchor(&root, "src/que/saiu.rs:7"),
            EvidenceState::Inferred,
            "arquivo ausente ⇒ INFERIDA"
        );
        // Traversal e absoluto não são verificáveis: INFERIDA, não pânico.
        assert_eq!(
            state_for_anchor(&root, "../segredo.rs:1"),
            EvidenceState::Inferred
        );
        assert_eq!(
            state_for_anchor(&root, "/etc/passwd:1"),
            EvidenceState::Inferred
        );
        // `:42` puro não é caminho.
        assert_eq!(state_for_anchor(&root, ":42"), EvidenceState::Inferred);
        let _ = fs::remove_dir_all(&root);
    }

    /// Aceite do PR36: sem arquivo apontado = AUSENTE.
    #[test]
    fn absent_anchors_are_the_placeholder_family() {
        let root = temp_root("absent");
        for anchor in ["", "   ", "—", crate::llm::prompts::TRIAGE_FILES_UNKNOWN] {
            assert_eq!(
                state_for_anchor(&root, anchor),
                EvidenceState::Absent,
                "âncora `{anchor}` deve ser AUSENTE"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn line_suffix_only_strips_when_it_is_the_line_number() {
        assert_eq!(
            strip_line_suffix("src/a.rs:42").as_deref(),
            Some("src/a.rs"),
            ":42 é linha"
        );
        assert_eq!(
            strip_line_suffix("src/a.rs").as_deref(),
            Some("src/a.rs"),
            "sem dois-pontos, a âncora inteira é o caminho"
        );
        // Outro uso de dois-pontos não é âncora de linha: preserva o texto.
        assert_eq!(
            strip_line_suffix("http://x/y.rs").as_deref(),
            Some("http://x/y.rs")
        );
        assert_eq!(strip_line_suffix(":42"), None, ":42 puro não é caminho");
    }

    #[test]
    fn labels_are_the_vocabulary_the_prompt_uses() {
        assert_eq!(EvidenceState::Confirmed.as_str(), "CONFIRMADA");
        assert_eq!(EvidenceState::Inferred.as_str(), "INFERIDA");
        assert_eq!(EvidenceState::Absent.as_str(), "AUSENTE");
        assert_eq!(EvidenceState::LABELS.len(), 3);
    }
}
