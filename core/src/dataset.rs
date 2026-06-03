//! `kira-cored dataset export` — reads audit JSONL, outputs LLaMA-Factory format.

use anyhow::{Context, Result};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;

use crate::DatasetAction;

pub fn run(action: DatasetAction) -> Result<()> {
    match action {
        DatasetAction::Export {
            judge,
            since,
            count,
            labeled_only,
            output,
        } => {
            if !judge {
                anyhow::bail!("only --judge is supported for now");
            }
            export_judge(since, count, labeled_only, output)
        }
    }
}

#[derive(Debug, Deserialize)]
struct JudgeRecord {
    ts: String,
    spec: String,
    output: String,
    judgment: Judgment,
    model: String,
    #[allow(dead_code)]
    temperature: f64,
    human_label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Judgment {
    score: f64,
    verdict: String,
    rationale: String,
}

#[derive(Serialize)]
struct LlamaFactoryEntry {
    instruction: String,
    input: String,
    output: String,
}

fn audit_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let path = PathBuf::from(home).join(".local/share/kira/audit/judge-decisions.jsonl");
    anyhow::ensure!(path.exists(), "no judge audit file at {}", path.display());
    Ok(path)
}

fn export_judge(
    since: Option<String>,
    count: Option<usize>,
    labeled_only: bool,
    output_path: Option<String>,
) -> Result<()> {
    let path = audit_path()?;
    let file = std::fs::File::open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let reader = std::io::BufReader::new(file);

    let since_dt = since
        .as_deref()
        .map(|s| {
            DateTime::parse_from_rfc3339(&format!("{s}T00:00:00Z"))
                .or_else(|_| DateTime::parse_from_rfc3339(s))
                .context("--since must be ISO date (e.g. 2026-01-01 or RFC3339)")
        })
        .transpose()?;

    let mut writer: Box<dyn Write> = match output_path {
        Some(ref p) => Box::new(
            std::fs::File::create(p)
                .with_context(|| format!("cannot create output file {p}"))?,
        ),
        None => Box::new(std::io::stdout().lock()),
    };

    let mut exported = 0_usize;
    let mut skipped = 0_usize;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: JudgeRecord = if let Ok(r) = serde_json::from_str(&line) { r } else {
            skipped += 1;
            continue;
        };

        if labeled_only && record.human_label.is_none() {
            skipped += 1;
            continue;
        }

        if let Some(ref dt) = since_dt
            && let Ok(ts) = DateTime::parse_from_rfc3339(&record.ts)
                && ts < *dt {
                    skipped += 1;
                    continue;
                }

        if let Some(max) = count
            && exported >= max {
                break;
            }

        let expected_output = serde_json::json!({
            "score": record.judgment.score as u32,
            "verdict": record.judgment.verdict,
            "rationale": record.judgment.rationale,
        });

        let entry = LlamaFactoryEntry {
            instruction: "Evaluate this output against the spec. Return JSON with score (0-10), verdict (accept/reject), and rationale.".into(),
            input: format!("<SPEC>\n{}\n</SPEC>\n\n<OUTPUT>\n{}\n</OUTPUT>", record.spec, record.output),
            output: serde_json::to_string(&expected_output)?,
        };

        let json = serde_json::to_string(&entry)?;
        writeln!(writer, "{json}")?;
        exported += 1;
    }

    eprintln!("exported {exported} entries ({skipped} skipped) from {}", path.display());
    if let Some(ref p) = output_path {
        eprintln!("written to {p}");
    }

    Ok(())
}
