//! Wrapper do revisor externo **rea** (onda TE, PR41): o dono chama
//! `kira rea <args>` e o KIRA só orquestra — resolve o binário ANTES do
//! spawn (mesma régua do scan, [`crate::binresolve`]), captura stdout e
//! stderr e imprime a saída SEMPRE redigida por [`crate::redact`] e capada
//! em [`OUTPUT_CAP_BYTES`]. O output cru do rea nunca chega ao terminal do
//! dono — a lição do scan vale por extenso: o corpo da ferramenta é dado
//! não-confiável e pode carregar segredo.
//!
//! Escopo fechado do PR41: sem daemon, sem LLM, sem escrita no board ou no
//! repo. Falha do binário ⇒ erro acionável compacto (primeira linha de
//! stderr, ≤200 chars) — nunca o corpo cru. `KIRA_REA_BIN` segue a convenção
//! `KIRA_SCAN_<TOOL>_BIN` (valor presente e não vazio é contrato do usuário —
//! o arquivo apontado spawna como veio; inexistente ⇒ o mesmo "instale o
//! rea" SEM spawn).

use std::path::PathBuf;
use std::process::Command;

use crate::error::Error;

/// Override de binário (mesma convenção de `KIRA_SCAN_<TOOL>_BIN`).
pub const REA_ENV: &str = "KIRA_REA_BIN";

/// Cap de saída: 8 KiB — coerente com `MAX_EXCERPT_CHARS` × n do packer:
/// o wrapper é leitura de revisão, não substitui o arquivo completo.
pub const OUTPUT_CAP_BYTES: usize = 8 * 1024;

/// Programa procurado no PATH quando não há override.
const PROGRAM: &str = "rea";

/// Binário resolvido + se o spawn precisa do wrapper `cmd /C` (só Windows,
/// para `.cmd`/`.bat` — o std os executaria como binário).
#[derive(Debug)]
pub(crate) struct Resolved {
    path: PathBuf,
    via_cmd: bool,
}

/// Resolução no padrão da casa (`binresolve`): 1) override `KIRA_REA_BIN`
/// (presente e não vazio = contrato — arquivo inexistente ⇒ "instale o rea"
/// sem spawn); 2) PATH × PATHEXT; 3) nada ⇒ [`Error::ReaMissing`]. NUNCA
/// parseia texto localizado do OS: a busca é função pura do valor do PATH.
pub(crate) fn resolve_explicit(
    override_bin: Option<&str>,
    path_env: Option<&std::ffi::OsStr>,
) -> Result<Resolved, Error> {
    if let Some(bin) = override_bin {
        let path = PathBuf::from(bin);
        if !path.is_file() {
            return Err(Error::ReaMissing);
        }
        return Ok(Resolved {
            via_cmd: cfg!(windows) && crate::binresolve::is_cmd_script(bin),
            path,
        });
    }
    match crate::binresolve::find_on_path(path_env, PROGRAM) {
        Some(path) => Ok(Resolved {
            via_cmd: cfg!(windows) && crate::binresolve::is_cmd_script(&path.to_string_lossy()),
            path,
        }),
        None => Err(Error::ReaMissing),
    }
}

/// Resolução lendo o ambiente de verdade (o call site do comando).
pub(crate) fn resolve() -> Result<Resolved, Error> {
    let override_bin = std::env::var(REA_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let path_env = std::env::var_os("PATH");
    resolve_explicit(override_bin.as_deref(), path_env.as_deref())
}

/// Monta o `Command` com o caminho RESOLVIDO (nunca o nome nu): `.cmd`/`.bat`
/// no Windows passam pelo mesmo wrapper `cmd /C` do scan (o código do scan
/// vive lá — aqui só delegamos a construção de linha para `shellline`).
fn command_from_resolved(resolved: &Resolved, args: &[String]) -> Command {
    if resolved.via_cmd {
        let owned: Vec<std::ffi::OsString> = args.iter().map(std::ffi::OsString::from).collect();
        let line = crate::shellline::cmd_command_line(&resolved.path, &owned);
        let mut command = Command::new("cmd");
        command.arg("/C");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.raw_arg(line);
        }
        #[cfg(not(windows))]
        {
            // Inalcançável (via_cmd só é true no Windows), mas mantém o build
            // honesto em outros alvos e nos testes.
            let _ = line;
            command.arg(&resolved.path);
            command.args(args);
        }
        command
    } else {
        let mut command = Command::new(&resolved.path);
        command.args(args);
        command
    }
}

