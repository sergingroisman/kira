//! Scanner of .NET: runs `dotnet list package --vulnerable --include-transitive
//! --format json` and parses the result with a pure function of one `String`
//! (offline tests read the real-format fixtures from `tests/fixtures/scan`).
//!
//! The runtime only executes the tool: a missing `dotnet` means an actionable
//! error, and a non-zero exit code never echoes the raw stderr/stdout body.

use std::path::Path;
use std::process::Output;

use serde_json::Value;

use super::{
    Finding, ScanError, ToolKind, exit_code_label, relative_path, spawn_error, stderr_hint,
    tool_command,
};

/// The exact command executed on each detected .NET scope.
pub const DOTNET_ARGS: [&str; 6] = [
    "list",
    "package",
    "--vulnerable",
    "--include-transitive",
    "--format",
    "json",
];

/// Runs the command in `dir` and parses the JSON into findings. The binary is
/// RESOLVED first (which-style) — absence never reaches the spawn, and the
/// resolved full path is what runs.
pub fn scan_dir(dir: &Path) -> Result<Vec<Finding>, ScanError> {
    let mut command = tool_command(ToolKind::Dotnet, &DOTNET_ARGS)?;
    command.current_dir(dir);
    let output = command
        .output()
        .map_err(|source| spawn_error(ToolKind::Dotnet, source))?;
    if !output.status.success() {
        // Absence was already handled by the RESOLUTION (no spawn): a
        // non-zero exit here is a plain tool failure — no OS text parsing.
        let stderr_text = String::from_utf8_lossy(&output.stderr);
        return Err(ScanError::ToolFailure {
            tool: "dotnet list package",
            exit: exit_code_label(output.status.code()),
            hint: stderr_hint(&stderr_text),
        });
    }
    parse_output(&output_stdout(&output), dir)
}

