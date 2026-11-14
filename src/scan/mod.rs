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

mod decode;
mod dotnet;
mod finding;
mod node;

pub use finding::Finding;
pub use finding::severity_brief;
pub use node::NodeAuditTool;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::binresolve::{find_on_path, is_cmd_script};
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

    /// The tool ran and exited non-zero (or an audit exited non-zero with
    /// unparseable JSON): compact and actionable, never the raw body — at
    /// most the FIRST line of stderr (trimmed, ≤200 chars) as a hint.
    #[error(
        "a ferramenta {tool} falhou (exit {exit}) — confira versão/instalação e rode kira scan de novo{hint}"
    )]
    ToolFailure {
        tool: &'static str,
        exit: String,
        hint: String,
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
        #[source]
        source: std::io::Error,
    },

    /// A scan worker thread died without producing an error (panic). Rare by
    /// construction (workers only spawn processes and parse JSON); the message
    /// says what to do instead of hiding the crash.
    #[error(
        "uma thread do scan morreu sem deixar erro — rode o kira scan de novo; se persistir, reporte com o comando completo e o stack detectado"
    )]
    WorkerPanic,

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
    /// Workspace members folded into this stack (pnpm/npm workspaces): a
    /// `package.json` without its own lockfile under a workspace root is NOT
    /// a stack — the root's single `audit` run already covers it. Dirs are
    /// relative to the scan root and sorted; omitted from the JSON when empty
    /// so non-workspace `findings.json` files stay byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
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

/// Which tool the scan invokes, for command construction, binary overrides
/// and the actionable "install X" error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolKind {
    Dotnet,
    Npm,
    Pnpm,
}

impl ToolKind {
    /// Default executable on the PATH (no `KIRA_SCAN_*_BIN` set).
    fn program(self) -> &'static str {
        match self {
            Self::Dotnet => "dotnet",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
        }
    }

    /// Env var (`KIRA_SCAN_<TOOL>_BIN`) that overrides the binary: value is
    /// the FULL path of an executable/shim. Documents CI without the tool,
    /// custom packaging and offline tests.
    pub fn bin_env(self) -> &'static str {
        match self {
            Self::Dotnet => "KIRA_SCAN_DOTNET_BIN",
            Self::Npm => "KIRA_SCAN_NPM_BIN",
            Self::Pnpm => "KIRA_SCAN_PNPM_BIN",
        }
    }

    /// Actionable error of a tool that could not be spawned because it does
    /// not exist ("instale o X"), not a tool that ran and failed.
    fn missing_error(self) -> ScanError {
        match self {
            Self::Dotnet => ScanError::DotnetMissing,
            Self::Npm => ScanError::ToolMissing {
                message:
                    "o scan Node/TS via npm exige o npm — instale em https://nodejs.org/download e rode de novo"
                        .to_string(),
            },
            Self::Pnpm => ScanError::ToolMissing {
                message:
                    "o scan Node/TS via pnpm exige o pnpm — instale com `npm install -g pnpm` (ou `corepack enable pnpm`) e rode de novo"
                        .to_string(),
            },
        }
    }
}

/// The resolved tool binary: FULL path (the spawn never uses the bare name)
/// plus how to spawn it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Resolved {
    pub path: PathBuf,
    /// Spawn wrapped in `cmd /C chcp 65001 >nul 2>&1 & <path>`: Windows only
    /// and only for `.cmd`/`.bat` (the std would try to exec them as
    /// binaries). The `chcp` stays: it fixes the MOJIBAKE of a REAL tool's
    /// output, not only of the wrapper's own text.
    pub via_cmd: bool,
}

/// Resolves the tool binary BEFORE spawning (`which`-style, pure Rust — no
/// OS-localized text is ever parsed; text-based classification of the OS is
/// PROHIBITED because cmd output is localized and may arrive mojibaked).
///
/// 1. `KIRA_SCAN_<TOOL>_BIN` present and non-empty: the user's contract —
///    the pointed path spawns as-is (absence of the file is the same
///    actionable "instale o X").
/// 2. No override: search the PATH — Windows: each PATH dir × PATHEXT
///    (default `.COM/.EXE/.BAT/.CMD`, case-insensitive lookup as NTFS is);
///    Linux/macOS: a file named `<tool>` with the execute bit set.
/// 3. Nothing found => [`ToolKind::missing_error`] with NO spawn at all.
pub(crate) fn resolve_tool(kind: ToolKind) -> Result<Resolved, ScanError> {
    let override_bin = override_bin(kind);
    let path_env = std::env::var_os("PATH");
    resolve_tool_explicit(kind, override_bin.as_deref(), path_env.as_deref())
}

