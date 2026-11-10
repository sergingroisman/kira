//! Enforcement of ADR-0016 (CLI-first, LLM-last).
//!
//! A rule that lives only in a document is intent, not a constraint: within a
//! few PRs someone (human or agent) adds `use crate::llm::...` to a module that
//! should be deterministic and the whole "the CLI does the work" contract
//! silently erodes. This test makes that erosion a red build.
//!
//! Two assertions:
//! 1. Only the allowlisted modules may reference the LLM layer at all.
//! 2. Every allowlisted module that *decides* with the model must carry a
//!    `// LLM porque:` justification at the call site — the answer to "why not a
//!    CLI or a parser?" has to be written down where the call happens.

use std::fs;
use std::path::{Path, PathBuf};

/// Composition root: builds the backend and hands it over. Wiring, not deciding.
const COMPOSITION_ROOT: &str = "main.rs";

/// Modules allowed to reference the LLM layer. Anything else is a regression.
const ALLOWED_EXACT: [&str; 1] = ["triage.rs"];
const ALLOWED_PREFIXES: [&str; 2] = ["llm/", "loop/"];

/// Modules allowed *and* required to justify each decision with the model.
/// `wiki/` joins this list when it starts consolidating with an LLM (PR30).
const MUST_JUSTIFY: [&str; 1] = ["triage.rs"];

const JUSTIFICATION: &str = "// LLM porque:";

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|err| panic!("ler {dir:?}: {err}"));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|err| panic!("entrada em {dir:?}: {err}"))
            .path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn references_llm(source: &str) -> bool {
    source.contains("crate::llm::") || source.contains("kira::llm::")
}

#[test]
fn only_allowlisted_modules_reference_the_llm() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);

    let mut offenders = Vec::new();
    for file in files {
        let rel = relative(&file, &src);
        if rel == COMPOSITION_ROOT
            || ALLOWED_EXACT.contains(&rel.as_str())
            || ALLOWED_PREFIXES.iter().any(|p| rel.starts_with(p))
        {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("ler {file:?}: {err}"));
        if references_llm(&source) {
            offenders.push(rel);
        }
    }

    assert!(
        offenders.is_empty(),
        "ADR-0016 violado: estes módulos referenciam a camada LLM sem permissão: {offenders:?}.\n\
         Resolva com CLI/parser/cálculo (camadas 1-3) ou, se for julgamento de verdade, \
         justifique e adicione o módulo à allowlist em tests/llm_allowlist.rs e no ADR-0016."
    );
}

#[test]
fn allowlisted_modules_justify_each_decision() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for rel in MUST_JUSTIFY {
        let path = src.join(rel);
        let source = fs::read_to_string(&path).unwrap_or_else(|err| panic!("ler {path:?}: {err}"));
        if !references_llm(&source) {
            continue; // módulo ainda não fala com modelo: nada a justificar
        }
        assert!(
            source.contains(JUSTIFICATION),
            "{rel} chama a camada LLM sem '{JUSTIFICATION}' no call site (ADR-0016): \
             escreva por que CLI/parser/cálculo não resolvem este passo."
        );
    }
}

#[test]
fn composition_root_wires_without_deciding() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(COMPOSITION_ROOT);
    let source = fs::read_to_string(&path).unwrap_or_else(|err| panic!("ler {path:?}: {err}"));
    if references_llm(&source) {
        assert!(
            !source.contains("build_prompt"),
            "main.rs é composition root: pode construir o backend, não pode montar prompt \
             nem decidir com o modelo (ADR-0016)."
        );
    }
}
