//! End-to-end de `kira digest` (ADR-0038, M3) com um Ollama falso em `127.0.0.1`.
//!
//! Contratos verificados no binário real:
//! - sem `kira trust` o comando **recusa** e diz o que fazer;
//! - com trust + ingest, gera os posts e grava rascunho em `.kira/drafts/`;
//! - rodar de novo não refaz nada (cache por procedência);
//! - trocar `--lang` **regenera** (o idioma entra no `cache_key`);
//! - `--budget 0` para com nota e exit 0, nunca erro;
//! - `--pause 1` entrega um post e deixa o resto na fila com briefing.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const FAKE_BODY: &str = "{\"model\":\"fake\",\"message\":{\"role\":\"assistant\",\"content\":\"What this is\\nKIRA e um CLI.\\n\"},\"done\":true}";

/// Ollama falso com corpo customizado (HF-3: o teste do teto precisa de uma
/// resposta gigante e o do segredo, de um token fake no corpo).
fn spawn_fake_ollama_body(fake_body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let fake_body = fake_body.to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let fake_body = fake_body.clone();
            std::thread::spawn(move || {
                let mut buffer = [0u8; 8192];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{fake_body}",
                    fake_body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            });
        }
    });
    format!("http://{addr}")
}

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
    ollama: String,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        Self::new_with_body(tag, FAKE_BODY)
    }

    /// Sandbox cujo Ollama falso responde um corpo específico (HF-3: teto e
    /// segredo precisam de respostas diferentes da padrão).
    fn new_with_body(tag: &str, fake_body: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "kira-digest-e2e-{}-{tag}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&store).expect("store");
        std::fs::write(cwd.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("manifesto");
        std::fs::write(cwd.join("src/main.rs"), "fn main() {}\n").ok();
        std::fs::create_dir_all(cwd.join("src")).expect("src");
        std::fs::write(cwd.join("src/main.rs"), "fn main() {}\n").expect("main");
        Self {
            base,
            cwd,
            store,
            ollama: spawn_fake_ollama_body(fake_body),
        }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
            .env("KIRA_SECRET_STORE", "off")
            .env("KIRA_OLLAMA_URL", &self.ollama)
            .env("KIRA_MODEL", "fake")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_BOARD")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    /// Estado de partida: fatos ingeridos e repositório aceito.
    fn prepare(&self) {
        let (code, stdout, stderr) = self.run(&["ingest", "--skip-scan"]);
        assert_eq!(code, 0, "ingest: {stdout}\n{stderr}");
        let (code, stdout, stderr) = self.run(&["trust"]);
        assert_eq!(code, 0, "trust: {stdout}\n{stderr}");
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.cwd.join(rel)).unwrap_or_default()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn without_trust_the_command_refuses_and_says_what_to_do() {
    let sandbox = Sandbox::new("sem-trust");
    let (code, _, stderr) = sandbox.run(&["ingest", "--skip-scan"]);
    assert_eq!(code, 0, "ingest: {stderr}");
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "pt-BR"]);
    assert_ne!(
        code, 0,
        "sem trust o digest não pode rodar. stdout: {stdout}"
    );
    assert!(
        stderr.contains("kira trust"),
        "a mensagem precisa dizer o comando: {stderr}"
    );
    assert!(
        !sandbox.cwd.join(".kira/digest").exists(),
        "nada foi gerado sem aceite"
    );
}

#[test]
fn digest_generates_the_two_posts_and_writes_the_briefing() {
    let sandbox = Sandbox::new("gera");
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("2 post(s) novo(s)"), "stdout: {stdout}");

    let intro = sandbox.read(".kira/drafts/intro.md");
    assert!(intro.contains("What this is"), "rascunho do intro: {intro}");
    assert!(
        intro.contains("lang: pt-BR"),
        "idioma não ficou registrado: {intro}"
    );
    let c4 = sandbox.read(".kira/drafts/c4-container.md");
    assert!(!c4.is_empty(), "rascunho do c4 ausente");

    let briefing = sandbox.read(".kira/briefing.md");
    assert!(
        briefing.contains("pt-BR"),
        "briefing sem idioma: {briefing}"
    );
    assert!(
        briefing.contains("kira publish"),
        "fila vazia deve apontar o publish: {briefing}"
    );
}

#[test]
fn a_second_run_reuses_the_cache() {
    let sandbox = Sandbox::new("cache");
    sandbox.prepare();
    let (code, _, stderr) = sandbox.run(&["digest", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "primeira rodada: {stderr}");
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "segunda rodada: {stderr}");
    assert!(
        stdout.contains("0 post(s) novo(s)") && stdout.contains("2 no cache"),
        "a segunda rodada não pode refazer trabalho: {stdout}"
    );
}

