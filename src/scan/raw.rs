//! Digest bruto do scan (PR37, docs/te-implementacao.md §7): o output DECODIFICADO
//! de cada ferramenta é gravado em `<root>/.kira/raw/<tool>.json` como evidência
//! auditável — depurar BOM/UTF-16 ou JSON malformado exige ver o que a ferramenta
//! REALMENTE disse, não só o resumo.
//!
//! Regras fechadas na PR:
//! - Nome fixo por ferramenta ⇒ overwrite é a ROTAÇÃO (mais recente vence): sem
//!   diretório crescendo, sem timestamp, sem env nova.
//! - Teto de 512 KiB por arquivo, fixo em const (D5). O truncamento é NOTADO:
//!   linha marcadora no arquivo + nota acionável no resumo do `kira scan` —
//!   evidência silenciosamente cortada não é evidência.
//! - Redação na ESCRITA (D6, padrão ADR-0021): toda linha passa por `crate::redact`
//!   ANTES de tocar o disco — nada de segredo em repouso, nem em dado derivado.
//! - O texto gravado já saiu do `decode_tool_output` (BOM/UTF-16): o raw nasce
//!   UTF-8 válido por construção.
//! - Fail-open: o raw é evidência SECUNDÁRIA — falha de escrita vira nota no
//!   resumo, nunca mata o scan (mesmo contrato do store e do índice).

use std::path::PathBuf;

use crate::redact::redact;

/// Teto por arquivo (D5): 512 KiB fixos em const — um raw que estoura isso não
/// serve para citação com endereço de linha de qualquer jeito.
const RAW_CAP_BYTES: usize = 512 * 1024;

/// Reserva para a linha marcadora `# truncado em <N> bytes` (512 KiB tem 6
/// dígitos — 32 bytes de folga bastam e evitam aritmética circular de
/// "o marcador entra antes de saber o tamanho do número").
const MARKER_RESERVE: usize = 32;

/// Saída decodificada de UMA execução de ferramenta: o que o scan_dir obteve.
/// Por que um struct e não dois parâmetros soltos: o par (exit, stdout) viaja
/// junto de dentro do `scan_dir` até cá — dar nome aos campos evita inverter
/// a ordem em algum call site futuro.
pub struct RawRun {
    /// Exit code da ferramenta (o `code()` real; -1 quando foi morta por sinal).
    pub exit: i32,
    /// Stdout DECODIFICADO (UTF-8 garantido pelo `decode_tool_output`).
    pub stdout: String,
}

