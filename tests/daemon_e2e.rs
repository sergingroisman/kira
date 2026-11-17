//! E2E do `kira daemon` (DAEMON-01): o contrato no limite do CLI é um daemon
//! por projeto (lock recusado com erro acionável, `--force` quebra), cadência
//! VINDA do agendamento (recusa sem agenda; ticka o vencido; `--ticks` é o
//! teto de CI), lock liberado na saída e status honesto.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-daemon-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        fs::write(cwd.join("README.md"), "# projeto\n").expect("README");
        Self { cwd, store }
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
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    /// O lock do daemon deste projeto.
    fn lock(&self) -> PathBuf {
        self.cwd.join(".kira").join("daemon.lock")
    }
}

#[test]
fn daemon_without_any_schedule_refuses_instead_of_faking_work() {
    let sandbox = Sandbox::new("sem-agenda");
    let (code, stdout, stderr) = sandbox.run(&["daemon", "run", "--ticks", "1"]);
    assert_ne!(code, 0, "sem agenda o daemon não tem o que fazer: {stdout}");
    assert!(stdout.is_empty(), "erro não vai para stdout: {stdout}");
    assert!(
        stderr.contains("sem agendamento declarado"),
        "acionável: {stderr}"
    );
    assert!(
        stderr.contains("kira schedule add"),
        "aponta o comando: {stderr}"
    );
    assert!(
        !sandbox.lock().exists(),
        "recusa não deixa lock: {stderr:?}"
    );
}

#[test]
fn daemon_ticks_the_due_schedule_releases_the_lock_and_is_reusable() {
    let sandbox = Sandbox::new("agenda");
    // Agenda fresca: "nunca rodou" ⇒ vencida agora (mesma decisão do
    // `kira loop --scheduled`). O digest é pulado por falta de trust — nota,
    // não erro: o daemon é executor da cadência, não conserta o dono.
    let (code, _, stderr) =
        sandbox.run(&["schedule", "add", "--action", "digest", "--every", "1h"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let (code, stdout, stderr) = sandbox.run(&["daemon", "run", "--ticks", "1", "--max-wait", "2"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("daemon vivo: PID"), "{stdout}");
    assert!(stdout.contains("tick 1:"), "{stdout}");
    assert!(
        stdout.contains("teto de 1 tick(s) atingido"),
        "teto sai com nota: {stdout}"
    );
    assert!(
        stdout.contains("digest pulado: repositório não aceito"),
        "sem trust o digest é nota, não erro: {stdout}"
    );
    // O lock morreu com o daemon: nada órfão no disco.
    assert!(!sandbox.lock().exists(), "lock foi liberado na saída");
    // Reuso: um novo daemon nasce sem --force (o lock antigo não travou).
    let (code, _, stderr) = sandbox.run(&["daemon", "run", "--ticks", "1", "--max-wait", "2"]);
    assert_eq!(code, 0, "reuso sem force: {stderr}");
}

#[test]
fn daemon_refuses_a_second_lock_and_force_breaks_it() {
    let sandbox = Sandbox::new("lock");
    // Agenda para o `--force` ter trabalho real depois de quebrar o lock.
    let (code, _, stderr) =
        sandbox.run(&["schedule", "add", "--action", "digest", "--every", "1h"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    // Lock "vivo" escrito à mão: o conteúdo tem PID + instante.
    fs::create_dir_all(sandbox.cwd.join(".kira")).expect("criar .kira");
    fs::write(sandbox.lock(), "pid=999999\nsince=2026-11-17T00:00:00Z\n").expect("lock de teste");
    let (code, stdout, stderr) = sandbox.run(&["daemon", "run", "--ticks", "1"]);
    assert_ne!(code, 0, "lock vivo deve recusar: {stdout}");
    assert!(stdout.is_empty(), "erro não vai para stdout: {stdout}");
    assert!(
        stderr.contains("já há um daemon neste projeto"),
        "acionável: {stderr}"
    );
    assert!(stderr.contains("PID 999999"), "identifica o dono: {stderr}");
    assert!(
        stderr.contains("kira daemon --force"),
        "aponta a saída: {stderr}"
    );
    // --force é o consentimento explícito: quebra o lock e roda.
    let (code, _, stderr) = sandbox.run(&[
        "daemon",
        "run",
        "--force",
        "--ticks",
        "1",
        "--max-wait",
        "2",
    ]);
    assert_eq!(code, 0, "force quebra e roda: {stderr}");
    assert!(!sandbox.lock().exists(), "lock liberado na saída");
}

#[test]
fn daemon_status_reads_the_lock_and_is_honest_about_absence() {
    let sandbox = Sandbox::new("status");
    let (code, stdout, _) = sandbox.run(&["daemon", "status"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("nenhum daemon vivo"),
        "sem lock é dito com a causa: {stdout}"
    );
    fs::create_dir_all(sandbox.cwd.join(".kira")).expect("criar .kira");
    fs::write(sandbox.lock(), "pid=4242\nsince=2026-11-17T01:02:03Z\n").expect("lock de teste");
    let (code, stdout, _) = sandbox.run(&["daemon", "status"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("daemon: lock"), "{stdout}");
    assert!(stdout.contains("PID 4242"), "{stdout}");
    assert!(stdout.contains("desde 2026-11-17T01:02:03Z"), "{stdout}");
    assert!(stdout.contains("kira schedule list"), "{stdout}");
}