/// O idioma faz parte do `cache_key` (ADR-0038 D6): trocar `--lang` regenera.
#[test]
fn changing_the_language_regenerates_instead_of_serving_stale_cache() {
    let sandbox = Sandbox::new("idioma");
    sandbox.prepare();
    let (code, _, stderr) = sandbox.run(&["digest", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "rodada pt-BR: {stderr}");
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "en"]);
    assert_eq!(code, 0, "rodada en: {stderr}");
    assert!(
        stdout.contains("2 post(s) novo(s)"),
        "idioma novo precisa regenerar: {stdout}"
    );
    assert!(
        sandbox.read(".kira/drafts/intro.md").contains("lang: en"),
        "o rascunho precisa estar no idioma novo"
    );
}

#[test]
fn budget_zero_stops_with_a_note_and_exit_zero() {
    let sandbox = Sandbox::new("budget");
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "pt-BR", "--budget", "0"]);
    assert_eq!(code, 0, "orçamento estourado não é erro: {stderr}");
    assert!(
        stdout.contains("orçamento"),
        "faltou a nota de orçamento: {stdout}"
    );
    let briefing = sandbox.read(".kira/briefing.md");
    assert!(
        briefing.contains("pendente"),
        "briefing precisa mostrar a fila: {briefing}"
    );
    assert!(
        briefing.contains("kira digest --resume"),
        "com fila pendente o próximo passo é resume: {briefing}"
    );
}

#[test]
fn pause_after_one_post_leaves_the_rest_in_the_queue() {
    let sandbox = Sandbox::new("pause");
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "pt-BR", "--pause", "1"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("pausado"), "stdout: {stdout}");
    let first = sandbox.read(".kira/drafts/intro.md");
    let second = sandbox.read(".kira/drafts/c4-container.md");
    // A ordem é determinística (created_at, slug): `c4-container` vem antes de
    // `intro`. O contrato do --pause é "um só", não "qual deles".
    assert!(
        first.is_empty() ^ second.is_empty(),
        "exatamente um post por rodada: intro={} c4={}",
        !first.is_empty(),
        !second.is_empty()
    );

    // Retomar entrega o que faltava, sem refazer o primeiro.
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "pt-BR", "--resume"]);
    assert_eq!(code, 0, "resume: {stderr}");
    assert!(
        stdout.contains("1 post(s) novo(s)"),
        "o resume entrega só o que faltava: {stdout}"
    );
    assert!(!sandbox.read(".kira/drafts/c4-container.md").is_empty());
}

// ---------------------------------------------------------------------------
// HF-3 — guardrail determinístico de saída (front-matter + teto + redact)
// ---------------------------------------------------------------------------

/// Resposta do modelo acima do teto do post: o guardrail recusa com nota
/// acionável e NADA é gravado — truncar texto do modelo seria publicar meia
/// frase com cara de inteira.
#[test]
fn draft_over_the_cap_is_refused_with_a_note_and_nothing_is_written() {
    // Conteúdo do tamanho exato do teto: com o front-matter somado, o
    // rascunho passa do limite e a recusa é inevitável (sem número mágico:
    // o teto é a constante pública do módulo).
    let huge = "a".repeat(kira::post::DRAFT_CHAR_CAP);
    let body = format!(
        "{{\"model\":\"fake\",\"message\":{{\"role\":\"assistant\",\"content\":\"{huge}\"}},\"done\":true}}"
    );
    let sandbox = Sandbox::new_with_body("teto", &body);
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["digest", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "recusa do guardrail é nota, não erro: {stderr}");
    assert!(
        stdout.contains("guardrail de saída recusou") && stdout.contains("acima do teto"),
        "nota acionável do teto: {stdout}"
    );
    let drafts = std::fs::read_dir(sandbox.cwd.join(".kira").join("drafts"))
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0);
    assert_eq!(drafts, 0, "nada pode ser gravado acima do teto");
}

/// Token fake na resposta do modelo: o rascunho gravado já sai mascarado —
/// segredo nunca chega ao disco, nem em dado derivado (regra dura da casa).
#[test]
fn draft_masks_a_fake_token_before_writing() {
    let body = "{\"model\":\"fake\",\"message\":{\"role\":\"assistant\",\"content\":\"Setup: token=AbC123XyZ789QqQ e ghp_0123456789abcdefghijABCDEFGHIJ012345\\nTexto normal.\\n\"},\"done\":true}";
    let sandbox = Sandbox::new_with_body("segredo", body);
    sandbox.prepare();
    let (code, _, stderr) = sandbox.run(&["digest", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "digest feliz: {stderr}");
    // A ordem de claim é (created_at, slug): `c4-container` vem antes de
    // `intro` — o teste não depende de qual slug, só do primeiro gravado.
    let draft = sandbox.read(".kira/drafts/c4-container.md");
    assert!(!draft.is_empty(), "o rascunho foi gravado");
    assert!(
        !draft.contains("AbC123XyZ789QqQ"),
        "token de atribuição vazou no rascunho: {draft}"
    );
    assert!(
        !draft.contains("ghp_0123456789abcdefghijABCDEFGHIJ012345"),
        "token em formato conhecido vazou: {draft}"
    );
    assert!(draft.contains("Texto normal."), "corpo preservado: {draft}");
    assert!(
        draft.starts_with("---\n"),
        "front-matter obrigatório no rascunho: {draft}"
    );
}
