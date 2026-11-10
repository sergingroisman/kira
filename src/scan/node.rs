//! Scanner of Node/TS: runs `pnpm audit --json` (pnpm) or `npm audit --json`
//! (npm) and parses the result with pure functions of one `String` (offline
//! tests read real-format fixtures from `tests/fixtures/scan`).
//!
//! Both audit CLIs exit non-zero exactly when vulnerabilities are found, so a
//! non-zero code is only an error when the JSON does not parse; the raw body
//! is never echoed (errors stay compact and actionable).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use super::{
    Finding, ScanError, ToolKind, exit_code_label, spawn_error, tool_command, tool_run_failure,
};

/// Which package manager audit to run: choice made in the stack detection
/// (pnpm-lock.yaml => pnpm, otherwise npm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeAuditTool {
    Pnpm,
    Npm,
}

impl NodeAuditTool {
    /// The invocation kind of this audit (command build + env override).
    fn kind(self) -> ToolKind {
        match self {
            Self::Pnpm => ToolKind::Pnpm,
            Self::Npm => ToolKind::Npm,
        }
    }
}

/// Runs `<pnpm|npm> audit --json` in `dir` and parses the JSON into findings.
pub fn scan_dir(tool: NodeAuditTool, dir: &Path) -> Result<Vec<Finding>, ScanError> {
    let kind = tool.kind();
    let mut command = tool_command(kind);
    command.args(["audit", "--json"]).current_dir(dir);
    let output = command
        .output()
        .map_err(|source| spawn_error(kind, source))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let parsed = match tool {
        NodeAuditTool::Npm => parse_npm_output(&stdout),
        NodeAuditTool::Pnpm => parse_pnpm_output(&stdout),
    };
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    match parsed {
        Ok(findings) => Ok(findings),
        // Audit CLIs exit non-zero exactly when vulnerabilities exist: the
        // JSON parse has already been tried — if it failed, the compact error
        // (never the raw body) carries at most one stderr line as a hint. On
        // Windows the `cmd /C` wrapper may also be reporting that the tool
        // itself is missing ("is not recognized") — that becomes the
        // actionable "instale o X" instead.
        Err(_source) if !output.status.success() => Err(tool_run_failure(
            kind,
            match tool {
                NodeAuditTool::Npm => "npm audit",
                NodeAuditTool::Pnpm => "pnpm audit",
            },
            exit_code_label(output.status.code()),
            &stderr_text,
        )),
        Err(source) => Err(source),
    }
}

