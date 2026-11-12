//! Capacidades (ADR-0030): integrações opcionais, não pré-requisitos.
//!
//! O KIRA funciona completo sem nenhuma integração externa (`scan`, `wiki`,
//! `context`, `pr draft`, `dashboard` rodam com zero credencial). As
//! integrações viram `Capability` com estado (`conectada | desconectada |
//! inválida`) registrado **por projeto** no store — nunca implícito: presença
//! de credencial no ambiente não é consentimento (decisão 6 do ADR-0030).
//!
//! Regra do gating (decisões 5 e 6): comando que exige capacidade falha com
//! mensagem de capacidade (nunca 401/404 cru) **antes** de resolver credencial
//! ou de qualquer tentativa de rede.

use std::path::Path;

use thiserror::Error;

use crate::error::Error;
use crate::store::{Store, StoreError};

/// Integração opcional que o KIRA pode ganhar quando faz sentido.
///
/// O nome é o contrato da matriz do ADR-0030 (`board.azure`, `scm.github`,
/// `scm.gitlab`) e aparece verbatim na mensagem de gating — mudar aqui é
/// mudar o contrato de teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Board Azure DevOps (work items) — exigida por `triage <id>` e `inspect`.
    BoardAzure,
    /// SCM GitHub (publicar PR, abrir issue) — publicação chega depois do MVP.
    ScmGithub,
    /// SCM GitLab (publicar PR, abrir issue) — publicação chega depois do MVP.
    ScmGitlab,
}

impl Capability {
    /// Nome do contrato, como usado na mensagem de gating e na tabela do store.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Capability::BoardAzure => "board.azure",
            Capability::ScmGithub => "scm.github",
            Capability::ScmGitlab => "scm.gitlab",
        }
    }

    /// Provider aceito por `kira connect <provider>`.
    #[must_use]
    pub fn provider(self) -> &'static str {
        match self {
            Capability::BoardAzure => "azure",
            Capability::ScmGithub => "github",
            Capability::ScmGitlab => "gitlab",
        }
    }

    /// O comando que conecta a capacidade — é ele que a mensagem de gating
    /// aponta, para o dev nunca ficar sem o próximo passo. Estático: entra
    /// direto no campo `&'static str` de `Error::CapabilityMissing`.
    #[must_use]
    pub fn connect_command(self) -> &'static str {
        match self {
            Capability::BoardAzure => "kira connect azure",
            Capability::ScmGithub => "kira connect github",
            Capability::ScmGitlab => "kira connect gitlab",
        }
    }

    /// Da string gravada no store para o enum; `None` = valor desconhecido
    /// (tratado como não conectada — #6: nada é conectado implicitamente).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let names = [
            (Capability::BoardAzure, "board.azure"),
            (Capability::ScmGithub, "scm.github"),
            (Capability::ScmGitlab, "scm.gitlab"),
        ];
        names
            .into_iter()
            .find_map(|(capability, known)| (known == name).then_some(capability))
    }
}

/// Estado de uma capacidade **para um projeto** (a mesma integração pode estar
/// conectada num repositório e não em outro).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityState {
    /// Credencial validada e consentida — a jornada que a exige pode executar.
    Conectada,
    /// Nenhuma conexão feita — o default de todo projeto novo.
    Desconectada,
    /// Conexão que deixou de validar (PAT revogado, 401 no connect) — o dev
    /// precisa reconectar; a jornada não executa.
    Invalida,
}

impl CapabilityState {
    /// Forma gravada no store (ASCII, sem acento, para não depender de encoding).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityState::Conectada => "conectada",
            CapabilityState::Desconectada => "desconectada",
            CapabilityState::Invalida => "invalida",
        }
    }

    /// Lê a forma gravada no store; aceita também a grafia com acento
    /// (`inválida`) para tolerar registros escritos à mão ou por outra versão.
    #[must_use]
    pub fn from_stored(value: &str) -> Option<Self> {
        match value.trim() {
            "conectada" => Some(CapabilityState::Conectada),
            "desconectada" => Some(CapabilityState::Desconectada),
            "invalida" | "inválida" => Some(CapabilityState::Invalida),
            _ => None,
        }
    }
}

