//! `git worktree` shell-out wrappers.
//!
//! Per plan T5: tokio::process, parse `--porcelain` output for determinism,
//! no `git2` dependency.

use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

async fn run_git(repo_root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .kill_on_drop(true)
        .output()
        .await
        .with_context(|| format!("spawn `git {}` failed", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "git {} failed (exit={:?}): {}",
            args.join(" "),
            output.status.code(),
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Find the toplevel of the repo containing `start`.
pub async fn toplevel(start: &Path) -> Result<std::path::PathBuf> {
    let out = run_git(start, &["rev-parse", "--show-toplevel"]).await?;
    Ok(std::path::PathBuf::from(out.trim()))
}

/// `git diff <base>...<head>` — three-dot form: diff against the merge-base.
/// Returns the patch text (utf-8 lossy).
pub async fn diff_three_dot(repo_root: &Path, base: &str, head: &str) -> Result<String> {
    let range = format!("{base}...{head}");
    run_git(repo_root, &["diff", &range]).await
}

/// `git worktree add -b <branch> <path> <base>`
pub async fn worktree_add(
    repo_root: &Path,
    path: &str,
    branch: &str,
    base: &str,
) -> Result<()> {
    run_git(repo_root, &["worktree", "add", "-b", branch, path, base]).await?;
    Ok(())
}

/// `git worktree remove --force <path>`
pub async fn worktree_remove(repo_root: &Path, path: &str) -> Result<()> {
    run_git(repo_root, &["worktree", "remove", "--force", path]).await?;
    Ok(())
}

/// `git branch -D <branch>`. Returns Ok(true) on delete, Ok(false) if absent.
pub async fn branch_delete(repo_root: &Path, branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["branch", "-D", branch])
        .current_dir(repo_root)
        .kill_on_drop(true)
        .output()
        .await
        .context("spawn `git branch -D` failed")?;
    if output.status.success() {
        Ok(true)
    } else {
        // not-a-branch is the common acceptable failure
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") || stderr.contains("not a branch") {
            Ok(false)
        } else {
            anyhow::bail!("git branch -D {} failed: {}", branch, stderr.trim())
        }
    }
}

/// Parse `git worktree list --porcelain` into rows.
/// Each row: `(path, head_sha, branch_or_detached)`.
pub async fn worktree_list(repo_root: &Path) -> Result<Vec<WorktreeListRow>> {
    let out = run_git(repo_root, &["worktree", "list", "--porcelain"]).await?;
    let mut rows: Vec<WorktreeListRow> = Vec::new();
    let mut cur = WorktreeListRow::default();
    for line in out.lines() {
        if line.is_empty() {
            if !cur.path.is_empty() {
                rows.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if let Some(p) = line.strip_prefix("worktree ") {
            cur.path = p.to_string();
        } else if let Some(h) = line.strip_prefix("HEAD ") {
            cur.head = h.to_string();
        } else if let Some(b) = line.strip_prefix("branch ") {
            cur.branch = Some(b.to_string());
        } else if line == "detached" {
            cur.branch = None;
        } else if line == "bare" {
            cur.bare = true;
        }
    }
    if !cur.path.is_empty() {
        rows.push(cur);
    }
    Ok(rows)
}

#[derive(Debug, Default, Clone)]
pub struct WorktreeListRow {
    pub path: String,
    pub head: String,
    pub branch: Option<String>,
    pub bare: bool,
}
