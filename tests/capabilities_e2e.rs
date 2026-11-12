//! End-to-end do MVP-1 (capacidades e gating, ADR-0030): a matriz de
//! capacidades vira tabela de casos — cada linha roda o binário e confere
//! mensagem + exit code.
//!
//! Nada de rede externa: quando a capacidade falta, a prova é por AUSÊNCIA de
//! servidor (listener com prazo que captura qualquer conexão — se alguém
//! conectar, o teste falha) + timeout curto no binário.

use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

const FIXTURE_REL: &str = "tests/fixtures/work_item_4821.json";

/// Mensagem EXATA do contrato (ADR-0030 #5): aperece verbatim no stderr do
/// `triage <id>` e do `inspect` sem capacidade.
const BOARD_AZURE_MISSING: &str = "esta jornada precisa de board.azure — rode: kira connect azure";

/// Sandbox: projeto temporário (com .git e package.json, para o scan achar
/// stack) + store isolado + nenhuma credencial no ambiente.
struct Sandbox {
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("kira-cap-e2e-{tag}-{}-{nanos}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(cwd.join(".git"))
            .unwrap_or_else(|err| panic!("criar {cwd:?}/.git: {err}"));
        std::fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        std::fs::write(cwd.join("package.json"), "{\"name\":\"sandbox\"}\n")
            .unwrap_or_else(|err| panic!("escrever package.json: {err}"));
        Self { cwd, store }
    }

    fn fixture(&self) -> String {
        format!("{}/{}", env!("CARGO_MANIFEST_DIR"), FIXTURE_REL)
    }

    /// Roda o binário com env limpo de credencial: sem PAT/org/projeto do
    /// Azure (é isso que prova "comandos locais não exigem capacidade").
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        self.run_with(args, &[], &[])
    }

    /// Roda com ajustes extras de env (o teste de `PAT falso` precisa SETAR
    /// credencial de mentira para provar que o gating vem antes dela).
    fn run_with(
        &self,
        args: &[&str],
        envs: &[(&str, String)],
        removes: &[&str],
    ) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("config-absente.toml"))
            .env("KIRA_SECRET_STORE", "off")
            // Porta morta local: o backend de IA nunca responde — a linha
            // `triage --file` da matriz falha no LLM (determinístico), nunca
            // num Ollama real da máquina de quem roda o teste.
            .env("KIRA_OLLAMA_URL", "http://127.0.0.1:9")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_LLM")
            .env_remove("KIRA_BOARD");
        for (name, value) in envs {
            command.env(name, value);
        }
        for name in removes {
            command.env_remove(name);
        }
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// Roda com env ZERADO (`env_clear`) e só o essencial reconstruído — a
    /// linha da matriz que prova "roda com env limpo" na forma mais forte.
    fn run_env_clear(&self, args: &[&str]) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command
            .args(args)
            .current_dir(&self.cwd)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", &self.store)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("config-absente.toml"))
            .env("KIRA_SECRET_STORE", "off");
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?} (env limpo): {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

/// Servidor falso de Ollama que desiste após `deadline`: retorno `None`
/// significa que NADA conectou — é a prova de ausência de rede (o mesmo
/// padrão dos e2e existentes para "sem servidor, nada conectou").
fn spawn_deadline_ollama(deadline: Duration) -> (String, std::thread::JoinHandle<Option<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|err| panic!("bind: {err}"));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|err| panic!("endereço local: {err}"));
    let handle = std::thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|err| panic!("nonblocking: {err}"));
        let start = Instant::now();
        loop {
            if start.elapsed() >= deadline {
                // O prazo venceu sem nenhuma conexão: provado que ninguém ligou.
                return None;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    listener
                        .set_nonblocking(false)
                        .unwrap_or_else(|err| panic!("blocking: {err}"));
                    stream
                        .set_nonblocking(false)
                        .unwrap_or_else(|err| panic!("blocking stream: {err}"));
                    let mut raw = vec![0u8; 4096];
                    let read = stream.read(&mut raw).unwrap_or(0);
                    raw.truncate(read);
                    return Some(String::from_utf8_lossy(&raw).into_owned());
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return None,
            }
        }
    });
    (format!("http://{addr}"), handle)
}

