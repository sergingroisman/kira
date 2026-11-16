//! End-to-end test of `kira schedule` + `kira loop --scheduled` (F2, PR16).
//!
//! O agendamento é cron interno SEM daemon: o teste prova as três promessas do
//! item — (1) declarar/listar/remover uma cadência, (2) recusar ação e cadência
//! inválidas com erro acionável em vez de gravar lixo, e (3) o ciclo agendado
//! rodar SÓ a fase vencida e registrar o run (o segundo tick não repete).
//!
//! Nada de rede e nada de LLM: o projeto é um fixture e o digest é pulado por
//! falta de trust — o que interessa aqui é a decisão "é hora?", não o conteúdo.

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
            "kira-schedule-e2e-{}-{tag}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&store).expect("store");
        // Um arquivo qualquer: o ingest precisa de um projeto, não de um app.
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
            .env_remove("KIRA_OLLAMA_URL")
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
fn schedule_add_list_rm_roundtrip() {
    let sandbox = Sandbox::new("roundtrip");

    let (code, stdout, stderr) =
        sandbox.run(&["schedule", "add", "--action", "digest", "--every", "30m"]);
    assert_eq!(code, 0, "add falhou: {stderr}");
    assert!(
        stdout.contains("agendamento 1 criado: digest a cada 30m"),
        "{stdout}"
    );
    // O aviso de que não é serviço vai junto do add, não escondido no --help.
    assert!(stdout.contains("só roda com o KIRA vivo"), "{stdout}");

    let (code, stdout, stderr) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0, "list falhou: {stderr}");
    assert!(stdout.contains("agendamentos (1)"), "{stdout}");
    assert!(stdout.contains("id 1 · digest · a cada 30m"), "{stdout}");
    assert!(stdout.contains("vencido (nunca rodou)"), "{stdout}");
    assert!(
        stdout.contains("ativa") || stdout.contains("ativo"),
        "{stdout}"
    );

    let (code, stdout, stderr) = sandbox.run(&["schedule", "rm", "1"]);
    assert_eq!(code, 0, "rm falhou: {stderr}");
    assert!(stdout.contains("agendamento 1 removido"), "{stdout}");

    let (code, stdout, _) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("nenhum agendamento"), "{stdout}");
}

#[test]
fn schedule_add_rejects_unknown_action_and_bad_cadence() {
    let sandbox = Sandbox::new("invalid");

    let (code, _stdout, stderr) =
        sandbox.run(&["schedule", "add", "--action", "deploy", "--every", "30m"]);
    assert_ne!(code, 0, "ação inválida deve falhar");
    assert!(
        stderr.contains("ação de agendamento desconhecida"),
        "{stderr}"
    );
    assert!(stderr.contains("`scan`, `digest` e `publish`"), "{stderr}");

    // Abaixo do piso de 60s: laço quente é recusado na hora de gravar.
    let (code, _stdout, stderr) =
        sandbox.run(&["schedule", "add", "--action", "scan", "--every", "5s"]);
    assert_ne!(code, 0, "cadência curta deve falhar");
    assert!(stderr.contains("cadência"), "{stderr}");
    assert!(stderr.contains("60s"), "{stderr}");

    // Cadência ilegível não vira default silencioso.
    let (code, _stdout, stderr) = sandbox.run(&[
        "schedule",
        "add",
        "--action",
        "scan",
        "--every",
        "meia-hora",
    ]);
    assert_ne!(code, 0, "cadência ilegível deve falhar");
    assert!(stderr.contains("cadência"), "{stderr}");

    // Nada foi gravado: erro de validação não deixa linha pela metade.
    let (code, stdout, _) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("nenhum agendamento"), "{stdout}");
}

