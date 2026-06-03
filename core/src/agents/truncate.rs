//! Token-aware truncation of tool output.
//!
//! The legacy caps (5 MiB file_read, 256 KiB bash) exist for *memory* safety —
//! they are far too large for the LLM *context window* (256 KiB ≈ 64k tokens,
//! 5 MiB ≈ 1.3M tokens). Injecting that much text wastes tokens and can blow
//! the window outright.
//!
//! [`truncate_for_context`] keeps a head + tail window (line- and byte-bounded)
//! with an explicit elision marker in the middle, so the model still sees the
//! start and end of the output — the parts that usually matter — at a fraction
//! of the token cost. UTF-8 safe (never slices mid-codepoint).

/// Outcome of a context truncation pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Truncated {
    /// The (possibly shrunk) text to feed back to the model.
    pub text: String,
    /// Whether any content was elided.
    pub truncated: bool,
    /// Byte length of the original (pre-truncation) text.
    pub original_bytes: usize,
    /// Line count of the original text.
    pub original_lines: usize,
}

/// Fraction of the budget given to the head (vs the tail).
const HEAD_RATIO: f64 = 0.7;

/// Keep the head and tail of `text` within `max_lines` and `max_bytes`,
/// eliding the middle. Returns the original unchanged when it already fits.
pub fn truncate_for_context(text: &str, max_lines: usize, max_bytes: usize) -> Truncated {
    let original_bytes = text.len();
    let original_lines = text.lines().count();

    if original_bytes <= max_bytes && original_lines <= max_lines {
        return Truncated {
            text: text.to_string(),
            truncated: false,
            original_bytes,
            original_lines,
        };
    }

    let lines: Vec<&str> = text.lines().collect();
    let max_lines = max_lines.max(2);

    // Split the line budget head/tail.
    let mut head_n = ((max_lines as f64) * HEAD_RATIO).floor() as usize;
    head_n = head_n.min(lines.len());
    let mut tail_n = max_lines.saturating_sub(head_n);
    tail_n = tail_n.min(lines.len().saturating_sub(head_n));

    let head = lines[..head_n].join("\n");
    let tail = if tail_n > 0 {
        lines[lines.len() - tail_n..].join("\n")
    } else {
        String::new()
    };

    // Apply the byte budget on top of the line window.
    let head_budget = (max_bytes as f64 * HEAD_RATIO) as usize;
    let tail_budget = max_bytes.saturating_sub(head_budget);
    let head = take_prefix_bytes(&head, head_budget);
    let tail = take_suffix_bytes(&tail, tail_budget);

    let elided_lines = original_lines.saturating_sub(head_n + tail_n);
    let kept_bytes = head.len() + tail.len();
    let elided_bytes = original_bytes.saturating_sub(kept_bytes);
    let marker = format!(
        "\n… [truncated for context: {elided_lines} lines / {elided_bytes} bytes elided] …\n"
    );

    let mut out = String::with_capacity(head.len() + marker.len() + tail.len());
    out.push_str(&head);
    out.push_str(&marker);
    out.push_str(&tail);

    Truncated {
        text: out,
        truncated: true,
        original_bytes,
        original_lines,
    }
}

/// Keep at most `max_bytes` from the start of `s`, on a char boundary.
fn take_prefix_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Keep at most `max_bytes` from the end of `s`, on a char boundary.
fn take_suffix_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut start = s.len() - max_bytes;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_input_unchanged() {
        let t = truncate_for_context("hello\nworld", 100, 1000);
        assert!(!t.truncated);
        assert_eq!(t.text, "hello\nworld");
        assert_eq!(t.original_lines, 2);
    }

    #[test]
    fn line_budget_elides_middle() {
        let input: String = (0..1000).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
        let t = truncate_for_context(&input, 100, 1_000_000);
        assert!(t.truncated);
        assert_eq!(t.original_lines, 1000);
        assert!(t.text.contains("line0"), "keeps head");
        assert!(t.text.contains("line999"), "keeps tail");
        assert!(t.text.contains("truncated for context"), "has marker");
        assert!(!t.text.contains("line500"), "drops middle");
        // Kept far fewer lines than the original.
        assert!(t.text.lines().count() < 150);
    }

    #[test]
    fn byte_budget_enforced() {
        let input = "x".repeat(100_000);
        let t = truncate_for_context(&input, 10_000, 4_000);
        assert!(t.truncated);
        // Output stays near the byte budget (+ marker), not the 100k original.
        assert!(t.text.len() < 5_000, "len was {}", t.text.len());
    }

    #[test]
    fn multibyte_no_panic() {
        // Accented + multibyte content must never be sliced mid-codepoint.
        let input: String = (0..2000).map(|_| "configuração ção 日本語").collect::<Vec<_>>().join("\n");
        let t = truncate_for_context(&input, 50, 2_000);
        assert!(t.truncated);
        // Round-trips as valid UTF-8 (String guarantees it; the point is no panic).
        assert!(t.text.contains("configuração"));
    }
}
