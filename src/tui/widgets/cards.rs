//! Os seis cartões do Resumo (PR-UX2) e o grid que os posiciona.
//!
//! Cada cartão é uma função que recebe a ÁREA ÚTIL e pinta o que existe no
//! snapshot. Nenhum inventa número: contagem, idade, estado e a causa do dado
//! ausente saem todos de [`Snapshot`], e o que não couber vira `…` (corte) ou
//! some na ordem de prioridade do [`crate::tui::grid`] — nunca um zero fingido.
//!
//! Ordem da IA (decisão do dono): ação primeiro — `Risco` e `Sessões` —,
//! contexto depois (`Projeto`, `Atividade`), registro no fim (`Índice e wiki`,
//! `Pendências`).
//!
//! A série de tendência NÃO aparece no 80×24 (o cartão de Risco usa as linhas
//! para barras e índice); ela continua inteira no `?` e no `--snapshot`. Tirar
//! do cartão é esconder na tela, não perder o dado.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::tui::action::{Action, ExecState};
use crate::tui::glyph::Glyphs;
use crate::tui::grid::{self, CardKind};
use crate::tui::state::{self, Snapshot};
use crate::tui::theme::Palette;
use crate::tui::widgets::card;
use crate::tui::widgets::chips;
use crate::tui::widgets::{gauge, severity, trend};

/// O que só existe na tela (não no snapshot): execução viva e o tick do spinner.
#[derive(Debug, Clone, Copy, Default)]
pub struct Live<'a> {
    /// Execução em andamento ou recém-terminada (`o` abre o log).
    pub exec: Option<&'a ExecState>,
    /// Contador do loop, para o spinner não congelar.
    pub tick: u64,
}

impl Live<'_> {
    /// Nenhuma execução na tela.
    pub const fn idle() -> Live<'static> {
        Live {
            exec: None,
            tick: 0,
        }
    }
}

/// Rótulo do comando que está rodando (`scan`, `triage 4821`, `pr draft 900`).
fn action_label(action: Action) -> String {
    match action {
        Action::Scan => "scan".to_string(),
        Action::Triage { work_item } => format!("triage {work_item}"),
        Action::PrDraft { work_item } => format!("pr draft {work_item}"),
    }
}

/// Desenha o Resumo inteiro: planeja o grid e pinta cartão por cartão.
pub fn render_grid(
    buf: &mut Buffer,
    area: Rect,
    snapshot: &Snapshot,
    live: Live<'_>,
    focused: bool,
    palette: Palette,
    glyphs: Glyphs,
) {
    // Tudo que é igual para todos os cartões num só lugar: foco, paleta e
    // glifos. Sem isso a assinatura de `render_card` passa de sete argumentos
    // (limite do clippy) e vira um saco de parâmetros posicionais.
    let paint = Paint {
        focused,
        palette,
        glyphs,
    };
    for slot in grid::plan(area).slots {
        render_card(buf, slot.area, slot.kind, snapshot, live, paint);
    }
}

/// O que cada cartão precisa saber igual: foco, paleta e glifos da sessão.
#[derive(Debug, Clone, Copy)]
struct Paint {
    focused: bool,
    palette: Palette,
    glyphs: Glyphs,
}

/// Um cartão: moldura, área útil e conteúdo.
fn render_card(
    buf: &mut Buffer,
    area: Rect,
    kind: CardKind,
    snapshot: &Snapshot,
    live: Live<'_>,
    paint: Paint,
) {
    let Paint {
        focused,
        palette,
        glyphs,
    } = paint;
    // O Risco mostra o total no TÍTULO quando não sobra linha dentro; assim o
    // número principal nunca some em 80×24.
    let title_override = match kind {
        CardKind::Risco if compact(area) => Some(format!(
            "{} · total {}",
            kind.title(),
            snapshot.finding_rows.len()
        )),
        _ => None,
    };
    let block = match &title_override {
        Some(title) => card::frame_titled(title, focused, palette, glyphs),
        None => card::frame(kind, focused, palette, glyphs),
    };
    let inner = block.inner(area);
    ratatui::widgets::Widget::render(block, area, buf);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    match kind {
        CardKind::Risco => risco(inner, buf, snapshot, compact(area), palette, glyphs),
        CardKind::Sessoes => sessoes(inner, buf, snapshot, live, palette, glyphs),
        CardKind::Projeto => projeto(inner, buf, snapshot, palette),
        CardKind::Atividade => atividade(inner, buf, snapshot, palette, glyphs),
        CardKind::IndiceWiki => indice_wiki(inner, buf, snapshot, palette, glyphs),
        CardKind::Pendencias => pendencias(inner, buf, snapshot, palette),
    }
}

