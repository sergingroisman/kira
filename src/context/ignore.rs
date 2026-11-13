//! Fonte única do que a caminhada **não** ingere (ADR-0038, M1).
//!
//! Antes disto existiam três listas independentes: `SKIPPED_DIRS` (14 nomes) para o
//! grep, a mesma lista chamada pelo índice FTS5, e uma allowlist de 34 extensões.
//! Consequências reais de cada uma:
//!
//! - `packages` estava na lista de diretórios pulados, mas monorepo pnpm/npm guarda
//!   código legítimo ali — o retrieval ficava cego para o monorepo inteiro;
//! - a allowlist decidia por extensão, então stack fora dela (`.dart`, `.vue`,
//!   `.proto`, `.mts`, `.gradle.kts`, `.tf`, `.razor`) não existia para o KIRA;
//! - a regra "diretório começando com ponto é ignorado" valia também para a **raiz**
//!   da caminhada, então um projeto chamado `build` ou que mora em `~/.config/x`
//!   produzia índice vazio, em silêncio.
//!
//! Aqui a decisão é **denylist**, como no gitingest: ingere-se tudo que não é lixo
//! conhecido, binary sniff confirma o resto, e o que passa do teto de tamanho entra
//! **com nota** (nunca é descartado em silêncio).

/// Teto de leitura por arquivo da **ingestão** (índice e digest). O grep mantém o
/// próprio teto menor, porque ali cada byte vira token de prompt.
pub(crate) const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Diretórios que carregam contexto apesar do ponto inicial.
///
/// A regra do ponto existe para `.git`, `.venv`, `.next` — não para CI/CD. Pipeline,
/// devcontainer e templates de issue são contexto legítimo do projeto.
const DOT_DIRS_ALLOWED: &[&str] = &[
    ".github",
    ".gitlab",
    ".devcontainer",
    ".azuredevops",
    ".circleci",
];

/// Padrões de nome/path ignorados. Curado por linguagem a partir do
/// `DEFAULT_IGNORE_PATTERNS` do gitingest (MIT, https://github.com/coderamp-labs/gitingest),
/// com as entradas próprias do KIRA no fim. `*` e `?` são globs sobre o **nome**;
/// padrões com `/` são comparados com o caminho relativo (`vendor/bundle`).
pub(crate) const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    // --- VCS, editores, SO ---
    ".git",
    ".hg",
    ".svn",
    ".gitignore",
    ".gitattributes",
    ".gitmodules",
    ".idea",
    ".fleet",
    ".vscode",
    ".vs",
    "*.swp",
    "*.swo",
    "*~",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    // --- Python ---
    "__pycache__",
    "*.pyc",
    "*.pyo",
    "*.pyd",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".nox",
    ".hypothesis",
    ".coverage",
    "htmlcov",
    ".venv",
    "venv",
    "env",
    ".env",
    "*.egg-info",
    ".eggs",
    "poetry.lock",
    "Pipfile.lock",
    "uv.lock",
    // --- JavaScript / TypeScript ---
    "node_modules",
    "bower_components",
    ".npm",
    ".yarn",
    ".pnpm-store",
    ".pnp.js",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lock",
    "bun.lockb",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".parcel-cache",
    ".cache",
    ".output",
    "dist",
    "build",
    "out",
    "coverage",
    ".nyc_output",
    "storybook-static",
    "*.map",
    "*.min.js",
    "*.min.css",
    // --- Java / Kotlin / Android ---
    "*.class",
    "*.jar",
    "*.war",
    "*.ear",
    "*.nar",
    ".gradle",
    ".settings",
    ".classpath",
    ".project",
    "gradle-app.setting",
    "local.properties",
    "*.hprof",
    // --- C / C++ / binários ---
    "*.o",
    "*.obj",
    "*.so",
    "*.dll",
    "*.dylib",
    "*.exe",
    "*.lib",
    "*.a",
    "*.pdb",
    "*.bin",
    "*.out",
    "*.elf",
    "*.wasm",
    // --- Swift / Xcode ---
    ".build",
    "*.xcodeproj",
    "*.xcworkspace",
    "*.pbxuser",
    "*.mode1v3",
    "*.mode2v3",
    "*.perspectivev3",
    "*.xcuserstate",
    "xcuserdata",
    ".swiftpm",
    "DerivedData",
    "Pods",
    // --- Ruby ---
    "*.gem",
    ".bundle",
    "vendor/bundle",
    "Gemfile.lock",
    ".ruby-version",
    ".ruby-gemset",
    // --- Rust ---
    "Cargo.lock",
    "target",
    "*.rs.bk",
    // --- Go ---
    "vendor",
    "*.test",
    "*.prof",
    // --- .NET ---
    "obj",
    "bin",
    "TestResults",
    "*.suo",
    "*.user",
    "*.userosscache",
    "*.sln.docstates",
    "*.nupkg",
    "*.snupkg",
    // --- Dart / Flutter ---
    ".dart_tool",
    ".pub-cache",
    ".flutter-plugins",
    ".flutter-plugins-dependencies",
    // --- Terraform / infra ---
    ".terraform",
    "*.tfstate",
    "*.tfstate.*",
    "crash.log",
    "*.tfvars",
    // --- mídia e dados (binary sniff confirma) ---
    "*.png",
    "*.jpg",
    "*.jpeg",
    "*.gif",
    "*.ico",
    "*.webp",
    "*.bmp",
    "*.pdf",
    "*.zip",
    "*.gz",
    "*.tar",
    "*.7z",
    "*.rar",
    "*.mp4",
    "*.mov",
    "*.mp3",
    "*.wav",
    "*.woff",
    "*.woff2",
    "*.ttf",
    "*.eot",
    "*.otf",
    "*.db",
    "*.sqlite",
    "*.sqlite3",
    "*.db-wal",
    "*.db-shm",
    // --- ruído geral ---
    "*.log",
    "*.tmp",
    "*.bak",
    ".terraform.lock.hcl",
    // --- KIRA: o que é artefato do próprio produto, não fonte ---
    ".kira",
];