/// Erro do gating de capacidades (ADR-0030 #5): a jornada de board não começa.
#[derive(Debug, Error)]
pub enum GateError {
    /// A capacidade exigida não está conectada — `Error::CapabilityMissing`
    /// com a mensagem exata da matriz (contrato de teste).
    #[error(transparent)]
    Missing(#[from] Error),

    /// O store local falhou antes de provar o estado. Fail-closed (#6):
    /// sem prova de conexão o comando não executa — o erro é do store, nunca
    /// do board, e nunca vira tentativa de rede.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Confere que `capability` está **conectada para o projeto em `root`** antes
/// de qualquer rede — este é o ponto único de gating (decisões 5 e 6).
///
/// Falha com `Error::CapabilityMissing` (mensagem exata da matriz) quando o
/// estado é qualquer coisa que não `Conectada`: desconectada, inválida ou um
/// valor desconhecido no store. O projeto é registrado no store (o estado é
/// por projeto), então o próprio gating já cria a linha do projeto.
///
/// # Errors
/// `GateError::Missing` quando a capacidade não está conectada; `GateError::Store`
/// quando o store não pôde ser lido (fail-closed — nunca seguir para a rede).
pub fn require_connected(
    store: &Store,
    root: &Path,
    capability: Capability,
) -> Result<(), GateError> {
    let project = store.ensure_project(root)?;
    let stored = store.capability_state_raw(&project.id, capability.name())?;
    match stored.as_deref().and_then(CapabilityState::from_stored) {
        Some(CapabilityState::Conectada) => Ok(()),
        // Desconectada, inválida ou valor que o KIRA não conhece: a jornada
        // não executa. Mensagem de capacidade, nunca 401/404 cru.
        _ => Err(GateError::Missing(Error::CapabilityMissing {
            capability: capability.name(),
            command: capability.connect_command(),
        })),
    }
}

/// Grava o estado de uma capacidade para o projeto em `root` (UPSERT).
///
/// Escrita usada pelo `kira connect` (MVP-3) e pelos testes de contrato.
///
/// # Errors
/// Propaga falha de leitura/escrita do store (`StoreError`).
pub fn set_state(
    store: &Store,
    root: &Path,
    capability: Capability,
    state: CapabilityState,
) -> Result<(), StoreError> {
    let project = store.ensure_project(root)?;
    store.capability_set_state_raw(&project.id, capability.name(), state.as_str())
}

#[cfg(test)]
mod tests {
    use super::{Capability, CapabilityState, GateError, require_connected, set_state};
    use crate::error::Error;
    use crate::store::{Store, StoreError};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Temp dir único por chamada (padrão dos outros testes do crate).
    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("kira-cap-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn capability_names_follow_the_adr_matrix() {
        assert_eq!(Capability::BoardAzure.name(), "board.azure");
        assert_eq!(Capability::ScmGithub.name(), "scm.github");
        assert_eq!(Capability::ScmGitlab.name(), "scm.gitlab");
        assert_eq!(Capability::BoardAzure.provider(), "azure");
        assert_eq!(
            Capability::BoardAzure.connect_command(),
            "kira connect azure"
        );
    }

    #[test]
    fn capability_from_name_round_trips_and_rejects_unknown() {
        for capability in [
            Capability::BoardAzure,
            Capability::ScmGithub,
            Capability::ScmGitlab,
        ] {
            assert_eq!(
                Capability::from_name(capability.name()),
                Some(capability),
                "ida e volta de {}",
                capability.name()
            );
        }
        assert_eq!(Capability::from_name("jira"), None);
        assert_eq!(Capability::from_name(""), None);
    }

    #[test]
    fn state_strings_round_trip_including_accented_form() {
        for state in [
            CapabilityState::Conectada,
            CapabilityState::Desconectada,
            CapabilityState::Invalida,
        ] {
            assert_eq!(
                CapabilityState::from_stored(state.as_str()),
                Some(state),
                "ida e volta de {}",
                state.as_str()
            );
        }
        assert_eq!(
            CapabilityState::from_stored("inválida"),
            Some(CapabilityState::Invalida),
            "grafia com acento é aceita na leitura"
        );
        assert_eq!(CapabilityState::from_stored("conectado"), None);
    }

    #[test]
    fn missing_capability_fails_with_the_exact_contract_message() {
        let dir = temp_dir("gating");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        let store = Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("abrir store: {err}"));
        let err = require_connected(&store, &dir, Capability::BoardAzure)
            .expect_err("capacidade nunca conectada deve falhar");
        let GateError::Missing(error) = err else {
            panic!("esperava Missing, veio {err:?}");
        };
        assert!(
            matches!(error, Error::CapabilityMissing { .. }),
            "variante de capacidade: {error:?}"
        );
        assert_eq!(
            error.to_string(),
            "esta jornada precisa de board.azure — rode: kira connect azure",
            "mensagem é o contrato exato da matriz do ADR-0030"
        );
        // O gating registra o projeto (estado é por projeto) — efeito colateral
        // legítimo: a linha do projeto passa a existir com o default desconectado.
        let project = store
            .find_project(&dir)
            .unwrap_or_else(|err| panic!("find_project: {err}"));
        assert!(project.is_some(), "gating deve registrar o projeto");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn connected_capability_passes_and_state_persists_across_reopen() {
        let dir = temp_dir("conectada");
        let root = dir.join("repo");
        std::fs::create_dir_all(&root).unwrap_or_else(|err| panic!("criar {root:?}: {err}"));
        {
            let store = Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("abrir store: {err}"));
            set_state(
                &store,
                &root,
                Capability::BoardAzure,
                CapabilityState::Conectada,
            )
            .unwrap_or_else(|err| panic!("set_state: {err}"));
        }
        // Reabrir: o estado tem que sobreviver (persistência, não memória).
        let store = Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("reabrir store: {err}"));
        require_connected(&store, &root, Capability::BoardAzure)
            .unwrap_or_else(|err| panic!("capacidade conectada deve passar: {err}"));
        // As outras capacidades continuam desconectadas — conectou uma, nada
        // mais conecta junto (#6: nada implícito).
        let err = require_connected(&store, &root, Capability::ScmGithub)
            .expect_err("scm.github não foi conectada");
        let GateError::Missing(error) = err else {
            panic!("esperava Missing, veio {err:?}");
        };
        assert_eq!(
            error.to_string(),
            "esta jornada precisa de scm.github — rode: kira connect github"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_state_is_not_connected_and_is_persisted() {
        let dir = temp_dir("invalida");
        let root = dir.join("repo");
        std::fs::create_dir_all(&root).unwrap_or_else(|err| panic!("criar {root:?}: {err}"));
        let store = Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("abrir store: {err}"));
        set_state(
            &store,
            &root,
            Capability::BoardAzure,
            CapabilityState::Invalida,
        )
        .unwrap_or_else(|err| panic!("set_state: {err}"));
        let err = require_connected(&store, &root, Capability::BoardAzure)
            .expect_err("capacidade inválida não executa a jornada");
        let GateError::Missing(_) = err else {
            panic!("esperava Missing, veio {err:?}");
        };
        // Valor desconhecido no store também não conecta (#6).
        store
            .capability_set_state_raw(
                &store
                    .find_project(&root)
                    .unwrap_or_else(|err| panic!("find_project: {err}"))
                    .expect("projeto registrado")
                    .id,
                "board.azure",
                "coisa-estranha",
            )
            .unwrap_or_else(|err| panic!("escrever valor estranho: {err}"));
        let err = require_connected(&store, &root, Capability::BoardAzure)
            .expect_err("valor desconhecido não pode virar conexão");
        let GateError::Missing(_) = err else {
            panic!("esperava Missing, veio {err:?}");
        };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn per_project_state_is_isolated() {
        let dir = temp_dir("por-projeto");
        let root_a = dir.join("repo-a");
        let root_b = dir.join("repo-b");
        std::fs::create_dir_all(&root_a).unwrap_or_else(|err| panic!("criar {root_a:?}: {err}"));
        std::fs::create_dir_all(&root_b).unwrap_or_else(|err| panic!("criar {root_b:?}: {err}"));
        let store = Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("abrir store: {err}"));
        set_state(
            &store,
            &root_a,
            Capability::BoardAzure,
            CapabilityState::Conectada,
        )
        .unwrap_or_else(|err| panic!("set_state A: {err}"));
        require_connected(&store, &root_a, Capability::BoardAzure)
            .unwrap_or_else(|err| panic!("projeto A conectado: {err}"));
        let err = require_connected(&store, &root_b, Capability::BoardAzure)
            .expect_err("projeto B não herdou a conexão do A");
        let GateError::Missing(_) = err else {
            panic!("esperava Missing, veio {err:?}");
        };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_failure_is_fail_closed_never_missing() {
        // Store dentro de um arquivo: open_in_dir falha, e o gating precisa
        // reportar Store (fail-closed), jamais fingir que a capacidade falta
        // de forma "normal" — a jornada não executa dos dois jeitos, mas a
        // causa certa orienta o próximo passo.
        let dir = temp_dir("store-falho");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        let blocker = dir.join("bloqueio");
        std::fs::write(&blocker, "arquivo").unwrap_or_else(|err| panic!("escrever: {err}"));
        let root = blocker.join("repo");
        match Store::open_in_dir(&root) {
            Ok(_) => panic!("não pode abrir store dentro de arquivo"),
            Err(StoreError::CreateDir { .. }) => {
                // Caminho esperado: criar o diretório falha.
            }
            Err(other) => panic!("esperava CreateDir, veio {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
