//! BOM-aware decoding of tool output (ADR-0023 P1, M7).
//!
//! Why this exists: the audit CLIs do not always emit plain UTF-8. A real
//! capture of `pnpm audit` made through a PowerShell redirect came out as
//! UTF-16 LE **with BOM**, and `serde_json` refuses any leading BOM — the
//! parse died before the first JSON byte, and the user saw an unactionable
//! "saída inválida" instead of findings. The decode is a pure function of
//! the raw bytes: the BOM decides the encoding, and no locale text is ever
//! parsed (the same doctrine as `binresolve`: resolve by shape, not by OS
//! messages).
//!
//! Scope note: the three BOMs of ADR-0023 P1 (UTF-8, UTF-16 LE, UTF-16 BE).
//! UTF-32 shares its first two bytes with UTF-16 LE and is NOT detected —
//! no audit CLI in the wild emits UTF-32, and guessing it would corrupt the
//! common UTF-16 LE case.

/// Decodes the stdout of a tool run into text.
///
/// - UTF-8 BOM (`EF BB BF`): strip, keep the UTF-8 bytes.
/// - UTF-16 LE BOM (`FF FE`) / UTF-16 BE BOM (`FE FF`): decode the 16-bit
///   units (surrogate pairs included) with the std lossy decoder — a lone
///   surrogate becomes the replacement char, never a panic.
/// - No BOM: exactly today's behaviour (`from_utf8_lossy`), byte for byte.
#[must_use]
pub fn decode_tool_output(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(text) = utf16_with_bom(bytes, 0xFF, 0xFE, true) {
        return text;
    }
    if let Some(text) = utf16_with_bom(bytes, 0xFE, 0xFF, false) {
        return text;
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Decodes UTF-16 guarded by its BOM pair (`little_endian` selects LE vs BE).
fn utf16_with_bom(bytes: &[u8], first: u8, second: u8, little_endian: bool) -> Option<String> {
    let rest = bytes.strip_prefix(&[first, second][..])?;
    let mut units = Vec::with_capacity(rest.len() / 2);
    for chunk in rest.chunks(2) {
        // A trailing odd byte (truncated pipe) is dropped, as lossy UTF-8
        // would drop a partial sequence — never a panic on broken output.
        if chunk.len() == 2 {
            let pair = [chunk[0], chunk[1]];
            units.push(if little_endian {
                u16::from_le_bytes(pair)
            } else {
                u16::from_be_bytes(pair)
            });
        }
    }
    Some(String::from_utf16_lossy(&units))
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"advisories":{},"muted":[],"metadata":{"vulnerabilities":{}}}"#;

    fn utf16le_with_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    fn utf16be_with_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn plain_bytes_keep_the_old_lossy_behaviour() {
        assert_eq!(decode_tool_output(b"{\"a\":1}"), "{\"a\":1}");
        // Invalid UTF-8 without BOM: lossy, exactly as before this module.
        assert_eq!(decode_tool_output(&[0xFF_u8, 0x41]), "\u{FFFD}A");
    }

    #[test]
    fn utf8_bom_is_stripped_before_the_parse() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(JSON.as_bytes());
        assert_eq!(decode_tool_output(&bytes), JSON);
    }

    #[test]
    fn utf16le_bom_decodes_to_the_same_text_and_parses_as_json() {
        let decoded = decode_tool_output(&utf16le_with_bom(JSON));
        assert_eq!(decoded, JSON);
        // The actual bug: serde_json refuses the BOM, so the decode must
        // hand it clean text.
        let value: serde_json::Value =
            serde_json::from_str(&decoded).expect("utf16le decodificado deve parsear");
        assert!(value.get("advisories").is_some());
    }

    #[test]
    fn utf16be_bom_decodes_to_the_same_text() {
        assert_eq!(decode_tool_output(&utf16be_with_bom(JSON)), JSON);
    }

    #[test]
    fn surrogate_pairs_survive_and_a_truncated_tail_never_panics() {
        let text = "café — 𝄞 ok";
        assert_eq!(decode_tool_output(&utf16le_with_bom(text)), text);
        let mut bytes = utf16le_with_bom(JSON);
        bytes.pop(); // corte no meio de um par: byte sobrando, sem pânico
        assert!(decode_tool_output(&bytes).starts_with("{\"advisories\""));
    }

    #[test]
    fn lone_surrogate_becomes_the_replacement_char_not_a_panic() {
        let bytes: Vec<u8> = [0xFF_u8, 0xFE]
            .into_iter()
            .chain(0xD800u16.to_le_bytes())
            .collect();
        assert_eq!(decode_tool_output(&bytes), "\u{FFFD}");
    }
}
