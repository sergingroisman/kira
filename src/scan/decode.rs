//! Decodificação tolerante da saída de ferramenta (M7/P1 — fecha a promessa
//! F3 das jornadas: "parse tolerante BOM/UTF-16").
//!
//! Por que existe: no Windows a saída de ferramentas reais pode chegar em
//! UTF-16 ou com BOM — o caso clássico é o `dotnet`/`npm` redirecionado por
//! PowerShell (`>` produz UTF-16LE com BOM) e wrappers `.cmd` que ecoam com
//! codepage trocada. `String::from_utf8_lossy` sozinho transforma cada byte
//! alto em U+FFFD e o JSON não parseia mais — o scan morreria com "JSON
//! inválido" onde há saída perfeitamente boa, só codificada diferente.
//!
//! Regra da casa respeitada: sem dependência nova — o std já decodifica
//! UTF-16 (`String::from_utf16_lossy`), e a detecção é pelo BOM (bytes),
//! NUNCA por texto de OS localizado.

/// Decodifica a saída crua de uma ferramenta:
/// - BOM UTF-8 (`EF BB BF`) ⇒ descarta o BOM e decodifica UTF-8;
/// - BOM UTF-16LE (`FF FE`) ⇒ decodifica UTF-16 little-endian;
/// - BOM UTF-16BE (`FE FF`) ⇒ decodifica UTF-16 big-endian;
/// - sem BOM ⇒ UTF-8 (lossy no fim, como antes — bytes soltos não viram erro).
///
/// Unidade ímpar no fim de UTF-16 é descartada (o lossy do std cuida do resto):
/// saída truncada não pode virar panic nem dado inventado.
#[must_use]
pub fn decode_tool_output(bytes: &[u8]) -> String {
    const BOM_UTF8: [u8; 3] = [0xEF, 0xBB, 0xBF];
    const BOM_UTF16_LE: [u8; 2] = [0xFF, 0xFE];
    const BOM_UTF16_BE: [u8; 2] = [0xFE, 0xFF];

    if let Some(rest) = bytes.strip_prefix(&BOM_UTF8) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&BOM_UTF16_LE) {
        return String::from_utf16_lossy(&utf16_units(rest, u16::from_le_bytes));
    }
    if let Some(rest) = bytes.strip_prefix(&BOM_UTF16_BE) {
        return String::from_utf16_lossy(&utf16_units(rest, u16::from_be_bytes));
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Pares de bytes → code units UTF-16 na endianess dada; sobra ímpar sai
/// (o resto do `as_chunks` é descartado de propósito — saída truncada não
/// pode virar panic nem dado inventado).
fn utf16_units(bytes: &[u8], from: fn([u8; 2]) -> u16) -> Vec<u16> {
    let (pairs, _resto) = bytes.as_chunks::<2>();
    pairs.iter().map(|pair| from(*pair)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UTF-16LE com BOM (o que o `>` do PowerShell produz no Windows).
    fn utf16le_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    /// UTF-16BE com BOM.
    fn utf16be_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn decodes_utf8_with_and_without_bom() {
        assert_eq!(decode_tool_output(b"{\"a\":1}"), "{\"a\":1}");
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice("{\"a\":1}".as_bytes());
        assert_eq!(
            decode_tool_output(&with_bom),
            "{\"a\":1}",
            "BOM UTF-8 some, o JSON fica"
        );
    }

    #[test]
    fn decodes_utf16_le_and_be_with_bom() {
        let json = "{\"vulnerabilities\":{}}";
        assert_eq!(decode_tool_output(&utf16le_bom(json)), json);
        assert_eq!(decode_tool_output(&utf16be_bom(json)), json);
    }

    #[test]
    fn decodes_accented_text_through_utf16() {
        // Acentuação é o caso real: mojibake de UTF-16 lido como UTF-8 vira
        // lixo; aqui o texto volta inteiro.
        let text = "vulnerável — crítica";
        assert_eq!(decode_tool_output(&utf16le_bom(text)), text);
    }

    #[test]
    fn odd_trailing_byte_does_not_panic() {
        let mut bytes = utf16le_bom("ok");
        bytes.push(0x41); // unidade ímpar no fim (saída truncada)
        assert_eq!(decode_tool_output(&bytes), "ok");
    }

    #[test]
    fn empty_and_lone_bom_decode_to_empty() {
        assert_eq!(decode_tool_output(b""), "");
        assert_eq!(decode_tool_output(&[0xFF, 0xFE]), "");
        assert_eq!(decode_tool_output(&[0xEF, 0xBB, 0xBF]), "");
    }

    /// Aceite do M7: a FIXTURE em UTF-16LE com BOM parseia pelo extrator real
    /// do npm audit — prova de ponta a ponta do decode (bytes → String →
    /// Finding), não só do decodificador.
    #[test]
    fn bom_fixture_parses_through_the_npm_extractor() {
        let raw = include_bytes!("../../tests/fixtures/scan/scan_npm_audit_utf16le.json");
        assert_eq!(
            &raw[..2],
            &[0xFF, 0xFE],
            "a fixture precisa nascer UTF-16LE com BOM"
        );
        let text = decode_tool_output(raw);
        let findings = crate::scan::node::parse_npm_output(&text)
            .unwrap_or_else(|err| panic!("JSON decodificado precisa parsear: {err}"));
        assert_eq!(findings.len(), 1, "um advisory na fixture: {findings:?}");
        assert_eq!(findings[0].package, "lodash");
        assert_eq!(findings[0].severity, "moderate");
        assert_eq!(findings[0].version, "4.17.15");
    }
}
