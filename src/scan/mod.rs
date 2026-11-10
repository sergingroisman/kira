//! `kira scan` — orchestration of stack-standard audit tools (M1 part 1).
//!
//! KIRA does not analyze: it detects the stack of the folder, runs the
//! standard tool of each stack, normalizes one [`Finding`] per advisory and
//! writes a DETERMINISTIC `.kira/findings.json` (findings sorted by
//! fingerprint; the M1 gate: two runs on the same state produce the same
//! fingerprint set). Nothing here uses AI — the grounded triage comes later.
//!
//! Offline conveniences: every parse is a pure function of one `String`,
//! tested with captured-format fixtures; the CLI tests only touch the
//! "tool missing" and "no findings found" paths plus the deterministic file.

mod dotnet;
mod finding;
mod node;

pub use finding::Finding;
pub use node::NodeAuditTool;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::store::{self, display_path};

/// Errors of `kira scan`: compact and actionable (no raw body of tool output).
#[derive(Debug, Error)]
pub enum ScanError {
    /// No marker file found in the folder (`--path`) or its 2 levels.
    #[error(
        "nenhum stack detectado em '{path}' — rode o kira scan no diretório do projeto (com .csproj/.sln/global.json ou package.json) ou passe --path <dir>"
    )]
    NothingDetected { path: String },

    /// The `dotnet` executable is not on the PATH.
    #[error(
        "o scan .NET exige o SDK do .NET — baixe em https://dotnet.microsoft.com/download e rode de novo (ex.: winget install Microsoft.DotNet.SDK.8)"
    )]
    DotnetMissing,

    /// CWD unavailable and no `--path`.
    #[error("não foi possível obter o diretório atual — passe --path <dir> ao kira scan")]
    NoRoot,

    /// Missing npm/pnpm executable (message carries the fix).
    #[error("{message}")]
    ToolMissing { message: String },

    /// The tool exited non-zero and produced no parseable JSON — the raw
    /// output is never echoed; the message points to the manual run.
    #[error(
        "\"{tool}\" falhou (código {code}) no diretório '{dir}' — rode o comando à mão no diretório para ver o erro completo (o output cru foi omitido)"
    )]
    ToolFail {
        tool: &'static str,
        code: String,
        dir: String,
    },

    /// The tool answered something that is not valid JSON.
    #[error(
        "\"{tool}\" respondeu algo que não é JSON ({detail}) — rode o comando à mão para conferir e atualize a versão do {tool}"
    )]
    BadToolJson { tool: &'static str, detail: String },

    /// The local filesystem failed (create `.kira/`, read scopes, write...).
    #[error("falha de arquivo em '{path}' — verifique as permissões do diretório: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    /// Anything else from the runtime (never the raw tool body).
    #[error("{message}")]
    Runtime { message: String },
}

/// Flow scanned for stack: `dotnet`, `pnpm` or `npm`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StackKind {
    Dotnet,
    Pnpm,
    Npm,
}

impl StackKind {
    /// Stack label (the same one recorded in the stdout and in the trail).
    pub fn label(self) -> &'static str {
        match self {
            Self::Dotnet => "dotnet",
            Self::Pnpm => "pnpm",
            Self::Npm => "npm",
        }
    }
}

/// Stack detected in one scope of the scanned folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub kind: StackKind,
    /// Scope dir relative to the scan root (`"."` at the root itself).
    pub dir: String,
    /// Marker files (project manifests/locks) that detected this stack.
    pub markers: Vec<String>,
}

impl Stack {
    /// Scope dir relative to the scan root (`"."` at the root itself).
    fn scope_of(&self, root: &Path) -> PathBuf {
        if self.dir == "." {
            root.to_path_buf()
        } else {
            root.join(&self.dir)
        }
    }
}

