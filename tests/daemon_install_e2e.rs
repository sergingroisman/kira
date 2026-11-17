//! E2E do `kira daemon install|uninstall` (DAEMON-02): o contrato no limite
//! do CLI é uma tarefa determinística (`kira-<nome>-<8hex>`) cujo comando
//! interno é o PRÓPRIO binário (`daemon run --path <raiz>` — sem shell, sem
//! segredo), instalação idempotente (sobrescreve, nunca duplica) e uninstall
//! idempotente (tarefa ausente é "nada a remover", sai 0).
//!
//! Hermeticidade: a ferramenta do SO é resolvida pelo MESMO contrato do scan
//! — a env de override (`KIRA_SCHTASKS_BIN` / `KIRA_LAUNCHCTL_BIN`) aponta um
//! shim que LOGA os args e devolve exit controlável. Nenhum agendador real é
//! tocado; a validação real (Task Scheduler/launchd) é no hardware do dono
//! (bloqueio B5 do plano). Linux é fora do MVP: erro acionável, exit != 0.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    cwd: PathBuf,
    store: PathBuf,
    /// Onde o shim grava a linha de args que recebeu.
    #[allow(dead_code)] // só os e2e de um SO o leem; os outros alvos o veiam ocioso
    shim_log: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-daemon-install-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        fs::write(cwd.join("README.md"), "# projeto\n").expect("README");
        Self {
            cwd,
            store,
            shim_log: base.join("shim-args.log"),
        }
    }

    #[allow(dead_code)] // usado pelos testes de um SO só; os outros alvos o veem ocioso
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_SECRET_STORE", "off")
            // O shim loga aqui; o teste lê o arquivo depois.
            .env("KIRA_SHIM_LOG", &self.shim_log)
            .env_remove("KIRA_SCHTASKS_BIN")
            .env_remove("KIRA_LAUNCHCTL_BIN")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_SHIM_QUERY");
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    #[allow(dead_code)] // idem: só os testes do macOS/Linux o usam
    fn base(&self) -> &Path {
        self.cwd.parent().unwrap_or(self.cwd.as_path())
    }
}

// ---------------------------------------------------------------------------
// Shims (um por SO) — logam args e devolvem exit conforme KIRA_SHIM_QUERY.
// ---------------------------------------------------------------------------

/// Retorna o caminho do shim já gravado, pronto para a env de override.
#[cfg(windows)]
fn install_shim(sandbox: &Sandbox) -> PathBuf {
    let shim = sandbox.base().join("agendador.cmd");
    fs::write(
        &shim,
        "@echo off\r\necho %* >> \"%KIRA_SHIM_LOG%\"\r\nif \"%KIRA_SHIM_QUERY%\"==\"absente\" exit /b 1\r\nexit /b 0\r\n",
    )
    .expect("escrever shim .cmd");
    shim
}

#[cfg(target_os = "macos")]
fn install_shim(sandbox: &Sandbox) -> PathBuf {
    let shim = sandbox.base().join("agendador.sh");
    fs::write(
        &shim,
        "#!/bin/sh\necho \"$@\" >> \"$KIRA_SHIM_LOG\"\nif [ \"$KIRA_SHIM_QUERY\" = \"absente\" ]; then exit 1; fi\nexit 0\n",
    )
    .expect("escrever shim sh");
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod shim");
    }
    shim
}

/// Log do shim: o que o agendador RECEBEU (args em ordem).
#[allow(dead_code)] // só os e2e por SO o usam
fn shim_calls(sandbox: &Sandbox) -> Vec<String> {
    let text = fs::read_to_string(&sandbox.shim_log).unwrap_or_default();
    text.lines().map(str::to_string).collect()
}

