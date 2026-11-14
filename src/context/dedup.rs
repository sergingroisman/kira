//! Dedup contíguo dos trechos do pack (PR38, onda TE).
//!
//! Os trechos que o packer monta carregam linhas repetidas em bloco — imports
//! duplicados, boilerplate colado — e cada repetição gastava orçamento de
//! prompt com informação que o modelo já tinha. O dedup mantém a PRIMEIRA
//! ocorrência (a âncora `caminho:linha` continua apontando o primeiro lugar
//! de verdade, PR36 incluso: mesma linha ⇒ mesma tag de evidência) e conta
//! as omitidas num marcador contável (` ... ×N`). DENTRO do mesmo trecho e
//! ENTRE trechos do MESMO arquivo: dedup entre arquivos diferentes quebra a
//! semântica de âncora e, medido no fixture dourado, não bate a barra —
//! fica fora (teste prova a exclusão).
//!
//! Duas invariantes:
//!
//! 1. **Determinismo**: função pura da lista ordenada — mesmo estado ⇒ mesmo
//!    pack byte a byte (gate do M1).
//! 2. **Barra medida** (RTK, PR38): a estratégia adotada só entrou porque o
//!    fixture dourado prova corte ≥ [`DEDUP_MIN_CUT_PCT`]; const, nunca env
//!    (mesmo padrão do limiar D9/PR34).

use std::collections::{HashMap, HashSet};

use super::repo::CodeExcerpt;

/// Barra do RTK (PR38): uma estratégia de dedup só entra no packer se provar
/// redução ≥ 20% no fixture dourado (`tests/fixtures/context/dedup/`).
pub const DEDUP_MIN_CUT_PCT: f64 = 20.0;

/// Sufixo do marcador de omissão: a primeira ocorrência fica e as repetidas
/// saem contadas (`... ×3` = três linhas idênticas omitidas depois desta).
/// Literal e determinístico — nunca inventa conteúdo no lugar do que saiu.
#[must_use]
pub fn omitted_marker(omitted: usize) -> String {
    format!(" ... ×{omitted}")
}

/// Custo de um trecho no render do pack — a MESMA conta que
/// `context::pack_partial` usa para o orçamento (texto + caminho + tag de
/// evidência + separação, mais o marcador quando o dedup omitiu repetidas).
/// Uma função só, para a economia medida e o orçamento não divergirem.
#[must_use]
pub(crate) fn cost_of_excerpt(excerpt: &CodeExcerpt) -> usize {
    let mut cost = excerpt.text.chars().count()
        + excerpt.path.chars().count()
        + excerpt.evidence.as_str().len()
        + 15;
    if excerpt.omitted > 0 {
        cost += omitted_marker(excerpt.omitted).chars().count();
    }
    cost
}

/// Result of one dedup pass: the excerpts that survive (in order) and the
/// prompt characters the pass saved.
///
/// `PartialEq` e não `Eq`: `CodeExcerpt` carrega `rank: Option<f64>`, e a
/// igualdade aqui serve ao determinismo dos testes (mesmo estado ⇒ igual).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Deduped {
    /// Trechos pós-dedup, na mesma ordem da entrada.
    pub excerpts: Vec<CodeExcerpt>,
    /// Chars de prompt que o dedup evitou (custo das repetidas − marcadores).
    pub chars_saved: usize,
}

