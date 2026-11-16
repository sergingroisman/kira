//! Círculo de aprovação humana (F2, PR18) — a **guarda da autonomia**.
//!
//! Regra do mandato: nenhuma escrita no mundo (Azure DevOps, repo do projeto,
//! blog) acontece por decisão do ciclo. Ela nasce como **pedido** no store,
//! fica `pending` e só `kira approve <id>` a destrava. Este módulo é a única
//! porta: quem quer escrever chama [`require_approved`] e, sem aprovação, não
//! passa — fail-closed, sem exceção por "modo agendado".
//!
//! Três invariantes que o código prova:
//!
//! 1. **Fail-closed.** Pedido inexistente, `pending` ou `rejected` ⇒ erro
//!    acionável. Não existe caminho "sem pedido, então pode".
//! 2. **Uma aprovação vale para UM alvo.** A guarda confere `action` **e**
//!    `target` do pedido: aprovar "comentar no item 42" não autoriza
//!    "comentar no item 43".
//! 3. **O resumo é redigido.** O store guarda uma linha legível do efeito, já
//!    passada por [`crate::redact`] — o payload cru nunca toca o banco.
//!
//! Escopo do PR18: a guarda e o círculo. O consumidor real (o `fix --apply`)
//! é o PR27 e **tem** de chamar [`require_approved`]; até lá o e2e prova a
//! guarda e o loop só relata os pedidos pendentes.

use crate::error::Error;
use crate::redact;
use crate::store::Store;

/// Estados de um pedido — a máquina é `pending` → `approved` | `rejected`.
pub const PENDING: &str = "pending";
pub const APPROVED: &str = "approved";
pub const REJECTED: &str = "rejected";

/// Classes de escrita no mundo que a guarda reconhece. Allowlist fechada: ação
/// nova exige item novo (nunca um `String` livre vindo do ciclo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldWrite {
    /// `kira fix --apply`: escreve no repositório do projeto (PR27).
    FixApply,
    /// Comentário/estado no work item do Azure DevOps.
    BoardComment,
    /// Publicação remota (blog/PR) — nunca a wiki local.
    PublishRemote,
}

impl WorldWrite {
    pub const ALL: [WorldWrite; 3] = [Self::FixApply, Self::BoardComment, Self::PublishRemote];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixApply => "fix.apply",
            Self::BoardComment => "board.comment",
            Self::PublishRemote => "publish.remote",
        }
    }

    /// Lê a classe de volta do store/CLI. Desconhecida é erro — uma linha
    /// escrita por versão futura não pode ser interpretada como outra coisa.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value.trim() {
            "fix.apply" => Ok(Self::FixApply),
            "board.comment" => Ok(Self::BoardComment),
            "publish.remote" => Ok(Self::PublishRemote),
            other => Err(Error::ApprovalNotApproved {
                id: 0,
                state: format!("ação desconhecida ({other})"),
            }),
        }
    }
}

/// Cria um pedido `pending` e devolve o id. O resumo é REDIGIDO aqui, na
/// entrada: quem grava não precisa lembrar de mascarar segredo.
pub fn request(
    store: &Store,
    action: WorldWrite,
    target: &str,
    summary: &str,
) -> Result<i64, Error> {
    let summary = redact::redact(summary.trim());
    let target = target.trim();
    Ok(store.add_approval(action.as_str(), target, &summary)?)
}

/// A guarda. Passa **apenas** com um pedido `approved` cuja `action` e
/// `target` batem com a escrita pretendida.
///
/// Qualquer outro estado devolve erro acionável — é o que impede o ciclo de
/// "seguir mesmo assim": não existe retorno `true` por ausência de dado.
pub fn require_approved(
    store: &Store,
    id: i64,
    action: WorldWrite,
    target: &str,
) -> Result<(), Error> {
    let Some(row) = store.get_approval(id)? else {
        return Err(Error::ApprovalNotFound { id });
    };
    if row.state != APPROVED {
        return Err(Error::ApprovalNotApproved {
            id,
            state: row.state,
        });
    }
    if row.action != action.as_str() || row.target != target.trim() {
        return Err(Error::ApprovalTargetMismatch { id });
    }
    Ok(())
}

