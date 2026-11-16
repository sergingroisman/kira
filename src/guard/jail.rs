//! Camada 2 do sandbox do `fix --apply` (ADR-0011, PR27): **confinamento por
//! canonicalização** + **rlimits** no processo filho.
//!
//! A camada 1 ([`super::path::Guard`]) decide por nome e por prefixo — barata,
//! mas textual: um symlink ou junction dentro do projeto aponta para onde o
//! texto não mostra. Esta camada resolve o caminho de verdade (canonicaliza) e
//! só então compara com a raiz. As duas juntas são o mínimo honesto; nenhuma
//! delas é sandbox de kernel (o ADR diz isso por extenso).
//!
//! Três invariantes:
//!
//! 1. **Canonicalizar DEPOIS do join, nunca antes.** O caminho final é o que o
//!    SO usaria; conferir o texto do pedido pega `..` e deixa passar symlink.
//! 2. **Raiz canonicalizada uma vez**, no [`Jail::new`]. Se a raiz for resolvida
//!    de novo a cada checagem, um cwd trocado no meio muda o raio de explosão.
//! 3. **rlimits são do FILHO**, aplicados em `pre_exec` (entre fork e exec) —
//!    limitar o próprio KIRA mataria o agente que precisa reportar o erro.
//!
//! Limite declarado no ADR-0011: **accident-prevention, não sandbox de
//! segurança**. Um processo que já executa código arbitrário pode sair do
//! diretório por mecanismos que `std` não controla; para código hostil, VM.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use super::path::GuardError;

/// Tetos aplicados ao processo filho. Valores conservadores: o adversário é
/// **erro de agente** (um loop patológico, um `--force` a mais), não atacante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Teto de CPU (segundos de CPU, não de relógio) — `RLIMIT_CPU`.
    pub cpu_seconds: u64,
    /// Teto de espaço de endereçamento em bytes — `RLIMIT_AS`.
    pub address_space_bytes: u64,
    /// Teto de processos/threads do usuário — `RLIMIT_NPROC`.
    pub max_processes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            cpu_seconds: 60,
            // 2 GiB: um compilador Rust cabe, um OOM do agente não.
            address_space_bytes: 2 * 1024 * 1024 * 1024,
            max_processes: 128,
        }
    }
}

/// O sandbox de uma raiz de projeto.
#[derive(Debug, Clone)]
pub struct Jail {
    root: PathBuf,
    limits: Limits,
}

impl Jail {
    /// Cria o jail sobre `root`, **canonicalizando** a raiz.
    ///
    /// Raiz inexistente é erro: canonicalizar um caminho que não existe é
    /// impossível, e "assumir" o texto cru abriria a porta que esta camada
    /// existe para fechar.
    pub fn new(root: &Path) -> Result<Self, GuardError> {
        let root = root
            .canonicalize()
            .map_err(|source| GuardError::UnresolvableRoot {
                root: root.display().to_string(),
                source,
            })?;
        Ok(Self {
            root,
            limits: Limits::default(),
        })
    }