/// Trunca `text` no teto de [`OUTPUT_CAP_BYTES`] SEM cortar um char UTF-8
/// ao meio (o rea pode imprimir acento/emoji; o cap é em bytes, o corte é
/// em char boundary — mesmo comportamento do `first_stderr_line` capped).
fn cap_output(text: &str) -> (String, bool) {
    if text.len() <= OUTPUT_CAP_BYTES {
        return (text.to_string(), false);
    }
    let mut end = OUTPUT_CAP_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

/// Linha marcadora quando a saída passou do teto — honesta sobre o que
/// faltou (o resto está no rea, não no KIRA).
pub fn truncation_marker() -> String {
    format!(
        "... (saída do rea truncada em {} KiB — rode o rea direto para o restante)",
        OUTPUT_CAP_BYTES / 1024
    )
}

/// Roda `kira rea <args>`: resolve, spawna com os args passthrough e devolve
/// o texto para imprimir (stdout + stderr capturados, cada linha redigida,
/// cap único de [`OUTPUT_CAP_BYTES`] somando os dois streams). Exit ≠ 0 ⇒
/// [`Error::ReaFailed`] com a primeira linha de stderr como dica — o corpo
/// cru nunca volta.
pub fn run(args: &[String]) -> Result<String, Error> {
    let resolved = resolve()?;
    let output = command_from_resolved(&resolved, args)
        .output()
        .map_err(|source| Error::ReaSpawn { source })?;

    if !output.status.success() {
        let exit = output
            .status
            .code()
            .map_or_else(|| "sinal".to_string(), |code| code.to_string());
        let hint = crate::binresolve::first_stderr_line(&String::from_utf8_lossy(&output.stderr));
        let hint = if hint.is_empty() {
            String::new()
        } else {
            format!(" (dica: {hint})")
        };
        return Err(Error::ReaFailed { exit, hint });
    }

    // Redação linha a linha (padrão ADR-0021: nada cru para fora) — a saída
    // completa é composta antes do cap para o corte cair onde der (o dono
    // vê o começo do relatório, nunca um stream cortado no meio de linha).
    let mut full = String::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        full.push_str(&crate::redact::redact(line));
        full.push('\n');
    }
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    if !stderr_text.trim().is_empty() {
        for line in stderr_text.lines() {
            full.push_str(&crate::redact::redact(line));
            full.push('\n');
        }
    }

    let (capped, truncated) = cap_output(&full);
    let mut text = capped;
    if truncated {
        text.push('\n');
        text.push_str(&truncation_marker());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_empty_is_ignored_and_missing_path_is_actionable() {
        // Override vazio/branco não é contrato — vale o PATH.
        let err = resolve_explicit(Some("   "), None).unwrap_err();
        assert!(err.to_string().contains("instale o rea"), "{err}");
        assert!(err.to_string().contains("KIRA_REA_BIN"), "{err}");
    }

    #[test]
    fn override_missing_file_never_spawns() {
        let err = resolve_explicit(Some("/caminho/que/não/existe/rea"), None).unwrap_err();
        assert!(err.to_string().contains("instale o rea"), "{err}");
    }

    #[test]
    fn cap_respects_char_boundary() {
        // 'é' são 2 bytes. Com um 'a' na frente, as fronteiras de char caem
        // nos índices ÍMPARES, então o teto de 8192 (par) cai no MEIO de um
        // char — o cap tem que recuar para a fronteira anterior.
        let mut accent = String::from("a");
        accent.push_str(&"é".repeat(OUTPUT_CAP_BYTES / 2));
        assert!(
            !accent.is_char_boundary(OUTPUT_CAP_BYTES),
            "o fixture precisa cortar no meio de um char"
        );
        let (capped, truncated) = cap_output(&accent);
        assert!(truncated);
        assert!(accent.is_char_boundary(capped.len()));
        assert_eq!(capped.len(), OUTPUT_CAP_BYTES - 1);
        // O corte caiu entre dois 'é' (não no meio de um): o último char é
        // inteiro, e o 'a' do começo continua lá.
        assert!(capped.starts_with('a'));
        assert!(capped.ends_with('é'));
    }

    #[test]
    fn cap_under_limit_is_untouched() {
        let (capped, truncated) = cap_output("curto");
        assert!(!truncated);
        assert_eq!(capped, "curto");
    }

    #[test]
    fn truncation_marker_explains_the_cap() {
        let marker = truncation_marker();
        assert!(marker.contains("8 KiB"), "{marker}");
        assert!(marker.contains("rea"), "{marker}");
    }
}