/// Cartão apertado: sem linha sobrando para total/ferramentas.
fn compact(area: Rect) -> bool {
    area.height.saturating_sub(2) < 6
}

fn risco(
    inner: Rect,
    buf: &mut Buffer,
    snapshot: &Snapshot,
    compact: bool,
    palette: Palette,
    glyphs: Glyphs,
) {
    let severities = state::severity_bars(snapshot);
    // A última linha útil é o índice; com folga, a penúltima é total/ferramentas.
    let gauge_row = inner.height.saturating_sub(1);
    let text_row = if compact {
        None
    } else {
        Some(gauge_row.saturating_sub(1))
    };
    let bars_budget = text_row.unwrap_or(gauge_row);

    if severities.is_empty() {
        card::put(
            buf,
            inner,
            0,
            &state::empty_findings_line(snapshot),
            Style::default().fg(palette.missing),
        );
    } else {
        let mut area = card::row_rect(inner, 0);
        area.height = bars_budget;
        severity::render(area, buf, &severities, palette, glyphs);
    }

    if let Some(row) = text_row {
        let tools = state::tools_line(&snapshot.finding_rows);
        let text = if tools.is_empty() {
            format!("total {}", snapshot.finding_rows.len())
        } else {
            format!("total {} · {tools}", snapshot.finding_rows.len())
        };
        card::put(buf, inner, row, &text, Style::default().fg(palette.value));
    }

    match state::index_usage(snapshot) {
        Some((used, cap)) => gauge::render(
            card::row_rect(inner, gauge_row),
            buf,
            "índice",
            used,
            cap,
            palette,
            glyphs,
        ),
        None => card::put(
            buf,
            inner,
            gauge_row,
            &state::index_line(snapshot),
            Style::default().fg(palette.missing),
        ),
    }
}

fn sessoes(
    inner: Rect,
    buf: &mut Buffer,
    snapshot: &Snapshot,
    live: Live<'_>,
    palette: Palette,
    glyphs: Glyphs,
) {
    let mut row = 0;
    // Execução viva primeiro: é o único estado que muda sozinho na tela.
    if let Some(exec) = live.exec.filter(|exec| exec.is_running()) {
        let elapsed_secs = snapshot
            .now_secs
            .map(|now| (now - exec.started_unix).max(0))
            .unwrap_or(0);
        let line = chips::running_line(
            &action_label(exec.action),
            &format!(
                "executando há {}",
                state::format_duration_ms(elapsed_secs * 1_000)
            ),
            live.tick,
            palette,
            glyphs,
        );
        render_line(buf, inner, row, line, glyphs);
        row += 1;
    }
    if snapshot.session_rows.is_empty() && row == 0 {
        card::put(
            buf,
            inner,
            0,
            &state::empty_sessions_line(snapshot),
            Style::default().fg(palette.missing),
        );
        return;
    }
    for session in &snapshot.session_rows {
        if row >= inner.height {
            break;
        }
        let line = chips::session_line(session, snapshot.now_secs, palette, glyphs);
        render_line(buf, inner, row, line, glyphs);
        row += 1;
    }
}

fn projeto(inner: Rect, buf: &mut Buffer, snapshot: &Snapshot, palette: Palette) {
    let project = snapshot
        .project
        .as_ref()
        .map(|project| project.name.as_str())
        .unwrap_or("—");
    let root = snapshot
        .project
        .as_ref()
        .map(|project| project.root_path.as_str())
        .unwrap_or("—");
    let dev = match (&snapshot.dev_name, &snapshot.dev_note) {
        (Some(name), _) => name.as_str(),
        (None, Some(note)) => note.as_str(),
        (None, None) => "—",
    };
    let pat = match (&snapshot.pat_masked, &snapshot.pat_source) {
        (Some(masked), Some(source)) => format!("{masked} ({source})"),
        (Some(masked), None) => masked.clone(),
        (None, Some(note)) => note.clone(),
        (None, None) => "—".to_string(),
    };
    card::put(
        buf,
        inner,
        0,
        &format!("proj {project} · root {root}"),
        Style::default().fg(palette.value),
    );
    card::put(
        buf,
        inner,
        1,
        &format!("dev {dev} · pat {pat}"),
        Style::default().fg(if snapshot.pat_masked.is_some() {
            palette.value
        } else {
            palette.missing
        }),
    );
    if let Some(note) = &snapshot.store_note {
        card::put(
            buf,
            inner,
            2,
            &format!("store: {note}"),
            Style::default().fg(palette.missing),
        );
    }
}

