//! Enforces the AGENTS.md hard rule with a build failure instead of a review
//! comment: **no `unwrap()`/`expect()` on user input, network or parse**.
//!
//! `unwrap()` is banned outright in runtime code. `expect()` is allowed only with
//! an inline `// allow-expect:` justification on the same or the two preceding
//! lines — the reason has to live next to the call, not in a PR description.
//!
//! `#[cfg(test)] mod tests` blocks are exempt: a test that cannot panic is not
//! testing anything.

use std::fs;
use std::path::{Path, PathBuf};

const MARKER: &str = "allow-expect:";

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|err| panic!("ler {dir:?}: {err}"));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|err| panic!("entrada em {dir:?}: {err}"))
            .path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
}

#[test]
fn runtime_code_never_unwraps_and_justifies_every_expect() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);

    let mut violations = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(&src)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("ler {file:?}: {err}"));

        // Everything after the first `#[cfg(test)]` is test code.
        let runtime = source.split("#[cfg(test)]").next().unwrap_or(&source);
        let lines: Vec<&str> = runtime.lines().collect();

        for (index, line) in lines.iter().enumerate() {
            if is_comment(line) {
                continue;
            }
            if line.contains(".unwrap()") {
                violations.push(format!("{rel}:{} unwrap() em runtime", index + 1));
            }
            if line.contains(".expect(") {
                let from = index.saturating_sub(3);
                let justified = lines[from..index].iter().any(|prev| prev.contains(MARKER));
                if !justified {
                    violations.push(format!(
                        "{rel}:{} expect() sem '{MARKER}' nas 3 linhas anteriores",
                        index + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "AGENTS.md violado (input/rede/parse nunca podem panicar):\n  {}",
        violations.join("\n  ")
    );
}
