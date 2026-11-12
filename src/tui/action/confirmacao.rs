//! Linhas do popup de confirmação (ADR-0033 §3): alvo, efeitos e
//! pré-requisitos, na ordem em que o dono precisa ler antes de apertar Enter.
//!
//! Função pura sobre o [`ActionPlan`]: o widget só escolhe a cor de cada
//! `Kind`, então o conteúdo do popup é testável sem terminal.

use crate::store;
use crate::tui::action::{ActionPlan, Status};

/// Papel da linha — vira cor no render, nunca texto novo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Alvo (root) — dado neutro, mas o mais importante.
    Target,
    /// Efeito declarado (grava/rede/ia).
    Effect,
    /// Pré-requisito presente.
    Ok,
    /// Pré-requisito ausente.
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
}

/// Conteúdo do popup, sem o rodapé (que o widget acrescenta).
pub fn lines(plan: &ActionPlan) -> Vec<Line> {
    let mut lines = vec![Line {
        kind: Kind::Target,
        text: format!("alvo   {}", store::display_path(&plan.root)),
    }];
    for effect in &plan.effects {
        lines.push(Line {
            kind: Kind::Effect,
            text: effect.describe(),
        });
    }
    // Ação local não tem efeito: dizer isso é melhor que deixar o bloco vazio
    // e o dono supor que "não mostrou nada" significa "não faz nada".
    if plan.effects.is_empty() {
        lines.push(Line {
            kind: Kind::Effect,
            text: "local  imprime o rascunho no overlay (sem gravar)".to_string(),
        });
    }
    for prerequisite in &plan.prerequisites {
        lines.push(Line {
            kind: if prerequisite.status == Status::Ok {
                Kind::Ok
            } else {
                Kind::Fail
            },
            text: prerequisite.describe(),
        });
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::action::{Action, Effect, Prerequisite, check};

    fn plan(action: Action) -> ActionPlan {
        ActionPlan {
            action,
            program: std::path::PathBuf::from("C:/bin/kira.exe"),
            argv: action.argv(std::path::Path::new("C:/proj")),
            root: std::path::PathBuf::from("C:/proj"),
            effects: action.effects(),
            prerequisites: vec![
                Prerequisite {
                    label: "PAT resolvível (env ou keyring)".to_string(),
                    status: Status::Ok,
                    fix: None,
                },
                Prerequisite {
                    label: "pnpm fora do PATH".to_string(),
                    status: Status::Fail,
                    fix: Some("instale o pnpm".to_string()),
                },
            ],
        }
    }

    #[test]
    fn confirm_lists_target_effects_and_prerequisites_in_order() {
        let lines = lines(&plan(Action::Triage { work_item: 4821 }));
        assert_eq!(lines[0].kind, Kind::Target);
        assert!(lines[0].text.contains("alvo"), "{}", lines[0].text);
        let effects: Vec<&Line> = lines
            .iter()
            .filter(|line| line.kind == Kind::Effect)
            .collect();
        assert_eq!(effects.len(), 4, "triage declara 4 efeitos: {effects:?}");
        assert!(effects.iter().any(|line| line.text.starts_with("rede")));
        assert!(lines.iter().any(|line| line.kind == Kind::Ok));
        let fail = lines
            .iter()
            .find(|line| line.kind == Kind::Fail)
            .unwrap_or_else(|| panic!("falta não apareceu: {lines:?}"));
        assert_eq!(fail.text, "falta  pnpm fora do PATH (instale o pnpm)");
    }

    #[test]
    fn local_action_says_so_instead_of_showing_an_empty_block() {
        let lines = lines(&plan(Action::PrDraft { work_item: 7 }));
        assert!(
            lines
                .iter()
                .any(|line| line.text.starts_with("local") && line.text.contains("sem gravar")),
            "{lines:?}"
        );
        // Nenhum efeito real declarado para `pr draft` (contrato local).
        assert_eq!(
            Action::PrDraft { work_item: 7 }.effects(),
            Vec::<Effect>::new()
        );
        let _ = check::resolve_all;
    }
}
