//! Fixtures de entrada/saída do prompt de triage (PR6).
//!
//! Cada caso em `tests/fixtures/triage/<slug>/` é um contrato fechado:
//!
//! - `item.json` — work item como o Azure DevOps devolve (entrada);
//! - `model.txt` — resposta crua de um modelo local (4B, com markdown solto);
//! - `prompt.txt` — prompt do usuário que o KIRA deve montar, byte a byte;
//! - `expected.txt` — saída formatada nas três seções fixas.
//!
//! O teste compara os dois arquivos gerados contra os dourados. Mudar o prompt
//! ou o formatador quebra aqui de propósito: a mudança tem que vir no mesmo
//! commit, com `TRIAGE_PROMPT_VERSION` nova e o diff das fixtures revisado.
//!
//! Para regenerar os dourados depois de uma mudança intencional:
//! `KIRA_UPDATE_FIXTURES=1 cargo test --test triage_prompt_fixtures`.

use std::fs;
use std::path::{Path, PathBuf};

use kira::triage;
use kira::work_item::WorkItem;

const CASES: [&str; 2] = ["sessao-expirada", "sem-descricao"];

fn case_dir(slug: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("triage")
        .join(slug)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("ler {:?}: {err}", path.display()))
}

fn assert_matches_golden(path: &Path, actual: &str) {
    if std::env::var("KIRA_UPDATE_FIXTURES").is_ok_and(|value| value == "1") {
        fs::write(path, actual).unwrap_or_else(|err| panic!("gravar {:?}: {err}", path.display()));
        return;
    }
    let expected = read(path);
    assert_eq!(
        actual,
        expected,
        "{} divergiu do dourado.\n\
         Revise o diff. Se a mudança é intencional, suba TRIAGE_PROMPT_VERSION e rode\n\
         KIRA_UPDATE_FIXTURES=1 cargo test --test triage_prompt_fixtures\n\
         --- esperado ---\n{expected}\n--- obtido ---\n{actual}",
        path.display()
    );
}

#[test]
fn every_case_has_the_four_files() {
    for slug in CASES {
        let dir = case_dir(slug);
        for name in ["item.json", "model.txt", "prompt.txt", "expected.txt"] {
            let path = dir.join(name);
            assert!(path.is_file(), "fixture ausente: {}", path.display());
        }
    }
}

#[test]
fn prompt_and_output_match_the_golden_fixtures() {
    for slug in CASES {
        let dir = case_dir(slug);
        let item = WorkItem::from_api_json(&read(&dir.join("item.json")))
            .unwrap_or_else(|err| panic!("fixture de {slug} inválida: {err}"));

        // Sem context pack: o dourado cobre só item + prompt do sistema, que é
        // determinístico. O contexto (wiki/grep) tem teste próprio no packer.
        let prompt = triage::build_prompt(&item);
        assert_matches_golden(&dir.join("prompt.txt"), &prompt);

        let output = triage::format_triage(&read(&dir.join("model.txt")));
        assert_matches_golden(&dir.join("expected.txt"), &output);
    }
}

/// A saída formatada de todo caso tem que ser relida pelas três seções fixas —
/// é o que a captura episódica (PR30) e o `--eval` (W2-bis) consomem.
#[test]
fn every_golden_output_round_trips_through_the_three_sections() {
    for slug in CASES {
        let output = read(&case_dir(slug).join("expected.txt"));
        let sections = triage::sections_of(&output);
        for (index, name) in kira::llm::prompts::TRIAGE_SECTIONS.iter().enumerate() {
            assert!(
                output.contains(&format!("{name}:")),
                "{slug}: seção {name} ausente na saída dourada"
            );
            assert!(
                !sections[index].is_empty(),
                "{slug}: seção {name} vazia (use — para vazio)"
            );
        }
    }
}
