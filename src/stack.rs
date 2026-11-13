//! Detector de stack por marcador (ADR-0038, M1).
//!
//! O KIRA precisa saber **o que é** o repositório antes de perfilar, escolher os
//! arquivos-chave do digest e decidir quais ferramentas de segurança fazem sentido.
//! A regra é marcador em arquivo, nunca extensão de arquivo: extensão diz que existe
//! um `.rs` perdido, marcador diz que o projeto **é** Rust.
//!
//! Determinismo: o resultado é uma lista ordenada por [`Stack`] (ordem fixa do enum),
//! independente da ordem de leitura do diretório.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Linguagens/ecossistemas reconhecidos. A ordem do enum é a ordem de saída.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stack {
    Rust,
    Node,
    Go,
    Dotnet,
    Java,
    Kotlin,
    Dart,
    Python,
    Ruby,
    Php,
    Swift,
    Terraform,
    Protobuf,
}

impl Stack {
    pub fn as_str(self) -> &'static str {
        match self {
            Stack::Rust => "rust",
            Stack::Node => "node",
            Stack::Go => "go",
            Stack::Dotnet => "dotnet",
            Stack::Java => "java",
            Stack::Kotlin => "kotlin",
            Stack::Dart => "dart",
            Stack::Python => "python",
            Stack::Ruby => "ruby",
            Stack::Php => "php",
            Stack::Swift => "swift",
            Stack::Terraform => "terraform",
            Stack::Protobuf => "protobuf",
        }
    }

    /// Ferramenta de auditoria de dependência que o `scan` tentaria para esta stack.
    /// `None` = não existe ferramenta padronizada; o digest registra isso como nota
    /// em vez de fingir cobertura.
    pub fn audit_tool(self) -> Option<&'static str> {
        match self {
            Stack::Node => Some("pnpm audit"),
            Stack::Dotnet => Some("dotnet list package --vulnerable"),
            Stack::Rust => Some("cargo audit"),
            Stack::Go => Some("govulncheck"),
            Stack::Python => Some("pip-audit"),
            Stack::Ruby => Some("bundler-audit"),
            _ => None,
        }
    }
}

/// Perfil de stack do projeto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProfile {
    /// Todas as stacks detectadas, ordenadas.
    pub all: Vec<Stack>,
    /// Refinamentos legíveis (`next`, `react-native`, `pnpm-workspace`, `terraform`).
    pub flavors: Vec<String>,
    /// O que não foi possível decidir, com o motivo.
    pub notes: Vec<String>,
}

impl StackProfile {
    /// A stack "principal": a primeira na ordem do enum, quando existe.
    pub fn primary(&self) -> Option<Stack> {
        self.all.first().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }
}

/// Marcadores de arquivo único: presença do nome ⇒ stack.
const MARKERS: &[(Stack, &str)] = &[
    (Stack::Rust, "Cargo.toml"),
    (Stack::Node, "package.json"),
    (Stack::Go, "go.mod"),
    (Stack::Dotnet, "global.json"),
    (Stack::Java, "pom.xml"),
    (Stack::Java, "build.gradle"),
    (Stack::Kotlin, "build.gradle.kts"),
    (Stack::Dart, "pubspec.yaml"),
    (Stack::Python, "pyproject.toml"),
    (Stack::Python, "requirements.txt"),
    (Stack::Python, "setup.py"),
    (Stack::Python, "Pipfile"),
    (Stack::Ruby, "Gemfile"),
    (Stack::Php, "composer.json"),
    (Stack::Swift, "Package.swift"),
];

/// Detecta a stack a partir dos nomes de arquivo na **raiz** do projeto.
///
/// Função pura de propósito: o teste monta o conjunto de nomes e exercita todas as
/// combinações sem tocar o disco.
pub fn detect_from_names(names: &BTreeSet<String>) -> StackProfile {
    let mut found: BTreeSet<Stack> = BTreeSet::new();
    let mut flavors: BTreeSet<String> = BTreeSet::new();
    let mut notes: Vec<String> = Vec::new();

    for (stack, marker) in MARKERS {
        if names.contains(*marker) {
            found.insert(*stack);
        }
    }
    // Sufixos: `.sln`/`.csproj` (.NET), `.xcodeproj` (Swift), `.tf` (Terraform), `.proto`.
    for name in names {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".sln") || lower.ends_with(".csproj") || lower.ends_with(".fsproj") {
            found.insert(Stack::Dotnet);
        }
        if lower.ends_with(".xcodeproj") || lower.ends_with(".xcworkspace") {
            found.insert(Stack::Swift);
        }
        if lower.ends_with(".tf") {
            found.insert(Stack::Terraform);
        }
        if lower.ends_with(".proto") {
            found.insert(Stack::Protobuf);
        }
        if lower == "settings.gradle" || lower == "settings.gradle.kts" {
            found.insert(Stack::Java);
        }
        if lower == "pnpm-workspace.yaml" || lower == "pnpm-lock.yaml" {
            flavors.insert("pnpm-workspace".to_string());
        }
    }

    if found.is_empty() {
        notes.push("nenhum marcador de stack na raiz — o digest perfila por extensão".to_string());
    }

    StackProfile {
        all: found.into_iter().collect(),
        flavors: flavors.into_iter().collect(),
        notes,
    }
}