/// Matriz do ADR-0030 como tabela de casos: cada linha roda o binário e
/// confere mensagem + exit code. Comando vivo exige capacidade; comandos
/// locais rodam com env limpo; `connect` responde "implementação no MVP-3".
#[test]
fn matriz_de_capacidades_do_adr_0030_vira_tabela_de_casos() {
    let sandbox = Sandbox::new("matriz");
    let fixture = sandbox.fixture();

    // (nome, args, substring obrigatória na saída, exit zero?)
    let casos: Vec<(&str, Vec<&str>, &str, bool)> = vec![
        (
            "scan — roda sem capacidade",
            vec!["scan"],
            "kira scan em",
            true,
        ),
        ("codex (wiki list) — roda", vec!["wiki", "list"], "", true),
        (
            "lens (context --file) — roda",
            vec!["context", "--file", &fixture],
            "",
            true,
        ),
        (
            "scribe (pr draft --file) — roda",
            vec!["pr", "draft", "4821", "--file", &fixture],
            "",
            true,
        ),
        (
            "watch (dashboard --snapshot) — roda",
            vec!["dashboard", "--snapshot"],
            "",
            true,
        ),
        (
            "judge --file (triage --file) — não exige capacidade, só backend de IA",
            vec!["triage", "--file", &fixture],
            "Ollama não está rodando",
            false,
        ),
        (
            "judge <id> (triage <id>) — board.azure faltando",
            vec!["triage", "42"],
            BOARD_AZURE_MISSING,
            false,
        ),
        (
            "read (inspect) — board.azure faltando",
            vec!["inspect", "42"],
            BOARD_AZURE_MISSING,
            false,
        ),
        (
            "connect azure — mensagem exata do MVP-3",
            vec!["connect", "azure"],
            "conecte com: kira connect azure (implementação no MVP-3)",
            false,
        ),
        (
            "connect github — mesmo padrão de 'ainda não implementado'",
            vec!["connect", "github"],
            "ainda não implementado",
            false,
        ),
        (
            "connect gitlab — mesmo padrão de 'ainda não implementado'",
            vec!["connect", "gitlab"],
            "ainda não implementado",
            false,
        ),
    ];

    for (nome, args, deve_conter, exit_zero) in casos {
        let (code, stdout, stderr) = sandbox.run(&args);
        let saida = format!("{stdout}{stderr}");
        assert_eq!(
            exit_zero,
            code == 0,
            "linha da matriz '{nome}': exit code {code} inesperado — saída: {saida}"
        );
        if !deve_conter.is_empty() {
            assert!(
                saida.contains(deve_conter),
                "linha da matriz '{nome}': mensagem esperada não veio — saída: {saida}"
            );
        }
    }
}

/// Linhas locais da matriz na forma mais forte: env ZERADO (não apenas sem as
/// variáveis do Azure) — zero credencial, zero integração, e os comandos rodam.
#[test]
fn comandos_locais_rodam_com_env_completamente_limpo() {
    let sandbox = Sandbox::new("env-limpo");
    let fixture = sandbox.fixture();
    for args in [
        vec!["scan"],
        vec!["wiki", "list"],
        vec!["context", "--file", fixture.as_str()],
        vec!["pr", "draft", "4821", "--file", fixture.as_str()],
        vec!["dashboard", "--snapshot"],
    ] {
        let (code, stdout, stderr) = sandbox.run_env_clear(&args);
        assert_eq!(
            code,
            0,
            "'kira {}' deve rodar com env limpo — saída: {stdout}{stderr}",
            args.join(" ")
        );
        let saida = format!("{stdout}{stderr}");
        assert!(
            !saida.contains("esta jornada precisa de"),
            "'kira {}' nunca pode exigir capacidade: {saida}",
            args.join(" ")
        );
    }
}

/// ADR-0030 #6: presença de credencial NÃO é consentimento — nem com PAT
/// falso no ambiente o comando de board executa: o gating vem ANTES de
/// resolver a credencial e de qualquer rede (nada de reqwest, nada de PAT).
#[test]
fn triage_e_inspect_com_pat_falso_ainda_falham_por_capacidade() {
    let sandbox = Sandbox::new("pat-falso");
    let envs = [
        ("AZURE_DEVOPS_PAT", "PAT-FALSO-DE-TESTE".to_string()),
        ("AZURE_DEVOPS_ORG", "org-inexistente".to_string()),
        ("AZURE_DEVOPS_PROJECT", "Projeto".to_string()),
    ];
    for args in [vec!["triage", "42"], vec!["inspect", "42"]] {
        let started = Instant::now();
        let (code, _stdout, stderr) = sandbox.run_with(&args, &envs, &[]);
        assert_ne!(
            code,
            0,
            "'kira {}' com PAT falso não pode executar a jornada",
            args.join(" ")
        );
        assert!(
            stderr.contains(BOARD_AZURE_MISSING),
            "'kira {}' deve falhar com mensagem de capacidade, nunca 401/PAT: {stderr}",
            args.join(" ")
        );
        assert!(
            !stderr.contains("PAT inválido") && !stderr.contains("HTTP "),
            "gating veio antes da credencial — não pode haver erro de rede/auth: {stderr}"
        );
        // Sem tentativa de rede (DNS/HTTP contra dev.azure.com custaria muito
        // mais): o gating encerra em segundos.
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "'kira {}' demorou demais ({:?}) — parece ter tentado a rede",
            args.join(" "),
            started.elapsed()
        );
    }
}