/// `true` quando o **nome** do diretório está na denylist ou obedece a regra do ponto.
///
/// O chamador deve aplicar isto somente a `depth > 0`: a raiz da caminhada é sempre
/// ingerida, mesmo que se chame `build`, `bin` ou comece com ponto (ADR-0038).
pub fn is_ignored_dir(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.starts_with('.') {
        return !DOT_DIRS_ALLOWED.contains(&name);
    }
    DEFAULT_IGNORE_PATTERNS
        .iter()
        .any(|p| matches_pattern(p, name))
}

/// `true` quando o **nome** do arquivo está na denylist (locks, bundles, mídia, minificados).
///
/// Extensão desconhecida **não** é motivo para ignorar: é assim que stack nova entra
/// sem atualizar lista nenhuma.
pub fn is_ignored_file(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    DEFAULT_IGNORE_PATTERNS
        .iter()
        .any(|p| matches_pattern(p, name))
}

/// Glifo glob mínimo sobre o **nome**: `*` (qualquer sequência), `?` (um caractere).
/// Um padrão com `/` é tratado como sufixo de caminho relativo (`vendor/bundle`).
pub(crate) fn matches_pattern(pattern: &str, name: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return matches_pattern(suffix, name);
    }
    if pattern.contains('/') {
        return name == pattern || name.ends_with(&format!("/{pattern}"));
    }
    glob(pattern, name)
}

/// Matcher recursivo de `*`/`?`, sem dependência (ADR-0023: Rust puro).
fn glob(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    // Programação dinâmica por linha: `reach[j]` = padrão p[..i] casou com n[..j].
    let mut reach = vec![false; n.len() + 1];
    reach[0] = true;
    for (i, pc) in p.iter().enumerate() {
        let mut next = vec![false; n.len() + 1];
        match pc {
            '*' => {
                // `*` casa zero ou mais: carrega o alcance da esquerda.
                let mut any = false;
                for j in 0..=n.len() {
                    any = any || reach[j];
                    next[j] = any;
                }
            }
            '?' => {
                next[1..=n.len()].copy_from_slice(&reach[0..n.len()]);
            }
            literal => {
                let literal = literal.to_ascii_lowercase();
                for j in 1..=n.len() {
                    next[j] = reach[j - 1] && n[j - 1].to_ascii_lowercase() == literal;
                }
            }
        }
        let _ = i;
        reach = next;
    }
    reach[n.len()]
}