/// `KIRA_SCAN_<TOOL>_BIN` value (trimmed), ignoring empty/absent.
fn override_bin(kind: ToolKind) -> Option<String> {
    std::env::var(kind.bin_env())
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Pure core of [`resolve_tool`] (unit-testable without env mutation).
fn resolve_tool_explicit(
    kind: ToolKind,
    override_bin: Option<&str>,
    path_env: Option<&std::ffi::OsStr>,
) -> Result<Resolved, ScanError> {
    if let Some(bin) = override_bin {
        let path = PathBuf::from(bin);
        if !path.is_file() {
            return Err(kind.missing_error());
        }
        return Ok(Resolved {
            via_cmd: cfg!(windows) && is_cmd_script(bin),
            path,
        });
    }
    match find_on_path(path_env, kind.program()) {
        Some(path) => Ok(Resolved {
            via_cmd: cfg!(windows) && is_cmd_script(&path.to_string_lossy()),
            path,
        }),
        None => Err(kind.missing_error()),
    }
}

/// Builds the command that runs the tool of `kind` (callers add their own
/// args and `current_dir`, never touching the program): resolution happens
/// FIRST and the spawn always uses the RESOLVED full path.
pub(crate) fn tool_command(kind: ToolKind, args: &[&str]) -> Result<Command, ScanError> {
    let resolved = resolve_tool(kind)?;
    Ok(command_from_resolved(&resolved, args))
}

/// Pure command builder over one [`Resolved`] (unit-testable without env):
/// `.cmd`/`.bat` on Windows go via `cmd /C chcp 65001 …` (the std never
/// resolves them and the codepage must stay UTF-8 for real tool output),
/// everything else spawns directly.
fn command_from_resolved(resolved: &Resolved, args: &[&str]) -> Command {
    if resolved.via_cmd {
        // `cmd /C` recebe UMA linha, com os argumentos dentro dela. Antes, o
        // `chcp ... &` ia como argumento separado e o std o citava inteiro: o
        // `cmd` lia tudo como nome de programa e o scan morria com
        // "The filename, directory name, or volume label syntax is incorrect."
        // (bug real, reproduzido no CI do windows-latest). Os argumentos
        // também precisam entrar na linha: `Command::arg` depois do `raw_arg`
        // não tem efeito.
        let owned: Vec<std::ffi::OsString> = args.iter().map(std::ffi::OsString::from).collect();
        let line = crate::shellline::cmd_command_line(&resolved.path, &owned);
        let mut command = Command::new("cmd");
        command.arg("/C");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.raw_arg(line);
        }
        #[cfg(not(windows))]
        {
            // Inalcançável (via_cmd só é true no Windows), mas mantém o build
            // honesto em outros alvos e nos testes.
            let _ = line;
            command.arg(&resolved.path);
            command.args(args);
        }
        command
    } else {
        let mut command = Command::new(&resolved.path);
        command.args(args);
        command
    }
}

/// Maps a failed spawn to the actionable error: never-confuse "could not run
/// the tool" with "tool is missing" — only a real NotFound (tool/nonexistent
/// binary) is "instale o X"; anything else stays a compact runtime message
/// (no raw OS noise about the tool internals).
pub(crate) fn spawn_error(kind: ToolKind, source: std::io::Error) -> ScanError {
    if matches!(source.kind(), std::io::ErrorKind::NotFound) {
        kind.missing_error()
    } else {
        ScanError::Runtime {
            message: format!("não foi possível executar o {}: {source}", kind.program()),
        }
    }
}

/// Compact hint for the [`ScanError::ToolFailure`] message: first non-empty
/// line of the tool stderr, trimmed, control chars removed, capped at 200
/// chars. Empty stderr => empty string (no hint at all). Never the raw body.
pub(crate) fn stderr_hint(stderr: &str) -> String {
    let hint = crate::binresolve::first_stderr_line(stderr);
    if hint.is_empty() {
        String::new()
    } else {
        format!(" (dica: {hint})")
    }
}

