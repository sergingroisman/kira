//! Deterministic repository grep for the context packer (ADR-0015, PR5).
//!
//! The KIRA greps, the model only interprets: no tool-calling, no index, no
//! embedding. Every excerpt leaves this module stamped `caminho:linha` so a
//! conclusion can be audited against a real file, and every line passes through
//! [`crate::redact`] before it can reach a prompt — a repository file is exactly
//! where a pasted PAT or connection string lives.
//!
//! Determinism is the contract: the walk is sorted by file name, hits are ranked
//! by a pure score (priority file, then terms matched, then path, then line), so
//! two greps over the same tree produce byte-identical output.

use std::collections::BTreeSet;
use std::path::Path;

use walkdir::WalkDir;

/// Text file bigger than this is skipped: a lockfile or a bundle is noise, and
/// reading it would spend the token budget on nothing.
pub(crate) const MAX_FILE_BYTES: u64 = 128 * 1024;

/// One excerpt never exceeds this many characters (a minified line would
/// otherwise eat the whole window).
const MAX_LINE_CHARS: usize = 200;

/// Hits per file, so one noisy file cannot fill the pack.
pub(crate) const MAX_HITS_PER_FILE: usize = 3;

/// Files read in one grep. The walk is sorted by name, so the same tree always
/// yields the same first N files — the stop is deterministic, not racy.
const MAX_FILES_SCANNED: usize = 400;

/// Bytes inspected for a NUL when deciding whether a file is binary.
const BINARY_PROBE_BYTES: usize = 2_048;

// A decisão de pular diretório/arquivo mora em `crate::context::ignore`
// (ADR-0038). Aqui ficam só as constantes de leitura.
/// One auditable excerpt: `path:line` plus the (redacted) text of that line.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodeExcerpt {
    /// Path relative to the project root, always with `/` separators.
    pub path: String,
    /// 1-based line number in that file.
    pub line: usize,
    /// Trimmed, whitespace-collapsed and redacted line content.
    pub text: String,
    /// bm25 rank of the FTS5 hit (smaller = more relevant); `None` when the
    /// excerpt came from the grep walk, which does not rank (ADR-0021).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<f64>,
    /// True when the file was named by a finding (`findings.json`) — the
    /// explain view shows it, and the ranking gives it the bonus.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub priority: bool,
}

impl CodeExcerpt {
    /// `caminho:linha` — the anchor the model is told to cite.
    #[must_use]
    pub fn anchor(&self) -> String {
        format!("{}:{}", self.path, self.line)
    }
}

/// Result of one repository grep.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RepoGrep {
    /// Ranked excerpts, best first.
    pub excerpts: Vec<CodeExcerpt>,
    /// Files actually read.
    pub files_scanned: usize,
    /// Files that produced at least one hit.
    pub files_with_hits: usize,
    /// True when [`MAX_FILES_SCANNED`] cut the walk short.
    pub truncated: bool,
}

impl RepoGrep {
    /// True when the grep had nothing to say.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.excerpts.is_empty()
    }
}

/// Hir of one line before ranking.
struct Hit {
    /// Files named by `findings.json` outrank the rest.
    priority: bool,
    /// How many distinct terms the line matched.
    score: usize,
    excerpt: CodeExcerpt,
}

/// Pure core: every line of `content` containing at least one `term`
/// (case-insensitive), in file order, redacted and bounded.
///
/// `path` is only used as the anchor prefix, so this stays testable without any
/// filesystem access.
#[must_use]
pub fn grep_text(path: &str, content: &str, terms: &[String]) -> Vec<CodeExcerpt> {
    scored_text(path, content, terms)
        .into_iter()
        .map(|(_, excerpt)| excerpt)
        .collect()
}

/// Same as [`grep_text`], but keeps the score so the cross-file ranking can use
/// it. The score is computed on the raw line, never on the redacted copy.
fn scored_text(path: &str, content: &str, terms: &[String]) -> Vec<(usize, CodeExcerpt)> {
    let mut found = Vec::new();
    if terms.is_empty() {
        return found;
    }
    for (index, line) in content.lines().enumerate() {
        let score = score_line(&line.to_lowercase(), terms);
        if score == 0 {
            continue;
        }
        found.push((
            score,
            CodeExcerpt {
                path: path.to_string(),
                line: index + 1,
                text: clean_line(line),
                rank: None,
                priority: false,
            },
        ));
        if found.len() == MAX_HITS_PER_FILE {
            break;
        }
    }
    found
}