/// Pure parse of `npm audit --json`: one finding per advisor registered in
/// `vulnerabilities.<pkg>.via` (entries of type object; string entries are
/// only dependency references). The installed version and the dependency path
/// come from `findings` (the npm report's own index).
pub fn parse_npm_output(raw: &str) -> Result<Vec<Finding>, ScanError> {
    let value: Value = serde_json::from_str(raw).map_err(|source| ScanError::BadToolJson {
        tool: "npm audit",
        detail: source.to_string(),
    })?;
    let mut installed = BTreeMap::new();
    if let Some(report) = value.get("findings").and_then(Value::as_array) {
        for entry in report {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let version = entry
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let path = entry
                .get("paths")
                .and_then(Value::as_array)
                .and_then(|paths| paths.first())
                .and_then(Value::as_str)
                .unwrap_or("package.json");
            installed.insert(name.to_string(), (version.to_string(), path.to_string()));
        }
    }
    let mut out = Vec::new();
    if let Some(vulns) = value.get("vulnerabilities").and_then(Value::as_object) {
        for (name, entry) in vulns {
            let severity = entry
                .get("severity")
                .and_then(Value::as_str)
                .unwrap_or("desconhecida");
            let (version, path) = installed.get(name).cloned().unwrap_or_else(|| {
                let version = entry
                    .get("range")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                (version, "package.json".to_string())
            });
            if let Some(vias) = entry.get("via").and_then(Value::as_array) {
                for advisory in vias {
                    let title = advisory
                        .as_object()
                        .and_then(|map| map.get("title"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|title| !title.is_empty());
                    let Some(title) = title else {
                        // Plain strings are only dependency references.
                        continue;
                    };
                    out.push(Finding {
                        tool: "audit".to_string(),
                        rule: "vulnerable".to_string(),
                        package: name.clone(),
                        version: version.clone(),
                        severity: severity.to_string(),
                        path: path.clone(),
                        message: title.to_string(),
                        fingerprint: String::new(),
                    });
                }
            }
        }
    }
    Ok(out)
}

/// Pure parse of `pnpm audit --json`: array of events with
/// `code: "audit-advisory"` and the advisory's data in `data` (github security
/// advisory registry format).
pub fn parse_pnpm_output(raw: &str) -> Result<Vec<Finding>, ScanError> {
    let value: Value = serde_json::from_str(raw).map_err(|source| ScanError::BadToolJson {
        tool: "pnpm audit",
        detail: source.to_string(),
    })?;
    let empty = Vec::new();
    let events = value.as_array().unwrap_or(&empty);
    let mut out = Vec::new();
    for event in events {
        if event.get("code").and_then(Value::as_str) != Some("audit-advisory") {
            continue;
        }
        let data = event.get("data").cloned().unwrap_or(Value::Null.clone());
        let package = data
            .get("module_name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let severity = data
            .get("severity")
            .and_then(Value::as_str)
            .or_else(|| event.get("severity").and_then(Value::as_str))
            .unwrap_or("desconhecida");
        let message = data
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or("advisory do pnpm audit");
        let version = data
            .get("version")
            .and_then(Value::as_str)
            .or_else(|| data.get("vulnerable_versions").and_then(Value::as_str))
            .unwrap_or_default();
        let path = data
            .get("paths")
            .and_then(Value::as_array)
            .and_then(|paths| paths.first())
            .and_then(Value::as_str)
            .unwrap_or("pnpm-lock.yaml");
        out.push(Finding {
            tool: "audit".to_string(),
            rule: "vulnerable".to_string(),
            package: package.to_string(),
            version: version.to_string(),
            severity: severity.to_string(),
            path: path.to_string(),
            message: message.to_string(),
            fingerprint: String::new(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{finding, stderr_hint};

    const NPM: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scan/scan_npm_audit.json"
    ));
    const NPM_ORDERED: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scan/scan_npm_audit_reordered.json"
    ));
    const NPM_EMPTY: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scan/scan_npm_audit_none.json"
    ));
    const PNPM: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scan/scan_pnpm_audit.json"
    ));

    #[test]
    fn parses_npm_vulnerabilities_with_formal_data() {
        let findings = finding::finalize(
            parse_npm_output(NPM).unwrap_or_else(|err| panic!("parse npm: {err}")),
        );
        assert_eq!(findings.len(), 2, "2 via advisors: {findings:?}");
        let minimist = findings
            .iter()
            .find(|f| f.package == "minimist")
            .unwrap_or_else(|| panic!("finding do minimist: {findings:?}"));
        assert_eq!(minimist.tool, "audit");
        assert_eq!(minimist.rule, "vulnerable");
        assert_eq!(
            minimist.version, "1.2.0",
            "versão instalada vem de findings"
        );
        assert_eq!(minimist.severity, "high", "severidade como vem no JSON");
        assert_eq!(minimist.path, ". > minimist");
        assert_eq!(minimist.message, "Prototype Pollution");
        assert!(
            findings.iter().any(|f| f.package == "lodash"
                && f.severity == "moderate"
                && f.version == "4.17.15"),
            "finding do lodash: {findings:?}"
        );
    }

    /// Entries of `vulnerabilities` reordered => the same fingerprint list.
    #[test]
    fn reordered_npm_map_yields_the_same_fingerprints() {
        let first =
            finding::finalize(parse_npm_output(NPM).unwrap_or_else(|err| panic!("parse 1: {err}")));
        let second = finding::finalize(
            parse_npm_output(NPM_ORDERED).unwrap_or_else(|err| panic!("parse 2: {err}")),
        );
        assert_eq!(first, second, "ordem estável por fingerprint");
    }

    #[test]
    fn npm_audit_without_vulnerabilities_is_empty() {
        let findings = finding::finalize(
            parse_npm_output(NPM_EMPTY).unwrap_or_else(|err| panic!("parse npm vazio: {err}")),
        );
        assert!(findings.is_empty(), "nenhum achado: {findings:?}");
    }

    #[test]
    fn parses_pnpm_advisories() {
        let findings = finding::finalize(
            parse_pnpm_output(PNPM).unwrap_or_else(|err| panic!("parse pnpm: {err}")),
        );
        assert_eq!(findings.len(), 2, "2 audit-advisory: {findings:?}");
        let lodash = findings
            .iter()
            .find(|f| f.package == "lodash")
            .unwrap_or_else(|| panic!("finding do lodash: {findings:?}"));
        assert_eq!(lodash.tool, "audit");
        assert_eq!(lodash.rule, "vulnerable");
        assert_eq!(lodash.severity, "high");
        assert!(
            findings
                .iter()
                .any(|f| f.package == "shelljs" && f.version == "<0.8.5"),
            "versão vem do advisory: {findings:?}"
        );
    }

    #[test]
    fn missing_tool_error_is_actionable() {
        let message = ToolKind::Npm.missing_error().to_string();
        assert!(message.contains("npm"), "{message}");
        assert!(message.contains("nodejs.org"), "{message}");
        let message = ToolKind::Pnpm.missing_error().to_string();
        assert!(message.contains("npm install -g pnpm"), "{message}");
    }

    /// Exit non-zero of the audit is normal (vulnerabilities found): the parse
    /// is still tried before any error. Here only the compact error shape.
    #[test]
    fn tool_fail_error_is_compact_without_body() {
        let failed = ScanError::ToolFailure {
            tool: "npm audit",
            exit: "1".to_string(),
            hint: stderr_hint("npm error! something specific\nignored line\n"),
        };
        let message = failed.to_string();
        assert!(
            message.contains("a ferramenta npm audit falhou"),
            "{message}"
        );
        assert!(message.contains("(exit 1)"), "{message}");
        assert!(
            message.contains("(dica: npm error! something specific)"),
            "{message}"
        );
        assert!(!message.contains('\n'), "compacto: {message}");
    }
}