fn atividade(inner: Rect, buf: &mut Buffer, snapshot: &Snapshot, palette: Palette, glyphs: Glyphs) {
    let mut row = 0;
    if snapshot.scan_series.len() >= 2 && inner.height >= 2 {
        trend::render(
            card::row_rect(inner, 0),
            buf,
            &snapshot.scan_series,
            palette,
            glyphs,
        );
        row = 1;
    }
    let last = match (&snapshot.last_scan_age_secs, &snapshot.last_scan_local) {
        (Some(age), Some(local)) => {
            format!("último scan {} ({local})", state::format_age_secs(*age))
        }
        (Some(age), None) => format!("último scan {}", state::format_age_secs(*age)),
        _ => "último scan — (nenhum scan na trilha)".to_string(),
    };
    card::put(
        buf,
        inner,
        row,
        &last,
        Style::default().fg(if snapshot.last_scan_age_secs.is_some() {
            palette.value
        } else {
            palette.missing
        }),
    );
    row = row.saturating_add(1);
    if row < inner.height {
        let scans = match snapshot.scan_series.len() {
            0 => "0 scans na trilha".to_string(),
            1 => "1 scan na trilha — mínimo 2 para tendência".to_string(),
            count => format!("{count} scans na trilha"),
        };
        card::put(buf, inner, row, &scans, Style::default().fg(palette.footer));
    }
}

fn indice_wiki(
    inner: Rect,
    buf: &mut Buffer,
    snapshot: &Snapshot,
    palette: Palette,
    glyphs: Glyphs,
) {
    // A primeira linha é sempre o índice (gauge com número ou a causa) — a
    // linha da wiki vem depois dela, nunca por cima.
    match state::index_usage(snapshot) {
        Some((used, cap)) => gauge::render(
            card::row_rect(inner, 0),
            buf,
            "índice",
            used,
            cap,
            palette,
            glyphs,
        ),
        None => card::put(
            buf,
            inner,
            0,
            &state::index_line(snapshot),
            Style::default().fg(palette.missing),
        ),
    }
    let row = 1;
    if row < inner.height {
        if snapshot.wiki_rows.is_empty() {
            card::put(
                buf,
                inner,
                row,
                &state::empty_wiki_line(snapshot),
                Style::default().fg(palette.missing),
            );
        } else {
            let mut kinds: Vec<&str> = snapshot
                .wiki_rows
                .iter()
                .map(|page| page.kind.as_str())
                .collect();
            kinds.sort_unstable();
            kinds.dedup();
            card::put(
                buf,
                inner,
                row,
                &format!(
                    "wiki {} páginas · {} tipos",
                    snapshot.wiki_rows.len(),
                    kinds.len()
                ),
                Style::default().fg(palette.value),
            );
        }
    }
}

fn pendencias(inner: Rect, buf: &mut Buffer, snapshot: &Snapshot, palette: Palette) {
    let causes = state::pending_causes(snapshot);
    if causes.is_empty() {
        card::put(
            buf,
            inner,
            0,
            "— nenhuma pendência registrada",
            Style::default().fg(palette.value),
        );
        return;
    }
    // Três no cartão; o resto continua no `?` (a causa integral, sem corte).
    // Quando a lista não cabe, a ÚLTIMA linha é do marcador `(+N em ?)` — o
    // aviso de que existe mais não pode ser a primeira coisa a ser cortada.
    let max_lines = usize::from(inner.height);
    let mut shown = causes.len().min(3).min(max_lines);
    let mut rest = causes.len().saturating_sub(shown);
    if rest > 0 && shown == max_lines && max_lines > 1 {
        shown -= 1;
        rest = causes.len().saturating_sub(shown);
    }
    for (row, cause) in causes.iter().take(shown).enumerate() {
        card::put(
            buf,
            inner,
            u16::try_from(row).unwrap_or(0),
            cause,
            Style::default().fg(palette.missing),
        );
    }
    if rest > 0 {
        let row = u16::try_from(shown).unwrap_or(0);
        if row < inner.height {
            card::put(
                buf,
                inner,
                row,
                &format!("(+{rest} em ?)"),
                Style::default().fg(palette.footer),
            );
        }
    }
}

