//! E2E do `kira start` (START-01): o contrato no limite do CLI é UM comando,
//! zero interação — pasta nova → store/facts/trust/agenda criados sem UMA
//! pergunta; `--dry-run` mostra o plano e não cria NADA; re-rodar nunca
//! duplica agendamento (o estado de retomada é o próprio mundo — ADR-0043).
//!
//! Hermeticidade: `KIRA_OLLAMA_URL` aponta para uma porta morta local — o
//! digest do primeiro tick falha RÁPIDO e determinístico (connection refused
//! conta como `digest_failed`, o tick não morre), sem tocar num Ollama vivo
//! da máquina do dono. O que o teste prova do ciclo é o que não depende de
//! modelo: ingest, trust, agenda e o tick inteiro saindo com exit 0.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    cwd: PathBuf,
    store: PathBuf,
    config: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-start-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        let config = base.join("config.toml");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        // Pasta NOVA de verdade: um README e nada de stack (o scan dentro do
        // ingest não spawna ferramenta nenhuma — determinístico).
        fs::write(cwd.join("README.md"), "# projeto\n").expect("README");
        Self { cwd, store, config }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", &self.config)
            .env("KIRA_SECRET_STORE", "off")
            // Porta morta LOCAL: o digest falha rápido, o tick não morre e
            // nenhum Ollama vivo da máquina é tocado (hermético nos 3 SOs).
            .env("KIRA_OLLAMA_URL", "http://127.0.0.1:1")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_LLM")
            .env_remove("KIRA_LANG")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    fn kira_dir(&self) -> PathBuf {
        self.cwd.join(".kira")
    }
}

/// O dry-run mostra o plano e NÃO cria nada: sem `.kira/`, sem store.db, sem
/// config.toml — e sem sessão gravada (consulta pura, mesma regra de
/// history/note/profile).
#[test]
fn dry_run_shows_the_plan_and_touches_nothing() {
    let sandbox = Sandbox::new("dry");
    let (code, stdout, stderr) = sandbox.run(&["start", "--dry-run"]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("plano do kira start"), "{stdout}");
    assert!(stdout.contains("nada executado"), "{stdout}");
    // O plano diz o que FARIA — os quatro passos reais, com a agenda default.
    assert!(stdout.contains("scan a cada 30m"), "{stdout}");
    assert!(stdout.contains("digest a cada 2h"), "{stdout}");
    assert!(stdout.contains("primeiro tick"), "{stdout}");
    assert!(stdout.contains("kira daemon run"), "{stdout}");
    // E o mundo não mudou: NADA de efeito colateral num dry-run.
    assert!(
        !sandbox.kira_dir().exists(),
        "dry-run criou .kira — efeito colateral proibido"
    );
    assert!(
        !sandbox.store.join("store.db").exists(),
        "dry-run criou store.db — a leitura da agenda é read-only (ADR-0043 D4)"
    );
    assert!(
        !sandbox.config.exists(),
        "dry-run criou config.toml — efeito colateral proibido"
    );
}

/// O contrato central: pasta nova → UM comando → sem UMA pergunta → store,
/// facts, trust e agenda criados, com o resumo apontando o próximo passo.
#[test]
fn start_on_a_fresh_folder_never_asks_anything() {
    let sandbox = Sandbox::new("fresco");
    let (code, stdout, stderr) = sandbox.run(&["start"]);
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    // Zero interação: a pergunta do nome do dev NUNCA aparece (stdin não é
    // TTY e o start é não-interativo por contrato, não por acidente).
    assert!(!stdout.contains("Como devo te chamar?"), "{stdout}");
    assert!(!stdout.contains("(s/N)"), "{stdout}");
    // Passo 1 — setup não-interativo criou o config.toml com defaults.
    assert!(sandbox.config.exists(), "config.toml não foi criado");
    // Passo 1b — o ingest do setup deixou os facts de pé.
    assert!(
        sandbox.kira_dir().join("facts.md").exists(),
        ".kira/facts.md não foi criado"
    );
    // Passo 2 — a confiança local nasceu no start (o digest agora pode rodar).
    assert!(
        sandbox.kira_dir().join("trust.json").exists(),
        ".kira/trust.json não foi criado"
    );
    // Passo 3 — a agenda default nasceu: scan 30m + digest 2h.
    assert!(stdout.contains("criado: scan a cada 30m"), "{stdout}");
    assert!(stdout.contains("criado: digest a cada 2h"), "{stdout}");
    // Passo 4 — o primeiro tick rodou o ciclo de verdade (o digest falha na
    // porta morta e CONTA a falha — o tick segue de pé, nunca morre por isso).
    assert!(stdout.contains("tick 1:"), "{stdout}");
    // O resumo final aponta o próximo passo — nunca "configuração concluída".
    assert!(stdout.contains("pronto: start concluído"), "{stdout}");
    assert!(stdout.contains("kira daemon run"), "{stdout}");
    assert!(!stdout.contains("configuração concluída"), "{stdout}");
    // O store de trilha existe (o start grava sessão; a leitura do plano no
    // dry-run não criava — agora o start real criou).
    assert!(sandbox.store.join("store.db").exists());
}

/// O estado de retomada é o próprio mundo (ADR-0043 D2): re-rodar `kira start`
/// não duplica agendamento, não repergunta nada e sai 0 — cada passo deriva
/// o estado real antes de agir.
#[test]
fn restarting_never_duplicates_the_schedule() {
    let sandbox = Sandbox::new("retoma");
    let (first_code, first_stdout, first_stderr) = sandbox.run(&["start"]);
    assert_eq!(
        first_code, 0,
        "primeiro start: {first_stdout}\n{first_stderr}"
    );
    let (second_code, second_stdout, second_stderr) = sandbox.run(&["start"]);
    assert_eq!(
        second_code, 0,
        "segundo start: {second_stdout}\n{second_stderr}"
    );
    // Nenhum agendamento novo: as duas fases já estavam declaradas.
    assert!(
        second_stdout.contains("já declarado — nada criado"),
        "{second_stdout}"
    );
    assert!(
        !second_stdout.contains("criado: scan"),
        "segundo start duplicou o scan: {second_stdout}"
    );
    assert!(
        !second_stdout.contains("criado: digest"),
        "segundo start duplicou o digest: {second_stdout}"
    );
    // A prova no próprio comando da casa: exatamente 2 agendamentos.
    let (list_code, list_stdout, list_stderr) = sandbox.run(&["schedule", "list"]);
    assert_eq!(list_code, 0, "{list_stdout}\n{list_stderr}");
    assert!(list_stdout.contains("agendamentos (2)"), "{list_stdout}");
    // E a segunda passada seguiu o ciclo sem refazer pergunta nenhuma.
    assert!(second_stdout.contains("tick 1:"), "{second_stdout}");
}
