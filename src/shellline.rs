//! Montagem da linha de comando para `cmd.exe` (Windows).
//!
//! Existe porque o mesmo bug nasceu duas vezes em lugares diferentes: passar
//! `chcp 65001 >nul 2>&1 &` como um ARGUMENTO ao lado do programa faz o
//! `Command` do std re-citar o conjunto, e o `cmd` passa a ler a string inteira
//! como nome de programa — o erro real no Windows é
//! `The filename, directory name, or volume label syntax is incorrect.`
//!
//! A regra é única: `cmd /C` recebe **uma** linha, com o `chcp` embutido, cada
//! token citado e a linha montada aqui (nunca pelo `Command` do std).

use std::ffi::OsString;
use std::path::Path;

/// Prefixo que liga o code page UTF-8 na sessão do `cmd` (o output de uma
/// ferramenta real chega no code page do console; sem isso, mojibake).
pub(crate) const CHCP_PREFIX: &str = "chcp 65001 >nul 2>&1 & ";

/// Monta a linha única: `chcp ... & "programa" "arg1" "arg2"`.
pub(crate) fn cmd_command_line(bin: &Path, args: &[OsString]) -> String {
    let mut line = String::from(CHCP_PREFIX);
    line.push_str(&cmd_quote(&bin.to_string_lossy()));
    for arg in args {
        line.push(' ');
        line.push_str(&cmd_quote(&arg.to_string_lossy()));
    }
    line
}

/// Cita um token para o `cmd.exe`: entre aspas, aspas internas viram apóstrofo
/// (`cmd` não tem escape confiável para `"` dentro de argumento citado) e
/// CR/LF viram espaço (uma quebra de linha encerraria o comando).
pub(crate) fn cmd_quote(raw: &str) -> String {
    let mut quoted = String::with_capacity(raw.len() + 2);
    quoted.push('"');
    for ch in raw.chars() {
        match ch {
            '"' => quoted.push('\''),
            '\r' | '\n' => quoted.push(' '),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn line_is_single_and_starts_with_the_code_page_prefix() {
        let line = cmd_command_line(
            &PathBuf::from(r"C:\tools\npm.cmd"),
            &[OsString::from("audit"), OsString::from("--json")],
        );
        assert!(line.starts_with(CHCP_PREFIX), "{line}");
        assert_eq!(
            line,
            r#"chcp 65001 >nul 2>&1 & "C:\tools\npm.cmd" "audit" "--json""#
        );
        assert!(
            !line.contains('\n'),
            "a linha nunca pode ter quebra: {line}"
        );
    }

    #[test]
    fn paths_with_spaces_stay_one_token() {
        let line = cmd_command_line(
            &PathBuf::from(r"C:\Program Files\nodejs\npm.cmd"),
            &[OsString::from("audit")],
        );
        assert!(
            line.contains(r#""C:\Program Files\nodejs\npm.cmd""#),
            "{line}"
        );
    }

    #[test]
    fn quotes_become_apostrophes_and_newlines_become_spaces() {
        assert_eq!(cmd_quote("a\"b"), "\"a'b\"");
        assert_eq!(cmd_quote("linha1\r\nlinha2"), "\"linha1  linha2\"");
    }
}
