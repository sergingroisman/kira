//! Limpeza da saída do subprocesso antes de desenhar (ADR-0033 §6).
//!
//! Duas funções com papéis distintos e ordem obrigatória:
//! 1. [`sanitize_line`] tira ANSI/OSC/caracteres de controle e resolve `\r`
//!    (barra de progresso: só o último segmento importa);
//! 2. [`push_line`] aplica `crate::redact` e o teto de buffer.
//!
//! Segredo nunca aparece no overlay: mesmo que o filho ecoe um token, a linha
//! é redigida antes de entrar no `ExecState` — e o `ExecState` não vai para o
//! DB (o log não é trilha).

use crate::redact;

/// Teto de linhas guardadas no overlay de execução.
pub const MAX_LINES: usize = 5_000;

/// Teto de bytes somados das linhas guardadas (512 KiB).
pub const MAX_BYTES: usize = 512 * 1024;

/// De onde a linha veio (o rótulo `stdout`/`stderr` do overlay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

/// Uma linha já sanitizada e redigida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogLine {
    Stdout(String),
    Stderr(String),
    /// Nota do próprio KIRA (cancelamento, teto, causa).
    Note(String),
}

impl LogLine {
    /// Texto puro, sem rótulo (o widget escolhe a cor por variante).
    pub fn text(&self) -> &str {
        match self {
            LogLine::Stdout(text) | LogLine::Stderr(text) | LogLine::Note(text) => text,
        }
    }
}

/// Remove ANSI (CSI/OSC), `\r` de progresso, tabs e controles C0/C1.
///
/// Determinística e total: qualquer entrada vira texto desenhável, sem panic.
pub fn sanitize_line(raw: &str) -> String {
    // `\r` sobrescreve a linha: o último segmento não-vazio é o que "ficou".
    let last = raw
        .rsplit('\r')
        .find(|segment| !segment.trim().is_empty())
        .unwrap_or("");
    let mut out = String::with_capacity(last.len());
    let mut chars = last.chars().peekable();
    while let Some(current) = chars.next() {
        match current {
            '\u{1b}' => match chars.peek().copied() {
                // CSI: ESC [ … byte final @–~
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                // OSC: ESC ] … BEL ou ST (ESC \)
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Sequência curta de 2 chars (ex.: ESC (B).
                Some(_) => {
                    chars.next();
                }
                None => {}
            },
            '\t' => out.push_str("    "),
            c if (c as u32) < 0x20 || (0x7f..=0x9f).contains(&(c as u32)) => {}
            c => out.push(c),
        }
    }
    out.trim_end().to_string()
}

/// Empilha uma linha: sanitiza, redige e respeita os dois tetos.
///
/// Linha que era só escape (`\r`, cor, cursor) não vira linha vazia na tela;
/// `dropped` conta o que saiu pela frente para o overlay avisar honestamente.
pub fn push_line(lines: &mut Vec<LogLine>, dropped: &mut usize, stream: Stream, raw: &str) {
    let clean = sanitize_line(raw);
    if clean.is_empty() && !raw.trim().is_empty() {
        return; // era só controle: não há o que mostrar
    }
    let safe = redact::redact(&clean);
    match stream {
        Stream::Out => lines.push(LogLine::Stdout(safe)),
        Stream::Err => lines.push(LogLine::Stderr(safe)),
    }
    trim(lines, dropped);
}

/// Nota do KIRA (cancelamento, teto atingido) — também redigida.
pub fn push_note(lines: &mut Vec<LogLine>, dropped: &mut usize, text: &str) {
    lines.push(LogLine::Note(redact::redact(text)));
    trim(lines, dropped);
}

/// Aplica os tetos removendo do INÍCIO (o que acabou de chegar é o que importa).
fn trim(lines: &mut Vec<LogLine>, dropped: &mut usize) {
    let mut bytes: usize = lines.iter().map(|line| line.text().len()).sum();
    while lines.len() > MAX_LINES || (bytes > MAX_BYTES && lines.len() > 1) {
        let removed = lines.remove(0);
        bytes = bytes.saturating_sub(removed.text().len());
        *dropped += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_log_line_strips_ansi_osc_cr_and_controls() {
        assert_eq!(sanitize_line("\u{1b}[31mvermelho\u{1b}[0m"), "vermelho");
        assert_eq!(sanitize_line("\u{1b}]0;título\u{7}texto"), "texto");
        // Barra de progresso: só o último estado fica.
        assert_eq!(sanitize_line("  0%\r 40%\r100%"), "100%");
        assert_eq!(sanitize_line("a\tb"), "a    b");
        assert_eq!(sanitize_line("a\u{7}b\u{0}"), "ab");
        assert_eq!(sanitize_line("\u{1b}[2K\u{1b}[1G"), "");
        // Sem panic em lixo aberto.
        let _ = sanitize_line("\u{1b}[");
        let _ = sanitize_line("\u{1b}]0;sem fim");
    }

    #[test]
    fn redact_runs_after_sanitize() {
        let mut lines = Vec::new();
        let mut dropped = 0;
        push_line(
            &mut lines,
            &mut dropped,
            Stream::Err,
            "\u{1b}[33mPAT = Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT\u{1b}[0m",
        );
        assert_eq!(lines.len(), 1);
        let text = lines[0].text();
        assert!(!text.contains("Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT"), "{text}");
        assert!(text.contains("[REDACTED]"), "{text}");
        assert!(matches!(lines[0], LogLine::Stderr(_)));
        // Linha vazia (ou só controle) não ocupa espaço.
        push_line(&mut lines, &mut dropped, Stream::Out, "\u{1b}[0m");
        assert_eq!(lines.len(), 1);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn line_and_byte_caps_drop_the_oldest() {
        let mut lines = Vec::new();
        let mut dropped = 0;
        for index in 0..MAX_LINES + 3 {
            push_line(
                &mut lines,
                &mut dropped,
                Stream::Out,
                &format!("linha {index}"),
            );
        }
        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(dropped, 3);
        assert_eq!(lines[0].text(), "linha 3", "o início é o que sai");

        let mut lines = Vec::new();
        let mut dropped = 0;
        let big = "x".repeat(MAX_BYTES / 4 + 1);
        for _ in 0..6 {
            push_line(&mut lines, &mut dropped, Stream::Out, &big);
        }
        assert!(
            lines.len() < 6,
            "o teto de bytes também vale: {}",
            lines.len()
        );
        assert!(dropped > 0);
    }

    #[test]
    fn notes_are_redacted_too() {
        let mut lines = Vec::new();
        let mut dropped = 0;
        // A forma `chave=valor` é a que o `redact` cobre (um PAT nu no meio de
        // uma frase livre não é reconhecível sem contexto).
        push_note(
            &mut lines,
            &mut dropped,
            "pat = Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT recusado",
        );
        assert!(!lines[0].text().contains("Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT"));
        assert!(matches!(lines[0], LogLine::Note(_)));
    }
}