/// Number of distinct terms present in an already-lowercased line.
fn score_line(lower_line: &str, terms: &[String]) -> usize {
    terms
        .iter()
        .filter(|term| lower_line.contains(term.as_str()))
        .count()
}

/// Trims, collapses whitespace, masks secrets and caps the length of one line.
/// Shared with the indexer: the text that enters the FTS5 index is exactly the
/// text a grep excerpt would carry — redacted on the way in (ADR-0021).
pub(crate) fn clean_line(line: &str) -> String {
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let redacted = crate::redact::redact(&collapsed);
    if redacted.chars().count() <= MAX_LINE_CHARS {
        return redacted;
    }
    let mut truncated: String = redacted.chars().take(MAX_LINE_CHARS).collect();
    truncated.push('…');
    truncated
}

/// Greps the repository under `root`.
///
/// `priority_paths` (typically the `path` of each `finding`) are read first and
/// outrank everything else; `skip_paths` are never read (the work item file the
/// caller already put in the prompt); `max_excerpts` bounds the result.
#[must_use]
pub fn grep(
    root: &Path,
    terms: &[String],
    priority_paths: &[String],
    skip_paths: &[String],
    max_excerpts: usize,
) -> RepoGrep {
    let mut out = RepoGrep::default();
    if terms.is_empty() || max_excerpts == 0 {
        return out;
    }
    let mut hits: Vec<Hit> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut skipped: BTreeSet<String> = BTreeSet::new();
    for path in skip_paths {
        if let Some(rel) = normalize_rel(path) {
            skipped.insert(rel);
        }
    }

    for raw in priority_paths {
        let rel = normalize_rel(raw);
        let rel = match rel {
            Some(rel) => rel,
            None => continue,
        };
        if skipped.contains(&rel) || !seen.insert(rel.clone()) {
            continue;
        }
        let Some(content) = read_text_file(&root.join(&rel)) else {
            continue;
        };
        out.files_scanned += 1;
        for hit in collect(&rel, &content, terms, true) {
            hits.push(hit);
        }
    }

    // M7/P5: `.gitignore` aninhado vale também no grep (mesmo matcher do
    // índice — o pack nunca cita arquivo que o git excluiria). Repo sem
    // `.gitignore`: conjunto vazio, nada muda.
    let gitignore = super::gitignore::GitIgnore::load(root);
    let walker = WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            if entry.file_type().is_dir() {
                if is_skipped_dir(entry.path(), entry.depth()) {
                    return false;
                }
                match relative_path(root, entry.path()) {
                    Some(rel) => !gitignore.is_ignored(&rel, true),
                    None => true,
                }
            } else {
                true
            }
        });

    for entry in walker {
        if out.files_scanned >= MAX_FILES_SCANNED {
            out.truncated = true;
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = match relative_path(root, entry.path()) {
            Some(rel) => rel,
            None => continue,
        };
        // The wiki is already retrieved through FTS5 (PR31): grepping it again
        // would send the same knowledge twice and burn the budget. W2-fix
        // PR-F3: the wiki lives inside the dot-directory `.kira/wiki/`, which
        // the leading-dot rule already skips — no special case left here.
        if skipped.contains(&rel)
            || gitignore.is_ignored(&rel, false)
            || !is_source_file(&rel)
            || !seen.insert(rel.clone())
        {
            continue;
        }
        let Some(content) = read_text_file(entry.path()) else {
            continue;
        };
        out.files_scanned += 1;
        for hit in collect(&rel, &content, terms, false) {
            hits.push(hit);
        }
    }

    hits.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| a.excerpt.path.cmp(&b.excerpt.path))
            .then_with(|| a.excerpt.line.cmp(&b.excerpt.line))
    });
    out.files_with_hits = hits
        .iter()
        .map(|hit| hit.excerpt.path.as_str())
        .collect::<BTreeSet<&str>>()
        .len();
    out.excerpts = hits
        .into_iter()
        .take(max_excerpts)
        .map(|hit| hit.excerpt)
        .collect();
    out
}

/// Scores every hit of one file; the per-file cap is applied inside
/// [`grep_text`].
fn collect(rel: &str, content: &str, terms: &[String], priority: bool) -> Vec<Hit> {
    scored_text(rel, content, terms)
        .into_iter()
        .map(|(score, mut excerpt)| {
            excerpt.priority = priority;
            Hit {
                priority,
                score,
                excerpt,
            }
        })
        .collect()
}

