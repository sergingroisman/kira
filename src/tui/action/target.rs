//! Folha de alvo: `t`/`d` não têm id, então a TUI pede antes de montar o plano.
//!
//! Lista os ids já CONHECIDOS (args de sessões da trilha + páginas
//! `wiki/bugs/<id>.md`) e aceita digitar um novo. Tudo aqui é puro: a folha
//! não executa, não grava e não consulta rede.

use crate::tui::action::{Action, Request};
use crate::tui::state::Snapshot;

/// Comprimento máximo do id digitado (u32 tem 10 dígitos; 9 é folgado).
const ID_MAX_CHARS: usize = 9;

/// Ids conhecidos, mais recente primeiro (`dedup` sem repetir a mesma origem).
pub fn known_work_items(snapshot: &Snapshot) -> Vec<u32> {
    let mut ids: Vec<u32> = Vec::new();
    for row in &snapshot.session_rows {
        if let Some(id) = first_id(&row.args) {
            ids.push(id);
        }
    }
    for page in &snapshot.wiki_rows {
        if let Some(id) = page
            .slug
            .strip_prefix("bugs/")
            .and_then(|rest| rest.parse::<u32>().ok())
        {
            ids.push(id);
        }
    }
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids.dedup();
    ids
}

/// Primeiro id numérico dos args gravados (`"4821"`, `"4821 --json"`).
fn first_id(args: &str) -> Option<u32> {
    args.split_whitespace()
        .find_map(|token| token.parse::<u32>().ok())
        .filter(|id| *id > 0)
}

/// Estado da folha de alvo (muta só por teclado; sem I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSheet {
    pub request: Request,
    pub known: Vec<u32>,
    /// Dígitos digitados (`""` = usar a lista).
    pub typed: String,
    pub cursor: usize,
}

impl TargetSheet {
    pub fn new(request: Request, known: Vec<u32>) -> Self {
        Self {
            request,
            known,
            typed: String::new(),
            cursor: 0,
        }
    }

    /// Título do bloco (`alvo do triage`).
    pub fn title(&self) -> &'static str {
        self.request.target_title()
    }

    /// Aceita dígito; devolve `false` quando a tecla não pertence à folha.
    pub fn push_digit(&mut self, value: char) -> bool {
        if !value.is_ascii_digit() || self.typed.len() >= ID_MAX_CHARS {
            return false;
        }
        self.typed.push(value);
        true
    }

    pub fn backspace(&mut self) {
        self.typed.pop();
    }

    /// Tab / Shift+Tab andam na lista de conhecidos (sem wrap infinito).
    pub fn cycle(&mut self, direction: isize) {
        if self.known.is_empty() {
            return;
        }
        let len = self.known.len() as isize;
        let next = (self.cursor as isize + direction).rem_euclid(len);
        self.cursor = next as usize;
    }

    /// Id que o Enter confirma: o digitado vence a lista; `None` = inválido.
    pub fn selected(&self) -> Option<u32> {
        if !self.typed.is_empty() {
            return self.typed.parse::<u32>().ok().filter(|id| *id > 0);
        }
        self.known.get(self.cursor).copied()
    }

    /// Ação pronta para o plano (`None` = id inválido ⇒ Enter não executa).
    pub fn action(&self) -> Option<Action> {
        self.request.into_action(self.selected())
    }

    /// Linhas do popup (o widget só troca a cor do marcador).
    pub fn lines(&self) -> Vec<String> {
        self.lines_with(crate::tui::glyph::Glyphs::default())
    }

    /// [`lines`] com o conjunto de glifos da sessão.
    ///
    /// O campo digitado usa cercas (`▌…▏` / `[…]`) e a seleção usa o marcador do
    /// modo: em `KIRA_GLYPHS=ascii` a folha de alvo não vira caixinha.
    pub fn lines_with(&self, glyphs: crate::tui::glyph::Glyphs) -> Vec<String> {
        let (open, close) = (glyphs.field_open(), glyphs.field_close());
        let typed = if self.typed.is_empty() {
            self.known
                .get(self.cursor)
                .map(|id| id.to_string())
                .unwrap_or_default()
        } else {
            self.typed.clone()
        };
        let mut lines = vec![
            "work item (id numérico)".to_string(),
            format!("{open}{typed}{close}"),
            String::new(),
            "conhecidos".to_string(),
        ];
        if self.known.is_empty() {
            lines.push("— nenhum id na trilha ou na wiki; digite o id".to_string());
        } else {
            let marker = glyphs.selection().trim_end();
            for (index, id) in self.known.iter().enumerate() {
                let chosen = if index == self.cursor && self.typed.is_empty() {
                    marker
                } else {
                    " "
                };
                lines.push(format!("{chosen} {id}"));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{SessionLine, WikiPage};

    fn session(args: &str) -> SessionLine {
        SessionLine {
            id: 1,
            command: "triage".to_string(),
            args: args.to_string(),
            model: None,
            started_unix: None,
            started_at: String::new(),
            local_started_at: None,
            duration_ms: 0,
            ok: true,
            summary_first_line: String::new(),
            summary: String::new(),
        }
    }

    fn page(slug: &str) -> WikiPage {
        WikiPage {
            slug: slug.to_string(),
            path: format!(".kira/wiki/{slug}.md"),
            kind: "bug".to_string(),
            title: slug.to_string(),
            updated_at: String::new(),
            updated_unix: None,
            content: String::new(),
        }
    }

    #[test]
    fn known_ids_come_from_session_args_and_wiki_slugs() {
        let snapshot = Snapshot {
            session_rows: vec![session("4821"), session("4790 --json"), session("—")],
            wiki_rows: vec![page("bugs/4810"), page("contexto")],
            ..Snapshot::default()
        };
        assert_eq!(known_work_items(&snapshot), vec![4821, 4810, 4790]);
    }

    #[test]
    fn typing_digits_beats_the_list_and_invalid_input_is_refused() {
        let mut sheet = TargetSheet::new(Request::Triage, vec![4821, 4790]);
        assert_eq!(sheet.selected(), Some(4821));
        assert!(!sheet.push_digit('x'), "letra não entra");
        assert!(sheet.push_digit('7'));
        assert_eq!(sheet.typed, "7");
        assert_eq!(sheet.selected(), Some(7), "o digitado vence a lista");
        assert_eq!(sheet.action(), Some(Action::Triage { work_item: 7 }));
        sheet.backspace();
        assert_eq!(sheet.selected(), Some(4821), "volta para a lista");
    }

    #[test]
    fn zero_is_never_a_valid_target() {
        let mut sheet = TargetSheet::new(Request::PrDraft, Vec::new());
        assert!(sheet.push_digit('0'));
        assert_eq!(sheet.selected(), None);
        assert_eq!(sheet.action(), None, "Enter não executa com id zero");
        assert!(
            sheet
                .lines()
                .iter()
                .any(|line| line.contains("digite o id"))
        );
    }

    #[test]
    fn tab_cycles_without_running_off_the_list() {
        let mut sheet = TargetSheet::new(Request::Triage, vec![3, 2, 1]);
        sheet.cycle(1);
        assert_eq!(sheet.selected(), Some(2));
        sheet.cycle(-1);
        assert_eq!(sheet.selected(), Some(3));
        sheet.cycle(-1);
        assert_eq!(sheet.selected(), Some(1), "rem_euclid volta para o fim");
        let single = TargetSheet::new(Request::Triage, vec![9]);
        assert_eq!(single.lines()[3], "conhecidos");
        assert!(single.lines()[4].starts_with("➤ 9"));
    }
}
