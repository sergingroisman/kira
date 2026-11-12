//! Pré-requisitos de uma ação, resolvidos sobre o [`ActionCtx`] — sem spawnar
//! nada e sem rede. `Status::Fail` NÃO bloqueia a execução (o CLI continua
//! sendo a fonte de verdade), mas o popup mostra `falta` e o dono decide.

use crate::tui::action::{Action, ActionCtx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prerequisite {
    pub label: String,
    pub status: Status,
    /// Causa/ação quando falta (`rode kira setup`, `instale o pnpm`).
    pub fix: Option<String>,
}

impl Prerequisite {
    /// Linha do popup: `ok     dotnet no PATH` / `falta  pnpm no PATH`.
    pub fn describe(&self) -> String {
        let prefix = match self.status {
            Status::Ok => "ok",
            Status::Warn => "aviso",
            Status::Fail => "falta",
        };
        match &self.fix {
            Some(fix) if self.status != Status::Ok => format!("{prefix:<6} {} ({fix})", self.label),
            _ => format!("{prefix:<6} {}", self.label),
        }
    }
}

/// Todos os pré-requisitos da ação, na ordem em que o popup mostra.
pub fn resolve_all(action: Action, ctx: &ActionCtx<'_>) -> Vec<Prerequisite> {
    match action {
        // O scan orquestra as ferramentas da stack; mostrar as duas evita o
        // "rodei e não achou nada" quando uma delas simplesmente não existe.
        Action::Scan => vec![tool("dotnet", ctx), tool("pnpm", ctx)],
        Action::Triage { work_item } => vec![
            azure_target(ctx),
            azure_pat(ctx),
            work_item_check(work_item),
        ],
        // `pr draft` monta o rascunho só do store/wiki: nada de PAT ou rede.
        Action::PrDraft { work_item } => vec![work_item_check(work_item)],
    }
}

fn tool(name: &'static str, ctx: &ActionCtx<'_>) -> Prerequisite {
    if (ctx.tools)(name) {
        Prerequisite {
            label: format!("{name} no PATH"),
            status: Status::Ok,
            fix: None,
        }
    } else {
        Prerequisite {
            label: format!("{name} fora do PATH"),
            status: Status::Fail,
            fix: Some(format!("instale o {name} e rode `kira scan` de novo")),
        }
    }
}

fn azure_target(ctx: &ActionCtx<'_>) -> Prerequisite {
    match ctx.org_project {
        Some((org, project)) => Prerequisite {
            label: format!("organização/projeto: {org}/{project}"),
            status: Status::Ok,
            fix: None,
        },
        None => Prerequisite {
            label: "organização/projeto não configurados".to_string(),
            status: Status::Fail,
            fix: Some("rode `kira setup`".to_string()),
        },
    }
}

fn azure_pat(ctx: &ActionCtx<'_>) -> Prerequisite {
    if ctx.pat_present {
        Prerequisite {
            label: "PAT resolvível (env ou keyring)".to_string(),
            status: Status::Ok,
            fix: None,
        }
    } else {
        Prerequisite {
            label: "sem PAT nesta máquina".to_string(),
            status: Status::Fail,
            fix: Some("defina AZURE_DEVOPS_PAT ou rode `kira setup`".to_string()),
        }
    }
}

fn work_item_check(work_item: u32) -> Prerequisite {
    if work_item > 0 {
        Prerequisite {
            label: format!("work item #{work_item}"),
            status: Status::Ok,
            fix: None,
        }
    } else {
        Prerequisite {
            label: "work item não informado".to_string(),
            status: Status::Fail,
            fix: Some("informe um id numérico".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx<'a>(
        org_project: Option<(&'a str, &'a str)>,
        pat_present: bool,
        tools: &'a dyn Fn(&str) -> bool,
    ) -> ActionCtx<'a> {
        ActionCtx {
            root: Some(Path::new("C:/proj")),
            program: Path::new("C:/bin/kira.exe"),
            org_project,
            pat_present,
            tools,
        }
    }

    #[test]
    fn scan_lists_both_stack_tools_with_honest_status() {
        fn only_dotnet(name: &str) -> bool {
            name == "dotnet"
        }
        let tools = only_dotnet as fn(&str) -> bool;
        let resolved = resolve_all(Action::Scan, &ctx(None, false, &tools));
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].status, Status::Ok);
        assert_eq!(resolved[1].status, Status::Fail);
        assert!(
            resolved[1]
                .describe()
                .starts_with("falta  pnpm fora do PATH"),
            "{}",
            resolved[1].describe()
        );
    }

    #[test]
    fn triage_declares_org_pat_and_item() {
        let all = |_: &str| true;
        let tools = all as fn(&str) -> bool;
        let resolved = resolve_all(
            Action::Triage { work_item: 42 },
            &ctx(Some(("org", "proj")), true, &tools),
        );
        let lines: Vec<String> = resolved.iter().map(Prerequisite::describe).collect();
        assert!(lines[0].contains("org/proj"), "{lines:?}");
        assert!(lines[1].contains("PAT"), "{lines:?}");
        assert!(lines[2].contains("#42"), "{lines:?}");
        assert!(resolved.iter().all(|item| item.status == Status::Ok));

        let missing = resolve_all(Action::Triage { work_item: 42 }, &ctx(None, false, &tools));
        assert!(
            missing[0].describe().contains("kira setup"),
            "{}",
            missing[0].describe()
        );
    }

    #[test]
    fn draft_only_needs_the_item() {
        let none = |_: &str| false;
        let tools = none as fn(&str) -> bool;
        let resolved = resolve_all(Action::PrDraft { work_item: 3 }, &ctx(None, false, &tools));
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].status, Status::Ok);
    }
}