/// Normalizes a finding path into a relative path usable with `root.join`:
/// drops `./` and refuses absolute or parent-traversing values.
pub(crate) fn normalize_rel(raw: &str) -> Option<String> {
    let trimmed = raw.trim().replace('\\', "/");
    let trimmed = trimmed.trim_start_matches("./");
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.split('/').any(|part| part == "..")
    {
        return None;
    }
    Some(trimmed.to_string())
}

/// Path of `path` relative to `root`, with `/` separators; `None` when the
/// file lives outside the root.
pub(crate) fn relative_path(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

/// `true` quando este diretório sai da caminhada.
///
/// `depth == 0` é a raiz: **nunca** sai, mesmo chamando `build`, `bin` ou começando
/// com ponto. Era o bug que zerava o índice de um projeto com nome "reservado"
/// (ADR-0038). A decisão do resto vem de [`crate::context::ignore`].
pub(crate) fn is_skipped_dir(path: &Path, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    crate::context::ignore::is_ignored_dir(&name)
}

/// `true` quando o arquivo deve entrar no índice/contexto.
///
/// Decisão por **denylist** (ADR-0038): extensão desconhecida entra de graça, e
/// `.dart`, `.vue`, `.proto`, `.mts` deixam de ser invisíveis sem mexer em lista
/// nenhuma. Lockfiles continuam fora: são gerados e enormes.
pub(crate) fn is_source_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if name.ends_with(".lock") || name == "package-lock.json" {
        return false;
    }
    !crate::context::ignore::is_ignored_file(name)
}