/// Prova por ausência de servidor: `triage <id>` sem capacidade NÃO chega ao
/// backend de IA (o listener com prazo fica 2s esperando; ninguém conecta).
#[test]
fn triage_sem_capacidade_nao_toca_nenhum_servidor() {
    let sandbox = Sandbox::new("sem-rede");
    let (base_url, server) = spawn_deadline_ollama(Duration::from_secs(2));
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(["triage", "42"])
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env("KIRA_CONFIG", sandbox.store.join("config-absente.toml"))
        .env("KIRA_SECRET_STORE", "off")
        .env("KIRA_OLLAMA_URL", &base_url)
        .env_remove("AZURE_DEVOPS_PAT")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT");
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("executar kira triage 42: {err}"));
    assert_eq!(
        output.status.code(),
        Some(1),
        "gating deve falhar a jornada"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(stderr.contains(BOARD_AZURE_MISSING), "{stderr}");
    // O prazo venceu sem conexão: nada chegou ao backend de IA.
    let request = server
        .join()
        .unwrap_or_else(|_| panic!("thread do servidor entrou em pânico"));
    assert!(
        request.is_none(),
        "triage sem capacidade NÃO pode contatar o backend de IA — recebeu: {:?}",
        request
    );
}

/// `kira connect <provider>`: aceita o argumento, responde a mensagem do
/// MVP-3 (azure) / "ainda não implementado" (github/gitlab) e sai com exit
/// != 0 — nada é conectado de verdade, nada é gravado como conectado.
#[test]
fn connect_azure_responde_mensagem_do_mvp3_com_exit_nao_zero() {
    let sandbox = Sandbox::new("connect");
    for (provider, esperado) in [
        (
            "azure",
            "conecte com: kira connect azure (implementação no MVP-3)",
        ),
        (
            "github",
            "conecte com: kira connect github (ainda não implementado — fica para um MVP posterior)",
        ),
        (
            "gitlab",
            "conecte com: kira connect gitlab (ainda não implementado — fica para um MVP posterior)",
        ),
    ] {
        let (code, stdout, stderr) = sandbox.run(&["connect", provider]);
        assert_ne!(code, 0, "connect {provider} não pode fingir sucesso");
        let saida = format!("{stdout}{stderr}");
        assert!(
            saida.contains(esperado),
            "connect {provider}: mensagem esperada não veio — saída: {saida}"
        );
    }
    // Provider desconhecido: erro claro, nunca "conectado".
    let (code, stdout, stderr) = sandbox.run(&["connect", "jira"]);
    assert_ne!(code, 0);
    let saida = format!("{stdout}{stderr}");
    assert!(
        saida.contains("provider 'jira' desconhecido"),
        "provider inválido precisa dizer quais existem: {saida}"
    );
    // Nada foi conectado em nenhum dos casos: o estado continua desconectado.
    let (code, _stdout, stderr) = sandbox.run(&["triage", "42"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains(BOARD_AZURE_MISSING),
        "nenhum connect pode ter conectado board.azure: {stderr}"
    );
}

/// Migração de `user_version` pelo caminho do binário: abrir o KIRA num store
/// da versão anterior mantém as tabelas antigas (e os dados) e cria a de
/// capacidades — e o estado vazio continua significando "desconectada".
#[test]
fn store_da_versao_anterior_migra_sem_perder_dados() {
    let sandbox = Sandbox::new("migracao");
    // Cria um store atual (o gating abre o store e grava a sessão falha) e o
    // rebaixa ao estado que a v3 deixava (sem `capabilities`, user_version = 3).
    let (code, _stdout, stderr) = sandbox.run(&["triage", "42"]);
    assert_ne!(code, 0, "preparar store: {stderr}");
    let db_path = sandbox.store.join("store.db");
    {
        let conn = rusqlite::Connection::open(&db_path)
            .unwrap_or_else(|err| panic!("abrir store.db: {err}"));
        conn.execute_batch(
            "DROP TABLE IF EXISTS capabilities;
             PRAGMA user_version = 3;",
        )
        .unwrap_or_else(|err| panic!("rebaixar store: {err}"));
    }
    // Próxima abertura migra e o KIRA segue funcionando.
    let (code, _stdout, stderr) = sandbox.run(&["triage", "42"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains(BOARD_AZURE_MISSING),
        "migração não pode ligar capacidade implícita: {stderr}"
    );
    let conn = rusqlite::Connection::open(&db_path)
        .unwrap_or_else(|err| panic!("reabrir store.db: {err}"));
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or_else(|err| panic!("user_version: {err}"));
    assert_eq!(version, 4, "store rebaixado deve migrar para a v4");
    for tabela in [
        "projects",
        "sessions",
        "facts",
        "wiki_page",
        "repo_file",
        "capabilities",
    ] {
        let exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [tabela],
                |row| row.get(0),
            )
            .unwrap_or_else(|err| panic!("conferir tabela {tabela}: {err}"));
        assert_eq!(exists, 1, "tabela '{tabela}' deve existir após a migração");
    }
    // E o dado de antes da migração continua lá (aditiva, nunca destrutiva).
    let sessions: i64 = conn
        .query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))
        .unwrap_or_else(|err| panic!("sessions: {err}"));
    assert!(
        sessions >= 1,
        "a sessão registrada antes do rebaixamento deve sobreviver à migração"
    );
}