/// Decoded stdout of the tool run (lossy keeps the parse testable).
fn output_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Pure parse of the `dotnet list package --format json` output (v2 of the
/// JSON model): one finding per `<package>.vulnerabilities` entry and one for
/// deprecated packages (rule `outdated`). `root` is used only to build a
/// relative `path` from the project's `filePath`.
pub fn parse_output(raw: &str, root: &Path) -> Result<Vec<Finding>, ScanError> {
    let value: Value = serde_json::from_str(raw).map_err(|source| ScanError::BadToolJson {
        tool: "dotnet list package",
        detail: source.to_string(),
    })?;
    let mut findings = Vec::new();
    if let Some(projects) = value.get("projects").and_then(Value::as_array) {
        for project in projects {
            let file_path = project
                .get("filePath")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let path = relative_path(root, file_path);
            if let Some(frameworks) = project.get("frameworks").and_then(Value::as_array) {
                for framework in frameworks {
                    for group in ["transitivePackages", "topLevelPackages"] {
                        if let Some(packages) = framework.get(group).and_then(Value::as_array) {
                            for package in packages {
                                findings.extend(findings_of_package(package, &path));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(findings)
}

/// Findings of one package entry: `vulnerable` for each advisory recorded in
/// `vulnerabilities`; `outdated` when the entry is marked as deprecated.
fn findings_of_package(package: &Value, path: &str) -> Vec<Finding> {
    let tool = "dotnet";
    let name = package
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = package
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut out = Vec::new();
    if let Some(vulns) = package.get("vulnerabilities").and_then(Value::as_array) {
        for vuln in vulns {
            let severity = vuln
                .get("severity")
                .and_then(Value::as_str)
                .unwrap_or("desconhecida");
            let message = vuln
                .get("advisoryTitle")
                .and_then(Value::as_str)
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| vuln_title(vuln));
            out.push(Finding {
                tool: tool.to_string(),
                rule: "vulnerable".to_string(),
                package: name.to_string(),
                version: version.to_string(),
                severity: severity.to_string(),
                path: path.to_string(),
                message: message.trim().to_string(),
                fingerprint: String::new(),
            });
        }
    }
    if out.is_empty() && is_deprecated(package) {
        out.push(Finding {
            tool: tool.to_string(),
            rule: "outdated".to_string(),
            package: name.to_string(),
            version: version.to_string(),
            severity: "informativo".to_string(),
            path: path.to_string(),
            message: deprecation_message(package),
            fingerprint: String::new(),
        });
    }
    out
}

/// Title of a vulnerability with the shape not foreseen in the fixture.
fn vuln_title(vuln: &Value) -> &str {
    vuln.get("title")
        .and_then(Value::as_str)
        .unwrap_or("vulnerabilidade sem título")
}

/// `deprecated: true` (bool) or `deprecated: { "reasons": [...] }` (object of
/// the SDK).
fn is_deprecated(package: &Value) -> bool {
    match package.get("deprecated") {
        Some(Value::Bool(value)) => *value,
        Some(object) => !matches!(object, Value::Null),
        _ => false,
    }
}

/// deprecation message: reasons of the SDK or a fixed text.
fn deprecation_message(package: &Value) -> String {
    let reasons = package
        .get("deprecated")
        .and_then(Value::as_object)
        .and_then(|map| map.get("reasons").and_then(Value::as_array))
        .map(|reasons| {
            reasons
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|joined| !joined.is_empty());
    match reasons {
        Some(reasons) => format!("pacote deprecado ({reasons})"),
        None => "pacote deprecado".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VULNERABLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scan/scan_dotnet_vulnerable.json"
    ));
    const REORDERED: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scan/scan_dotnet_reordered.json"
    ));
    const NONE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/scan/scan_dotnet_none.json"
    ));

    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(".")
    }

    #[test]
    fn parses_one_finding_per_vulnerability_and_one_for_deprecated() {
        let findings = crate::scan::finding::finalize(
            parse_output(VULNERABLE, &root())
                .unwrap_or_else(|err| panic!("parse da fixture dotnet: {err}")),
        );
        assert_eq!(
            findings.len(),
            3,
            "2 vulnerabilities + 1 deprecado: {findings:?}"
        );

        let json_tagged =
            serde_json::to_value(&findings).unwrap_or_else(|err| panic!("serializar: {err}"));
        let json_tagged = json_tagged.as_array().unwrap_or_else(|| panic!("array"));
        assert!(
            json_tagged
                .iter()
                .all(|f| f.get("tool").and_then(Value::as_str) == Some("dotnet")),
            "tool = dotnet em todos: {json_tagged:?}"
        );
        let rules: Vec<&str> = json_tagged
            .iter()
            .filter_map(|f| f.get("rule").and_then(Value::as_str))
            .collect();
        assert!(rules.contains(&"vulnerable"), "{rules:?}");
        assert!(rules.contains(&"outdated"), "{rules:?}");
        let severities: Vec<&str> = json_tagged
            .iter()
            .filter_map(|f| f.get("severity").and_then(Value::as_str))
            .collect();
        assert!(
            severities.contains(&"High"),
            "severidade como vem no JSON: {severities:?}"
        );
        assert!(severities.contains(&"Moderate"), "{severities:?}");
        assert!(
            json_tagged
                .iter()
                .all(|f| f.get("path").and_then(Value::as_str) == Some("App.csproj")),
            "path do projeto (relativo ao root): {json_tagged:?}"
        );
        let npgsql = findings
            .iter()
            .find(|f| f.package == "Npgsql.Webty")
            .unwrap_or_else(|| panic!("finding do pacote deprecado: {findings:?}"));
        assert_eq!(npgsql.rule, "outdated");
        assert_eq!(npgsql.severity, "informativo");
        assert_eq!(npgsql.version, "8.0.4");
    }

    /// Fixture with the same data in another order => the same fingerprint
    /// list, in the same order (the stable-gate order).
    #[test]
    fn reordered_fixture_yields_the_same_fingerprint_list() {
        let first =
            parse_output(VULNERABLE, &root()).unwrap_or_else(|err| panic!("parse 1: {err}"));
        let second =
            parse_output(REORDERED, &root()).unwrap_or_else(|err| panic!("parse 2: {err}"));
        let first = crate::scan::finding::finalize(first);
        let second = crate::scan::finding::finalize(second);
        assert_eq!(
            first, second,
            "fingerprints iguais mesmo com entradas reordenadas"
        );
    }

    #[test]
    fn zero_findings_when_the_output_has_no_package() {
        let findings = crate::scan::finding::finalize(
            parse_output(NONE, &root())
                .unwrap_or_else(|err| panic!("parse da fixture vazia: {err}")),
        );
        assert!(findings.is_empty(), "nenhum achado: {findings:?}");
    }

    #[test]
    fn tool_missing_and_tool_fail_errors_are_actionable_without_body() {
        let missing = ScanError::DotnetMissing;
        let message = missing.to_string();
        assert!(message.contains("SDK do .NET"), "{message}");
        assert!(
            message.contains("dotnet.microsoft.com/download"),
            "erro deve apontar a instalação: {message}"
        );

        let failed = ScanError::ToolFailure {
            tool: "dotnet list package",
            exit: "-1".to_string(),
            hint: super::super::stderr_hint("MSBUILD : error MSB1009\nat line 2\n"),
        };
        let message = failed.to_string();
        assert!(
            message.contains("a ferramenta dotnet list package falhou"),
            "{message}"
        );
        assert!(message.contains("(exit -1)"), "{message}");
        assert!(
            message.contains("(dica: MSBUILD : error MSB1009)"),
            "{message}"
        );
        assert!(
            !message.contains('\n'),
            "erros de scan são compactos: {message}"
        );

        let without_hint = ScanError::ToolFailure {
            tool: "dotnet list package",
            exit: "1".to_string(),
            hint: String::new(),
        };
        let message = without_hint.to_string();
        assert!(
            message.ends_with("rode kira scan de novo"),
            "sem stderr não existe dica: {message}"
        );
    }
}
