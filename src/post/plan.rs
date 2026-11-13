//! Plano do `digest`: quais posts existem para um projeto (ADR-0038, M3).
//!
//! Função **pura**: recebe os fatos já renderizados (`.kira/facts.md`) e devolve a
//! lista de posts que deveriam existir. Sem I/O, sem relógio, sem modelo — o que
//! torna o plano testável e o `cache_key` reprodutível.
//!
//! A procedência de cada post é o FNV-1a 64 de `(kind, digest)`: mudar o digest
//! (repositório mudou) reenfileira o post; rodar de novo com o mesmo digest não
//! gera trabalho (é o mesmo mecanismo do `post_enqueue`).

/// Um post a garantir na fila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedPost {
    pub slug: String,
    pub kind: String,
    /// FNV-1a 64 (hex) sobre `kind + "\n" + digest`.
    pub source_hash: String,
}

/// Kinds que esta entrega gera. Ordem fixa = ordem de geração.
pub const KINDS: [&str; 2] = ["intro", "c4-container"];

/// Monta o plano a partir do digest já renderizado.
///
/// `lang` **não** entra no plano porque não muda *quais* posts existem — ele entra
/// no `cache_key` do `post_enqueue`, onde dispara a regeneração.
pub fn plan(digest: &str) -> Vec<PlannedPost> {
    KINDS
        .iter()
        .map(|kind| PlannedPost {
            slug: slug_for(kind),
            kind: (*kind).to_string(),
            source_hash: fnv1a64_hex(&format!("{kind}\n{digest}")),
        })
        .collect()
}

/// Slug estável do post: `kind` em kebab, nunca derivado do título gerado.
pub fn slug_for(kind: &str) -> String {
    kind.replace(' ', "-").to_ascii_lowercase()
}

/// FNV-1a 64 bits em hex — o mesmo algoritmo do store, feito à mão (sem dependência).
pub fn fnv1a64_hex(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_deterministic_and_ordered() {
        let a = plan("# Digest — demo\n");
        let b = plan("# Digest — demo\n");
        assert_eq!(a, b);
        assert_eq!(
            a.iter().map(|p| p.kind.as_str()).collect::<Vec<_>>(),
            vec!["intro", "c4-container"]
        );
    }

    #[test]
    fn a_changed_digest_changes_every_source_hash() {
        let a = plan("digest v1");
        let b = plan("digest v2");
        assert_ne!(a[0].source_hash, b[0].source_hash);
        assert_ne!(a[1].source_hash, b[1].source_hash);
    }

    #[test]
    fn slugs_are_stable_kebab() {
        assert_eq!(slug_for("c4-container"), "c4-container");
        assert_eq!(slug_for("Intro"), "intro");
    }

    #[test]
    fn fnv_matches_the_known_value() {
        // Valor de referência do FNV-1a 64 para a string vazia.
        assert_eq!(fnv1a64_hex(""), "cbf29ce484222325");
        assert_eq!(fnv1a64_hex("a"), "af63dc4c8601ec8c");
    }
}
