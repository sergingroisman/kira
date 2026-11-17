//! One scan finding (`kira scan`): stable fingerprint and deterministic order.
//!
//! The fingerprint is FNV-1a 64-bit over `tool|rule|package|version|message`
//! (reusing the hand-rolled hash from [`crate::store`], no new dependency) and
//! the final list is always sorted by fingerprint — that is the M1
//! determinism gate: two runs over the same state produce the same set of
//! fingerprints in the same order. No AI here.

use serde::{Deserialize, Serialize};

use crate::store::fnv1a64;

/// One normalized finding of one standard tool (`dotnet list package`,
/// `npm audit`, `pnpm audit`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// `dotnet` for the .NET scanner, `audit` for the npm/pnpm scanner.
    pub tool: String,
    /// `vulnerable` (security advisory), `outdated` (deprecated package).
    pub rule: String,
    pub package: String,
    pub version: String,
    /// Severity exactly as it came in the tool's JSON (`High`, `high`, ...).
    pub severity: String,
    /// Project/file or dependency relation related to the finding.
    pub path: String,
    pub message: String,
    /// FNV-1a 64-bit hex over the identity fields (assigned by
    /// [`fingerprint`]/[`finalize`]).
    pub fingerprint: String,
}

/// Trims only at the ends: whitespace outside the identity text does not
/// change the fingerprint.
fn clean(text: &str) -> String {
    text.trim().to_string()
}

/// FNV-1a 64-bit hex over `tool|rule|package|version|message` (trimmed).
pub fn fingerprint(tool: &str, rule: &str, package: &str, version: &str, message: &str) -> String {
    let identity = format!(
        "{}|{}|{}|{}|{}",
        clean(tool),
        clean(rule),
        clean(package),
        clean(version),
        clean(message)
    );
    format!("{:016x}", fnv1a64(identity.as_bytes()))
}

/// Assigns the `fingerprint` of every finding and sorts the list ascending by
/// fingerprint (stable, deterministic). This is the list serialized to
/// `.kira/findings.json`.
pub fn finalize(mut findings: Vec<Finding>) -> Vec<Finding> {
    for finding in &mut findings {
        finding.fingerprint = fingerprint(
            &finding.tool,
            &finding.rule,
            &finding.package,
            &finding.version,
            &finding.message,
        );
    }
    findings.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
    findings
}

/// FC-01: contagens por severidade canônica (lowercase) + `other` — o insumo
/// da série temporal que alimenta o `kira forecast` no store. Função pura.
#[must_use]
pub fn severity_counts(findings: &[Finding]) -> crate::store::FindingsCounts {
    let mut counts = crate::store::FindingsCounts {
        total: findings.len(),
        ..Default::default()
    };
    for finding in findings {
        match finding.severity.trim().to_ascii_lowercase().as_str() {
            "critical" => counts.critical += 1,
            "high" => counts.high += 1,
            "medium" => counts.medium += 1,
            "low" => counts.low += 1,
            // Severidade crua fora das buckets (ex.: `Moderate`) conta em
            // `other` — o dado entra na série, nunca some.
            _ => counts.other += 1,
        }
    }
    counts
}

