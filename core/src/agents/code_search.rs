//! Native in-process code search using the ripgrep libraries.
//!
//! Replaces the `bash grep/rg` → `file_read whole file` round-trip with a
//! single structured tool call: walk the worktree (respecting `.gitignore`),
//! regex-match in-process, and return only the matching lines with location.
//! Far fewer tokens than dumping whole files into the context, and no
//! subprocess spawn.

use std::path::Path;

use anyhow::{Context, Result};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use serde::Serialize;

/// A single match: file (relative to the search root), 1-based line, and the
/// trimmed line text.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CodeMatch {
    pub path: String,
    pub line: u64,
    pub text: String,
}

/// Search parameters.
pub struct SearchOpts {
    /// Hard cap on total matches returned (across all files).
    pub max_results: usize,
    /// Case-insensitive matching.
    pub case_insensitive: bool,
    /// Optional glob filter (e.g. `*.rs`) applied relative to the root.
    pub glob: Option<String>,
    /// Optional sub-directory (relative to the worktree root) to scope to.
    pub subdir: Option<String>,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            max_results: 40,
            case_insensitive: true,
            glob: None,
            subdir: None,
        }
    }
}

/// Run a regex search under `root`. Synchronous (blocking IO + CPU) — callers
/// on an async runtime should wrap this in `spawn_blocking`.
pub fn search_code(root: &Path, pattern: &str, opts: &SearchOpts) -> Result<Vec<CodeMatch>> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(opts.case_insensitive)
        .build(pattern)
        .with_context(|| format!("invalid search regex: {pattern}"))?;

    let start = match &opts.subdir {
        Some(s) => root.join(s),
        None => root.to_path_buf(),
    };

    let mut builder = WalkBuilder::new(&start);
    builder.standard_filters(true); // .gitignore, hidden files, parents
    // Honour .gitignore even when the root isn't a git repo (default only
    // applies git ignore files inside a detected repo).
    builder.require_git(false);
    if let Some(glob) = &opts.glob {
        let mut ov = ignore::overrides::OverrideBuilder::new(&start);
        if ov.add(glob).is_ok()
            && let Ok(over) = ov.build()
        {
            builder.overrides(over);
        }
    }

    let mut matches: Vec<CodeMatch> = Vec::new();

    for entry in builder.build() {
        if matches.len() >= opts.max_results {
            break;
        }
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .line_number(true)
            .build();

        let cap = opts.max_results;
        let _ = searcher.search_path(
            &matcher,
            path,
            UTF8(|lnum, line| {
                matches.push(CodeMatch {
                    path: rel.clone(),
                    line: lnum,
                    text: line.trim_end().to_string(),
                });
                // Stop searching this file once the global cap is reached.
                Ok(matches.len() < cap)
            }),
        );
    }

    Ok(matches)
}

/// Render matches as compact markdown grouped by file (`path:line: text`).
pub fn render_matches_md(matches: &[CodeMatch]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let mut current = String::new();
    for m in matches {
        if m.path != current {
            let _ = writeln!(out, "\n{}:", m.path);
            current = m.path.clone();
        }
        let _ = writeln!(out, "  {}: {}", m.line, m.text);
    }
    out.trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn looks_conceptual() {}\nlet x = 1;\n").unwrap();
        fs::write(dir.path().join("b.txt"), "looks_conceptual mentioned here\n").unwrap();
        fs::create_dir_all(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/c.rs"), "// looks_conceptual in sub\n").unwrap();
        // Ignored by .gitignore — must not appear in results.
        fs::write(dir.path().join(".gitignore"), "ignored/\n").unwrap();
        fs::create_dir_all(dir.path().join("ignored")).unwrap();
        fs::write(dir.path().join("ignored/d.rs"), "looks_conceptual hidden\n").unwrap();
        dir
    }

    #[test]
    fn finds_matches_across_files() {
        let dir = fixture();
        let out = search_code(dir.path(), "looks_conceptual", &SearchOpts::default()).unwrap();
        let paths: Vec<&str> = out.iter().map(|m| m.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("a.rs")));
        assert!(paths.iter().any(|p| p.ends_with("b.txt")));
        assert!(paths.iter().any(|p| p.contains("c.rs")));
    }

    #[test]
    fn respects_gitignore() {
        let dir = fixture();
        let out = search_code(dir.path(), "looks_conceptual", &SearchOpts::default()).unwrap();
        assert!(
            !out.iter().any(|m| m.path.contains("ignored")),
            "gitignored files must be skipped"
        );
    }

    #[test]
    fn glob_filters_extension() {
        let dir = fixture();
        let opts = SearchOpts {
            glob: Some("*.rs".to_string()),
            ..Default::default()
        };
        let out = search_code(dir.path(), "looks_conceptual", &opts).unwrap();
        assert!(out.iter().all(|m| m.path.ends_with(".rs")), "only .rs expected");
        assert!(!out.is_empty());
    }

    #[test]
    fn max_results_caps_output() {
        let dir = tempfile::tempdir().unwrap();
        let body: String = (0..100).map(|_| "match\n").collect();
        fs::write(dir.path().join("big.rs"), body).unwrap();
        let opts = SearchOpts {
            max_results: 5,
            ..Default::default()
        };
        let out = search_code(dir.path(), "match", &opts).unwrap();
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn invalid_regex_errors() {
        let dir = fixture();
        assert!(search_code(dir.path(), "(unclosed", &SearchOpts::default()).is_err());
    }
}
