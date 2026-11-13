//! Confiança no repositório: o gate antes de qualquer coisa ir para um modelo (ADR-0038).
//!
//! O `ingest` é read-only e roda sempre. O `digest` — que lê código do projeto e o
//! envia ao Ollama — só roda com o repositório **aceito** pelo dev. É a mesma ideia
//! do workspace trust do VS Code: a decisão é do humano, uma vez por caminho, e o
//! registro guarda o **hash do caminho canônico** (não o conteúdo), então mover o
//! repositório pede aceite de novo — que é o comportamento seguro.
//!
//! Arquivo: `<root>/.kira/trust.json`. Sem trust, nada sai da máquina.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Arquivo de aceite dentro de `.kira/`.
pub const TRUST_FILE: &str = "trust.json";

/// Erros de leitura/escrita do aceite.
#[derive(Debug, thiserror::Error)]
pub enum TrustError {
    #[error("não consegui ler {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("não consegui gravar {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("raiz inválida: {0}")]
    Root(String),
}

/// O que fica gravado no aceite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRecord {
    /// Caminho canônico aceito (POSIX slashes, para o arquivo ser portátil de leitura).
    pub root: String,
    /// Hash do caminho canônico — é o que autoriza, não o texto.
    pub hash: String,
    /// Quando o dev aceitou (RFC3339).
    pub accepted_at: String,
}

/// Caminho do arquivo de aceite.
pub fn trust_path(root: &Path) -> PathBuf {
    root.join(".kira").join(TRUST_FILE)
}

/// Lê o aceite, se existir e for legível. Arquivo corrompido = sem aceite.
pub fn record(root: &Path) -> Option<TrustRecord> {
    let text = std::fs::read_to_string(trust_path(root)).ok()?;
    serde_json::from_str(&text).ok()
}

/// `true` quando o repositório foi aceito **neste** caminho.
pub fn is_trusted(root: &Path) -> bool {
    let Some(record) = record(root) else {
        return false;
    };
    match current_hash(root) {
        Some(hash) => hash == record.hash,
        None => false,
    }
}

/// Registra o aceite. Idempotente: aceitar de novo só reescreve o mesmo hash.
pub fn accept(root: &Path) -> Result<TrustRecord, TrustError> {
    let hash = current_hash(root).ok_or_else(|| TrustError::Root(root.display().to_string()))?;
    let record = TrustRecord {
        root: canonical_posix(root),
        hash,
        accepted_at: crate::store::now_rfc3339_utc(),
    };
    let path = trust_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| TrustError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let body = serde_json::to_string_pretty(&record).unwrap_or_default();
    std::fs::write(&path, format!("{body}\n")).map_err(|source| TrustError::Write {
        path: path.display().to_string(),
        source,
    })?;
    crate::ingest::ensure_local_marker(root);
    Ok(record)
}

/// Revoga o aceite. `false` = não havia nada para revogar.
pub fn revoke(root: &Path) -> Result<bool, TrustError> {
    let path = trust_path(root);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(TrustError::Read {
            path: path.display().to_string(),
            source,
        }),
    }
}

/// Hash do caminho canônico — o mesmo `project_id` que o store usa.
fn current_hash(root: &Path) -> Option<String> {
    crate::store::project_id_for_root(root).ok()
}

/// Caminho canônico com `/`, para o arquivo ser legível em qualquer SO.
fn canonical_posix(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kira-trust-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".kira")).expect("mkdir");
        dir
    }

    #[test]
    fn a_repository_is_untrusted_until_accepted() {
        let dir = sandbox("aceite");
        assert!(!is_trusted(&dir), "sem aceite, não é confiável");
        accept(&dir).expect("aceitar");
        assert!(is_trusted(&dir), "depois do aceite, é confiável");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accept_is_idempotent_and_records_the_canonical_path() {
        let dir = sandbox("idem");
        let first = accept(&dir).expect("primeiro aceite");
        let second = accept(&dir).expect("segundo aceite");
        assert_eq!(first.hash, second.hash, "mesmo caminho, mesmo hash");
        assert!(
            second
                .root
                .contains(&dir.file_name().unwrap().to_string_lossy().to_string())
        );
        assert!(!second.root.contains('\\'), "caminho gravado com /");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn revoke_removes_the_acceptance() {
        let dir = sandbox("revoga");
        accept(&dir).expect("aceitar");
        assert!(revoke(&dir).expect("revogar"));
        assert!(!is_trusted(&dir));
        assert!(
            !revoke(&dir).expect("revogar de novo"),
            "segunda vez não havia nada"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Aceite de outro caminho não vale: o hash é do caminho, não do conteúdo.
    #[test]
    fn acceptance_does_not_travel_to_another_directory() {
        let a = sandbox("a");
        let b = sandbox("b");
        accept(&a).expect("aceitar a");
        assert!(!is_trusted(&b), "aceite de A não vale para B");
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn tampered_file_counts_as_no_acceptance() {
        let dir = sandbox("adulterado");
        accept(&dir).expect("aceitar");
        std::fs::write(trust_path(&dir), "{ isto não é json").expect("adulterar");
        assert!(!is_trusted(&dir), "arquivo corrompido não autoriza nada");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
