//! Renderizador markdown mínimo do painel de wiki (W6-PR4, ADR-0013).
//!
//! Feito aqui, do zero, SEM dependência nova: a wiki do KIRA escreve markdown
//! simples (`#` títulos, `-`/`*` listas, fence de código, negrito/itálico,
//! links, citação, régua) e o dashboard só precisa LER — um crate de markdown
//! completo compraria 90% de funcionalidade que nunca será pintada em texto.
//!
//! Contratos (docs/kira-w6-dashboard.md W6-PR4):
//! - **Nada de HTML executado**: tags viram texto literal — um renderizador
//!   terminal não executa nada, então `<script>` é só caracteres na tela;
//! - **Wrap finito**: linhas longas são quebradas na largura dada (nunca
//!   estouram a borda do painel, nunca scroll horizontal);
//! - **Determinístico**: a mesma entrada rende as mesmas linhas, sempre;
//! - **Markdown inválido nunca panica**: fence aberto, `**` solto, linha
//!   quebrada no meio de nada — tudo degrada para texto comum.

/// Quebra de linha por palavra quando a linha passa da largura. Função pura:
/// largura 1..=2 rende 1 char por linha (nunca 0, que significaria sumiço).
fn wrap_line(line: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if line.chars().count() <= width {
        return vec![line.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in line.split(' ') {
        // Palavra mais larga que a linha inteira: corta dura (sem criar
        // palavra infinita — o código pode ter URL gigante).
        if word.chars().count() > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut rest = word.to_string();
            while rest.chars().count() > width {
                let cut: String = rest.chars().take(width).collect();
                lines.push(cut);
                rest = rest.chars().skip(width).collect();
            }
            current = rest;
            continue;
        }
        if current.is_empty() {
            current = word.to_string();
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// Remove os marcadores de negrito (`**x**`/`__x__`) e itálico (`*x*`/`_x_`)
/// de uma linha — o terminal não tem fonte itálica honesta, então "desligar
/// o marcador" é a decisão da spec: a ênfase NÃO vira decoração inventada.
fn strip_inline_emphasis(line: &str) -> String {
    let mut out = line.to_string();
    for marker in ["**", "__", "*"] {
        // Loop while: pares consecutivos (**a** **b**) na mesma linha.
        while let Some(open) = out.find(marker) {
            let Some(close_rel) = out[open + marker.len()..].find(marker) else {
                break; // marcador solto: fica como texto (nada de sumir).
            };
            let inner = out[open + marker.len()..open + marker.len() + close_rel].to_string();
            if inner.is_empty() {
                // Par vazio (ex.: `**` solto, que o `*` enxerga como par
                // vazio): fica como texto — nada de sumir marcador.
                break;
            }
            out = format!(
                "{}{}{}",
                &out[..open],
                inner,
                &out[open + marker.len() + close_rel + marker.len()..]
            );
        }
    }
    out
}

/// `[texto](url)` → `texto (url)` — num terminal, a URL é parte da informação
/// (o dono leu num blog: precisa saber para onde o link aponta).
fn strip_links(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let Some(close_txt) = rest[open..].find("](") else {
            out.push_str(rest);
            return out;
        };
        let Some(close_url_rel) = rest[open + close_txt + 2..].find(')') else {
            out.push_str(rest);
            return out;
        };
        let text = &rest[open + 1..open + close_txt];
        let url = &rest[open + close_txt + 2..open + close_txt + 2 + close_url_rel];
        out.push_str(&rest[..open]);
        if url.is_empty() {
            out.push_str(text);
        } else {
            out.push_str(text);
            out.push_str(" (");
            out.push_str(url);
            out.push(')');
        }
        rest = &rest[open + close_txt + 2 + close_url_rel + 1..];
    }
    out.push_str(rest);
    out
}

/// Code span `` `x` `` → `x` (sem cor de sintaxe — mono é o contrato).
fn strip_code_spans(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let Some(close_rel) = rest[open + 1..].find('`') else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..open + 1 + close_rel]);
        rest = &rest[open + 1 + close_rel + 1..];
    }
    out.push_str(rest);
    out
}