    /// Trocas os tetos padrão (usado pelo `fix --apply` quando ele existir).
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Resolve o caminho de uma escrita e devolve o caminho **real** a usar.
    ///
    /// O pai precisa existir (o KIRA não cria árvore por engano — mesma régua
    /// do [`super::path::Guard`]); canonicaliza-se o PAI e junta-se o nome,
    /// porque o arquivo de destino normalmente ainda não existe. Depois disso o
    /// caminho resolvido é conferido contra a raiz: symlink que sai do projeto
    /// é recusado **aqui**, mesmo com nome inocente.
    pub fn resolve_for_write(&self, candidate: &Path) -> Result<PathBuf, GuardError> {
        if candidate
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(GuardError::ParentTraversal {
                candidate: candidate.display().to_string(),
            });
        }
        let absolute = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.root.join(candidate)
        };
        let name = absolute
            .file_name()
            .ok_or_else(|| GuardError::OutsideRoot {
                candidate: absolute.display().to_string(),
                root: self.root.display().to_string(),
            })?;
        let parent = absolute.parent().ok_or_else(|| GuardError::OutsideRoot {
            candidate: absolute.display().to_string(),
            root: self.root.display().to_string(),
        })?;
        let real_parent =
            parent
                .canonicalize()
                .map_err(|source| GuardError::UnresolvableParent {
                    candidate: parent.display().to_string(),
                    source,
                })?;
        let resolved = real_parent.join(name);
        if !resolved.starts_with(&self.root) {
            return Err(GuardError::OutsideRoot {
                candidate: resolved.display().to_string(),
                root: self.root.display().to_string(),
            });
        }
        Ok(resolved)
    }

    /// Roda `program` com `args`, com o diretório de trabalho preso à raiz e os
    /// rlimits aplicados no FILHO.
    ///
    /// `env_clear` de propósito: variável de ambiente é canal de fuga silencioso
    /// (proxy, `GIT_DIR`, `HOME`) e o filho do jail não precisa do ambiente do
    /// agente para nada que o KIRA faça.
    pub fn run(&self, program: &Path, args: &[String]) -> Result<Output, GuardError> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .env_clear();
        self.apply_limits(&mut command)?;
        command.output().map_err(|source| GuardError::SpawnFailed {
            program: program.display().to_string(),
            source,
        })
    }

    /// Aplica os rlimits no processo filho (`pre_exec` = entre fork e exec).
    #[cfg(unix)]
    fn apply_limits(&self, command: &mut Command) -> Result<(), GuardError> {
        use std::os::unix::process::CommandExt;
        let limits = self.limits;
        // SAFETY: `pre_exec` roda no filho, logo após o fork. Os três
        // `setrlimit` são async-signal-safe e não alocam — o que a doc do
        // `pre_exec` exige. Erro volta como `io::Error` e aborta o exec, então
        // um limite que não pode ser aplicado NUNCA vira execução sem limite.
        unsafe {
            command.pre_exec(move || {
                let pairs = [
                    (libc::RLIMIT_CPU, limits.cpu_seconds),
                    (libc::RLIMIT_AS, limits.address_space_bytes),
                    (libc::RLIMIT_NPROC, limits.max_processes),
                ];
                for (resource, value) in pairs {
                    let limit = libc::rlimit {
                        rlim_cur: value,
                        rlim_max: value,
                    };
                    if libc::setrlimit(resource, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        Ok(())
    }

    /// Windows (MVP): sem `setrlimit`. Confinamento por canonicalização e
    /// deny-list continuam valendo; Job Objects ficam como item futuro no ADR.
    #[cfg(not(unix))]
    fn apply_limits(&self, _command: &mut Command) -> Result<(), GuardError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(tag: &str) -> (PathBuf, Jail) {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("kira-jail-{}-{tag}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("projeto")).expect("projeto");
        std::fs::create_dir_all(base.join("fora")).expect("fora");
        let jail = Jail::new(&base.join("projeto")).expect("jail");
        (base, jail)
    }

    #[test]
    fn accepts_a_path_inside_the_root() {
        let (base, jail) = sandbox("dentro");
        let resolved = jail.resolve_for_write(Path::new("novo.txt")).expect("ok");
        assert_eq!(resolved, jail.root().join("novo.txt"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn rejects_parent_traversal_before_touching_the_disk() {
        let (base, jail) = sandbox("traversal");
        let err = jail
            .resolve_for_write(Path::new("../fora/roubado.txt"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("'..'"), "{err}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn rejects_an_absolute_path_outside_the_root() {
        let (base, jail) = sandbox("absoluto");
        let outside = base.join("fora").join("x.txt");
        let err = jail.resolve_for_write(&outside).unwrap_err().to_string();
        assert!(err.contains("fora do projeto"), "{err}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_that_points_outside_the_root() {
        // O caso que a deny-list textual NÃO pega: nome inocente, destino fora.
        let (base, jail) = sandbox("symlink");
        let link = jail.root().join("saida");
        std::os::unix::fs::symlink(base.join("fora"), &link).expect("symlink");
        let err = jail
            .resolve_for_write(Path::new("saida/roubado.txt"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("fora do projeto"), "{err}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn missing_parent_is_reported_instead_of_created() {
        let (base, jail) = sandbox("sem-pai");
        let err = jail
            .resolve_for_write(Path::new("nao/existe/arquivo.txt"))
            .unwrap_err()
            .to_string();
        assert!(!err.is_empty(), "erro sem mensagem");
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn child_runs_inside_the_root_with_limits_applied() {
        let (base, jail) = sandbox("exec");
        let output = jail
            .run(Path::new("/bin/sh"), &["-c".to_string(), "pwd".to_string()])
            .expect("rodar");
        assert!(output.status.success(), "{output:?}");
        let cwd = String::from_utf8_lossy(&output.stdout);
        assert_eq!(cwd.trim(), jail.root().display().to_string());
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn cpu_limit_kills_a_runaway_child() {
        // Prova o rlimit de CPU: 1s de CPU e um laço infinito ⇒ o filho morre
        // por sinal em vez de rodar para sempre.
        let (base, jail) = sandbox("cpu");
        let jail = jail.with_limits(Limits {
            cpu_seconds: 1,
            ..Limits::default()
        });
        let output = jail
            .run(
                Path::new("/bin/sh"),
                &["-c".to_string(), "while :; do :; done".to_string()],
            )
            .expect("rodar");
        assert!(
            !output.status.success(),
            "filho sem teto de CPU terminou normalmente: {output:?}"
        );
        let _ = std::fs::remove_dir_all(base);
    }
}