/// Slug válido para o nome do arquivo: só `[a-z0-9_-]`, não vazio.
///
/// Por que este é O guard de path traversal (aceite (d) da PR37): o nome da
/// ferramenta é o único dado externo que vira componente de caminho — validar
/// o alfabeto ANTES de montar o path fecha `../`, separadores e casing de
/// Windows (NTFS é case-insensitive; `Npm` e `npm` seriam o mesmo arquivo).
fn valid_tool_slug(tool: &str) -> bool {
    !tool.is_empty()
        && tool
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Caminho do raw de uma ferramenta, com o guard acionável.
pub fn raw_path(root: &std::path::Path, tool: &str) -> Result<PathBuf, String> {
    if !valid_tool_slug(tool) {
        // Erro acionável: diz o que é aceitável, não devolve o input cru como explicação.
        return Err(format!(
            "nome de ferramenta inválido para o raw '{tool}' — use só [a-z0-9_-]; \
se isso veio do código do KIRA, reporte o bug no repo"
        ));
    }
    Ok(root.join(".kira").join("raw").join(format!("{tool}.json")))
}

/// Grava o digest bruto redigido. Devolve:
/// - `Ok(None)` — gravou inteiro, nada a notar;
/// - `Ok(Some(nota))` — gravou TRUNCADO no teto: a nota vai para o resumo do
///   scan (truncamento notado, nunca silencioso);
/// - `Err(causa)` — guard recusado ou I/O falhou: o chamador transforma em nota
///   de aviso (fail-open) e o scan segue.
pub fn write_raw(
    root: &std::path::Path,
    tool: &str,
    run: &RawRun,
) -> Result<Option<String>, String> {
    let path = raw_path(root, tool)?;

    // Corpo: linha por linha já redigida (redação na escrita, ADR-0021).
    let body: String = run
        .stdout
        .lines()
        .map(redact)
        .collect::<Vec<_>>()
        .join("\n");

    // Header de 3 linhas previsto na te-implementacao §7: proveniência mínima
    // (`# tool` / `# exit` / `# lines`) sem timestamp — determinismo intacto.
    let lines = body.lines().count();
    let header = format!("# tool: {tool}\n# exit: {}\n# lines: {lines}\n", run.exit);

    // Teto por ARQUIVO: header + corpo + reserva do marcador ≤ RAW_CAP_BYTES.
    // Corta em fronteira de char (o corpo pode ter UTF-8 de verdade — cortar
    // no meio de um codepoint corromperia a última linha da evidência).
    let budget = RAW_CAP_BYTES.saturating_sub(header.len() + MARKER_RESERVE);
    let (kept, truncated_at) = if body.len() > budget {
        let cut = cut_at_char_boundary(&body, budget);
        (&body[..cut], Some(cut))
    } else {
        (&body[..], None)
    };

    let mut text = format!("{header}{kept}");
    if let Some(cut) = truncated_at {
        // Marcador É a nota dentro do arquivo: quem abrir o raw sem ler o
        // resumo ainda vê que está lendo um digest parcial.
        text.push_str(&format!("\n# truncado em {cut} bytes\n"));
    } else if !kept.is_empty() {
        // Fim de linha final só quando há corpo — e o marcador já fecha com \n.
        text.push('\n');
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            format!(
                "criar {} falhou ({source}) — confira permissões do projeto",
                parent.display()
            )
        })?;
    }
    std::fs::write(&path, &text).map_err(|source| {
        format!(
            "escrever {} falhou ({source}) — confira permissões do projeto",
            path.display()
        )
    })?;

    Ok(truncated_at.map(|_| {
        format!(
            "raw de {tool} passou do teto de {} KiB e foi truncado — a linha \
'# truncado em' em .kira/raw/{tool}.json marca onde; o JSON completo fica com a \
própria ferramenta (rode-a à mão para capturar tudo)",
            RAW_CAP_BYTES / 1024
        )
    }))
}

