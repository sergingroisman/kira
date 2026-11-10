//! Write guard — layer 1 of the blast-radius containment (ADR-0011).
//!
//! The attack model is not a motivated attacker; it is a **trusted-but-fallible
//! agent** that types `rm -rf ../` or writes into `.git/`. This module answers one
//! question with an actionable answer: *may KIRA write this path?*
//!
//! It never creates, touches or truncates anything. It only resolves and decides,
//! so a rejection is proven not to have written by construction — and the tests
//! assert the file still does not exist afterwards.
//!
//! Comparison is case-insensitive on purpose: NTFS (the owner's default) treats
//! `.GIT` and `.git` as the same directory.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

/// Names that are never writable, regardless of where they appear.
const DENIED_NAMES: [&str; 8] = [
    ".git",
    "target",
    "node_modules",
    ".ssh",
    ".env",
    "id_rsa",
    "id_ed25519",
    ".git-credentials",
];

/// Extensions that are never writable (keys and materialised secrets).
const DENIED_EXTENSIONS: [&str; 4] = ["pem", "key", "pfx", "p12"];

/// Why a write was refused. Every message says what to do next.
///
/// No `PartialEq`: the `io::Error` payloads inside the resolution variants do not
/// implement it, and the tests assert on the variant + rendered message instead.
#[derive(Debug, Error)]
pub enum GuardError {
    /// The candidate path escapes the project root.
    #[error(
        "recusado: '{candidate}' fica fora do projeto '{root}' — o KIRA só escreve dentro da raiz do projeto; \
         rode o comando a partir da raiz ou passe o caminho relativo correto"
    )]
    OutsideRoot { candidate: String, root: String },

    /// The path contains a `..` component, which is rejected before resolution.
    #[error(
        "recusado: '{candidate}' usa '..' — caminho com '..' nunca é aceito; use o caminho relativo \
         a partir da raiz do projeto"
    )]
    ParentTraversal { candidate: String },

    /// A denied name or extension appears in the path.
    #[error(
        "recusado: '{candidate}' contém '{denied}', que nunca é gravável (código, build, credenciais ou \
         histórico do git); escolha outro destino"
    )]
    Denied { candidate: String, denied: String },

    /// The parent directory does not exist — KIRA does not create directories
    /// implicitly, so a typo cannot silently spawn a new tree.
    #[error(
        "não foi possível resolver o diretório pai de '{candidate}' ({source}) — crie o diretório antes, \
         o KIRA não cria diretórios por você"
    )]
    UnresolvableParent {
        candidate: String,
        #[source]
        source: std::io::Error,
    },

    /// The project root itself could not be resolved.
    #[error(
        "não foi possível resolver a raiz do projeto '{root}' ({source}) — confirme que o diretório existe e \
         rode o KIRA a partir dele"
    )]
    UnresolvableRoot {
        root: String,
        #[source]
        source: std::io::Error,
    },
}

/// A project root whose writes are confined to itself.
#[derive(Debug, Clone)]
pub struct Guard {
    root: PathBuf,
}

impl Guard {
    /// Resolves `root` once; every later check compares against this canonical
    /// form, so a symlinked path cannot be re-interpreted between calls.
    pub fn new(root: &Path) -> Result<Self, GuardError> {
        let root_canonical =
            root.canonicalize()
                .map_err(|source| GuardError::UnresolvableRoot {
                    root: root.display().to_string(),
                    source,
                })?;
        Ok(Self {
            root: root_canonical,
        })
    }

    /// The canonical project root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Decides whether `candidate` may be written and returns the canonical
    /// destination when it may.
    ///
    /// Rules, in order: no `..` component → no denied name/extension → resolves
    /// inside the root (after canonicalisation, which also defeats symlinks and
    /// junctions pointing outside).
    pub fn check_write(&self, candidate: &Path) -> Result<PathBuf, GuardError> {
        let shown = candidate.display().to_string();

        if candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(GuardError::ParentTraversal { candidate: shown });
        }

        if let Some(denied) = denied_component(candidate) {
            return Err(GuardError::Denied {
                candidate: shown,
                denied,
            });
        }

        // Resolve the parent (symlinks, junctions, `..` from the OS view) and keep
        // the file name as typed; the file itself may not exist yet.
        let file_name = candidate.file_name().ok_or_else(|| GuardError::Denied {
            candidate: shown.clone(),
            denied: "caminho sem nome de arquivo".to_string(),
        })?;
        let parent = if candidate.is_absolute() {
            candidate.parent().unwrap_or(&self.root).to_path_buf()
        } else {
            self.root.join(candidate.parent().unwrap_or(Path::new("")))
        };

        let parent_canonical =
            parent
                .canonicalize()
                .map_err(|source| GuardError::UnresolvableParent {
                    candidate: shown.clone(),
                    source,
                })?;