/// Desenha uma `Line` já montada (chips) numa linha interna.
fn render_line(
    buf: &mut Buffer,
    inner: Rect,
    row: u16,
    line: ratatui::text::Line<'static>,
    glyphs: Glyphs,
) {
    let area = card::row_rect(inner, row);
    if area.height == 0 {
        return;
    }
    let _ = glyphs;
    ratatui::widgets::Widget::render(ratatui::widgets::Paragraph::new(line), area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode, has_special_glyphs};
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn snapshot_two_findings() -> Snapshot {
        use crate::scan::Finding;
        use crate::tui::state::rows_from;
        let findings = vec![
            Finding {
                tool: "dotnet".to_string(),
                rule: "vulnerable".to_string(),
                package: "Newtonsoft.Json".to_string(),
                version: "12.0.3".to_string(),
                severity: "high".to_string(),
                path: "Kira.Backend".to_string(),
                message: "Advisory".to_string(),
                fingerprint: "a".to_string(),
            },
            Finding {
                tool: "audit".to_string(),
                rule: "vulnerable".to_string(),
                package: "lodash".to_string(),
                version: "1.0.0".to_string(),
                severity: "low".to_string(),
                path: "web".to_string(),
                message: "Advisory".to_string(),
                fingerprint: "b".to_string(),
            },
        ];
        Snapshot {
            findings: Some(2),
            finding_rows: rows_from(findings),
            scan_series: vec![5, 3, 2],
            index_files: Some(812),
            index_cap: 5000,
            ..Snapshot::default()
        }
    }

    fn render(width: u16, height: u16, snapshot: &Snapshot, glyphs: Glyphs) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_grid(
                    frame.buffer_mut(),
                    area,
                    snapshot,
                    Live::idle(),
                    true,
                    theme::mono(),
                    glyphs,
                );
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            out.push('\n');
        }
        out
    }

    /// O cartão de risco mostra os MESMOS números da seção de texto.
    #[test]
    fn risco_card_shows_the_same_numbers_as_the_text_section() {
        let snapshot = snapshot_two_findings();
        // 80×24: o total vai para o TÍTULO (não sobra linha dentro) e o gauge
        // do índice continua no cartão.
        let text = render(80, 21, &snapshot, Glyphs::default());
        assert!(text.contains("| Risco · total 2 |"), "{text}");
        assert!(text.contains("812/5000"), "{text}");
        // Layout largo: sobra linha, então as ferramentas aparecem escritas.
        let wide = render(120, 27, &snapshot, Glyphs::default());
        assert!(wide.contains("dotnet"), "ferramentas com folga: {wide}");
        assert!(wide.contains("total 2"), "{wide}");
    }

    #[test]
    fn projeto_card_shows_dash_with_cause_when_data_is_absent() {
        let snapshot = Snapshot::default();
        let text = render(80, 21, &snapshot, Glyphs::default());
        assert!(text.contains("| Projeto |"), "{text}");
        assert!(text.contains("proj —"), "{text}");
        assert!(text.contains("dev —"), "{text}");
    }

    #[test]
    fn pendencias_card_truncates_to_three_and_points_at_help() {
        let snapshot = Snapshot {
            store_note: Some("sem store".to_string()),
            dev_note: Some("sem dev_name no config".to_string()),
            pat_note: Some("sem PAT — rode kira setup".to_string()),
            index_note: Some("índice vazio".to_string()),
            findings_note: Some("nunca rodou kira scan".to_string()),
            ..Snapshot::default()
        };
        let text = render(80, 21, &snapshot, Glyphs::default());
        assert!(text.contains("| Pendências |"), "{text}");
        assert!(text.contains("em ?"), "aponta a ajuda: {text}");
    }

    /// O frame inteiro em ASCII, no grid largo, sem glifo especial.
    #[test]
    fn ascii_grid_has_no_special_glyph_in_either_layout() {
        let snapshot = snapshot_two_findings();
        for (width, height) in [(80u16, 21u16), (120, 27)] {
            let text = render(width, height, &snapshot, Glyphs::new(Mode::Ascii));
            assert!(!has_special_glyphs(&text), "{width}×{height}: {text}");
        }
    }
}
