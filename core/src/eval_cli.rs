//! `kira-cored eval --suite judge-calibration` — measure judge agreement with human labels.
//!
//! Reads labeled entries from `~/.local/share/kira/audit/judge-decisions.jsonl`
//! (entries where `human_label` is non-null), computes agreement between the
//! automated judge verdict and the human label, and exits non-zero if agreement
//! falls below the threshold (default 85%).

use anyhow::{Context, Result};
use std::io::BufRead;
use std::path::PathBuf;

use super::EvalSuite;

pub fn run(suite: EvalSuite, threshold: f64) -> Result<()> {
    match suite {
        EvalSuite::JudgeCalibration => run_judge_calibration(threshold),
    }
}

fn audit_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let path = PathBuf::from(home).join(".local/share/kira/audit/judge-decisions.jsonl");
    anyhow::ensure!(path.exists(), "no judge audit file at {}", path.display());
    Ok(path)
}

#[derive(serde::Deserialize)]
struct JudgeRecord {
    judgment: Judgment,
    human_label: Option<String>,
}

#[derive(serde::Deserialize)]
struct Judgment {
    score: f64,
    verdict: String,
}

fn run_judge_calibration(threshold: f64) -> Result<()> {
    let path = audit_path()?;
    let file = std::fs::File::open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let reader = std::io::BufReader::new(file);

    let mut total = 0_usize;
    let mut agreed = 0_usize;
    let mut zone_agree = ZoneStats::default();
    let mut zone_disagree = ZoneStats::default();
    let mut skipped = 0_usize;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: JudgeRecord = if let Ok(r) = serde_json::from_str(&line) {
            r
        } else {
            skipped += 1;
            continue;
        };
        let Some(ref human) = record.human_label else {
            skipped += 1;
            continue;
        };

        total += 1;
        let human_normalized = normalize_label(human);
        let auto_normalized = normalize_label(&record.judgment.verdict);
        let is_accept = human_normalized == "accept";

        if human_normalized == auto_normalized {
            agreed += 1;
            if is_accept {
                zone_agree.accepts += 1;
            } else {
                zone_agree.rejects += 1;
            }
        } else if is_accept {
            zone_disagree.accepts += 1;
        } else {
            zone_disagree.rejects += 1;
        }
    }

    if total == 0 {
        eprintln!("No labeled samples found in {}", path.display());
        eprintln!("Add human_label='accept'|'reject' to audit entries, then re-run.");
        anyhow::bail!("no labeled samples");
    }

    let agreement_pct = (agreed as f64 / total as f64) * 100.0;
    let pass = agreement_pct >= threshold;

    println!("Judge Calibration — {total} labeled samples ({skipped} skipped)");
    println!();
    println!("  Agreement:  {agreed}/{total}  ({agreement_pct:.1}%)");
    println!("  Threshold:  {threshold:.1}%");
    println!("  Result:     {}", if pass { "PASS ✓" } else { "FAIL ✗" });
    println!();
    println!("  Agreed   — accepts: {}  rejects: {}", zone_agree.accepts, zone_agree.rejects);
    println!("  Disagreed— accepts: {}  rejects: {}", zone_disagree.accepts, zone_disagree.rejects);

    // False accept rate (human said reject, judge said accept)
    let false_accepts = zone_disagree.rejects;
    // False reject rate (human said accept, judge said reject)
    let false_rejects = zone_disagree.accepts;
    if false_accepts + false_rejects > 0 {
        println!();
        println!("  False accepts (judge ACCEPT, human REJECT): {false_accepts}");
        println!("  False rejects (judge REJECT, human ACCEPT): {false_rejects}");
    }

    if !pass {
        anyhow::bail!(
            "judge agreement {agreement_pct:.1}% < threshold {threshold:.1}% — not ready for production"
        );
    }

    Ok(())
}

fn normalize_label(s: &str) -> &str {
    let s = s.trim();
    // "accept" / "approved" / "pass" → accept
    // "reject" / "rejected" / "fail" → reject
    if s.eq_ignore_ascii_case("accept")
        || s.eq_ignore_ascii_case("approved")
        || s.eq_ignore_ascii_case("pass")
    {
        "accept"
    } else {
        "reject"
    }
}

#[derive(Default)]
struct ZoneStats {
    accepts: usize,
    rejects: usize,
}