/// A escrita está liberada? Versão booleana da guarda, para call sites que já
/// registram a razão por outro caminho. `false` nunca esconde o erro de
/// [`require_approved`] — quem precisa da razão chama a versão com `Result`.
#[must_use]
pub fn is_approved(store: &Store, id: i64, action: WorldWrite, target: &str) -> bool {
    require_approved(store, id, action, target).is_ok()
}

/// Uma linha com o que espera decisão humana (usada como nota do ciclo).
/// Contar é barato e não abre o banco de novo a cada fase.
pub fn pending_note(store: &Store) -> Result<Option<String>, Error> {
    let pending = store.list_approvals(true)?;
    if pending.is_empty() {
        return Ok(None);
    }
    let ids = pending
        .iter()
        .map(|row| row.id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Some(format!(
        "{} escrita(s) no mundo aguardando aprovação humana (ids: {ids}) — nada executa sem `kira approve <id>`",
        pending.len()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn temp_store() -> (Store, std::path::PathBuf) {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("kira-approvals-unit-{}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        (Store::open_in_dir(&dir).expect("store"), dir)
    }

    #[test]
    fn world_write_round_trips_its_name() {
        for action in WorldWrite::ALL {
            assert_eq!(WorldWrite::parse(action.as_str()).unwrap(), action);
        }
        assert!(WorldWrite::parse("qualquer.coisa").is_err());
    }

    #[test]
    fn guard_is_fail_closed_for_pending_and_unknown_ids() {
        let (store, dir) = temp_store();
        let id = request(&store, WorldWrite::FixApply, "src/main.rs", "aplicar patch").unwrap();

        // Pending NÃO passa: a guarda não assume nada.
        let err = require_approved(&store, id, WorldWrite::FixApply, "src/main.rs")
            .unwrap_err()
            .to_string();
        assert!(err.contains("exige aprovação humana"), "{err}");
        assert!(err.contains("pending"), "{err}");
        assert!(!is_approved(
            &store,
            id,
            WorldWrite::FixApply,
            "src/main.rs"
        ));

        // Id inexistente também não passa.
        let err = require_approved(&store, 999, WorldWrite::FixApply, "src/main.rs")
            .unwrap_err()
            .to_string();
        assert!(err.contains("não há pedido de aprovação"), "{err}");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn approval_covers_one_action_and_one_target() {
        let (store, dir) = temp_store();
        let id = request(
            &store,
            WorldWrite::BoardComment,
            "item-42",
            "comentar no 42",
        )
        .unwrap();
        assert!(store.decide_approval(id, true, "dono").unwrap());

        // O alvo aprovado passa…
        require_approved(&store, id, WorldWrite::BoardComment, "item-42").unwrap();
        // …e nada além dele: outro alvo ou outra ação são recusados.
        assert!(matches!(
            require_approved(&store, id, WorldWrite::BoardComment, "item-43"),
            Err(Error::ApprovalTargetMismatch { .. })
        ));
        assert!(matches!(
            require_approved(&store, id, WorldWrite::FixApply, "item-42"),
            Err(Error::ApprovalTargetMismatch { .. })
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejection_keeps_the_guard_closed_and_summary_is_redacted() {
        let (store, dir) = temp_store();
        let id = request(
            &store,
            WorldWrite::PublishRemote,
            "blog/post",
            "publicar com token=AbC123XyZ789QqQ",
        )
        .unwrap();

        // O store não guarda o segredo cru.
        let row = store.get_approval(id).unwrap().expect("pedido");
        assert!(!row.summary.contains("AbC123XyZ789QqQ"), "{}", row.summary);
        assert!(row.summary.contains("[REDACTED]"), "{}", row.summary);

        assert!(store.decide_approval(id, false, "dono").unwrap());
        let err = require_approved(&store, id, WorldWrite::PublishRemote, "blog/post")
            .unwrap_err()
            .to_string();
        assert!(err.contains("rejected"), "{err}");

        // Decisão é única: a segunda tentativa não muda o estado.
        assert!(!store.decide_approval(id, true, "outro").unwrap());

        let _ = std::fs::remove_dir_all(dir);
    }
}
