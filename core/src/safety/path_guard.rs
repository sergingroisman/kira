//! SC4 — Path Traversal Defense.
//!
//! Validates that a target path lies within an allowed base directory.
//! Defenses applied:
//!   1. Reject explicit `..` components in the requested path.
//!   2. Resolve relative paths against canonicalized `base_dir`.
//!   3. Canonicalize the result when the target exists (defeats symlinks).
//!   4. Final `starts_with(base_canonical)` check.
//!
//! v0 limitation: symlink attacks via a non-existent target's parent are not
//! defeated (no incremental canonicalization). v0.5 ships full ancestor probing.

use anyhow::{Context, Result};
use std::path::{Component, Path, PathBuf};

/// Resolve `target` against `base_dir` and ensure it stays inside `base_dir`.
///
/// Works for both read (target exists) and write (target may not exist) cases.
/// Returns the resolved absolute `PathBuf`.
pub fn resolve_within(target: &str, base_dir: &str) -> Result<PathBuf> {
    let base_canonical = std::fs::canonicalize(base_dir)
        .with_context(|| format!("base_dir canonicalize failed: {base_dir}"))?;

    let target_path = Path::new(target);

    // 1. Reject explicit `..` components (defense in depth — canonicalize would catch
    //    existing-target cases, but write-case targets may not exist yet).
    for c in target_path.components() {
        if matches!(c, Component::ParentDir) {
            anyhow::bail!("SC4 path traversal blocked: '..' not allowed in target '{target}'");
        }
    }

    // 2. Build candidate: absolute → as-is; relative → join base_canonical.
    let candidate = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        base_canonical.join(target_path)
    };

    // 3. Lexical normalization (drops `.` components).
    let normalized: PathBuf = candidate.components().collect();

    // 4. When target exists, prefer canonical form (defeats symlinks).
    let final_path = std::fs::canonicalize(&normalized).unwrap_or(normalized);

    // 5. Prefix check.
    if !final_path.starts_with(&base_canonical) {
        anyhow::bail!(
            "SC4 path traversal blocked: target '{}' resolved to '{}' is outside base_dir '{}'",
            target,
            final_path.display(),
            base_canonical.display()
        );
    }

    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tempdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("kira-pg-test-{}", uuid::Uuid::now_v7()));
        fs::create_dir_all(&d).unwrap();
        // canonicalize — on macOS /var is a symlink to /private/var; tests must
        // compare against the canonical form to match resolve_within's output.
        fs::canonicalize(&d).unwrap()
    }

    #[test]
    fn relative_inside_passes() {
        let base = tempdir();
        let resolved = resolve_within("foo.txt", base.to_str().unwrap()).unwrap();
        assert!(resolved.starts_with(&base));
        assert!(resolved.ends_with("foo.txt"));
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn parent_dir_component_blocked() {
        let base = tempdir();
        let err = resolve_within("../escape.txt", base.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("'..' not allowed"));
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn absolute_outside_blocked() {
        let base = tempdir();
        let err = resolve_within("/etc/passwd", base.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("outside base_dir"));
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn nested_relative_passes() {
        let base = tempdir();
        let resolved = resolve_within("sub/inner.txt", base.to_str().unwrap()).unwrap();
        assert!(resolved.starts_with(&base));
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn missing_base_fails() {
        let err = resolve_within("foo", "/nonexistent-base-xyz-42").unwrap_err();
        assert!(err.to_string().contains("base_dir canonicalize failed"));
    }
}