/// Maior índice ≤ `max` que é fronteira de char (a fila anda para trás no
/// máximo 3 bytes — UTF-8 não tem continuation run maior que isso).
fn cut_at_char_boundary(s: &str, max: usize) -> usize {
    let mut cut = max.min(s.len());
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::decode::decode_tool_output;

    /// Diretório único por teste (nunca tocar o repo nem o store real).
    fn temp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kira-raw-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // resto de execução anterior não contamina
        std::fs::create_dir_all(&dir).expect("criar raiz de teste");
        dir
    }

    #[test]
    fn valid_slug_builds_the_expected_path() {
        let root = temp_root("path-ok");
        let path = raw_path(&root, "npm").expect("npm é slug válido");
        assert_eq!(path, root.join(".kira").join("raw").join("npm.json"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Aceite (d): path traversal é RECUSADO antes de qualquer I/O — nada é
    /// gravado fora de `.kira/` (nem dentro, nem em lugar nenhum).
    #[test]
    fn traversal_and_casing_are_recused_without_writing() {
        let root = temp_root("traversal");
        let run = RawRun {
            exit: 0,
            stdout: "{\"vulnerabilities\":{}}".to_string(),
        };
        for bad in ["../evil", "sub/evac", "a\\b", "..", "Npm", "npm ", ""] {
            assert!(
                write_raw(&root, bad, &run).is_err(),
                "slug '{bad}' não pode passar"
            );
        }
        assert!(
            !root.join(".kira").exists(),
            "o guard recusa ANTES de criar diretório algum"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Aceite (b): token no corpo sai `[REDACTED]` NO ARQUIVO EM DISCO — a
    /// redação é na escrita, então a prova é contra o arquivo, não contra o
    /// retorno da função.
    #[test]
    fn secret_is_redacted_on_disk() {
        let root = temp_root("redact");
        let run = RawRun {
            exit: 0,
            stdout:
                "{\"message\":\"Authorization: Bearer AbC123XyZ789QqQ\",\"vulnerabilities\":{}}"
                    .to_string(),
        };
        write_raw(&root, "npm", &run).expect("raw tem que gravar");
        let on_disk = std::fs::read_to_string(root.join(".kira").join("raw").join("npm.json"))
            .expect("ler raw de volta");
        assert!(
            !on_disk.contains("AbC123XyZ789QqQ"),
            "segredo vazou: {on_disk}"
        );
        assert!(on_disk.contains("[REDACTED]"), "{on_disk}");
        assert!(
            on_disk.starts_with("# tool: npm\n# exit: 0\n# lines: 1\n"),
            "{on_disk}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Aceite do M1 no artefato novo: mesma entrada ⇒ mesmos bytes (nome fixo,
    /// overwrite, sem timestamp) — e o overwrite NÃO acumula linha.
    #[test]
    fn same_input_is_byte_identical_across_writes() {
        let root = temp_root("determinismo");
        let run = RawRun {
            exit: 1,
            stdout: "{\"vulnerabilities\":{\"lodash\":{\"via\":[\"GHSA-35jh\"]}}}".to_string(),
        };
        write_raw(&root, "pnpm", &run).expect("primeira escrita");
        let first = std::fs::read(root.join(".kira").join("raw").join("pnpm.json"))
            .expect("ler primeiro raw");
        write_raw(&root, "pnpm", &run).expect("segunda escrita");
        let second = std::fs::read(root.join(".kira").join("raw").join("pnpm.json"))
            .expect("ler segundo raw");
        assert_eq!(first, second, "determinismo do raw");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Aceite (a): corpo acima do teto é cortado com a linha marcadora, o
    /// arquivo fica ≤ 512 KiB e a NOTA sai (truncamento nunca silencioso).
    #[test]
    fn oversized_body_is_truncated_with_marker_and_note() {
        let root = temp_root("truncado");
        let long_line = "x".repeat(RAW_CAP_BYTES + 1024);
        let run = RawRun {
            exit: 0,
            stdout: long_line,
        };
        let note = write_raw(&root, "dotnet", &run)
            .expect("gravação truncada também é sucesso")
            .expect("corpo acima do teto TEM que gerar nota");
        assert!(note.contains("512 KiB"), "{note}");
        let path = root.join(".kira").join("raw").join("dotnet.json");
        let on_disk = std::fs::read(&path).expect("ler raw");
        assert!(
            on_disk.len() <= RAW_CAP_BYTES,
            "arquivo passou do teto: {} bytes",
            on_disk.len()
        );
        let text = String::from_utf8(on_disk).expect("truncamento em fronteira de char");
        assert!(
            text.contains("# truncado em "),
            "marcador tem que estar no arquivo: {}...",
            &text[..200]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Aceite (c): entrada UTF-16LE com BOM decodificada pelo mesmo
    /// `decode_tool_output` do M7 ⇒ raw em UTF-8 válido em disco (a prova é
    /// o `read_to_string` — UTF-8 inválido não passaria).
    #[test]
    fn utf16_input_lands_as_valid_utf8_on_disk() {
        let root = temp_root("utf16");
        let json = "{\"vulnerabilities\":{}}";
        let mut bytes = vec![0xFF, 0xFE]; // BOM UTF-16LE
        for unit in json.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let run = RawRun {
            exit: 0,
            stdout: decode_tool_output(&bytes),
        };
        write_raw(&root, "npm", &run).expect("raw tem que gravar");
        let on_disk = std::fs::read_to_string(root.join(".kira").join("raw").join("npm.json"))
            .expect("UTF-8 válido com o JSON intacto");
        assert!(on_disk.contains(json), "{on_disk}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_stdout_writes_header_only() {
        let root = temp_root("vazio");
        let run = RawRun {
            exit: 0,
            stdout: String::new(),
        };
        write_raw(&root, "npm", &run).expect("raw vazio também é evidência");
        let on_disk = std::fs::read_to_string(root.join(".kira").join("raw").join("npm.json"))
            .expect("ler raw vazio");
        assert_eq!(on_disk, "# tool: npm\n# exit: 0\n# lines: 0\n");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cut_stops_at_a_char_boundary() {
        let s = "ábç".repeat(4); // cada á/bç tem bytes multibyte
        let cut = cut_at_char_boundary(&s, 4);
        assert!(s.is_char_boundary(cut));
        // 4 cai no meio do segundo char — o corte volta para a fronteira.
        assert!(cut < 4 || s.is_char_boundary(4));
    }
}
