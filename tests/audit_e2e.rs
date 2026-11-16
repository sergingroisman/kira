//! End-to-end do `kira audit --weekly` (F2, PR19): o exame de rotina.
//!
//! Prova o que o item promete pelo binário real: o relatório abre com a janela,
//! conta as sessões que o próprio KIRA gravou, diz o estado da fila, mostra as
//! aprovações e **não** inventa número quando não há dado. Nada de rede, nada
//! de LLM: o store é isolado com `KIRA_STORE_DIR`.

use std::path::PathBuf;
use std::process::Command;

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "kira-audit-e2e-{}-{tag}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&store).expect("store");
        std::fs::write(cwd.join("README.md"), "# fixture\n").expect("readme");
        Sandbox { base, cwd, store }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_SECRET_STORE", "off")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_LLM")
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn weekly_report_states_the_window_and_counts_the_sessions_it_recorded() {
    let sandbox = Sandbox::new("janela");

    // Dois comandos que gravam trilha (o `profile` não grava sessão; o
    // `schedule` grava) — a trilha é a fonte do relatório, não a memória.
    let (code, _, stderr) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0, "{stderr}");
    let (code, _, stderr) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0, "{stderr}");

    let (code, stdout, stderr) = sandbox.run(&["audit"]);
    assert_eq!(code, 0, "audit falhou: {stderr}");
    // A janela é declarada — número sem janela não é auditável.
    assert!(
        stdout.starts_with("auditoria do KIRA — últimos 7 dia(s)"),
        "{stdout}"
    );
    assert!(stdout.contains("desde "), "{stdout}");
    // As sessões que acabamos de rodar aparecem.
    assert!(stdout.contains("sessões: 2"), "{stdout}");
    assert!(stdout.contains("schedule: 2"), "{stdout}");
    // Estado das filas, mesmo vazio, é dito — nunca omitido.
    assert!(stdout.contains("posts: fila vazia"), "{stdout}");
    assert!(stdout.contains("aprovações: 0 pendente(s)"), "{stdout}");
    assert!(stdout.contains("LLM: 0 chamada(s)"), "{stdout}");
}

#[test]
fn custom_window_is_honoured_and_named() {
    let sandbox = Sandbox::new("janela-30");
    let (code, _, stderr) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0, "{stderr}");

    let (code, stdout, stderr) = sandbox.run(&["audit", "--days", "30"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.starts_with("auditoria do KIRA — últimos 30 dia(s)"),
        "{stdout}"
    );
}

#[test]
fn failures_are_surfaced_and_point_at_history() {
    let sandbox = Sandbox::new("falha");
    // `inspect` sem id e sem projeto falha (exit != 0) e a sessão fica gravada
    // com ok = 0 — o relatório tem de mostrar isso em vez de esconder.
    let (code, _stdout, _stderr) = sandbox.run(&["inspect", "999999999"]);
    assert_ne!(code, 0, "o comando de fixture deveria falhar");

    let (code, stdout, stderr) = sandbox.run(&["audit"]);
    assert_eq!(code, 0, "{stderr}");
    let tem_falha = stdout.contains("falha(s)");
    assert!(tem_falha, "relatório sem contagem de falha: {stdout}");
    assert!(
        stdout.contains("`kira history`") || stdout.contains("nada pendente"),
        "{stdout}"
    );
}