/// Estratégia ADOTADA (PR38): colapsa repetições de texto DENTRO da lista de
/// trechos, sempre restritas ao MESMO arquivo (a âncora é `caminho:linha`).
///
/// Um run contíguo no pack (linhas repetidas em bloco) e repetições do mesmo
/// texto espalhadas no mesmo arquivo caem no mesmo caso: a primeira
/// ocorrência fica com `omitted = N` e as outras saem. O colapso SEMPRE
/// paga o próprio marcador: o piso do custo de um trecho real (caminho +
/// tag de evidência + separação ≥ 23 chars) supera qualquer marcador de
/// até dois dígitos — dedup nunca aumenta o pack por construção (teste
/// prova o piso com o menor trecho possível).
pub(crate) fn same_file(excerpts: &[CodeExcerpt]) -> Deduped {
    // (caminho, texto) → (custo somado de TODAS as ocorrências, quantidade).
    let mut stats: HashMap<(&str, &str), (usize, usize)> = HashMap::new();
    for excerpt in excerpts {
        let entry = stats
            .entry((excerpt.path.as_str(), excerpt.text.as_str()))
            .or_insert((0, 0));
        entry.0 += cost_of_excerpt(excerpt);
        entry.1 += 1;
    }
    let mut out: Vec<CodeExcerpt> = Vec::with_capacity(excerpts.len());
    let mut seen: HashSet<(&str, &str)> = HashSet::new();
    let mut saved = 0usize;
    for excerpt in excerpts {
        let key = (excerpt.path.as_str(), excerpt.text.as_str());
        let (total_cost, count) = stats[&key];
        if count <= 1 {
            out.push(excerpt.clone());
            continue;
        }
        // Dentro do grupo o custo é idêntico (mesmo path/texto ⇒ mesma tag
        // de evidência: `state_for_anchor` só olha o caminho).
        let unit = total_cost / count;
        let duplicates = count - 1;
        if seen.insert(key) {
            // Primeira ocorrência: fica, carregando a contagem das omitidas.
            let mut kept = excerpt.clone();
            kept.omitted = duplicates;
            out.push(kept);
            saved += duplicates * unit - omitted_marker(duplicates).chars().count();
        }
        // Repetida: omitida (nem entra na lista).
    }
    Deduped {
        excerpts: out,
        chars_saved: saved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::evidence::EvidenceState;

    /// Trecho de teste: tag fixa para a conta ficar previsível.
    fn excerpt(path: &str, line: usize, text: &str) -> CodeExcerpt {
        CodeExcerpt {
            path: path.to_string(),
            line,
            text: text.to_string(),
            rank: None,
            priority: false,
            evidence: EvidenceState::Confirmed,
            omitted: 0,
        }
    }

    /// Aceite (b): a primeira ocorrência fica, a âncora é dela, e o marcador
    /// sai byte a byte como ` ... ×N`.
    #[test]
    fn first_occurrence_is_kept_with_a_countable_marker() {
        let input = vec![
            excerpt("src/a.rs", 4, "use crate::auth::session_token;"),
            excerpt("src/a.rs", 5, "use crate::auth::session_token;"),
            excerpt("src/a.rs", 6, "use crate::auth::session_token;"),
            excerpt("src/a.rs", 9, "let token = session_token();"),
        ];
        let deduped = same_file(&input);
        assert_eq!(
            deduped.excerpts.len(),
            2,
            "run inteiro colapsa: {deduped:?}"
        );
        let kept = &deduped.excerpts[0];
        assert_eq!(kept.anchor(), "src/a.rs:4", "âncora da primeira");
        assert_eq!(kept.omitted, 2, "as duas repetidas viram contador");
        assert_eq!(omitted_marker(kept.omitted), " ... ×2");
        assert_eq!(deduped.excerpts[1].omitted, 0, "linha única intocada");
        assert_eq!(deduped.chars_saved, 2 * cost_of_excerpt(&input[0]) - 7);
    }

    /// Aceite: dedup NUNCA atravessa arquivo — texto igual em arquivo
    /// diferente são evidências independentes, cada uma com a âncora dela.
    #[test]
    fn identical_text_in_a_different_file_is_never_collapsed() {
        let input = vec![
            excerpt("src/a.rs", 4, "use crate::auth::session_token;"),
            excerpt("src/b.rs", 2, "use crate::auth::session_token;"),
        ];
        let deduped = same_file(&input);
        assert_eq!(deduped.excerpts.len(), 2, "{deduped:?}");
        assert_eq!(deduped.chars_saved, 0);
    }

    /// Dedup nunca aumenta o pack — prova do PISO: o menor trecho possível
    /// (texto vazio, caminho de 1 char, tag mais curta) colapsado em par
    /// ainda paga o próprio marcador com sobra. É por isso que a função não
    /// precisa de guard: o colapso é sempre positivo por construção.
    #[test]
    fn even_the_smallest_group_pays_its_marker() {
        let minimal = CodeExcerpt {
            path: "a".to_string(),
            line: 1,
            text: String::new(),
            rank: None,
            priority: false,
            evidence: EvidenceState::default(),
            omitted: 0,
        };
        let input = vec![minimal.clone(), minimal.clone()];
        let deduped = same_file(&input);
        assert_eq!(deduped.excerpts.len(), 1, "par mínimo colapsa");
        assert_eq!(deduped.excerpts[0].omitted, 1);
        let unit = cost_of_excerpt(&minimal);
        assert!(
            unit > omitted_marker(1).chars().count(),
            "piso do trecho ({unit}) paga o marcador"
        );
        assert_eq!(deduped.chars_saved, unit - 7, "economia sempre > 0");
    }

    /// Aceite (determinismo): mesma entrada ⇒ mesma saída, duas vezes.
    #[test]
    fn the_same_input_produces_the_same_dedup_twice() {
        let input = vec![
            excerpt("src/a.rs", 4, "use crate::auth::session_token;"),
            excerpt("src/a.rs", 5, "use crate::auth::session_token;"),
        ];
        assert_eq!(same_file(&input), same_file(&input));
    }

    /// Aceite (a) — a BARRA com número: no fixture dourado, a estratégia
    /// adotada (dedup contíguo, mesmo arquivo) corta ≥ 20% dos chars do bloco
    /// de trechos. O número entra na mensagem do assert: é ele que o PR
    /// registra como economia medida.
    #[test]
    fn the_golden_fixture_proves_the_bar_for_the_adopted_strategy() {
        let content = include_str!("../../tests/fixtures/context/dedup/boilerplate.rs");
        let terms = vec!["session_token".to_string()];
        let excerpts = super::super::repo::grep_text("src/legacy/boilerplate.rs", content, &terms);
        assert!(
            excerpts.len() >= 3,
            "fixture tem o run de imports: {excerpts:?}"
        );
        let baseline: usize = excerpts.iter().map(cost_of_excerpt).sum();
        let deduped = same_file(&excerpts);
        let after: usize = deduped.excerpts.iter().map(cost_of_excerpt).sum();
        assert!(after < baseline, "dedup tem que cortar, não crescer");
        let cut_pct = 100.0 * (baseline - after) as f64 / baseline as f64;
        assert!(
            cut_pct >= DEDUP_MIN_CUT_PCT,
            "estratégia adotada tem que bater a barra: corte de {cut_pct:.1}% \
             (baseline {baseline} chars → {after} chars, economizados {})",
            deduped.chars_saved
        );
    }

    /// Aceite (e) — a EXCLUSÃO com número: o candidato de dedup ENTRE
    /// ARQUIVOS, medido na MESMA lista dourada (boilerplate + distinto), não
    /// chega perto da barra — e fica FORA do packer por isso (além de quebrar
    /// a semântica de âncora, razão que o plano do TE já registrava).
    ///
    /// O que se mede é o que o candidato ADICIONA de fato: runs contíguos de
    /// texto igual que ATRAVESSAM arquivo (colapsados numa ocorrência só,
    /// sozinhos — o run DENTRO de um arquivo já é a estratégia adotada, não
    /// é mérito do candidato). No fixture dourado, arquivos diferentes não
    /// compartilham linha nenhuma: o candidato não tem o que cortar.
    #[test]
    fn the_cross_file_candidate_stays_out_below_the_bar() {
        let terms = vec!["session_token".to_string()];
        let a = include_str!("../../tests/fixtures/context/dedup/boilerplate.rs");
        let b = include_str!("../../tests/fixtures/context/dedup/distinto.rs");
        let mut excerpts = super::super::repo::grep_text("src/legacy/boilerplate.rs", a, &terms);
        excerpts.extend(super::super::repo::grep_text(
            "src/other/distinto.rs",
            b,
            &terms,
        ));
        let baseline: usize = excerpts.iter().map(cost_of_excerpt).sum();
        let mut cross_saved = 0usize;
        let mut start = 0usize;
        while start < excerpts.len() {
            let mut end = start + 1;
            while end < excerpts.len() && excerpts[end].text == excerpts[start].text {
                end += 1;
            }
            let run = &excerpts[start..end];
            let files = run
                .iter()
                .map(|excerpt| excerpt.path.as_str())
                .collect::<HashSet<&str>>();
            if files.len() > 1 {
                let unit = cost_of_excerpt(&run[0]);
                let duplicates = run.len() - 1;
                let marker_cost = omitted_marker(duplicates).chars().count();
                if duplicates * unit > marker_cost {
                    cross_saved += duplicates * unit - marker_cost;
                }
            }
            start = end;
        }
        let cut_pct = 100.0 * cross_saved as f64 / baseline as f64;
        assert!(
            cut_pct < DEDUP_MIN_CUT_PCT,
            "candidato cross-file devia ficar FORA por não bater a barra, mas \
             mediu {cut_pct:.1}% (economia {cross_saved} de {baseline} chars) — \
             revisar o fixture"
        );
    }
}
