use std::iter::Peekable;
use std::str::Chars;

/// Converts an HTML fragment into plain text, preserving line breaks.
///
/// Pure, hand-written parser (no regex, no extra crates): block tags
/// (`<br>`, `<p>`, `<div>`, ...) become newlines, other tags are removed and
/// the usual named/numeric entities are decoded. Anything without a
/// recognized meaning is kept verbatim.
pub fn html_to_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => match read_tag_name(&mut chars) {
                Some(tag) => {
                    if is_block_tag(&tag) && !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                // Unterminated tag: drop the rest of the input.
                None => break,
            },
            '&' => decode_entity(&mut chars, &mut out),
            // Normalize CRLF/CR line endings to LF.
            '\r' => {}
            _ => out.push(c),
        }
    }
    normalize(&out)
}

/// Reads a tag name up to the closing `>`; returns `None` if unterminated.
fn read_tag_name(chars: &mut Peekable<Chars<'_>>) -> Option<String> {
    let mut name = String::new();
    let mut in_attributes = false;
    loop {
        let c = chars.next()?;
        match c {
            '>' => return Some(name),
            c if c.is_whitespace() => in_attributes = true,
            _ if in_attributes => {}
            // Self-closing marker (e.g. `<br/>`): part of no name.
            '/' => {}
            c => name.push(c.to_ascii_lowercase()),
        }
    }
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "br" | "hr"
            | "p"
            | "/p"
            | "div"
            | "/div"
            | "li"
            | "/li"
            | "ul"
            | "/ul"
            | "ol"
            | "/ol"
            | "tr"
            | "/tr"
            | "td"
            | "/td"
            | "th"
            | "/th"
            | "table"
            | "/table"
            | "blockquote"
            | "/blockquote"
            | "pre"
            | "/pre"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "/h1"
            | "/h2"
            | "/h3"
            | "/h4"
            | "/h5"
            | "/h6"
    )
}

/// Decodes an entity that starts right after `&`; keeps it verbatim when the
/// entity is unknown or unterminated.
fn decode_entity(chars: &mut Peekable<Chars<'_>>, out: &mut String) {
    let mut name = String::new();
    let mut terminated = false;
    while let Some(&c) = chars.peek() {
        if c == ';' {
            chars.next();
            terminated = true;
            break;
        }
        if (!c.is_ascii_alphanumeric() && c != '#') || name.len() >= 10 {
            break;
        }
        name.push(c);
        chars.next();
    }
    let replacement = if terminated {
        named_or_numeric(&name)
    } else {
        None
    };
    match replacement {
        Some(c) => out.push(c),
        None => {
            out.push('&');
            out.push_str(&name);
            if terminated {
                out.push(';');
            }
        }
    }
}

fn named_or_numeric(name: &str) -> Option<char> {
    match name {
        "amp" => return Some('&'),
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "quot" => return Some('"'),
        "apos" => return Some('\''),
        "nbsp" => return Some(' '),
        _ => {}
    }
    let code = if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        name.strip_prefix('#')
            .and_then(|digits| digits.parse::<u32>().ok())
    };
    code.and_then(char::from_u32)
}

/// Trims trailing whitespace per line, collapses 3+ newlines into 2 (one
/// blank line at most) and trims the outer edges.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newlines = 0usize;
    for c in text.chars() {
        match c {
            '\n' => {
                while out.ends_with(' ') || out.ends_with('\t') {
                    out.pop();
                }
                newlines += 1;
                if newlines <= 2 {
                    out.push('\n');
                }
            }
            c => {
                newlines = 0;
                out.push(c);
            }
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_plain_text_as_is() {
        assert_eq!(html_to_text("texto puro"), "texto puro");
    }

    #[test]
    fn removes_inline_tags() {
        assert_eq!(
            html_to_text("texto <b>negrito</b> e <span class=\"x\">fim</span>"),
            "texto negrito e fim"
        );
    }

    #[test]
    fn converts_br_and_block_tags_to_newlines() {
        assert_eq!(html_to_text("linha 1<br>linha 2"), "linha 1\nlinha 2");
        assert_eq!(html_to_text("<p>par 1</p><p>par 2</p>"), "par 1\npar 2");
    }

    #[test]
    fn decodes_common_entities() {
        assert_eq!(html_to_text("A &amp; B"), "A & B");
        assert_eq!(html_to_text("&lt;kra&gt;"), "<kra>");
        assert_eq!(html_to_text("&quot;q&quot;"), "\"q\"");
        assert_eq!(html_to_text("a&nbsp;b"), "a b");
        assert_eq!(html_to_text("feito&#39;s"), "feito's");
        assert_eq!(html_to_text("a&#233;b"), "aéb");
    }

    #[test]
    fn keeps_unknown_entities_verbatim() {
        assert_eq!(html_to_text("R&D e &weird; ficam"), "R&D e &weird; ficam");
    }

    #[test]
    fn drops_unterminated_tag() {
        assert_eq!(html_to_text("texto <div class=\"x"), "texto");
    }

    #[test]
    fn empty_or_tag_only_input_yields_empty_output() {
        assert_eq!(html_to_text(""), "");
        assert_eq!(html_to_text("<br>"), "");
    }
}