        let destination = parent_canonical.join(file_name);
        if !destination.starts_with(&self.root) {
            return Err(GuardError::OutsideRoot {
                candidate: destination.display().to_string(),
                root: self.root.display().to_string(),
            });
        }

        Ok(destination)
    }
}

/// First denied name or extension in `candidate`, if any.
fn denied_component(candidate: &Path) -> Option<String> {
    for component in candidate.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        let part = part.to_string_lossy();
        if DENIED_NAMES
            .iter()
            .any(|denied| part.eq_ignore_ascii_case(denied))
        {
            return Some(part.into_owned());
        }
        if let Some(extension) = Path::new(part.as_ref())
            .extension()
            .and_then(|e| e.to_str())
            && DENIED_EXTENSIONS
                .iter()
                .any(|denied| extension.eq_ignore_ascii_case(denied))
        {
            return Some(format!("*.{extension}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kira-guard-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    #[test]
    fn allows_a_plain_file_inside_the_root() {
        let dir = temp_root("allow");
        fs::create_dir_all(dir.join("src")).unwrap_or_else(|err| panic!("criar src: {err}"));
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));
        let destination = guard
            .check_write(Path::new("src/pay.rs"))
            .unwrap_or_else(|err| panic!("deveria permitir: {err}"));
        assert!(destination.ends_with("src/pay.rs") || destination.ends_with("pay.rs"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn denies_parent_traversal_without_touching_the_disk() {
        let dir = temp_root("traversal");
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));
        let err = guard
            .check_write(Path::new("../escape.txt"))
            .expect_err("traversal deve ser recusado");
        assert!(matches!(err, GuardError::ParentTraversal { .. }), "{err}");
        assert!(
            err.to_string().contains("'..'"),
            "mensagem acionável: {err}"
        );
        assert!(
            !dir.parent().unwrap_or(&dir).join("escape.txt").exists(),
            "nada pode ser criado fora da raiz"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn denies_git_and_build_directories() {
        let dir = temp_root("denied");
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));
        for candidate in [
            ".git/config",
            "target/debug/x.rs",
            "node_modules/pkg/index.js",
        ] {
            let err = guard
                .check_write(Path::new(candidate))
                .expect_err("deveria recusar");
            assert!(
                matches!(err, GuardError::Denied { .. }),
                "{candidate}: {err}"
            );
        }
        assert!(!dir.join(".git").exists(), "nada de escrita em .git");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn denied_names_are_case_insensitive_for_windows() {
        let dir = temp_root("case");
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));
        for candidate in [".GIT/config", "Target/debug/x", ".ENV"] {
            assert!(
                guard.check_write(Path::new(candidate)).is_err(),
                "{candidate} deveria ser recusado (NTFS é case-insensitive)"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn denies_key_and_secret_extensions() {
        let dir = temp_root("keys");
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));
        for candidate in ["chave.pem", "server.KEY", "cert.pfx"] {
            assert!(
                guard.check_write(Path::new(candidate)).is_err(),
                "{candidate} deveria ser recusado"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn denies_absolute_path_outside_the_root() {
        let dir = temp_root("absolute");
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));
        let outside = std::env::temp_dir().join("fora-do-projeto.txt");
        let err = guard
            .check_write(&outside)
            .expect_err("caminho absoluto fora da raiz deve ser recusado");
        assert!(matches!(err, GuardError::OutsideRoot { .. }), "{err}");
        assert!(!outside.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn denies_symlink_that_escapes_the_root() {
        let dir = temp_root("symlink");
        let outside = temp_root("symlink-outside");
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, dir.join("link"))
                .unwrap_or_else(|err| panic!("criar symlink: {err}"));
            let err = guard
                .check_write(Path::new("link/escape.txt"))
                .expect_err("symlink para fora deve ser recusado");
            assert!(matches!(err, GuardError::OutsideRoot { .. }), "{err}");
            assert!(!outside.join("escape.txt").exists());
        }
        #[cfg(not(unix))]
        {
            // Windows: criar symlink/junction exige privilégio; a resolução é a
            // mesma (canonicalize) e está coberta pelo teste de caminho absoluto.
            let _ = guard;
        }
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn missing_parent_is_actionable_instead_of_silently_creating_it() {
        let dir = temp_root("noparent");
        let guard = Guard::new(&dir).unwrap_or_else(|err| panic!("guard: {err}"));
        let err = guard
            .check_write(Path::new("nao/existe/arquivo.md"))
            .expect_err("pai inexistente deve ser recusado");
        assert!(
            matches!(err, GuardError::UnresolvableParent { .. }),
            "{err}"
        );
        assert!(
            !dir.join("nao").exists(),
            "não pode criar diretório sozinho"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
