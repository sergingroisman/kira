//! Fixtures de entrada/saída do prompt de triage (PR6) e dourado do prompt do
//! sistema (PR7).
//!
//! Cada caso em `tests/fixtures/triage/<slug>/` é um contrato fechado:
//!
//! - `item.json` — work item como o Azure DevOps devolve (entrada);
//! - `model.txt` — resposta crua de um modelo local (4B, com markdown solto);
//! - `prompt.txt` — prompt do usuário que o KIRA deve montar, byte a byte;
//! - `expected.txt` — saída formatada nas três seções fixas.
//!
//! E `tests/fixtures/prompts/triage-system.txt` guarda o texto exato do prompt
//! do sistema, comparado byte a byte contra a constante compilada.
//!
//! O teste compara os arquivos gerados contra os dourados. Mudar o prompt
//! ou o formatador quebra aqui de propósito: a mudança tem que vir no mesmo
//! commit, com `TRIAGE_PROMPT_VERSION` nova, `TRIAGE_SYSTEM_PROMPT_FNV1A64`
//! recalculada e o diff das fixtures revisado.
//!
//! Para regenerar os dourados depois de uma mudança intencional:
//! `KIRA_UPDATE_FIXTURES=1 cargo test --test triage_prompt_fixtures`.

use std::fs;
use std::path::{Path, PathBuf};

use kira::llm::prompts;
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

/// Reads a fixture with line endings normalized to LF.
///
/// The goldens are written with LF and `.gitattributes` pins them, but a Windows
/// checkout with `core.autocrlf` can still hand back CRLF: the comparison is
/// about prompt content, never about the platform's newline convention.
fn read(path: &Path) -> String {
    let raw =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("ler {:?}: {err}", path.display()));
    raw.replace("\r\n", "\n")
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

// --- PR7: dourado do prompt do SISTEMA -------------------------------------

/// `tests/fixtures/prompts/triage-system.txt` guarda o texto exato do prompt do
/// sistema. O prompt é o contrato com o modelo: qualquer byte a mais ou a menos
/// muda o comportamento do modelo 4B, então a comparação é byte a byte, e não
/// um `contains`.
fn system_prompt_golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("prompts")
        .join("triage-system.txt")
}

#[test]
fn system_prompt_matches_the_golden_text() {
    assert_matches_golden(&system_prompt_golden(), prompts::TRIAGE_SYSTEM_PROMPT);
}

/// Duas entradas para o mesmo fato: o texto dourado e a impressão digital
/// compilada. Se alguém editar o prompt e o dourado juntos, mas esquecer a
/// constante, o `cargo test` reprova aqui.
#[test]
fn system_prompt_fingerprint_is_the_fnv1a_of_the_golden_text() {
    let text = read(&system_prompt_golden());
    let expected = format!("{:016x}", kira::store::fnv1a64(text.as_bytes()));
    assert_eq!(
        prompts::TRIAGE_SYSTEM_PROMPT_FNV1A64,
        expected,
        "TRIAGE_SYSTEM_PROMPT_FNV1A64 não corresponde ao dourado {}",
        system_prompt_golden().display()
    );
}

/// O prompt do sistema não é um lugar para dado do dono: nenhum e-mail, CPF ou
/// token pode entrar nele (vai inteiro para o modelo local e para o repo).
#[test]
fn system_prompt_golden_carries_no_secret() {
    let text = read(&system_prompt_golden());
    assert_eq!(
        kira::redact::redact(&text),
        text,
        "o prompt do sistema tem conteúdo que seria mascarado — remova antes de commitar"
    );
}
