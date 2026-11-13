//! Renderização do digest: função **pura**, sem I/O e sem relógio (ADR-0038).
//!
//! O `.kira/digest.md` é o artefato determinístico do KIRA: mesma árvore, mesmos
//! bytes. Por isso este módulo não toca disco e não olha `SystemTime` — quem lê
//! arquivo é o chamador, que passa as linhas já recortadas e já redigidas.
//!
//! O digest é **índice**, não documento: árvore + perfil + até 12 arquivos-chave com
//! até 60 linhas cada. O texto longo e humano mora no post (`emit`), não aqui.

use std::collections::BTreeMap;

use crate::ingest::inventory::Inventory;
use crate::stack::StackProfile;

/// Arquivos-chave por digest. Mais que isso deixa de ser índice e vira dump.
pub const MAX_KEY_FILES: usize = 12;

/// Linhas por arquivo-chave.
pub const MAX_KEY_FILE_LINES: usize = 60;

/// Teto de segurança do artefato inteiro.
pub const MAX_DIGEST_CHARS: usize = 200_000;

/// Resumo dos achados do último `scan` (vem pronto do `findings.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingsBrief {
    pub total: usize,
    /// Brief por severidade, no formato que o `scan` já usa (`high 2, low 1`).
    pub by_severity: String,
}

/// Ordem fixa de prioridade: manifestos primeiro, entrypoints depois.
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "go.mod",
    "pubspec.yaml",
    "pyproject.toml",
    "requirements.txt",
    "Gemfile",
    "build.gradle.kts",
    "build.gradle",
    "global.json",
    "composer.json",
    "Package.swift",
];

/// Entrypoints conhecidos, em ordem de utilidade para o modelo.
const ENTRYPOINTS: &[&str] = &[
    "src/main.rs",
    "src/lib.rs",
    "src/main.ts",
    "src/index.ts",
    "src/App.tsx",
    "main.go",
    "Program.cs",
    "lib/main.dart",
    "app/main.py",
    "manage.py",
];

/// Escolhe os arquivos-chave do digest (puro: só olha os caminhos do inventário).
///
/// Prioridade: `README*` → manifestos → entrypoints; empate resolvido pelo caminho,
/// então a lista é a mesma em qualquer máquina e em qualquer ordem de leitura.
pub fn select_key_files(files: &[String]) -> Vec<String> {
    let mut scored: Vec<(u8, usize, String)> = Vec::new();
    for path in files {
        let name = path.rsplit('/').next().unwrap_or(path);
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("readme") && !path.contains('/') {
            scored.push((0, 0, path.clone()));
            continue;
        }
        if let Some(pos) = MANIFESTS.iter().position(|m| *m == name) {
            scored.push((1, pos, path.clone()));
            continue;
        }
        // `*.csproj`/`*.sln` valem como manifesto em qualquer pasta de projeto.
        if name.ends_with(".csproj") || name.ends_with(".sln") || name.ends_with(".fsproj") {
            scored.push((1, MANIFESTS.len(), path.clone()));
            continue;
        }
        if let Some(pos) = ENTRYPOINTS.iter().position(|e| *e == path) {
            scored.push((2, pos, path.clone()));
        }
    }
    scored.sort();
    scored.dedup_by(|a, b| a.2 == b.2);
    scored
        .into_iter()
        .take(MAX_KEY_FILES)
        .map(|(_, _, p)| p)
        .collect()
}

/// Monta o markdown do digest. Puro, ordenado, sem data.
pub fn render(
    project_name: &str,
    inventory: &Inventory,
    profile: &StackProfile,
    key_files: &[(String, String)],
    findings: Option<&FindingsBrief>,
    pipeline_notes: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Digest — {project_name}\n\n"));
    out.push_str(
        "<!-- artefato determinístico do `kira ingest`: sem data, caminhos relativos,\n\
         ordem fixa. Artefato INTERNO — sempre em inglês (ADR-0038). -->\n\n",
    );

    out.push_str("## Árvore\n\n");
    if inventory.is_empty() {
        out.push_str("_nenhum arquivo ingerido_\n\n");
    } else {
        out.push_str("```text\n");
        out.push_str(&inventory.tree());
        out.push_str("```\n\n");
    }

    out.push_str("## Perfil\n\n");
    out.push_str(&format!("- arquivos: {}\n", inventory.files.len()));
    out.push_str(&format!(
        "- bytes: {} ({} KB)\n",
        inventory.total_bytes,
        inventory.total_bytes / 1024
    ));
    if profile.is_empty() {
        out.push_str("- stacks: _nenhuma detectada_\n");
    } else {
        let names: Vec<&str> = profile.all.iter().map(|s| s.as_str()).collect();
        out.push_str(&format!("- stacks: {}", names.join(", ")));
        if !profile.flavors.is_empty() {
            out.push_str(&format!(" ({})", profile.flavors.join(", ")));
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "- extensões: {}\n",
        top_extensions(&inventory.by_extension)
    ));
    match findings {
        Some(brief) if brief.total > 0 => {
            out.push_str(&format!(
                "- achados do scan: {} ({})\n",
                brief.total, brief.by_severity
            ));
        }
        Some(_) => out.push_str("- achados do scan: nenhum\n"),
        None => out.push_str("- achados do scan: _sem `findings.json` — rode `kira scan`_\n"),
    }
    out.push('\n');

    out.push_str("## Arquivos-chave\n\n");
    if key_files.is_empty() {
        out.push_str("_nenhum README, manifesto ou entrypoint reconhecido_\n\n");
    } else {
        for (path, body) in key_files {
            out.push_str(&format!("### `{path}`\n\n"));
            out.push_str("```text\n");
            let mut lines = 0usize;
            for line in body.lines().take(MAX_KEY_FILE_LINES) {
                out.push_str(&format!("{line}\n"));
                lines += 1;
            }
            if lines == 0 {
                out.push('\n');
            }
            out.push_str("```\n\n");
        }
    }

    out.push_str("## Notas\n\n");
    let mut notes: Vec<&str> = Vec::new();
    notes.extend(pipeline_notes.iter().map(String::as_str));
    notes.extend(inventory.notes.iter().map(String::as_str));
    notes.extend(profile.notes.iter().map(String::as_str));
    if notes.is_empty() {
        out.push_str("_nada a reportar_\n");
    } else {
        for note in notes {
            out.push_str(&format!("- {note}\n"));
        }
    }

    if out.chars().count() > MAX_DIGEST_CHARS {
        let mut cut: String = out.chars().take(MAX_DIGEST_CHARS).collect();
        cut.push_str("\n\n_nota: digest cortado no teto de 200 000 caracteres_\n");
        return cut;
    }
    out
}