/// Runs `kira scan [--path <dir>]`: detect stacks, run audits, normalize
/// findings and write the deterministic file. Returns the stdout text (whose
/// first line is also the session trail summary).
pub fn run(path_flag: Option<&Path>) -> Result<String, ScanError> {
    let root = resolve_root(path_flag)?;
    let stacks = detect_stacks(&root)?;
    let mut raw_findings = Vec::new();
    for stack in &stacks {
        let scope = stack.scope_of(&root);
        let more = match stack.kind {
            StackKind::Dotnet => dotnet::scan_dir(&scope),
            StackKind::Pnpm => node::scan_dir(node::NodeAuditTool::Pnpm, &scope),
            StackKind::Npm => node::scan_dir(node::NodeAuditTool::Npm, &scope),
        }?;
        raw_findings.extend(more);
    }
    let findings = finding::finalize(raw_findings);
    let file_path = write_findings_file(&root, &findings, &stacks)?;
    let stacks_text = stack_lines(&root, &stacks);
    let counts_text = finding::count_lines(&findings);
    let counts_brief = finding::severity_brief(&findings);
    let stack_labels = stacks
        .iter()
        .map(|stack| stack.kind.label())
        .collect::<Vec<_>>()
        .join(", ");
    // First line = session trail summary (stacks + counts), also useful
    // standalone (the history shows only it).
    Ok(format!(
        "kira scan em '{}' — stacks: {} | achados: {} ({}) | arquivo: {}\n{stacks_text}\nachados por severidade (contagem por ferramenta):\n{counts_text}",
        display_path(&root),
        stack_labels,
        findings.len(),
        counts_brief,
        display_path(&file_path)
    ))
}

/// Scan root: `--path` wins (canonicalized); the default detects the project
/// root like the store (`.git` > manifests > cwd) from the current directory.
fn resolve_root(path_flag: Option<&Path>) -> Result<PathBuf, ScanError> {
    match path_flag {
        Some(path) => path.canonicalize().map_err(|source| ScanError::Io {
            path: path.display().to_string(),
            source,
        }),
        None => match std::env::current_dir() {
            Ok(cwd) => Ok(store::detect_project_root_from(&cwd)),
            Err(_) => Err(ScanError::NoRoot),
        },
    }
}

/// Stack detection: the folder itself and up to 2 levels of subdirectories;
/// .NET on any `*.csproj`/`*.sln`/`global.json`; Node/TS on `package.json`
/// (pnpm when `pnpm-lock.yaml`, npm otherwise — including `package-lock.json`).
pub fn detect_stacks(root: &Path) -> Result<Vec<Stack>, ScanError> {
    let mut stacks = Vec::new();
    for scope in scopes_up_to_two_levels(root)? {
        let relative = scope.strip_prefix(root).map_or_else(
            |_| posix_slashes(&scope.to_string_lossy()),
            |rest| {
                if rest.as_os_str().is_empty() {
                    ".".to_string()
                } else {
                    posix_slashes(&rest.to_string_lossy())
                }
            },
        );
        let file_names = list_files(&scope)?;
        if let Some(stack) = dotnet_stack(&relative, &file_names) {
            stacks.push(stack);
        }
        if let Some(stack) = node_stack(&relative, &file_names) {
            stacks.push(stack);
        }
    }
    if stacks.is_empty() {
        return Err(ScanError::NothingDetected {
            path: display_path(root),
        });
    }
    // Deterministic order: relative dir, then stack kind (stable for gates).
    stacks.sort_by(|a, b| (a.dir.as_str(), a.kind.label()).cmp(&(b.dir.as_str(), b.kind.label())));
    Ok(stacks)
}

/// Scope dirs: root + level 1 + level 2, in lexicographic order.
fn scopes_up_to_two_levels(root: &Path) -> Result<Vec<PathBuf>, ScanError> {
    let mut scopes = vec![root.to_path_buf()];
    for child in subdirectories(root)? {
        scopes.push(child.clone());
        for grandchild in subdirectories(&child)? {
            scopes.push(grandchild);
        }
    }
    scopes.sort();
    Ok(scopes)
}

