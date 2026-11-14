//! Routing de especialistas (HF-5, ADR-0040): kind/tarefa → tag do Ollama.
//!
//! A tabela `model_role` vive no `config.toml` (pares `model_role.<kind> =
//! <tag>`, escrita à mão ou pelo dono) sobre defaults embutidos por papel:
//! julgamento em `qwen3.5:4b`, kinds estreitos/baratos em 1–2 B instruct e
//! auditoria/segurança em coder 7B. **Nenhuma tag entra no routing sem o gate
//! de admissão** (proveniência + SHA-256 + template + probes, gravados no
//! store pelo `kira model admit`): tag não admitida cai no default da casa
//! com nota acionável — nunca erro silencioso, nunca modelo de papel.
//!
//! Módulo puro (ADR-0016, camada 3 — cálculo em Rust): nenhuma chamada de
//! modelo, nenhuma rede. A consulta de admissão chega como closure para o
//! módulo não depender do store. A política de quant/proveniência mora em
//! [`crate::model`] (é catálogo de modelos, não camada LLM).

use crate::llm::ollama::DEFAULT_MODEL;

/// Papel de um kind/tarefa no catálogo de especialistas (plano-finalizacao §3,
/// taxonomia honesta — nada de "especialista" por nome de modelo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Julgamento default (triage, posts gerais): `qwen3.5:4b`.
    Judgment,
    /// Kinds estreitos/baratos (resumo, glossário): 1–2 B instruct.
    Cheap,
    /// Auditoria de código/segurança (kinds `security`, c4 de backend):
    /// coder 7B — só entra como default do kind DEPOIS de medido no `--eval`.
    Coder,
}

impl Role {
    /// Tag default embutida do papel. O Coder aponta para o fine-tune coder
    /// da família Qwen de quantizer conhecido (bartowski) no piso Q4_K_M —
    /// mas só é usado de fato quando ADMITIDO (gate do `model admit`);
    /// enquanto isso o routing cai no default de julgamento, com nota.
    #[must_use]
    pub fn builtin_tag(self) -> &'static str {
        match self {
            Self::Judgment => DEFAULT_MODEL,
            Self::Cheap => "qwen3:1.7b",
            Self::Coder => "hf.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M",
        }
    }
}

/// Papel embutido de cada kind/tarefa. Kinds ainda sem especialista medido
/// ficam no papel de julgamento (o default eterno — nada de vibe).
#[must_use]
pub fn role_for_kind(kind: &str) -> Role {
    match kind {
        "security" | "c4-container" => Role::Coder,
        "glossary" | "summary" | "resumo" | "glossario" => Role::Cheap,
        _ => Role::Judgment,
    }
}

/// Prefixo das chaves de override no `config.toml`: `model_role.<kind> = tag`.
pub const CONFIG_MODEL_ROLE_PREFIX: &str = "model_role.";

/// Tag configurada para o kind (override do dono) — `None` sem override.
/// A leitura dos pares é feita pelo caller (`credentials::read_config`);
/// aqui só o casamento de chave, puro e determinístico.
#[must_use]
pub fn tag_for_kind(kind: &str, config: &[(String, String)]) -> Option<String> {
    let key = format!("{CONFIG_MODEL_ROLE_PREFIX}{kind}");
    config
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Resultado do routing de UMA tarefa: a tag (sem prefixo `ollama:`) e as
/// notas acionáveis que explicam a decisão (fallback por admissão — nada
/// silencioso).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routing {
    pub tag: String,
    pub notes: Vec<String>,
}