/// Processamento inline de uma linha normal (fora de bloco de código).
fn inline(text: &str) -> String {
    let text = strip_code_spans(text);
    let text = strip_links(&text);
    strip_inline_emphasis(&text)
}

/// Linhas de régua conhecidas (`---`, `***`, `___`, com 3+ do mesmo char).
fn is_rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-' || c == '*' || c == '_')
}

/// Rende markdown → linhas de texto puras, quebradas em `width` colunas.
///
/// A função é PURA e total: qualquer entrada rende algo (nada de `Err`,
/// nada de panic) — é o contrato "markdown inválido não panica" do gate.
pub fn render(source: &str, width: usize) -> Vec<String> {
    render_with(source, width, crate::tui::glyph::Glyphs::default())
}

/// [`render`] com o conjunto de glifos da sessão (`KIRA_GLYPHS=ascii`).
///
/// O texto markdown é o mesmo; muda só o desenho dos marcadores (`▌ ■ · • │ ──`
/// viram `= - . * | --`). Um renderizador que ignora o modo de glifo entrega
/// caixinha em console antigo no meio da leitura da wiki.
pub fn render_with(source: &str, width: usize, glyphs: crate::tui::glyph::Glyphs) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut in_code = false;
    let quote = glyphs.quote();
    for raw in source.lines() {
        if in_code {
            // Fim do bloco: a linha de fechamento some, o conteúdo fica com
            // o prefixo da citação (a barra é a "borda" do bloco — densidade
            // da persona, zero cor nova).
            if raw.trim().starts_with("```") {
                in_code = false;
            } else {
                lines.extend(wrap_line(&format!("{quote} {raw}"), width));
            }
            continue;
        }
        let trimmed = raw.trim();
        if trimmed.starts_with("```") {
            in_code = true;
            continue;
        }
        // Citação: `> texto` → `│ texto` (mesma barra do código, sem cor).
        if let Some(quoted) = trimmed.strip_prefix('>') {
            let content = inline(quoted.trim_start());
            if content.is_empty() {
                lines.push(quote.to_string());
            } else {
                lines.extend(wrap_line(&format!("{quote} {content}"), width));
            }
            continue;
        }
        if is_rule(trimmed) {
            // Régua: linha curta, largura fixa (determinismo — não depende da
            // largura do terminal).
            lines.push(glyphs.rule().to_string());
            continue;
        }
        // Título: marcador por nível, NUNCA o `#` cru (a saída é texto lido).
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level >= 1 {
            let text = trimmed[level..].trim_start();
            if text.is_empty() {
                continue; // `#` solto não rende linha fantasma.
            }
            let marker = glyphs.heading(level);
            let title = inline(text);
            lines.extend(wrap_line(&format!("{marker} {title}"), width));
            continue;
        }
        // Item de lista: `- x` ou `* x` → `• x` (fora de bloco de código).
        if let Some(item) = trimmed.strip_prefix("- ").or(trimmed.strip_prefix("* ")) {
            let content = inline(item);
            lines.extend(wrap_line(&format!("{} {content}", glyphs.bullet()), width));
            continue;
        }
        let rendered = inline(trimmed);
        if rendered.is_empty() {
            lines.push(String::new());
        } else {
            lines.extend(wrap_line(&rendered, width));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: texto de N linhas rendidas.
    fn rendered(source: &str, width: usize) -> String {
        render(source, width).join("\n")
    }

    #[test]
    fn headings_get_level_markers_never_the_hash() {
        let out = rendered("# Guia\n## Seção\n### Detalhe\n#### Mais", 80);
        assert!(out.contains("▌ Guia"), "{out}");
        assert!(out.contains("■ Seção"), "{out}");
        assert!(out.contains("· Detalhe"), "{out}");
        assert!(out.contains("· Mais"), "{out}");
        assert!(!out.contains('#'), "o `#` cru NUNCA sai na tela: {out}");
    }

    #[test]
    fn lists_become_bullets_for_dash_and_star() {
        let out = rendered("- item um\n* item dois", 80);
        assert!(out.contains("• item um"), "{out}");
        assert!(out.contains("• item dois"), "{out}");
        assert!(!out.contains("- item"), "{out}");
    }

    #[test]
    fn code_blocks_get_the_bar_prefix_and_no_syntax_color() {
        let out = rendered(
            "antes\n```rust\nlet x = 1;\nlet y = *x; // star\n```\ndepois",
            80,
        );
        assert!(out.contains("│ let x = 1;"), "{out}");
        // O `*` DENTRO do código é preservado (conteúdo literal de código).
        assert!(out.contains("let y = *x;"), "{out}");
        assert!(!out.contains("```"), "{out}");
        // E o bloco termina: `depois` volta ao fluxo normal sem prefixo.
        assert!(out.lines().any(|l| l == "depois"), "{out}");
    }

    #[test]
    fn unclosed_code_fence_never_panics_and_keeps_text() {
        // Markdown inválido de propósito: fence aberto sem fechamento —
        // todo o resto rende como código, sem panic.
        let out = rendered("```sql\nSELECT 1;\ne até aqui", 80);
        assert!(out.contains("│ SELECT 1;"), "{out}");
        assert!(out.contains("e até aqui"), "{out}");
    }

    #[test]
    fn code_spans_links_bold_and_italic_strip_their_markers() {
        let out = rendered(
            "use `kira scan` e [docs](https://exemplo.com) **negrito** e *itálico*",
            80,
        );
        assert!(out.contains("use kira scan"), "{out}");
        assert!(out.contains("docs (https://exemplo.com)"), "{out}");
        assert!(out.contains("negrito"), "{out}");
        assert!(out.contains("itálico"), "{out}");
        assert!(!out.contains("**"), "{out}");
        assert!(!out.contains('`'), "{out}");
    }

    #[test]
    fn quotes_rules_and_empty_lines_render() {
        let out = rendered("> aho\n\n---\n\ndepois", 80);
        assert!(out.contains("│ aho"), "{out}");
        assert!(out.contains("────"), "{out}");
        assert!(out.contains("depois"), "{out}");
    }

    #[test]
    fn html_tags_stay_as_literal_text_never_executed() {
        let out = rendered("veja <b>isto</b> e <script>alert(1)</script>", 80);
        // "Executar" seria sumir com a tag: a tag vira TEXTO na tela.
        assert!(out.contains("<b>isto</b>"), "{out}");
        assert!(out.contains("<script>alert(1)</script>"), "{out}");
    }

    #[test]
    fn long_lines_wrap_at_the_given_width_never_infinite() {
        let long = "palavra ".repeat(40);
        let lines = render(long.trim_end(), 20);
        assert!(
            lines.iter().all(|line| line.chars().count() <= 20),
            "toda linha dentro da largura: {lines:?}"
        );
        assert!(lines.len() > 1, "a linha longa foi quebrada: {lines:?}");
        // Largura 1 nunca rende linha vazia infinita.
        let narrow = render("abc", 1);
        assert_eq!(narrow, vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_source_renders_nothing_and_garbage_never_panics() {
        assert!(render("", 80).is_empty());
        // Casos inválidos clássicos: nada panica, tudo vira texto honesto.
        let garbage = "**\n[a](\n>[`\n```\n-  \n#### \n";
        let out = render(garbage, 40);
        assert!(out.len() < 40, "sem explosão de linhas: {out:?}");
        // E o mesmo com caracteres multi-byte cortados no wrap.
        let utf8 = render("açãoãêééççççç com emoji 🎉🎉🎉🎉🎉 e mais texto", 5);
        assert!(utf8.iter().all(|l| l.chars().count() <= 5), "{utf8:?}");
    }

    #[test]
    fn wrap_is_word_aware_for_normal_text() {
        let lines = render("curta tambem", 10);
        assert_eq!(lines, vec!["curta", "tambem"]);
    }

    #[test]
    fn loose_emphasis_marker_stays_as_text() {
        // `**` solto (sem par): permanece como texto — nada de sumir.
        let out = rendered("havia ** por aqui", 80);
        assert!(out.contains("** por aqui"), "{out}");
    }
}