/// Existing subdirectories of `dir`, sorted by name (deterministic).
fn subdirectories(dir: &Path) -> Result<Vec<PathBuf>, ScanError> {
    let mut out = Vec::new();
    for entry in dir.read_dir().map_err(|source| ScanError::Io {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// File names (not dirs) of `dir`, sorted (deterministic markers).
fn list_files(dir: &Path) -> Result<Vec<String>, ScanError> {
    let mut names = Vec::new();
    for entry in dir.read_dir().map_err(|source| ScanError::Io {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| ScanError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// .NET stack of a scope: any `*.csproj`, `*.sln` or a `global.json` (SDK).
fn dotnet_stack(relative: &str, file_names: &[String]) -> Option<Stack> {
    let markers: Vec<String> = file_names
        .iter()
        .filter(|name| {
            name.ends_with(".csproj") || name.ends_with(".sln") || name.as_str() == "global.json"
        })
        .cloned()
        .collect();
    if markers.is_empty() {
        return None;
    }
    Some(Stack {
        kind: StackKind::Dotnet,
        dir: relative.to_string(),
        markers,
    })
}

/// Node/TS stack of a scope: `package.json` (pnpm with its lock, npm otherwise).
fn node_stack(relative: &str, file_names: &[String]) -> Option<Stack> {
    if !file_names.iter().any(|name| name == "package.json") {
        return None;
    }
    if file_names.iter().any(|name| name == "pnpm-lock.yaml") {
        return Some(Stack {
            kind: StackKind::Pnpm,
            dir: relative.to_string(),
            markers: vec!["package.json".to_string(), "pnpm-lock.yaml".to_string()],
        });
    }
    // npm: lock canonicamente se chama `package-lock.json`; `package.json`
    // sem nenhum lock também roda com o npm (o `audit` usa o que existir).
    Some(Stack {
        kind: StackKind::Npm,
        dir: relative.to_string(),
        markers: file_names
            .iter()
            .filter(|name| name.as_str() == "package.json" || name.as_str() == "package-lock.json")
            .cloned()
            .collect(),
    })
}

/// One line per stack `kind (dir) — markers`.
fn stack_lines(root: &Path, stacks: &[Stack]) -> String {
    stacks
        .iter()
        .map(|stack| {
            let scope = stack.scope_of(root);
            format!(
                "{} ({}) — {}",
                stack.kind.label(),
                display_path(&scope),
                stack.markers.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Writes `<root>/.kira/findings.json` (with the marker note that `.kira/` is
/// local — nothing here belongs in git) and returns the file path.
///
/// The file holds ONLY deterministic data (findings sorted by fingerprint and
/// stacks in detection order): no timestamp, no system path that moves —
/// two runs on the same state must be byte-identical (M1 gate).
fn write_findings_file(
    root: &Path,
    findings: &[Finding],
    stacks: &[Stack],
) -> Result<PathBuf, ScanError> {
    let kira_dir = root.join(".kira");
    std::fs::create_dir_all(&kira_dir).map_err(|source| ScanError::Io {
        path: kira_dir.display().to_string(),
        source,
    })?;
    let note = kira_dir.join("README.md");
    if !note.is_file() {
        std::fs::write(&note, ".kira/ é LOCAL: nunca comite este diretório (adicione ao .gitignore do projeto). findings.json guarda o resultado do último `kira scan`.\n")
            .map_err(|source| ScanError::Io {
                path: note.display().to_string(),
                source,
            })?;
    }
    let document = serde_json::json!({ "findings": findings, "stacks": stacks });
    let text = serde_json::to_string_pretty(&document).map_err(|source| ScanError::Runtime {
        message: format!("não foi possível serializar os findings: {source}"),
    })?;
    let file_path = kira_dir.join("findings.json");
    std::fs::write(&file_path, text + "\n").map_err(|source| ScanError::Io {
        path: file_path.display().to_string(),
        source,
    })?;
    Ok(file_path)
}

/// Exit code of one tool run as a compact label (`0`, `-1`, `desconhecido`).
pub(crate) fn exit_code_label(code: Option<i32>) -> String {
    code.map_or_else(|| "desconhecido".to_string(), |code| code.to_string())
}

/// Normalizes backslashes to forward slashes so path strings (stack dirs,
/// findings paths) look identical on every OS and stay stable in
/// `findings.json` — readability and determinism depend on this.
fn posix_slashes(text: &str) -> String {
    text.replace('\\', "/")
}

/// Path of one tool relation relative to the scan root: absolute paths under
/// the root become relative; relative ones stay as they came (trimmed). The
/// `path` value keeps the finding report usable without leaking the machine
/// layout when possible, and separators are normalized to `/`.
pub(crate) fn relative_path(root: &Path, raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let candidate = Path::new(trimmed);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    match joined.strip_prefix(root) {
        Ok(rest) if !rest.as_os_str().is_empty() => posix_slashes(&rest.to_string_lossy()),
        _ => posix_slashes(&joined.to_string_lossy()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("kira-scan-{}-{name}-{nanos}", std::process::id()))
    }

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), "").unwrap_or_else(|err| panic!("criar {name}: {err}"));
    }

    #[test]
    fn detect_dotnet_stack_from_csproj() {
        let dir = temp_dir("dotnet");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        touch(&dir, "App.csproj");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 1, "{stacks:?}");
        assert_eq!(stacks[0].kind, StackKind::Dotnet);
        assert_eq!(stacks[0].dir, ".");
        assert_eq!(stacks[0].markers, vec!["App.csproj".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_node_stacks_per_lockfile() {
        let dir = temp_dir("node");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        touch(&dir, "package.json");
        touch(&dir, "package-lock.json");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].kind, StackKind::Npm);

        touch(&dir, "pnpm-lock.yaml");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(
            stacks[0].kind,
            StackKind::Pnpm,
            "lock do pnpm vence: {stacks:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_json_without_any_lock_is_npm() {
        let dir = temp_dir("npm-only");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        touch(&dir, "package.json");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].kind, StackKind::Npm);
        assert_eq!(stacks[0].markers, vec!["package.json".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two-level recursion: `package.json` in a second-level scope is found.
    #[test]
    fn detects_scopes_two_levels_deep() {
        let dir = temp_dir("deep");
        let deep = dir.join("packages").join("app");
        std::fs::create_dir_all(&deep).unwrap_or_else(|err| panic!("criar {deep:?}: {err}"));
        touch(&deep, "package.json");
        touch(&dir, "global.json"); // .NET kesmark in the root, node in level 2
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 2, "{stacks:?}");
        assert_eq!(stacks[0].kind, StackKind::Dotnet);
        assert_eq!(stacks[0].dir, ".");
        assert_eq!(stacks[1].kind, StackKind::Npm);
        assert_eq!(stacks[1].dir, "packages/app");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_folder_error_is_actionable() {
        let dir = temp_dir("empty");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        let error = detect_stacks(&dir).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("nenhum stack detectado"), "{message}");
        assert!(
            message.contains("--path"),
            "erro deve sugerir o próximo passo: {message}"
        );
        assert!(message.contains(".csproj"), "{message}");
        assert!(message.contains("package.json"), "{message}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fnv_fingerprint_of_the_finding_matches_the_hand_rolled_hash() {
        // covered deeply in finding.rs; here only that the reexport works.
        let hex = finding::fingerprint("dotnet", "vulnerable", "Npgsql", "8.0.4", "msg");
        assert_eq!(hex.len(), 16);
    }

    #[test]
    fn relative_path_keeps_project_paths_readable() {
        let base = temp_dir("rel");
        std::fs::create_dir_all(&base).unwrap_or_else(|err| panic!("criar: {err}"));
        assert_eq!(relative_path(&base, "App.csproj"), "App.csproj");
        assert_eq!(relative_path(&base, ""), "");
        let inside = base.join("App.csproj");
        assert_eq!(
            relative_path(&base, inside.to_str().unwrap_or_default()),
            "App.csproj"
        );
        assert_eq!(
            relative_path(&base, "/fora/da/base/App.csproj"),
            "/fora/da/base/App.csproj"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn exit_code_label_is_compact() {
        assert_eq!(exit_code_label(Some(1)), "1");
        assert_eq!(exit_code_label(Some(-1)), "-1");
        assert_eq!(exit_code_label(None), "desconhecido");
    }
}