/// Runs `kira scan [--path <dir>]`: detect stacks, run audits, normalize
/// findings and write the deterministic file. Returns the stdout text (whose
/// first line is also the session trail summary).
pub fn run(path_flag: Option<&Path>) -> Result<String, ScanError> {
    let root = resolve_root(path_flag)?;
    let stacks = detect_stacks(&root)?;
    let raw_findings = scan_stacks(&stacks, &root)?;
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

/// Audita um stack (processo externo: dotnet/npm/pnpm) num escopo de diretório.
fn scan_stack(stack: &Stack, root: &Path) -> Result<Vec<Finding>, ScanError> {
    let scope = stack.scope_of(root);
    match stack.kind {
        StackKind::Dotnet => dotnet::scan_dir(&scope),
        StackKind::Pnpm => node::scan_dir(node::NodeAuditTool::Pnpm, &scope),
        StackKind::Npm => node::scan_dir(node::NodeAuditTool::Npm, &scope),
    }
}

/// Audita todos os stacks detectados.
///
/// Por que thread aqui (e só aqui): cada scan passa a maior parte do tempo
/// ESPERANDO I/O do processo da ferramenta (dotnet/npm levam segundos), então
/// com 2+ stacks o wall-time cai pela metade com `std::thread::scope` — sem
/// rayon, sem dependência nova (auditoria P1, "multithread na dose certa").
/// Com 1 stack roda inline: thread para esperar um processo só é cerimônia.
///
/// Determinismo intacto: os resultados são agregados NA ORDEM das stacks
/// (que já é ordenada em `detect_stacks`) e o `finding::finalize` reordena por
/// fingerprint — mesma entrada ⇒ mesmos bytes em `findings.json`. O primeiro
/// erro na ordem das stacks vence, o mesmo contrato do antigo loop sequencial.
fn scan_stacks(stacks: &[Stack], root: &Path) -> Result<Vec<Finding>, ScanError> {
    if stacks.len() < 2 {
        let mut out = Vec::new();
        for stack in stacks {
            out.extend(scan_stack(stack, root)?);
        }
        return Ok(out);
    }
    let results: Vec<Result<Vec<Finding>, ScanError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = stacks
            .iter()
            .map(|stack| scope.spawn(move || scan_stack(stack, root)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or(Err(ScanError::WorkerPanic)))
            .collect()
    });
    let mut out = Vec::new();
    for result in results {
        out.extend(result?);
    }
    Ok(out)
}