/// Count lines by severity, each one with counts per tool, grouped by severity
/// name (lowercased) — comparison only for display; the stored value is the
/// raw severity.
pub fn count_lines(findings: &[Finding]) -> String {
    let mut counts: std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>> =
        std::collections::BTreeMap::new();
    for finding in findings {
        let severity = finding.severity.trim().to_lowercase();
        let entry = counts.entry(severity).or_default();
        *entry.entry(finding.tool.trim().to_string()).or_default() += 1;
    }
    counts
        .iter()
        .map(|(severity, tools)| {
            let parts = tools
                .iter()
                .map(|(tool, count)| format!("{tool} {count}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("  {severity}: {parts}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Short list of severities (with totals) for the summary line of the stdout
/// and of the trail: `high 2, moderate 1` (alphabetical, deterministic) or
/// `nenhum`.
pub fn severity_brief(findings: &[Finding]) -> String {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for finding in findings {
        *counts
            .entry(finding.severity.trim().to_lowercase())
            .or_default() += 1;
    }
    if counts.is_empty() {
        return "nenhum".to_string();
    }
    counts
        .iter()
        .map(|(severity, count)| format!("{severity} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-known vector of the hand-rolled FNV: empty input.
    #[test]
    fn fingerprint_is_the_store_fnv_over_the_joined_identity() {
        let identity = "audit|vulnerable|lodash|4.17.19|DoS";
        let manual = format!("{:016x}", fnv1a64(identity.as_bytes()));
        assert_eq!(
            fingerprint("audit", "vulnerable", "lodash", "4.17.19", "DoS"),
            manual
        );
    }

    /// Trimmed ends must not change the fingerprint (the audit tools embed
    /// advices with extra spaces).
    #[test]
    fn fingerprint_ignores_outer_whitespace() {
        assert_eq!(
            fingerprint("tool", "rule", "pkg", "1.0", " mensagem "),
            fingerprint("tool", "rule", "pkg", "1.0", "mensagem")
        );
    }

    #[test]
    fn finalize_assigns_and_sorts_by_fingerprint() {
        let unsorted = vec![
            Finding {
                tool: "audit".into(),
                rule: "vulnerable".into(),
                package: "minimist".into(),
                version: "1.2.0".into(),
                severity: "high".into(),
                path: ". > minimist".into(),
                message: "Prototype Pollution".into(),
                fingerprint: String::new(),
            },
            Finding {
                tool: "dotnet".into(),
                rule: "outdated".into(),
                package: "Npgsql".into(),
                version: "8.0.4".into(),
                severity: "informativo".into(),
                path: "App.csproj".into(),
                message: "pacote deprecado".into(),
                fingerprint: String::new(),
            },
        ];
        let sorted = finalize(unsorted);
        assert_eq!(sorted.len(), 2);
        assert!(
            sorted
                .windows(2)
                .all(|pair| pair[0].fingerprint <= pair[1].fingerprint),
            "ordem ascending by fingerprint: {sorted:?}"
        );
        assert!(
            sorted.iter().all(|finding| finding.fingerprint.len() == 16),
            "fingerprint hex de 64 bits: {sorted:?}"
        );
        // Stable: the same data in another entry order yields the same list.
        let shuffled = finalize(vec![sorted[1].clone(), sorted[0].clone()]);
        assert_eq!(shuffled, sorted, "o conjunto de fingerprints é o mesmo");
    }

    #[test]
    fn severity_counts_buckets_canonical_and_keeps_the_rest_in_other() {
        let finding = |severity: &str| Finding {
            tool: "audit".into(),
            rule: "vulnerable".into(),
            package: format!("pkg-{severity}"),
            version: "1.0.0".into(),
            severity: severity.into(),
            path: ".".into(),
            message: format!("msg {severity}"),
            fingerprint: format!("f-{severity}"),
        };
        let counts = severity_counts(&[
            finding("High"),
            finding("critical"),
            finding("CRITICAL"),
            finding("medium"),
            finding("low"),
            finding("Moderate"),
        ]);
        assert_eq!(counts.total, 6, "{counts:?}");
        assert_eq!(counts.critical, 2, "{counts:?}");
        assert_eq!(counts.high, 1, "{counts:?}");
        assert_eq!(counts.medium, 1, "{counts:?}");
        assert_eq!(counts.low, 1, "{counts:?}");
        // Severidade fora das buckets canônicas conta em `other` — o dado
        // entra na série, nunca é descartado.
        assert_eq!(counts.other, 1, "{counts:?}");
    }

    #[test]
    fn count_lines_group_by_severity_with_counts_per_tool() {
        let findings = vec![
            Finding {
                tool: "audit".into(),
                rule: "vulnerable".into(),
                package: "minimist".into(),
                version: "1.2.0".into(),
                severity: "high".into(),
                path: ". > minimist".into(),
                message: "Prototype Pollution".into(),
                fingerprint: String::new(),
            },
            Finding {
                tool: "dotnet".into(),
                rule: "vulnerable".into(),
                package: "Newtonsoft.Json".into(),
                version: "13.0.1".into(),
                severity: "High".into(),
                path: "App.csproj".into(),
                message: "Denial of Service".into(),
                fingerprint: String::new(),
            },
            Finding {
                tool: "dotnet".into(),
                rule: "outdated".into(),
                package: "Npgsql".into(),
                version: "8.0.4".into(),
                severity: "informativo".into(),
                path: "App.csproj".into(),
                message: "pacote deprecado".into(),
                fingerprint: String::new(),
            },
        ];
        assert_eq!(
            count_lines(&findings),
            "  high: audit 1, dotnet 1\n  informativo: dotnet 1"
        );
        assert_eq!(findings.len(), 3);
        assert_eq!(severity_brief(&findings), "high 2, informativo 1");
        assert_eq!(severity_brief(&[]), "nenhum");
    }
}
