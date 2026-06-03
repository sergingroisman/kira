//! Compressor (M19) — rule-based, no LLM.
//!
//! Pipeline:
//! 1. Normalize whitespace (collapse runs, strip trailing).
//! 2. Drop consecutive blank lines (keep at most 1).
//! 3. Strip `//`, `#`, and `/* */` style comments (best-effort, line-level).
//! 4. Optional `target_ratio` — caller sees whether the target was met.

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct CompressOpts {
    /// Target `output_len` / `input_len` ratio (0.0..=1.0). 0.5 = aim for half.
    pub target_ratio: f64,
    /// Strip line comments (`//`, `#`).
    pub strip_comments: bool,
}

impl Default for CompressOpts {
    fn default() -> Self {
        Self {
            target_ratio: 0.5,
            strip_comments: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CompressionResult {
    pub output: String,
    pub input_len: usize,
    pub output_len: usize,
    pub ratio: f64,
    /// Whether the achieved ratio is at or below `target_ratio`.
    pub target_met: bool,
}

static MULTI_WS: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
static BLANK_RUN: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| Regex::new(r"\n\s*\n(\s*\n)+").unwrap());
static LINE_COMMENT: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(?m)\s*(//|#)[^\n]*$").unwrap());
static BLOCK_COMMENT: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());

pub fn compress(input: &str, opts: CompressOpts) -> CompressionResult {
    let mut out = input.to_string();
    if opts.strip_comments {
        out = BLOCK_COMMENT.replace_all(&out, "").into_owned();
        out = LINE_COMMENT.replace_all(&out, "").into_owned();
    }
    out = MULTI_WS.replace_all(&out, " ").into_owned();
    out = BLANK_RUN.replace_all(&out, "\n\n").into_owned();
    out = out
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");

    let input_len = input.len();
    let output_len = out.len();
    let ratio = if input_len == 0 {
        0.0
    } else {
        output_len as f64 / input_len as f64
    };
    CompressionResult {
        output: out,
        input_len,
        output_len,
        ratio,
        target_met: ratio <= opts.target_ratio,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_zero_ratio() {
        let r = compress("", CompressOpts::default());
        assert_eq!(r.output_len, 0);
        assert!(r.ratio.abs() < f64::EPSILON);
    }

    #[test]
    fn whitespace_collapsed() {
        let input = "hello       world";
        let r = compress(input, CompressOpts::default());
        assert_eq!(r.output, "hello world");
        assert!(r.output_len < r.input_len);
    }

    #[test]
    fn blank_lines_collapsed() {
        let input = "a\n\n\n\n\nb";
        let r = compress(input, CompressOpts::default());
        assert_eq!(r.output, "a\n\nb");
    }

    #[test]
    fn line_comments_stripped() {
        let input = "let x = 1; // initial value\nlet y = 2;";
        let r = compress(input, CompressOpts::default());
        assert!(!r.output.contains("initial value"));
        assert!(r.output.contains("let y"));
    }

    #[test]
    fn block_comments_stripped() {
        let input = "/* doc block */ pub fn foo() {}";
        let r = compress(input, CompressOpts::default());
        assert!(!r.output.contains("doc block"));
        assert!(r.output.contains("pub fn foo"));
    }

    #[test]
    fn target_met_flag_correct() {
        let input = "       ".repeat(100);
        let r = compress(&input, CompressOpts { target_ratio: 0.1, strip_comments: false });
        assert!(r.target_met);
    }
}