/// `true` quando a amostra inicial do arquivo parece binária.
///
/// Um NUL no começo já basta: texto não tem NUL, e é exatamente o que
/// `read_text_file` do grep fazia antes de existir uma regra só.
pub fn looks_binary(sample: &[u8]) -> bool {
    sample.contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_patterns_ignores_build_output_and_keeps_source() {
        for name in [
            "node_modules",
            "target",
            "dist",
            "out",
            "obj",
            "Pods",
            "vendor",
        ] {
            assert!(is_ignored_dir(name), "{name} deveria ser ignorado");
        }
        for name in ["src", "packages", "app", "lib", "crates", "internal"] {
            assert!(!is_ignored_dir(name), "{name} NÃO deveria ser ignorado");
        }
    }

    /// Regressão do achado de M1: `packages/` era pulado e escondia monorepo inteiro.
    #[test]
    fn packages_is_walked_again() {
        assert!(!is_ignored_dir("packages"));
        assert!(!is_ignored_dir("packages/core"));
    }

    #[test]
    fn dot_directories_are_ignored_except_ci_ones() {
        for name in [".git", ".venv", ".next", ".kira", ".idea"] {
            assert!(is_ignored_dir(name), "{name} deveria ser ignorado");
        }
        for name in [".github", ".gitlab", ".devcontainer", ".circleci"] {
            assert!(!is_ignored_dir(name), "{name} é contexto e deveria entrar");
        }
    }

    #[test]
    fn lockfiles_and_bundles_are_ignored_but_manifests_are_not() {
        for name in [
            "package-lock.json",
            "yarn.lock",
            "Cargo.lock",
            "app.min.js",
            "bundle.js.map",
        ] {
            assert!(is_ignored_file(name), "{name} deveria ser ignorado");
        }
        for name in [
            "package.json",
            "Cargo.toml",
            "go.mod",
            "pubspec.yaml",
            "requirements.txt",
            "main.dart",
        ] {
            assert!(!is_ignored_file(name), "{name} é fonte e deveria entrar");
        }
    }

    #[test]
    fn unknown_extensions_are_not_ignored() {
        // A denylist não decide por extensão conhecida: stack nova entra de graça.
        for name in [
            "schema.thrift",
            "model.onnx",
            "page.winx",
            "build.gradle.kts",
            "main.mts",
        ] {
            assert!(
                !is_ignored_file(name),
                "{name} não deveria estar na denylist"
            );
        }
    }

    #[test]
    fn glob_handles_star_and_question_mark() {
        assert!(glob("*.pyc", "a.pyc"));
        assert!(glob("*.pyc", ".pyc"));
        assert!(!glob("*.pyc", "a.py"));
        assert!(glob("file?txt", "file1txt"));
        assert!(!glob("file?txt", "file12txt"));
        assert!(glob("*", "qualquer"));
    }

    #[test]
    fn path_patterns_match_the_tail_of_the_relative_path() {
        assert!(matches_pattern("vendor/bundle", "vendor/bundle"));
        assert!(matches_pattern("vendor/bundle", "app/vendor/bundle"));
        assert!(!matches_pattern("vendor/bundle", "vendor/bundles"));
    }

    #[test]
    fn binary_sniff_detects_nul_byte() {
        assert!(looks_binary(b"\x7fELF\x00\x01"));
        assert!(looks_binary(b"\x00"));
        assert!(!looks_binary(b"fn main() {}\n"));
        assert!(!looks_binary("acentuação ção".as_bytes()));
    }
}