/// Detecta a stack lendo a raiz do projeto (um nível, sem recursão).
pub fn detect(root: &Path) -> StackProfile {
    let mut names: BTreeSet<String> = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return StackProfile {
            all: Vec::new(),
            flavors: Vec::new(),
            notes: vec![format!("raiz ilegível: {}", root.display())],
        };
    };
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            names.insert(name.to_string());
        }
    }
    let mut profile = detect_from_names(&names);
    profile.flavors.extend(detect_node_flavors(root, &names));
    profile.flavors.sort();
    profile.flavors.dedup();
    profile
}

/// Refina Node em framework lendo `package.json` (`next`, `react-native`, `vite`, `nest`).
///
/// Falha em silêncio e sem nota de erro: `package.json` inválido é problema do `scan`,
/// e o perfil já tem a informação suficiente (`node`).
fn detect_node_flavors(root: &Path, names: &BTreeSet<String>) -> Vec<String> {
    if !names.contains("package.json") {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut deps: Vec<String> = Vec::new();
    for key in ["dependencies", "devDependencies"] {
        if let Some(map) = json.get(key).and_then(|v| v.as_object()) {
            deps.extend(map.keys().cloned());
        }
    }
    for (dep, flavor) in [
        ("next", "next"),
        ("react-native", "react-native"),
        ("expo", "expo"),
        ("vite", "vite"),
        ("@nestjs/core", "nest"),
        ("nuxt", "nuxt"),
        ("svelte", "svelte"),
        ("vue", "vue"),
    ] {
        if deps.iter().any(|d| d == dep) {
            out.push(flavor.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn rust_project_is_detected_by_cargo_toml() {
        let p = detect_from_names(&names(&["Cargo.toml", "src"]));
        assert_eq!(p.all, vec![Stack::Rust]);
        assert_eq!(p.primary(), Some(Stack::Rust));
    }

    #[test]
    fn go_and_dart_are_detected_without_extension_guessing() {
        let p = detect_from_names(&names(&["go.mod", "go.sum", "pubspec.yaml"]));
        assert_eq!(p.all, vec![Stack::Go, Stack::Dart]);
        assert!(p.notes.is_empty(), "notas: {:?}", p.notes);
    }

    #[test]
    fn dotnet_and_swift_come_from_suffixes() {
        let p = detect_from_names(&names(&["Minha.Solucao.sln", "App.csproj", "App.xcodeproj"]));
        assert_eq!(p.all, vec![Stack::Dotnet, Stack::Swift]);
    }

    #[test]
    fn monorepo_with_many_markers_keeps_enum_order() {
        let p = detect_from_names(&names(&["Cargo.toml", "package.json", "go.mod", "pnpm-workspace.yaml"]));
        assert_eq!(p.all, vec![Stack::Rust, Stack::Node, Stack::Go]);
        assert!(p.flavors.contains(&"pnpm-workspace".to_string()));
    }

    #[test]
    fn empty_root_reports_a_note_instead_of_guessing() {
        let p = detect_from_names(&names(&["README.md"]));
        assert!(p.is_empty());
        assert_eq!(p.notes.len(), 1);
    }

    #[test]
    fn node_flavors_are_read_from_package_json() {
        let dir = std::env::temp_dir().join(format!("kira-stack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");
        std::fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"next":"15.0.0","react":"19.0.0"}}"#,
        )
        .expect("package.json");
        let p = detect(&dir);
        assert_eq!(p.all, vec![Stack::Node]);
        assert!(p.flavors.contains(&"next".to_string()), "flavors: {:?}", p.flavors);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_tool_is_honest_about_missing_coverage() {
        assert_eq!(Stack::Node.audit_tool(), Some("pnpm audit"));
        assert_eq!(Stack::Dart.audit_tool(), None);
        assert_eq!(Stack::Terraform.audit_tool(), None);
    }
}