/// Reads a file as UTF-8 text, skipping oversized or binary ones. Any I/O error
/// is a silent skip: grep is context, never a way to fail `triage`.
pub(crate) fn read_text_file(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let probe = &bytes[..bytes.len().min(BINARY_PROBE_BYTES)];
    if probe.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_tree(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-repo-grep-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| panic!("criar {parent:?}: {err}"));
        }
        fs::write(&path, body).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
    }

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(|term| (*term).to_string()).collect()
    }

    #[test]
    fn anchors_carry_the_line_number_and_the_redacted_text() {
        let content = "fn main() {}\nlet ttl = session_ttl;\n";
        let hits = grep_text("src/auth/session.rs", content, &terms(&["session_ttl"]));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].anchor(), "src/auth/session.rs:2");
        assert_eq!(hits[0].text, "let ttl = session_ttl;");
    }

    #[test]
    fn a_secret_on_the_line_never_reaches_the_excerpt() {
        let content = "let pat = \"AbC123XyZ789QqQ\";\ncontato serjota@proton.me\n";
        let hits = grep_text("src/config.rs", content, &terms(&["pat", "contato"]));
        assert_eq!(hits.len(), 2);
        assert!(!hits[0].text.contains("AbC123XyZ789QqQ"), "{hits:?}");
        assert!(!hits[1].text.contains("serjota@proton.me"), "{hits:?}");
    }

    #[test]
    fn long_lines_are_truncated_and_whitespace_collapsed() {
        let long = format!("token {}", "x".repeat(500));
        let hits = grep_text("a.ts", &format!("   {long}   "), &terms(&["token"]));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.chars().count() <= MAX_LINE_CHARS + 1);
        assert!(!hits[0].text.starts_with(' '));
    }

    #[test]
    fn one_file_never_contributes_more_than_the_cap() {
        let content = (0..10)
            .map(|index| format!("token na linha {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let hits = grep_text("a.rs", &content, &terms(&["token"]));
        assert_eq!(hits.len(), MAX_HITS_PER_FILE);
    }

    #[test]
    fn build_and_vcs_directories_are_never_grepped() {
        let dir = temp_tree("skip");
        write(&dir, "src/a.rs", "token no fonte\n");
        write(&dir, "target/debug/b.rs", "token no build\n");
        write(&dir, "node_modules/pkg/c.js", "token na dependência\n");
        write(&dir, ".git/config", "token no git\n");
        write(&dir, ".kira/wiki/contexto.md", "token na wiki\n");

        let found = grep(&dir, &terms(&["token"]), &[], &[], 10);
        let paths: Vec<&str> = found
            .excerpts
            .iter()
            .map(|excerpt| excerpt.path.as_str())
            .collect();
        assert_eq!(paths, vec!["src/a.rs"], "{paths:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_and_oversized_files_are_skipped() {
        let dir = temp_tree("binary");
        write(&dir, "src/a.rs", "token no fonte\n");
        fs::write(dir.join("image.rs"), [0u8, 1, 2, 3])
            .unwrap_or_else(|err| panic!("escrever binário: {err}"));
        let huge = format!("token {}\n", "y".repeat(MAX_FILE_BYTES as usize + 10));
        write(&dir, "big.ts", &huge);

        let found = grep(&dir, &terms(&["token"]), &[], &[], 10);
        assert_eq!(found.excerpts.len(), 1, "{:?}", found.excerpts);
        assert_eq!(found.excerpts[0].path, "src/a.rs");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn findings_paths_outrank_the_rest_of_the_tree() {
        let dir = temp_tree("priority");
        write(&dir, "src/z.rs", "token token token\n");
        write(&dir, "src/App.csproj", "PackageReference token\n");

        let found = grep(
            &dir,
            &terms(&["token"]),
            &["src/App.csproj".to_string()],
            &[],
            10,
        );
        assert_eq!(
            found.excerpts[0].path, "src/App.csproj",
            "{:?}",
            found.excerpts
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// M7/P5 (aceite): o grep honra o mesmo `.gitignore` do índice — termo
    /// que só existe em arquivo ignorado nunca vira excerpt do pack.
    #[test]
    fn grep_skips_gitignored_files() {
        let dir = temp_tree("gitignore");
        write(&dir, ".gitignore", "secret.rs\nvendor/\n");
        write(&dir, "secret.rs", "fn tokensecret() {}\n");
        write(&dir, "vendor/lib.rs", "fn tokensecret() {}\n");
        write(&dir, "public.rs", "fn tokensecret() {}\n");

        let found = grep(&dir, &terms(&["tokensecret"]), &[], &[], 10);
        let paths: Vec<&str> = found
            .excerpts
            .iter()
            .map(|excerpt| excerpt.path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec!["public.rs"],
            "só o arquivo não-ignorado entra no grep: {paths:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_same_tree_produces_the_same_grep_twice() {
        let dir = temp_tree("determinism");
        write(&dir, "src/b.rs", "token b\ntoken b2\n");
        write(&dir, "src/a.rs", "token a\n");
        write(&dir, "src/c/x.ts", "token c\n");

        let first = grep(&dir, &terms(&["token"]), &[], &[], 10);
        let second = grep(&dir, &terms(&["token"]), &[], &[], 10);
        assert_eq!(first, second, "grep precisa ser determinístico");
        let paths: Vec<&str> = first
            .excerpts
            .iter()
            .map(|excerpt| excerpt.path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec!["src/a.rs", "src/b.rs", "src/b.rs", "src/c/x.ts"]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn paths_outside_the_root_are_refused() {
        assert_eq!(normalize_rel("../segredo.rs"), None);
        assert_eq!(normalize_rel("/etc/passwd"), None);
        assert_eq!(normalize_rel("./src/a.rs"), Some("src/a.rs".to_string()));
        let _ = normalize_rel("  ");
    }

    #[test]
    fn budget_of_excerpts_is_respected() {
        let dir = temp_tree("cap");
        write(&dir, "src/a.rs", "token a\n");
        write(&dir, "src/b.rs", "token b\n");
        let found = grep(&dir, &terms(&["token"]), &[], &[], 1);
        assert_eq!(found.excerpts.len(), 1);
        assert_eq!(found.files_with_hits, 2, "o ranking vê os dois arquivos");
        let _ = fs::remove_dir_all(&dir);
    }

    /// O arquivo do work item já está no prompt inteiro: grep nele só duplica.
    #[test]
    fn skip_paths_are_never_read() {
        let dir = temp_tree("skip-file");
        write(&dir, "item.json", "token do work item\n");
        write(&dir, "src/a.rs", "token do fonte\n");
        let found = grep(
            &dir,
            &terms(&["token"]),
            &[],
            &["item.json".to_string()],
            10,
        );
        let paths: Vec<&str> = found
            .excerpts
            .iter()
            .map(|excerpt| excerpt.path.as_str())
            .collect();
        assert_eq!(paths, vec!["src/a.rs"], "{paths:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