/// Resolve a tag de um kind/tarefa (função PURA — a consulta de admissão
/// chega como closure).
///
/// - `explicit` (`--model`/`KIRA_MODEL`) vence TUDO — o dono pediu, o dono
///   tem (nunca routing por cima de uma escolha explícita).
/// - Sem escolha explícita: override `model_role.<kind>` do config > tag
///   default do papel embutido.
/// - A tag default de julgamento ([`DEFAULT_MODEL`]) é da casa (oficial,
///   conferida por testes) e não passa pelo gate — é o fallback eterno.
/// - Qualquer outra tag só entra no routing ADMITIDA (`model_admission` no
///   store). Sem admissão → fallback no default com nota acionável dizendo
///   o comando que admite.
pub fn resolve_tag(
    kind: &str,
    config: &[(String, String)],
    explicit: Option<&str>,
    admitted: &dyn Fn(&str) -> Option<bool>,
) -> Routing {
    if let Some(tag) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return Routing {
            tag: tag.to_string(),
            notes: Vec::new(),
        };
    }
    let candidate =
        tag_for_kind(kind, config).unwrap_or_else(|| role_for_kind(kind).builtin_tag().to_string());
    if candidate == DEFAULT_MODEL {
        // Default de julgamento da casa: sempre roteável.
        return Routing {
            tag: candidate,
            notes: Vec::new(),
        };
    }
    match admitted(&candidate) {
        Some(true) => Routing {
            tag: candidate,
            notes: Vec::new(),
        },
        Some(false) => Routing {
            tag: DEFAULT_MODEL.to_string(),
            notes: vec![format!(
                "kind {kind}: tag {candidate} não entrou no routing — falta o gate de \
                 admissão; rode `kira model admit {candidate} --sha256 <sha-do-model-card> \
                 --probes-ok` (o run seguiu com {DEFAULT_MODEL})"
            )],
        },
        None => Routing {
            tag: DEFAULT_MODEL.to_string(),
            notes: vec![format!(
                "kind {kind}: admissão de {candidate} não pôde ser consultada (trilha \
                 indisponível) — o run seguiu com {DEFAULT_MODEL} em vez de rotear \
                 cegamente"
            )],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // Papéis e tabela
    // -------------------------------------------------------------------

    #[test]
    fn roles_map_kinds_to_the_plan_taxonomy() {
        assert_eq!(role_for_kind("intro"), Role::Judgment);
        assert_eq!(role_for_kind("triage"), Role::Judgment);
        assert_eq!(role_for_kind("glossary"), Role::Cheap);
        assert_eq!(role_for_kind("resumo"), Role::Cheap);
        assert_eq!(role_for_kind("security"), Role::Coder);
        assert_eq!(role_for_kind("c4-container"), Role::Coder);
        // Kind desconhecido NUNCA vira especialista por chute: julgamento.
        assert_eq!(role_for_kind("qualquer-coisa"), Role::Judgment);
    }

    #[test]
    fn builtin_tags_follow_the_owner_minimum() {
        assert_eq!(Role::Judgment.builtin_tag(), "qwen3.5:4b");
        // 1–2 B instruct para kinds estreitos; coder 7B só depois de medido.
        assert_eq!(Role::Cheap.builtin_tag(), "qwen3:1.7b");
        assert_eq!(
            Role::Coder.builtin_tag(),
            "hf.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M"
        );
    }

    #[test]
    fn config_override_wins_over_builtin() {
        let config = vec![(
            "model_role.intro".to_string(),
            "hf.co/bartowski/X-GGUF:Q5_K_M".to_string(),
        )];
        assert_eq!(
            tag_for_kind("intro", &config).as_deref(),
            Some("hf.co/bartowski/X-GGUF:Q5_K_M")
        );
        assert_eq!(tag_for_kind("c4-container", &config), None);
        // Override vazio/branco não conta (nunca routing para tag vazia).
        let vazio = vec![("model_role.intro".to_string(), "   ".to_string())];
        assert_eq!(tag_for_kind("intro", &vazio), None);
    }

    // -------------------------------------------------------------------
    // Routing com o gate de admissão
    // -------------------------------------------------------------------

    #[test]
    fn explicit_choice_wins_over_everything() {
        let config = vec![(
            "model_role.intro".to_string(),
            "hf.co/bartowski/X-GGUF:Q5_K_M".to_string(),
        )];
        let routing = resolve_tag("intro", &config, Some("fake"), &|_| Some(true));
        assert_eq!(routing.tag, "fake");
        assert!(routing.notes.is_empty(), "escolha explícita não gera nota");
    }

    #[test]
    fn default_judgment_tag_routes_without_admission() {
        // O default da casa não passa pelo gate (é oficial e conferido).
        let routing = resolve_tag("intro", &[], None, &|_| Some(false));
        assert_eq!(routing.tag, "qwen3.5:4b");
        assert!(routing.notes.is_empty());
    }

    #[test]
    fn unadmitted_tag_does_not_enter_the_routing() {
        // O gate: tag sem admissão NÃO roteia — cai no default com nota
        // acionável que diz o comando de admissão.
        let config = vec![(
            "model_role.c4-container".to_string(),
            "hf.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M".to_string(),
        )];
        let routing = resolve_tag("c4-container", &config, None, &|_| Some(false));
        assert_eq!(routing.tag, "qwen3.5:4b", "fallback no default da casa");
        assert_eq!(
            routing.notes.len(),
            1,
            "uma nota por fallback: {:?}",
            routing.notes
        );
        let note = &routing.notes[0];
        assert!(
            note.contains("não entrou no routing")
                && note.contains("kira model admit")
                && note.contains("--probes-ok"),
            "nota acionável: {note}"
        );
        // Sem override nenhum, o builtin do coder também precisa do gate.
        let routing = resolve_tag("c4-container", &[], None, &|_| Some(false));
        assert_eq!(routing.tag, "qwen3.5:4b");
        assert_eq!(routing.notes.len(), 1);
    }

    #[test]
    fn admitted_tag_routes_and_unreadable_store_falls_back_with_a_note() {
        let config = vec![(
            "model_role.c4-container".to_string(),
            "hf.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M".to_string(),
        )];
        let routing = resolve_tag("c4-container", &config, None, &|_| Some(true));
        assert_eq!(
            routing.tag,
            "hf.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M"
        );
        assert!(routing.notes.is_empty());
        // Trilha indisponível = fail-open VISÍVEL (nunca rotear cegamente).
        let routing = resolve_tag("c4-container", &config, None, &|_| None);
        assert_eq!(routing.tag, "qwen3.5:4b");
        assert!(
            routing.notes[0].contains("trilha indisponível"),
            "nota da causa: {}",
            routing.notes[0]
        );
    }
}