/// Top extensões por contagem, desempate alfabético (determinístico).
fn top_extensions(by_extension: &BTreeMap<String, usize>) -> String {
    let mut pairs: Vec<(&String, &usize)> = by_extension.iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let mut parts: Vec<String> = Vec::new();
    for (ext, count) in pairs.into_iter().take(10) {
        parts.push(format!("{ext} {count}"));
    }
    if parts.is_empty() {
        "_nenhuma_".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::inventory::InventoryFile;

    fn inv(paths: &[&str]) -> Inventory {
        let files: Vec<InventoryFile> = paths
            .iter()
            .map(|p| InventoryFile {
                path: (*p).to_string(),
                bytes: 10,
                truncated: false,
            })
            .collect();
        Inventory {
            project_name: "proj".to_string(),
            files,
            by_extension: BTreeMap::from([("rs".to_string(), 2)]),
            total_bytes: 20,
            notes: Vec::new(),
        }
    }

    #[test]
    fn key_files_priority_is_readme_then_manifest_then_entrypoint() {
        let files = vec![
            "src/main.rs".to_string(),
            "package.json".to_string(),
            "README.md".to_string(),
        ];
        let picked = select_key_files(&files);
        assert_eq!(picked, vec!["README.md", "package.json", "src/main.rs"]);
    }

    #[test]
    fn key_files_are_capped_and_sorted_for_ties() {
        let mut files: Vec<String> = (0..20).map(|i| format!("app{i}.csproj")).collect();
        files.push("README.md".to_string());
        let picked = select_key_files(&files);
        assert_eq!(picked.len(), MAX_KEY_FILES);
        assert_eq!(picked[0], "README.md");
    }

    #[test]
    fn render_is_byte_identical_across_calls() {
        let inventory = inv(&["README.md", "src/main.rs"]);
        let profile = StackProfile::default();
        let keys = vec![("README.md".to_string(), "# Proj\n".to_string())];
        let a = render("proj", &inventory, &profile, &keys, None, &[]);
        let b = render("proj", &inventory, &profile, &keys, None, &[]);
        assert_eq!(a, b);
        assert!(a.contains("## Árvore"), "{a}");
        assert!(a.contains("_sem `findings.json`"), "{a}");
    }

    #[test]
    fn render_never_emits_a_timestamp_or_absolute_path() {
        let inventory = inv(&["src/main.rs"]);
        let text = render(
            "proj",
            &inventory,
            &StackProfile::default(),
            &[],
            Some(&FindingsBrief {
                total: 2,
                by_severity: "high 2".to_string(),
            }),
            &["scan ausente".to_string()],
        );
        assert!(
            !text.contains("20:") && !text.contains("T00"),
            "sem hora: {text}"
        );
        assert!(!text.contains("/tmp/"), "sem caminho absoluto: {text}");
        assert!(text.contains("achados do scan: 2 (high 2)"), "{text}");
        assert!(text.contains("- scan ausente"), "{text}");
    }

    #[test]
    fn key_file_body_is_capped_at_the_line_limit() {
        let inventory = inv(&["src/main.rs"]);
        let body = (0..200)
            .map(|i| format!("linha {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let keys = vec![("src/main.rs".to_string(), body)];
        let text = render(
            "proj",
            &inventory,
            &StackProfile::default(),
            &keys,
            None,
            &[],
        );
        assert!(text.contains("linha 59"), "última linha do teto entra");
        assert!(!text.contains("linha 60"), "linha 61 em diante não entra");
    }
}