// ---------------------------------------------------------------------------
// Windows — Task Scheduler via schtasks
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod windows_tests {
    use super::*;

    const OVERRIDE_ENV: &str = "KIRA_SCHTASKS_BIN";

    /// Roda o kira com o override do shim ativo (e `KIRA_SHIM_QUERY` opcional).
    fn run_with_shim(
        sandbox: &Sandbox,
        shim: &Path,
        query: Option<&str>,
        args: &[&str],
    ) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command
            .args(args)
            .current_dir(&sandbox.cwd)
            .env("KIRA_STORE_DIR", &sandbox.store)
            .env("KIRA_SECRET_STORE", "off")
            .env("KIRA_SHIM_LOG", &sandbox.shim_log)
            .env(OVERRIDE_ENV, shim)
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT");
        if let Some(query) = query {
            command.env("KIRA_SHIM_QUERY", query);
        }
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    #[test]
    fn install_creates_a_deterministic_task_with_the_same_binary() {
        let sandbox = Sandbox::new("win-install");
        let shim = install_shim(&sandbox);
        let (code, stdout, stderr) = run_with_shim(&sandbox, &shim, None, &["daemon", "install"]);
        assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
        assert!(stdout.contains("daemon instalado"), "{stdout}");
        // Nome determinístico: kira-<nome>-<8 hex>.
        let task = stdout
            .lines()
            .find(|line| line.contains("daemon instalado: kira-"))
            .map(|line| {
                line.trim_start_matches("daemon instalado: ")
                    .trim()
                    .to_string()
            })
            .expect("nome da tarefa no stdout");
        assert!(
            task.starts_with("kira-projeto-") && task.len() == "kira-projeto-".len() + 8,
            "{task}"
        );
        // O que o agendador recebeu: /Create + /TN + /TR com o PRÓPRIO binário
        // e o --path da raiz + /SC ONLOGON + /F (sobrescreve, nunca duplica).
        let calls = shim_calls(&sandbox);
        assert_eq!(calls.len(), 1, "uma chamada ao agendador: {calls:?}");
        let call = &calls[0];
        assert!(call.contains("/Create"), "{call}");
        assert!(call.contains("/TN"), "{call}");
        assert!(call.contains(&task), "{call}");
        assert!(call.contains("/TR"), "{call}");
        assert!(
            call.contains("daemon run --path"),
            "o filho é o próprio kira: {call}"
        );
        assert!(call.contains("ONLOGON"), "{call}");
        assert!(call.contains("/F"), "{call}");
        // Nenhum segredo na linha (o daemon não precisa de PAT). O `--path`
        // contém "pat" — a checagem é por MARCA de credencial, não substring.
        let call_lower = call.to_lowercase();
        assert!(
            !call_lower.contains("azure_devops_pat") && !call_lower.contains("token="),
            "a linha do agendador não carrega credencial: {call}"
        );
        // O /TR interno tem aspas no caminho REAL, mas o shim `.cmd` captura
        // a forma mangled do wrapper (aspas internas viram apóstrofo) — o
        // valor exato é provado no teste unitário `schtasks_tr_*` (sem shim).
        assert!(
            call.contains("kira.exe") || call.contains("kira"),
            "o filho é o próprio binário: {call}"
        );
        // Instalar de novo: MESMO nome, /F de novo — nunca duplica.
        let _ = fs::remove_file(&sandbox.shim_log);
        let (code2, stdout2, stderr2) =
            run_with_shim(&sandbox, &shim, None, &["daemon", "install"]);
        assert_eq!(code2, 0, "{stdout2}\n{stderr2}");
        let calls2 = shim_calls(&sandbox);
        assert!(
            calls2.iter().all(|call| call.contains(&task)),
            "mesmo nome nas duas instalações: {calls2:?}"
        );
    }

    #[test]
    fn uninstall_is_idempotent_and_deletes_the_task() {
        let sandbox = Sandbox::new("win-uninstall");
        let shim = install_shim(&sandbox);
        // Tarefa ausente (shim query devolve exit 1): "nada a remover", sai 0.
        let (code, stdout, stderr) =
            run_with_shim(&sandbox, &shim, Some("absente"), &["daemon", "uninstall"]);
        assert_eq!(code, 0, "{stdout}\n{stderr}");
        assert!(stdout.contains("nada a remover"), "{stdout}");
        // O KIRA NUNCA decide por texto do OS: o query rodou antes do delete.
        assert!(
            shim_calls(&sandbox)
                .iter()
                .all(|call| call.contains("/Query")),
            "query decide, delete não é chamado sem ele: {calls}",
            calls = shim_calls(&sandbox).join(" | ")
        );
        // Tarefa presente (query exit 0): delete com /F, mensagem honesta.
        let _ = fs::remove_file(&sandbox.shim_log);
        let (code, stdout, stderr) = run_with_shim(&sandbox, &shim, None, &["daemon", "uninstall"]);
        assert_eq!(code, 0, "{stdout}\n{stderr}");
        assert!(stdout.contains("removida"), "{stdout}");
        let calls = shim_calls(&sandbox);
        assert!(
            calls.iter().any(|call| call.contains("/Delete")),
            "{calls:?}"
        );
        assert!(
            calls.iter().any(|call| call.contains("/F")),
            "delete com /F: {calls:?}"
        );
    }

    #[test]
    fn tool_failure_is_compact_and_actionable() {
        let sandbox = Sandbox::new("win-falha");
        // Shim que FALHA (exit 3) — o erro deve ser compacto, com dica da
        // primeira linha de stderr, nunca corpo cru.
        let shim = sandbox.base().join("agendador-falha.cmd");
        fs::write(
            &shim,
            "@echo off\r\necho erro do agendador simulado 1>&2\r\nexit /b 3\r\n",
        )
        .expect("escrever shim falha");
        let (code, stdout, stderr) = run_with_shim(&sandbox, &shim, None, &["daemon", "install"]);
        assert_ne!(code, 0, "falha do agendador ⇒ exit != 0");
        assert!(stdout.is_empty(), "erro não vai para stdout: {stdout}");
        assert!(
            stderr.contains("a ferramenta schtasks falhou (exit 3)"),
            "{stderr}"
        );
        assert!(
            stderr.contains("dica: erro do agendador simulado"),
            "dica compacta: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// macOS — launchd via launchctl + plist em ~/Library/LaunchAgents
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos_tests {
    use super::*;

    const OVERRIDE_ENV: &str = "KIRA_LAUNCHCTL_BIN";

    fn run_with_shim(
        sandbox: &Sandbox,
        shim: &Path,
        query: Option<&str>,
        args: &[&str],
    ) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command
            .args(args)
            .current_dir(&sandbox.cwd)
            .env("KIRA_STORE_DIR", &sandbox.store)
            .env("KIRA_SECRET_STORE", "off")
            .env("KIRA_SHIM_LOG", &sandbox.shim_log)
            .env(OVERRIDE_ENV, shim)
            // O plist nasce no HOME do sandbox — nada toca o LaunchAgents real.
            .env("HOME", sandbox.base())
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT");
        if let Some(query) = query {
            command.env("KIRA_SHIM_QUERY", query);
        }
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    /// O plist do kira no LaunchAgents do sandbox (nome leva o sufixo de 8 hex).
    fn plist_path(sandbox: &Sandbox) -> PathBuf {
        let dir = sandbox.base().join("Library").join("LaunchAgents");
        fs::read_dir(&dir)
            .expect("LaunchAgents no HOME do sandbox")
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().starts_with("kira-projeto-"))
                    .unwrap_or(false)
            })
            .expect("plist do kira no LaunchAgents")
    }

    #[test]
    fn install_writes_plist_and_loads_it() {
        let sandbox = Sandbox::new("mac-install");
        let shim = install_shim(&sandbox);
        let (code, stdout, stderr) = run_with_shim(&sandbox, &shim, None, &["daemon", "install"]);
        assert_eq!(code, 0, "{stdout}\n{stderr}");
        assert!(stdout.contains("daemon instalado"), "{stdout}");
        // O plist nasceu no HOME do sandbox, com o filho certo dentro.
        let plist = plist_path(&sandbox);
        assert!(
            plist.exists(),
            "plist no HOME do sandbox: {}",
            plist.display()
        );
        let content = fs::read_to_string(&plist).expect("plist legível");
        assert!(content.contains("<string>kira-projeto-"), "{content}");
        assert!(content.contains("<string>daemon</string>"), "{content}");
        assert!(content.contains("<string>run</string>"), "{content}");
        assert!(content.contains("<string>--path</string>"), "{content}");
        assert!(content.contains("RunAtLoad"), "{content}");
        // O launchctl recebeu `load <plist>` (exit code manda, texto nunca).
        let calls = shim_calls(&sandbox);
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].contains("load"), "{calls:?}");
        assert!(calls[0].contains("kira-projeto"), "{calls:?}");
        // Idempotência: reinstalar reescreve e recarrega (2 chamadas, mesmo nome).
        let (code2, stdout2, stderr2) =
            run_with_shim(&sandbox, &shim, None, &["daemon", "install"]);
        assert_eq!(code2, 0, "{stdout2}\n{stderr2}");
        assert!(plist.exists());
    }

    #[test]
    fn uninstall_is_idempotent_and_unloads() {
        let sandbox = Sandbox::new("mac-uninstall");
        let shim = install_shim(&sandbox);
        // O install cria o plist REAL — é o par install/uninstall que o
        // contrato prova (nada de plist fake escrito à mão).
        let (code, stdout, stderr) = run_with_shim(&sandbox, &shim, None, &["daemon", "install"]);
        assert_eq!(code, 0, "{stdout}\n{stderr}");
        let plist = plist_path(&sandbox);
        assert!(plist.exists(), "plist instalado: {}", plist.display());
        // Uninstall: unload + arquivo removido.
        let _ = fs::remove_file(&sandbox.shim_log);
        let (code, stdout, stderr) = run_with_shim(&sandbox, &shim, None, &["daemon", "uninstall"]);
        assert_eq!(code, 0, "{stdout}\n{stderr}");
        assert!(stdout.contains("descarregada"), "{stdout}");
        assert!(!plist.exists(), "plist removido do disco");
        assert!(
            shim_calls(&sandbox)
                .iter()
                .any(|call| call.contains("unload")),
            "{calls:?}",
            calls = shim_calls(&sandbox)
        );
        // Idempotência: uninstall DE NOVO = "nada a remover", launchctl nem roda.
        let _ = fs::remove_file(&sandbox.shim_log);
        let (code, stdout, stderr) = run_with_shim(&sandbox, &shim, None, &["daemon", "uninstall"]);
        assert_eq!(code, 0, "{stdout}\n{stderr}");
        assert!(stdout.contains("nada a remover"), "{stdout}");
        assert!(
            shim_calls(&sandbox).is_empty(),
            "sem plist não há spawn do launchctl: {calls:?}",
            calls = shim_calls(&sandbox)
        );
    }
}

// ---------------------------------------------------------------------------
// Linux — fora do MVP: erro acionável, exit != 0
// ---------------------------------------------------------------------------

#[cfg(not(any(windows, target_os = "macos")))]
mod linux_tests {
    use super::*;

    #[test]
    fn install_on_linux_is_an_actionable_error() {
        let sandbox = Sandbox::new("linux-install");
        let (code, stdout, stderr) = sandbox.run(&["daemon", "install"]);
        assert_ne!(code, 0, "install fora do MVP tem que falhar");
        assert!(
            stderr.contains("não está implementado"),
            "erro acionável, sem corpo cru: {stderr}"
        );
        assert!(
            stderr.contains("kira daemon run"),
            "o caminho alternativo está na mensagem: {stderr}"
        );
    }
}
