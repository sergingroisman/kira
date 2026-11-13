//! Prompts do `digest` — **sempre em inglês** (ADR-0038, D6).
//!
//! Convenção do repositório: o texto vive aqui, versionado, e o teste de allowlist
//! exige `// LLM porque:` no ponto de chamada. Mudar o texto sem subir
//! [`PROMPT_VERSION`] faz o cache reaproveitar post gerado pelo prompt antigo — por
//! isso a versão entra no `cache_key` da linha em `posts`.
//!
//! O idioma do **artefato** (`lang`) é dado da execução: estes prompts são em inglês
//! e instruem o modelo a responder no idioma pedido, com identificadores e caminhos
//! preservados em inglês.

/// Versão do contrato de prompt. Suba ao mudar qualquer texto abaixo.
pub const PROMPT_VERSION: &str = "m3-v1";

/// Instrução de sistema comum aos posts: grounding, sem invenção, citação obrigatória.
pub const SYSTEM: &str = "You are KIRA, a documentation agent for a software repository.
Rules you must follow:
- Use ONLY the facts provided (project profile, file tree, key files). Never invent files, packages or behaviour.
- Every concrete claim about the code must cite an anchor in the form `path:line`.
- Preserve identifiers, file paths, type names and commands exactly as written (they stay in English).
- If a required fact is missing, write `not available` instead of guessing.
- Output GitHub-flavoured Markdown only. No preamble, no closing remarks.";

/// Post `intro`: o que é o projeto, com que stack, por onde começar.
pub fn intro(lang: &str, project: &str, profile: &str, tree: &str) -> String {
    format!(
        "Write the introduction page for the project `{project}` in {lang}.
Sections, in this exact order:
## What this is
Two to four sentences grounded in the profile and tree below.
## Stack
A table with the detected stacks and the toolchain files that prove them.
## Where to start
Three to five bullets naming real paths from the tree, each followed by `path:line` when you cite code.
## What is not documented yet
Only if a fact is missing.

PROFILE
{profile}

TREE (truncated)
{tree}"
    )
}

/// Post `c4-container`: o diagrama de containers em Mermaid, com fronteiras reais.
pub fn c4_container(lang: &str, project: &str, profile: &str, key_files: &str) -> String {
    format!(
        "Write the C4 container page for the project `{project}` in {lang}.
Sections, in this exact order:
## Containers
Prose (max 120 words) describing the containers you can justify from the evidence.
## Diagram
Exactly one fenced ```mermaid block using the C4 syntax:
    C4Container
      title Container diagram for {project}
      Container(alias, \"Name\", \"technology\", \"responsibility\")
      Rel(from, to, \"uses\", \"protocol\")
Use only containers and relationships supported by the evidence. Never invent a database, queue or external system.
## Evidence
A bullet list of `path:line` anchors behind each container.

PROFILE
{profile}

KEY FILES
{key_files}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_are_english_and_pin_the_version() {
        assert!(SYSTEM.contains("Never invent"));
        assert_eq!(PROMPT_VERSION, "m3-v1");
    }

    #[test]
    fn prompt_carries_the_requested_language_and_the_evidence() {
        let text = intro("pt-BR", "demo", "stacks: rust", "src/main.rs");
        assert!(text.contains("in pt-BR"), "idioma pedido não aparece");
        assert!(text.contains("stacks: rust"), "perfil não aparece");
        assert!(text.contains("src/main.rs"), "árvore não aparece");
    }

    #[test]
    fn c4_prompt_asks_for_one_mermaid_block_only() {
        let text = c4_container("en", "demo", "stacks: node", "package.json:1");
        assert!(text.contains("```mermaid"), "sem cerca de mermaid");
        assert!(text.contains("C4Container"), "sem o tipo C4");
        assert!(
            text.contains("Exactly one"),
            "a regra de um bloco só precisa estar escrita"
        );
    }
}