/// Contagem de achados LIDA de volta do summary que o próprio `kira scan`
/// gravou na trilha (`… | achados: 7 (critical 1, high 2) | arquivo: …`).
///
/// O parser mora ao lado de quem ESCREVE o formato, de propósito: `findings.json`
/// é sobrescrito a cada execução, então a trilha (`sessions.summary`) é a única
/// memória histórica do scan — e esse texto é contrato NOSSO, não saída de
/// ferramenta de terceiro (que exigiria fixture de versão).
///
/// Qualquer linha fora do formato devolve `None`: o dashboard omite o ponto da
/// série em vez de desenhar `0`, que seria inventar uma queda que não houve.
pub fn findings_count_in_summary(summary: &str) -> Option<usize> {
    let first_line = summary.lines().next()?;
    let after = first_line.split_once("| achados: ")?.1;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<usize>().ok()
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
///
/// Workspace rule (W2-fix PR-F1): a `package.json` WITHOUT a lockfile of its
/// own whose ancestor (inside the scan) is a workspace root — `pnpm-lock.yaml`,
/// `pnpm-workspace.yaml` or a root `package.json` with a `workspaces` field —
/// is a MEMBER of that workspace, never an npm project: no `audit` runs on it,
/// because the root's single `pnpm|npm audit` already covers the workspace.
pub fn detect_stacks(root: &Path) -> Result<Vec<Stack>, ScanError> {
    let scopes = scopes_up_to_two_levels(root)?;
    // File names of every scope, listed once: dotnet and node share it.
    let mut per_scope: Vec<(String, Vec<String>)> = Vec::with_capacity(scopes.len());
    for scope in &scopes {
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
        per_scope.push((relative, list_files(scope)?));
    }
    let mut stacks = Vec::new();
    for (relative, file_names) in &per_scope {
        if let Some(stack) = dotnet_stack(relative, file_names) {
            stacks.push(stack);
        }
    }
    stacks.extend(node_stacks(&scopes, &per_scope));
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

/// Case-insensitive marker test on purpose: NTFS is case-insensitive, so
/// `App.CSPROJ`, `MinhaSolucao.SLN` and `GLOBAL.JSON` are valid on Windows and
/// must be detected as .NET markers — a byte-wise comparison silently skipped
/// `dotnet` tooling on those projects.
fn is_dotnet_marker(name: &str) -> bool {
    if name.eq_ignore_ascii_case("global.json") {
        return true;
    }
    match std::path::Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
    {
        Some(ext) => ext.eq_ignore_ascii_case("csproj") || ext.eq_ignore_ascii_case("sln"),
        None => false,
    }
}

/// .NET stack of a scope: any `*.csproj`, `*.sln` or a `global.json` (SDK).
fn dotnet_stack(relative: &str, file_names: &[String]) -> Option<Stack> {
    let markers: Vec<String> = file_names
        .iter()
        .filter(|name| is_dotnet_marker(name))
        .cloned()
        .collect();
    if markers.is_empty() {
        return None;
    }
    Some(Stack {
        kind: StackKind::Dotnet,
        dir: relative.to_string(),
        markers,
        members: Vec::new(),
    })
}

/// Node/TS stacks of every scope, folding workspace members into their root.
///
/// A scope with `package.json` is:
///
/// - a workspace ROOT — and its own stack — when it also has `pnpm-lock.yaml`,
///   `pnpm-workspace.yaml` or a `workspaces` field in `package.json`; pnpm wins
///   the kind when either pnpm marker is present, npm otherwise;
/// - a MEMBER of the nearest ancestor workspace root when it has NO lockfile
///   of its own — it folds into the root's `members` and never spawns a stack
///   or an `audit` call (the root's single run covers the workspace);
/// - a standalone npm project otherwise (the `audit` uses what exists).
fn node_stacks(scopes: &[PathBuf], per_scope: &[(String, Vec<String>)]) -> Vec<Stack> {
    let is_workspace_root: Vec<bool> = scopes
        .iter()
        .zip(per_scope)
        .map(|(scope, (_, files))| {
            files.iter().any(|name| name == "package.json")
                && (files
                    .iter()
                    .any(|name| name == "pnpm-lock.yaml" || name == "pnpm-workspace.yaml")
                    || package_json_has_workspaces(scope))
        })
        .collect();

    let mut stacks: Vec<Stack> = Vec::new();
    let mut members: Vec<Vec<String>> = vec![Vec::new(); scopes.len()];
    for (index, (relative, files)) in per_scope.iter().enumerate() {
        if !files.iter().any(|name| name == "package.json") {
            continue;
        }
        let has_pnpm_lock = files.iter().any(|name| name == "pnpm-lock.yaml");
        let has_npm_lock = files.iter().any(|name| name == "package-lock.json");
        if !has_pnpm_lock && !has_npm_lock && !is_workspace_root[index] {
            // Sem lock próprio: membro do workspace do ancestor, se houver um.
            if let Some(root_index) = nearest_workspace_root(scopes, &is_workspace_root, index) {
                members[root_index].push(relative.clone());
                continue;
            }
        }
        let kind = if has_pnpm_lock
            || (files.iter().any(|name| name == "pnpm-workspace.yaml") && !has_npm_lock)
        {
            StackKind::Pnpm
        } else {
            StackKind::Npm
        };
        stacks.push(Stack {
            kind,
            dir: relative.to_string(),
            markers: files
                .iter()
                .filter(|name| {
                    matches!(
                        name.as_str(),
                        "package.json"
                            | "package-lock.json"
                            | "pnpm-lock.yaml"
                            | "pnpm-workspace.yaml"
                    )
                })
                .cloned()
                .collect(),
            members: Vec::new(),
        });
    }
    // Attach the folded members to their roots, sorted (deterministic).
    for stack in &mut stacks {
        let Some(index) = per_scope.iter().position(|(dir, _)| dir == &stack.dir) else {
            continue;
        };
        members[index].sort();
        stack.members = std::mem::take(&mut members[index]);
    }
    stacks
}

/// Nearest ancestor of `scopes[index]` that is a workspace root (never the
/// scope itself); `None` outside workspaces.
fn nearest_workspace_root(
    scopes: &[PathBuf],
    is_workspace_root: &[bool],
    index: usize,
) -> Option<usize> {
    let mut ancestor = scopes[index].parent();
    while let Some(dir) = ancestor {
        if let Some(candidate) = scopes.iter().position(|scope| scope == dir)
            && is_workspace_root[candidate]
        {
            return Some(candidate);
        }
        ancestor = dir.parent();
    }
    None
}

/// Does this scope's `package.json` declare a `workspaces` field (monorepo
/// root marker)? An unreadable or invalid manifest answers "no" — detection
/// must never fail on a file the other markers can still classify.
fn package_json_has_workspaces(scope: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(scope.join("package.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    value.get("workspaces").is_some()
}

/// One line per stack `kind (dir) — markers`, plus ` — members: a, b` when the
/// stack folds workspace members.
fn stack_lines(root: &Path, stacks: &[Stack]) -> String {
    stacks
        .iter()
        .map(|stack| {
            let scope = stack.scope_of(root);
            let mut line = format!(
                "{} ({}) — {}",
                stack.kind.label(),
                display_path(&scope),
                stack.markers.join(", ")
            );
            if !stack.members.is_empty() {
                line.push_str(&format!(" — members: {}", stack.members.join(", ")));
            }
            line
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

/// Reads the last `.kira/findings.json` of the project.
///
/// `Ok(None)` when the file does not exist (no scan has run yet) — never an
/// error for the caller that only wants context. The file is written by
/// [`run`] and carries only deterministic data, so this stays a pure parser
/// over one `String` plus a read.
pub fn read_findings_file(root: &Path) -> Result<Option<Vec<Finding>>, ScanError> {
    let path = root.join(".kira").join("findings.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ScanError::Io {
                path: path.display().to_string(),
                source,
            });
        }
    };
    Ok(Some(parse_findings_document(&text)?))
}

/// Extracts the findings from the `{ findings, stacks }` document (the
/// `stacks` half is irrelevant here and ignored on purpose).
fn parse_findings_document(text: &str) -> Result<Vec<Finding>, ScanError> {
    #[derive(Deserialize)]
    struct Document {
        findings: Vec<Finding>,
    }
    let document: Document = serde_json::from_str(text).map_err(|source| ScanError::Runtime {
        message: format!("findings.json ilegível ({source}) — rode `kira scan` de novo"),
    })?;
    Ok(document.findings)
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
    // Rooted paths (absolute, or drive-less `/…`/`\…` on Windows) must NOT be
    // joined: on Windows `root.join("/x")` re-anchors to the current drive and
    // silently changes the path. They leave as they came.
    let joined = if candidate.has_root() {
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

    /// Reading back the file `kira scan` writes: absent file is `None`, and a
    /// present one yields exactly the findings (stacks half ignored).
    #[test]
    fn findings_file_is_read_back_or_absent_without_error() {
        let dir = temp_dir("findings-read");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        assert!(
            read_findings_file(&dir)
                .unwrap_or_else(|err| panic!("ler ausente: {err}"))
                .is_none(),
            "sem scan, o caller recebe None (não erro)"
        );

        std::fs::create_dir_all(dir.join(".kira"))
            .unwrap_or_else(|err| panic!("criar .kira: {err}"));
        std::fs::write(
            dir.join(".kira").join("findings.json"),
            r#"{"findings":[{"tool":"audit","rule":"vulnerable","package":"lodash","version":"4.17.20","severity":"high","path":"web","message":"prototype pollution","fingerprint":"0000000000000001"}],"stacks":[]}"#,
        )
        .unwrap_or_else(|err| panic!("escrever findings: {err}"));
        let findings = read_findings_file(&dir)
            .unwrap_or_else(|err| panic!("ler findings: {err}"))
            .unwrap_or_else(|| panic!("findings.json existia"));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "lodash");
        assert_eq!(severity_brief(&findings), "high 1");
        let _ = std::fs::remove_dir_all(&dir);
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

    /// Windows (NTFS) is case-insensitive: `App.CSPROJ` and `GLOBAL.JSON` are the
    /// same files as their lowercase forms and must be detected as .NET markers.
    #[test]
    fn detect_dotnet_stack_with_uppercase_markers() {
        let dir = temp_dir("dotnet-upper");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        touch(&dir, "App.CSPROJ");
        touch(&dir, "MinhaSolucao.SLN");
        touch(&dir, "GLOBAL.JSON");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 1, "{stacks:?}");
        assert_eq!(stacks[0].kind, StackKind::Dotnet);
        assert_eq!(
            stacks[0].markers,
            vec![
                "App.CSPROJ".to_string(),
                "GLOBAL.JSON".to_string(),
                "MinhaSolucao.SLN".to_string()
            ]
        );
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

    /// W2-fix PR-F1, reproduzido do teishoku real (11/Nov/2026): raiz pnpm
    /// (`package.json` + `pnpm-lock.yaml` + `pnpm-workspace.yaml`) e membros
    /// `apps/api` e `packages/shared` com `package.json` sem lock. Antes:
    /// "pnpm, npm, npm" e `npm audit` nos membros. Depois: UMA stack pnpm com
    /// 2 members — o `pnpm audit` da raiz cobre o workspace.
    #[test]
    fn pnpm_workspace_members_fold_into_the_root_stack() {
        let dir = temp_dir("pnpm-ws");
        let api = dir.join("apps").join("api");
        let shared = dir.join("packages").join("shared");
        std::fs::create_dir_all(&api).unwrap_or_else(|err| panic!("criar api: {err}"));
        std::fs::create_dir_all(&shared).unwrap_or_else(|err| panic!("criar shared: {err}"));
        touch(&dir, "package.json");
        touch(&dir, "pnpm-lock.yaml");
        touch(&dir, "pnpm-workspace.yaml");
        touch(&api, "package.json");
        touch(&shared, "package.json");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 1, "só a raiz é stack: {stacks:?}");
        assert_eq!(stacks[0].kind, StackKind::Pnpm);
        assert_eq!(stacks[0].dir, ".");
        assert_eq!(
            stacks[0].markers,
            vec![
                "package.json".to_string(),
                "pnpm-lock.yaml".to_string(),
                "pnpm-workspace.yaml".to_string()
            ]
        );
        assert_eq!(
            stacks[0].members,
            vec!["apps/api".to_string(), "packages/shared".to_string()],
            "membros dobrados na raiz, ordenados"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cada uma das três pistas de workspace basta sozinha: `pnpm-lock.yaml`
    /// no ancestor (o caso do teishoku), `workspaces` no package.json raiz
    /// ou `pnpm-workspace.yaml` presente.
    #[test]
    fn every_workspace_hint_alone_is_enough_to_fold_members() {
        for (tag, root_files, kind) in [
            (
                "lock",
                vec!["package.json", "pnpm-lock.yaml"],
                StackKind::Pnpm,
            ),
            (
                "yaml",
                vec!["package.json", "pnpm-workspace.yaml"],
                StackKind::Pnpm,
            ),
            ("field", vec!["package.json"], StackKind::Npm),
        ] {
            let dir = temp_dir(&format!("hint-{tag}"));
            let member = dir.join("packages").join("shared");
            std::fs::create_dir_all(&member).unwrap_or_else(|err| panic!("criar: {err}"));
            for name in root_files {
                touch(&dir, name);
            }
            if tag == "field" {
                std::fs::write(
                    dir.join("package.json"),
                    r#"{ "name": "root", "workspaces": ["packages/*"] }"#,
                )
                .unwrap_or_else(|err| panic!("escrever workspaces: {err}"));
            }
            touch(&member, "package.json");
            let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
            assert_eq!(stacks.len(), 1, "{tag}: só a raiz: {stacks:?}");
            assert_eq!(stacks[0].kind, kind, "{tag}: kind da raiz");
            assert_eq!(
                stacks[0].members,
                vec!["packages/shared".to_string()],
                "{tag}: membro dobrado"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// Monorepo npm (`workspaces` + `package-lock.json` na raiz): a raiz é npm
    /// válido, os membros dobram nela e nenhum `npm audit` roda nos membros.
    #[test]
    fn npm_workspace_stays_npm_and_folds_members() {
        let dir = temp_dir("npm-ws");
        let member = dir.join("packages").join("shared");
        std::fs::create_dir_all(&member).unwrap_or_else(|err| panic!("criar: {err}"));
        std::fs::write(
            dir.join("package.json"),
            r#"{ "name": "root", "workspaces": ["packages/*"] }"#,
        )
        .unwrap_or_else(|err| panic!("escrever package.json: {err}"));
        touch(&dir, "package-lock.json");
        touch(&member, "package.json");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 1, "{stacks:?}");
        assert_eq!(stacks[0].kind, StackKind::Npm, "npm válido");
        assert_eq!(
            stacks[0].markers,
            vec!["package-lock.json".to_string(), "package.json".to_string()],
            "markers na ordem byte-wise do list_files"
        );
        assert_eq!(stacks[0].members, vec!["packages/shared".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O membro com lockfile PRÓPRIO é um projeto independente (não dobra).
    #[test]
    fn member_with_its_own_lockfile_stays_its_own_stack() {
        let dir = temp_dir("own-lock");
        let api = dir.join("apps").join("api");
        std::fs::create_dir_all(&api).unwrap_or_else(|err| panic!("criar: {err}"));
        touch(&dir, "package.json");
        touch(&dir, "pnpm-lock.yaml");
        touch(&api, "package.json");
        touch(&api, "package-lock.json");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(
            stacks.len(),
            2,
            "raiz pnpm + membro independente: {stacks:?}"
        );
        assert_eq!(stacks[0].kind, StackKind::Pnpm);
        assert!(stacks[0].members.is_empty(), "{stacks:?}");
        assert_eq!(stacks[1].kind, StackKind::Npm);
        assert_eq!(stacks[1].dir, "apps/api");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fora de workspace, `package.json` sem lock continua npm (standalone).
    #[test]
    fn package_json_without_lock_outside_a_workspace_is_still_npm() {
        let dir = temp_dir("standalone");
        let nested = dir.join("tools").join("script");
        std::fs::create_dir_all(&nested).unwrap_or_else(|err| panic!("criar: {err}"));
        touch(&dir, "App.csproj");
        touch(&nested, "package.json");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 2, "{stacks:?}");
        assert_eq!(stacks[1].kind, StackKind::Npm);
        assert_eq!(stacks[1].dir, "tools/script");
        assert!(stacks[1].members.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `package.json` inválido na raiz não derruba a detecção: a pista
    /// `workspaces` falha aberta e os outros markers ainda classificam.
    #[test]
    fn invalid_root_package_json_fails_open_on_the_workspaces_hint() {
        let dir = temp_dir("invalid-pkg");
        let member = dir.join("packages").join("shared");
        std::fs::create_dir_all(&member).unwrap_or_else(|err| panic!("criar: {err}"));
        std::fs::write(dir.join("package.json"), "não é json")
            .unwrap_or_else(|err| panic!("escrever: {err}"));
        touch(&dir, "pnpm-lock.yaml");
        touch(&member, "package.json");
        let stacks = detect_stacks(&dir).unwrap_or_else(|err| panic!("detectar: {err}"));
        assert_eq!(stacks.len(), 1, "{stacks:?}");
        assert_eq!(stacks[0].kind, StackKind::Pnpm);
        assert_eq!(stacks[0].members, vec!["packages/shared".to_string()]);
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

    /// RESOLUÇÃO (PR-F4): a ferramenta é resolvida ANTES do spawn — override
    /// existente resolve como veio; no Windows `.cmd`/`.bat` vai via `cmd /C`,
    /// em qualquer outro lugar spawn direto.
    #[test]
    fn existing_override_resolves_the_pointed_file() {
        let dir = temp_dir("resolve-override");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        let shim = dir.join("npm.cmd");
        std::fs::write(&shim, "echo ok").unwrap_or_else(|err| panic!("escrever shim: {err}"));
        let resolved = resolve_tool_explicit(
            ToolKind::Npm,
            Some(shim.to_str().unwrap_or_default()),
            Some(std::ffi::OsStr::new("")),
        )
        .unwrap_or_else(|err| panic!("resolver override: {err}"));
        assert_eq!(resolved.path, shim);
        assert_eq!(
            resolved.via_cmd,
            cfg!(windows),
            ".cmd no Windows => cmd /C; em outro OS => direto"
        );

        let plain = dir.join("tool");
        std::fs::write(&plain, "x").unwrap_or_else(|err| panic!("escrever: {err}"));
        let resolved = resolve_tool_explicit(
            ToolKind::Npm,
            Some(plain.to_str().unwrap_or_default()),
            Some(std::ffi::OsStr::new("")),
        )
        .unwrap_or_else(|err| panic!("resolver override simples: {err}"));
        assert_eq!(resolved.path, plain);
        assert!(!resolved.via_cmd, "sem .cmd/.bat o spawn é direto sempre");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Override é contrato do usuário: arquivo apontado inexistente => o
    /// mesmo erro acionável "instale o X" (sem spawn, sem texto do OS).
    #[test]
    fn override_pointing_to_a_missing_file_is_instale() {
        let error = resolve_tool_explicit(
            ToolKind::Npm,
            Some("/definitivamente/ausente/npm"),
            Some(std::ffi::OsStr::new("")),
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exige o npm"), "{message}");
        assert!(message.contains("instale"), "{message}");
        assert!(message.contains("nodejs.org"), "{message}");

        // Espaços/valor vazio NÃO são override (o tool_command filtra), mas
        // uma string vazia passada direto também não pode confundir.
        assert!(resolve_tool_explicit(ToolKind::Npm, Some("   "), None).is_err());
    }

    /// PATH fake com o executável da ferramenta (exec bit no Unix, `.cmd` no
    /// Windows) é achado; PATH vazio/ausente => "instale o X" SEM spawn.
    #[test]
    fn fake_path_finds_the_tool_and_empty_path_is_missing() {
        let dir = temp_dir("resolve-path");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        let name = if cfg!(windows) { "npm.cmd" } else { "npm" };
        std::fs::write(dir.join(name), "shim").unwrap_or_else(|err| panic!("escrever shim: {err}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join(name), std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|err| panic!("chmod shim: {err}"));
        }
        let resolved = resolve_tool_explicit(ToolKind::Npm, None, Some(dir.as_os_str()))
            .unwrap_or_else(|err| panic!("resolver do PATH fake: {err}"));
        assert_eq!(resolved.path, dir.join(name));
        assert_eq!(
            resolved.via_cmd,
            cfg!(windows),
            ".cmd achado no PATH (Windows) => cmd /C"
        );

        let error =
            resolve_tool_explicit(ToolKind::Npm, None, Some(std::ffi::OsStr::new(""))).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exige o npm"), "{message}");
        assert!(
            resolve_tool_explicit(ToolKind::Npm, None, None).is_err(),
            "PATH ausente também é ausência da ferramenta"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Unix: arquivo sem bit de execução NÃO conta como ferramenta achada.
    #[cfg(unix)]
    #[test]
    fn file_without_exec_bit_is_not_a_tool() {
        let dir = temp_dir("resolve-noexec");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        std::fs::write(dir.join("npm"), "não executável")
            .unwrap_or_else(|err| panic!("escrever: {err}"));
        assert!(
            resolve_tool_explicit(ToolKind::Npm, None, Some(dir.as_os_str())).is_err(),
            "sem exec bit não é executável"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spawn sobre a resolução: caminho comum spawna direto; via_cmd => o
    /// wrapper `cmd /C chcp 65001` mantém o caminho resolvido como 3º token.
    #[test]
    fn resolved_path_spawns_direct_and_cmd_script_via_wrapper() {
        let resolved = Resolved {
            path: PathBuf::from("/tools/npm"),
            via_cmd: false,
        };
        let command = command_from_resolved(&resolved, &[]);
        assert_eq!(command.get_program(), "/tools/npm");

        let resolved = Resolved {
            path: PathBuf::from("C:\\tools\\npm.cmd"),
            via_cmd: true,
        };
        let command = command_from_resolved(&resolved, &["audit", "--json"]);
        assert_eq!(command.get_program(), "cmd");
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        // Em Windows o segundo argumento é a LINHA inteira (raw_arg); nos
        // outros alvos é o caminho + args. O contrato que importa: nunca existe
        // um argumento isolado "chcp ... &".
        assert_eq!(args.first().map(String::as_str), Some("/C"), "{args:?}");
        assert!(
            !args.iter().any(|a| a.trim() == "chcp 65001 >nul 2>&1 &"),
            "chcp não pode ir como argumento separado: {args:?}"
        );
    }

    /// Hint of `ToolFailure`: first non-empty stderr line, trimmed, control
    /// chars out, ≤200 chars — never more, never the raw body.
    #[test]
    fn stderr_hint_is_one_short_line_or_absent() {
        assert_eq!(stderr_hint(""), "");
        assert_eq!(stderr_hint("\n\n  \n"), "");
        assert_eq!(
            stderr_hint("\n  erro específico da ferramenta \nsegunda linha\n"),
            " (dica: erro específico da ferramenta)"
        );
        assert_eq!(stderr_hint("a\u{1}b\tc"), " (dica: abc)");
        let long = "x".repeat(500);
        let hint = stderr_hint(&long);
        assert_eq!(
            hint.chars().count(),
            200 + " (dica: )".chars().count(),
            "{hint}"
        );
        assert!(hint.chars().all(|ch| ch != '\n' && ch != '\r'));
    }

    /// O summary que o `run` grava é a fonte da série histórica do dashboard.
    /// O caso feliz usa a linha REAL do formato (mudar o texto do `run` quebra
    /// este teste — que é exatamente o alarme que se quer).
    #[test]
    fn findings_count_is_read_back_from_the_real_summary_format() {
        let summary = "kira scan em 'C:\\proj' — stacks: dotnet, npm | achados: 7 (critical 1, high 6) | arquivo: C:\\proj\\.kira\\findings.json\nstacks…\nachados por severidade…";
        assert_eq!(findings_count_in_summary(summary), Some(7));
        // Rodapé de linha única também serve (a trilha corta o summary).
        assert_eq!(
            findings_count_in_summary(
                "kira scan em '/' — stacks: npm | achados: 0 (nenhum) | arquivo: /f.json"
            ),
            Some(0)
        );
    }

    /// Fora do formato: `None`, nunca `Some(0)` — inventar zero desenharia uma
    /// queda que não houve na sparkline.
    #[test]
    fn a_foreign_or_truncated_summary_yields_none() {
        assert_eq!(findings_count_in_summary(""), None);
        assert_eq!(findings_count_in_summary("scan fixture: 3 achados"), None);
        assert_eq!(
            findings_count_in_summary("kira scan em '/' — stacks: npm | achados: muitos"),
            None
        );
        assert_eq!(
            findings_count_in_summary("kira scan em '/' | achados: 12"),
            Some(12),
            "o resto da linha não importa"
        );
    }
}