#[test]
fn schedule_rm_and_enable_of_unknown_id_are_actionable() {
    let sandbox = Sandbox::new("unknown");

    let (code, _stdout, stderr) = sandbox.run(&["schedule", "rm", "7"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("não há agendamento com id 7"), "{stderr}");
    assert!(stderr.contains("kira schedule list"), "{stderr}");

    let (code, _stdout, stderr) = sandbox.run(&["schedule", "enable", "7"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("não há agendamento com id 7"), "{stderr}");
}

#[test]
fn scheduled_loop_runs_only_the_due_phase_and_records_the_run() {
    let sandbox = Sandbox::new("due");

    // Só `publish` agendado: as outras fases não têm cadência declarada, então
    // o plano não as considera vencidas — é o opt-in por fase.
    let (code, _stdout, stderr) =
        sandbox.run(&["schedule", "add", "--action", "publish", "--every", "1h"]);
    assert_eq!(code, 0, "{stderr}");

    // 1º tick agendado: a fase nunca rodou ⇒ vencida ⇒ publica (sem rascunho,
    // "0 publicado(s)"), e o run é REGISTRADO.
    let (code, stdout, stderr) = sandbox.run(&["loop", "--once", "--scheduled"]);
    assert_eq!(code, 0, "loop agendado falhou: {stderr}");
    assert!(stdout.contains("tick 1:"), "{stdout}");
    assert!(
        !stdout.contains("publish pulado"),
        "vencida, não devia pular: {stdout}"
    );
    // Sem cadência para digest, o ciclo não roda o digest.
    assert!(
        stdout.contains("digest pulado: agendamento não vencido")
            || stdout.contains("digest pulado: repositório não aceito"),
        "{stdout}"
    );

    // O run ficou registrado: o `list` mostra o próximo tick derivado.
    let (code, stdout, _) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0);
    assert!(
        !stdout.contains("nunca rodou"),
        "run não registrado: {stdout}"
    );
    assert!(stdout.contains("em "), "próximo tick derivado: {stdout}");

    // 2º tick imediato: nada devido ⇒ nada acontece (anti-PR-inócuo).
    let (code, stdout, stderr) = sandbox.run(&["loop", "--once", "--scheduled"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("publish pulado: agendamento não vencido"),
        "{stdout}"
    );
    // `--once` roda um único tick por definição: nada de segundo tick.
    assert_eq!(
        stdout.matches("tick ").count(),
        1,
        "tick repetido não pode mudar estado: {stdout}"
    );
}

#[test]
fn scheduled_loop_without_schedules_warns_and_behaves_like_the_normal_loop() {
    let sandbox = Sandbox::new("empty");

    let (code, stdout, stderr) = sandbox.run(&["loop", "--once", "--scheduled"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("nenhum agendamento declarado"),
        "modo agendado sem agendamento precisa avisar: {stdout}"
    );
    // Sem cadência declarada o ciclo roda como sempre — agendar é restrição,
    // não pré-requisito.
    assert!(!stdout.contains("publish pulado"), "{stdout}");
}

#[test]
fn disabled_schedule_never_runs() {
    let sandbox = Sandbox::new("disabled");

    let (code, _stdout, stderr) = sandbox.run(&[
        "schedule",
        "add",
        "--action",
        "publish",
        "--every",
        "1h",
        "--disabled",
    ]);
    assert_eq!(code, 0, "{stderr}");

    let (code, stdout, _) = sandbox.run(&["schedule", "list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("desligado"), "{stdout}");

    let (code, stdout, stderr) = sandbox.run(&["loop", "--once", "--scheduled"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("publish pulado: agendamento não vencido"),
        "agendamento desligado não pode rodar: {stdout}"
    );

    // Ligar preserva a linha e o histórico; agora a fase está vencida.
    let (code, stdout, stderr) = sandbox.run(&["schedule", "enable", "1"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("agendamento 1 ligado"), "{stdout}");
    let (code, stdout, _) = sandbox.run(&["loop", "--once", "--scheduled"]);
    assert_eq!(code, 0);
    assert!(!stdout.contains("publish pulado"), "{stdout}");
}
