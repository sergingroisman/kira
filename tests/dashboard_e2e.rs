//! End-to-end tests of `kira dashboard` (W6-PR1..3, ADR-0014).
//!
//! Four proofs, all offline:
//! 1. `--snapshot` with a store/config fixture prints the header (name, `dev:`,
//!    masked PAT) and exits 0 — the raw PAT never appears;
//! 2. **read-only proof**: running the dashboard with and without `--snapshot`
//!    leaves the `store.db` hash byte-identical — nothing writes, not even the
//!    automatic session trail (the command is `records: false`);
//! 3. `KIRA_OLLAMA_URL` on a dead port still works — the dashboard never
//!    touches the network (no probe, no LLM);
//! 4. `--snapshot` with 3 recorded sessions (W6-PR3) prints the sessions
//!    table (#id, comando, modelo, duração, 1ª linha do summary) and the
//!    store hash stays byte-identical.
//!
//! No process-global env var is mutated: every override is passed to the
//! spawned binary only (same convention as `tests/store_e2e.rs`).

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use kira::store::{self, Store};

/// A fake PAT for the env-resolution path. The masked prefix `Aa2x` is what
/// the header must show; the full value must NEVER appear in the output.
const FAKE_PAT: &str = "Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT";

/// Isolated sandbox: project cwd + store dir + config file, all temp.
struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-dashboard-e2e-{}-{name}-{nanos}",
            std::process::id()
        ));
        let cwd = base.join("proj");
        std::fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar workdir {cwd:?}: {err}"));
        Sandbox {
            base: base.clone(),
            cwd,
            store: base.join("store"),
        }
    }

    /// Store fixture: a real project row + one recorded `scan` session (with a
    /// recent timestamp, so the header shows a live age). Closing the store
    /// checkpoints the WAL, leaving a single stable `store.db` to hash.
    /// `with_scan = false` seeds only the project (no scan session) — the
    /// "nunca rodou kira scan" honesty case.
    fn seed_store_with_scan(&self, with_scan: bool) {
        let store_db = Store::open_in_dir(&self.store)
            .unwrap_or_else(|err| panic!("abrir store fixture: {err}"));
        let project = store_db
            .ensure_project(&self.cwd)
            .unwrap_or_else(|err| panic!("criar projeto fixture: {err}"));
        if with_scan {
            store_db
                .record(
                    &project,
                    "scan",
                    "",
                    None,
                    &store::now_rfc3339_utc(),
                    1_200,
                    true,
                    "scan fixture para o dashboard",
                )
                .unwrap_or_else(|err| panic!("gravar sessão scan fixture: {err}"));
        }
    }

    /// Store fixture (W6-PR3): 3 sessões reais na trilha via `Store::record`
    /// (scan ok, triage ok com modelo, doctor que falhou) — o painel de
    /// sessões lê o MESMO banco que os comandos gravam. Fecha a store antes
    /// de voltar para o hash ficar estável (WAL checkpointed).
    fn seed_store_with_sessions(&self) {
        let store_db = Store::open_in_dir(&self.store)
            .unwrap_or_else(|err| panic!("abrir store fixture: {err}"));
        let project = store_db
            .ensure_project(&self.cwd)
            .unwrap_or_else(|err| panic!("criar projeto fixture: {err}"));
        let now = store::now_rfc3339_utc();
        let record = |command: &str,
                      args: &str,
                      model: Option<&str>,
                      started_at: &str,
                      duration_ms: i64,
                      ok: bool,
                      summary: &str| {
            store_db
                .record(
                    &project,
                    command,
                    args,
                    model,
                    started_at,
                    duration_ms,
                    ok,
                    summary,
                )
                .unwrap_or_else(|err| panic!("gravar sessão {command} fixture: {err}"));
        };
        // Três sessões, ids 1..=3 na ordem: mais recente primeiro no painel.
        record(
            "scan",
            "",
            None,
            "2026-11-09T09:00:00Z",
            1_200,
            true,
            "scan fixture: 3 achados\nlinha 2 interna do summary",
        );
        record(
            "triage",
            "4821",
            Some("qwen2.5:7b"),
            "2026-11-10T09:00:00Z",
            65_000,
            true,
            "nota gravada no episódio do bug",
        );
        record("doctor", "", None, &now, 400, false, "ollama não respondeu");
    }

    /// Wiki fixture (W6-PR4): 2 páginas reais em `.kira/wiki/` (contexto +
    /// bugs/4821) indexadas pelo MESMO caminho de produção (`Wiki::reindex`
    /// grava as linhas em `wiki_page`/`wiki_fts`). Fecha a store antes de
    /// voltar, para o hash ficar estável (WAL checkpointed).
    fn seed_wiki_fixture(&self) {
        let markdown_contexto = "# Contexto do projeto\n\nDecisão **com fofura** e `kira scan`.\n\n- item um\n- item dois\n\n> uma citação\n\n---\n\nveja [o guia](https://exemplo.com/guia)\n";
        let markdown_bug = "# Sessão 4821\n\ntoken expirado em **produção**\n";
        let wiki = kira::wiki::Wiki::open(&self.cwd)
            .unwrap_or_else(|err| panic!("abrir wiki fixture: {err}"));
        wiki.write("contexto", markdown_contexto)
            .unwrap_or_else(|err| panic!("escrever contexto fixture: {err}"));
        wiki.write("bugs/4821", markdown_bug)
            .unwrap_or_else(|err| panic!("escrever bug fixture: {err}"));
        let store_db = Store::open_in_dir(&self.store)
            .unwrap_or_else(|err| panic!("abrir store fixture: {err}"));
        let indexed = wiki
            .reindex(&store_db)
            .unwrap_or_else(|err| panic!("reindexar wiki fixture: {err}"));
        assert_eq!(indexed, 2, "as duas páginas foram indexadas");
    }

    /// Config fixture with `dev_name` — the header shows it as `dev: <nome>`.
    fn config_path(&self) -> PathBuf {
        let config = self.base.join("config.toml");
        std::fs::write(
            &config,
            "dev_name = \"Sérgio\"\norganization = \"org\"\nproject = \"proj\"\n",
        )
        .unwrap_or_else(|err| panic!("escrever config fixture: {err}"));
        config
    }

    /// Findings fixture (W6-PR2): 3 achados, 2 severidades (critical 1,
    /// high 2), escrito direto em `.kira/findings.json` — o painel lê o MESMO
    /// documento que o `kira scan` grava (`{"findings": [...]}`).
    fn write_findings_fixture(&self) {
        let dir = self.cwd.join(".kira");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar .kira fixture: {err}"));
        std::fs::write(
            dir.join("findings.json"),
            r#"{"findings":[
                {"tool":"audit","rule":"vulnerable","package":"lodash","version":"4.17.19","severity":"critical","path":"src/lib.rs:42","message":"Prototype Pollution\nCVE-2020-8203","fingerprint":"0a1b2c3d4e5f6001"},
                {"tool":"audit","rule":"outdated","package":"minimist","version":"1.2.0","severity":"High","path":"package.json","message":"Deprecated dependency","fingerprint":"0a1b2c3d4e5f6002"},
                {"tool":"dotnet","rule":"outdated","package":"Npgsql","version":"8.0.4","severity":"High","path":"App/Db.csproj","message":"Pacote deprecado","fingerprint":"0a1b2c3d4e5f6003"}
            ]}"#,
        )
        .unwrap_or_else(|err| panic!("escrever findings.json fixture: {err}"));
    }

    /// Hash of the fixture store DB (the object of the read-only proof).
    fn store_hash(&self) -> String {
        let path = self.store.join(store::DB_FILE);
        let bytes = std::fs::read(&path).unwrap_or_else(|err| panic!("ler {path:?}: {err}"));
        // SHA-256 à mão é caro; um hash estável e rápido basta para a prova:
        // a comparação antes/depois captura QUALQUER mutação de byte.
        format!(
            "{:016x}{:016x}",
            fnv1a(&bytes),
            fnv1a(&bytes.iter().rev().cloned().collect::<Vec<u8>>())
        )
    }
}

/// FNV-1a 64 sobre bytes — a mesma família de hash que a store usa para ids;
/// suficiente para detectar qualquer alteração do arquivo (a força da prova é
/// o `assert_eq!` antes/depois, não a resistência a colisões).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Coleta a saída de um `Command` já preparado (não-TTY por construção: o
/// stdout do filho é um pipe).
fn output_of(mut command: Command) -> (i32, String, String) {
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("executar kira: {err}"));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Spawns the built `kira` inside the sandbox (non-TTY by construction: the
/// child's stdout is a pipe), returning (exit code, stdout, stderr).
fn run_kira(sandbox: &Sandbox, extra_env: &[(&str, &str)], args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(args)
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env("KIRA_CONFIG", sandbox.config_path())
        .env("AZURE_DEVOPS_PAT", FAKE_PAT)
        // Nada de rede: Ollama apontado para uma porta morta em TODA execução.
        .env("KIRA_OLLAMA_URL", "http://127.0.0.1:1")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("NO_COLOR");
    for (key, value) in extra_env {
        command.env(key, value);
    }
    output_of(command)
}

/// Igual ao [`run_kira`], mas SEM nenhum PAT: o caso real do teste de fumaça
/// — `AZURE_DEVOPS_PAT` ausente e keyring vazio ⇒ `pat: —` com causa.
fn run_kira_without_pat(sandbox: &Sandbox, args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(args)
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env("KIRA_CONFIG", sandbox.config_path())
        .env_remove("AZURE_DEVOPS_PAT")
        .env("KIRA_SECRET_STORE", "off")
        .env("KIRA_OLLAMA_URL", "http://127.0.0.1:1")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("NO_COLOR");
    output_of(command)
}

#[test]
fn snapshot_without_pat_and_store_shows_one_line_pat_and_the_doctor_hint() {
    // Defeito real de fumaça: a causa de DUAS linhas do `Error::MissingPat`
    // estourava o campo `pat:` do cabeçalho. Correção: no painel, UMA linha
    // com o ponteiro para o `?`; a causa integral (com a dica do `kira
    // doctor`) vai para a seção de pendências do texto/snapshot.
    let sandbox = Sandbox::new("no-pat-no-store");
    let (code, out, err) = run_kira_without_pat(&sandbox, &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}{err}");
    // `pat: ` aparece em EXATAMENTE uma linha (um campo, uma linha) e essa
    // linha é o ponteiro curto — nunca a causa de duas linhas.
    let pat_lines: Vec<&str> = out.lines().filter(|line| line.contains("pat: ")).collect();
    assert_eq!(pat_lines.len(), 1, "um campo, uma linha: {out}");
    assert_eq!(
        pat_lines[0], "pat: — (PAT não encontrado — veja ? para o passo a passo)",
        "ponteiro curto no cabeçalho: {out}"
    );
    // A causa INTEGRAL está no texto/snapshot, incluindo a dica do doctor.
    assert!(
        out.contains("defina a variável de ambiente AZURE_DEVOPS_PAT"),
        "causa integral no snapshot: {out}"
    );
    assert!(
        out.contains("para diagnosticar a configuração desta máquina, rode `kira doctor`"),
        "dica do doctor no snapshot: {out}"
    );
    // E o campo `store:` não existe mais no cabeçalho (duplicava `projeto: —`).
    assert!(!out.contains("store:"), "campo store removido: {out}");
}

#[test]
fn snapshot_prints_header_with_dev_and_masked_pat_and_exits_zero() {
    let sandbox = Sandbox::new("snapshot");
    sandbox.seed_store_with_scan(true);
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    // Nome do projeto (a pasta do sandbox) e o campo do dev do config.
    assert!(out.contains("projeto: proj"), "{out}");
    assert!(out.contains("dev: Sérgio"), "{out}");
    // PAT mascarado: prefixo + `****`, nunca o valor cru.
    assert!(out.contains("Aa2x****"), "{out}");
    assert!(
        !out.contains(FAKE_PAT),
        "o PAT completo NUNCA pode aparecer na saída: {out}"
    );
    // Store com sessão de scan: idade visível; `achados: —` com a causa
    // concreta (o fixture tem scan, mas não tem findings.json).
    assert!(out.contains("último scan: há"), "{out}");
    assert!(
        out.contains("achados: — (findings.json ausente — rode `kira scan` de novo)"),
        "causa honesta esperada: {out}"
    );
    // Rodapé de teclas presente no modo texto também.
    assert!(out.contains("teclas: q sair"), "{out}");
}

#[test]
fn dashboard_with_and_without_snapshot_never_touch_the_store() {
    let sandbox = Sandbox::new("readonly-hash");
    sandbox.seed_store_with_scan(true);
    let before = sandbox.store_hash();
    assert!(!before.is_empty(), "store fixture precisa existir");

    // Sem --snapshot: stdout é pipe (não-TTY) => modo texto, MESMA promessa.
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    let after_plain = sandbox.store_hash();
    assert_eq!(
        before, after_plain,
        "dashboard (não-TTY) mutou o store.db — violação do contrato read-only"
    );

    // Com --snapshot: idem, byte a byte.
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    let after_snapshot = sandbox.store_hash();
    assert_eq!(
        before, after_snapshot,
        "dashboard --snapshot mutou o store.db — violação do contrato read-only"
    );
}

#[test]
fn dashboard_works_with_ollama_on_a_dead_port() {
    // A porta morta já é o default de run_kira; aqui é a prova explícita do
    // contrato 3 da spec: nenhum probe de rede bloqueia ou falha o snapshot.
    let sandbox = Sandbox::new("dead-port");
    sandbox.seed_store_with_scan(true);
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "não pode haver dependência de rede: {out}{err}");
    assert!(out.contains("llm: ollama"), "{out}");
    // Sem hang e sem spinner: a saída é completa e termina com o rodapé.
    assert!(out.trim_end().ends_with("? ajuda"), "{out}");
}

#[test]
fn missing_scan_shows_the_dash_with_the_nunca_rodou_cause() {
    // Projeto existe, mas nenhuma sessão de `scan` na trilha: a causa
    // honesta é "nunca rodou kira scan" — nunca estimativa.
    let sandbox = Sandbox::new("no-scan");
    sandbox.seed_store_with_scan(false);
    let (code, out, _) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("achados: — (nunca rodou kira scan)"), "{out}");
    assert!(
        out.contains("último scan: — (nunca rodou kira scan)"),
        "{out}"
    );
}

#[test]
fn absent_store_is_an_actionable_message_with_exit_zero() {
    // Contrato 4 da spec: sem store.db, o dashboard é diagnóstico (exit 0) e
    // diz o que fazer — "rode kira scan ou kira doctor".
    let sandbox = Sandbox::new("no-store");
    let (code, out, _) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("nenhuma trilha em"), "{out}");
    assert!(out.contains("rode `kira scan` ou `kira doctor`"), "{out}");
    assert!(out.contains("projeto: — ("), "{out}");
}

#[test]
fn snapshot_with_findings_fixture_shows_counts_and_location_readonly() {
    // W6-PR2: painel de achados no `--snapshot` — contagem por severidade no
    // cabeçalho do painel, colunas e `path:linha` — E a prova de read-only se
    // mantém: o hash do `store.db` tem que ser idêntico antes/depois.
    let sandbox = Sandbox::new("findings");
    sandbox.seed_store_with_scan(true);
    sandbox.write_findings_fixture();
    let before = sandbox.store_hash();
    assert!(!before.is_empty(), "store fixture precisa existir");

    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "saída: {out}{err}");

    // Contagem por severidade (3 achados: critical 1, high 2) + ferramentas.
    assert!(
        out.contains("painel achados — critical 1 · high 2 · medium 0 · low 0"),
        "cabeçalho de contagem esperado: {out}"
    );
    assert!(out.contains("ferramentas: audit 2, dotnet 1"), "{out}");
    assert!(
        out.contains("achados: 3"),
        "contagem total no header: {out}"
    );
    // Colunas da tabela + path:linha (LOCAL) + resumo.
    assert!(out.contains("SEVERIDADE"), "{out}");
    assert!(out.contains("FERRAMENTA"), "{out}");
    assert!(out.contains("LOCAL"), "{out}");
    assert!(out.contains("RESUMO"), "{out}");
    assert!(out.contains("src/lib.rs:42"), "path:linha esperado: {out}");
    assert!(out.contains("Prototype Pollution"), "{out}");
    // Ordenação canônica no texto: critical antes de high.
    let critical = out.find("critical").unwrap_or(usize::MAX);
    let high = out.find("high").unwrap_or(usize::MAX);
    assert!(critical < high, "critical→low na saída: {out}");
    // O achado cru NUNCA apaga o contrato read-only: nada de escrita.
    let after = sandbox.store_hash();
    assert_eq!(
        before, after,
        "dashboard --snapshot com painel de achados mutou o store.db — violação do contrato read-only"
    );
}

#[test]
fn corrupted_findings_fail_open_to_dash_with_cause_and_exit_zero() {
    // findings.json corrompido: lista vazia + a causa real, exit 0 — o
    // dashboard é diagnóstico, nunca pânico nem dado inventado.
    let sandbox = Sandbox::new("corrupted");
    sandbox.seed_store_with_scan(true);
    let dir = sandbox.cwd.join(".kira");
    std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar .kira: {err}"));
    std::fs::write(dir.join("findings.json"), "{\"findings\": [quebrado")
        .unwrap_or_else(|err| panic!("escrever findings quebrado: {err}"));
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "fail-open tem que ser exit 0: {out}{err}");
    assert!(out.contains("achados: — ("), "{out}");
    assert!(out.contains("ilegível"), "causa real da leitura: {out}");
    assert!(!out.contains("Prototype Pollution"), "{out}");
}

#[test]
fn snapshot_with_three_sessions_shows_the_table_and_keeps_the_store_readonly() {
    // W6-PR3: painel de sessões no `--snapshot` com fixture de 3 sessões —
    // `#id`, comando, modelo, duração e 1ª linha do summary, MAIS RECENTE
    // PRIMEIRO — e a prova de read-only se mantém: o hash do `store.db` tem
    // que ser idêntico antes/depois.
    let sandbox = Sandbox::new("sessions");
    sandbox.seed_store_with_sessions();
    let before = sandbox.store_hash();
    assert!(!before.is_empty(), "store fixture precisa existir");

    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "saída: {out}{err}");

    // Cabeçalho do painel com a contagem real do fixture.
    assert!(
        out.contains("painel sessões — últimas 3"),
        "cabeçalho do painel de sessões esperado: {out}"
    );
    // Colunas da tabela.
    assert!(out.contains("#ID"), "{out}");
    assert!(out.contains("QUANDO"), "{out}");
    assert!(out.contains("COMANDO"), "{out}");
    assert!(out.contains("MODELO"), "{out}");
    assert!(out.contains("DURAÇÃO"), "{out}");
    assert!(out.contains("RESUMO"), "{out}");
    // As três sessões com id, comando, modelo e duração.
    assert!(out.contains("#3"), "{out}");
    assert!(out.contains("#2"), "{out}");
    assert!(out.contains("#1"), "{out}");
    assert!(out.contains("doctor"), "{out}");
    assert!(out.contains("triage"), "{out}");
    assert!(out.contains("qwen2.5:7b"), "modelo na tabela: {out}");
    assert!(out.contains("1m5s"), "duração humanizada: {out}");
    // 1ª linha do summary — e NUNCA a segunda linha inteira na tabela.
    assert!(out.contains("scan fixture: 3 achados"), "{out}");
    assert!(
        !out.contains("linha 2 interna"),
        "a tabela só mostra a 1ª linha do summary: {out}"
    );
    // Mais recente primeiro (id 3, doctor, antes do id 1, scan).
    let doctor = out.find("doctor").unwrap_or(usize::MAX);
    let scan = out.find("#1").unwrap_or(usize::MAX);
    assert!(doctor < scan, "mais recente primeiro: {out}");
    // O achado cru NUNCA apaga o contrato read-only: nada de escrita.
    let after = sandbox.store_hash();
    assert_eq!(
        before, after,
        "dashboard --snapshot com painel de sessões mutou o store.db — violação do contrato read-only"
    );
}

#[test]
fn sessions_absent_store_shows_the_honest_cause() {
    // Projeto existe, mas zero sessões: `—` com a causa concreta — nunca
    // lista inventada nem causa genérica.
    let sandbox = Sandbox::new("no-sessions");
    sandbox.seed_store_with_scan(false);
    let (code, out, _) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("sessões: — (store sem sessões)"), "{out}");
}

#[test]
fn snapshot_with_wiki_fixture_shows_rendered_pages_and_keeps_the_store_readonly() {
    // W6-PR4: painel de wiki no `--snapshot` com fixture de 2 páginas — a
    // lista (tipo · título · idade) + o markdown RENDIDO pelo renderizador
    // próprio (títulos com marcador por nível, listas com `•`, ênfase
    // desligada, link expandido, citação com `│`, régua — NUNCA `#` nem
    // `**` nem HTML executado) — e a prova de read-only se mantém: o hash
    // do `store.db` tem que ser idêntico antes/depois.
    let sandbox = Sandbox::new("wiki");
    sandbox.seed_store_with_scan(true);
    sandbox.seed_wiki_fixture();
    let before = sandbox.store_hash();
    assert!(!before.is_empty(), "store fixture precisa existir");

    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "saída: {out}{err}");

    // Cabeçalho do painel com a contagem real do fixture.
    assert!(out.contains("painel wiki — 2 páginas"), "{out}");
    assert!(out.contains("TIPO"), "{out}");
    assert!(out.contains("TÍTULO"), "{out}");
    assert!(out.contains("ATUALIZAÇÃO"), "{out}");
    // Os títulos das duas páginas na lista.
    assert!(out.contains("Contexto do projeto"), "{out}");
    assert!(out.contains("Sessão 4821"), "{out}");
    // O markdown RENDIDO (sem `#`, sem `**`, sem code span cru).
    assert!(out.contains("▌ Contexto do projeto"), "{out}");
    // As duas páginas começam com `#` (h1): marcador de nível 1 nas duas.
    assert!(out.contains("▌ Sessão 4821"), "{out}");
    assert!(out.contains("• item um"), "{out}");
    assert!(out.contains("• item dois"), "{out}");
    assert!(out.contains("Decisão com fofura"), "{out}");
    // O code span da página foi desligado dentro do conteúdo rendido (o
    // texto é "Decisão com fofura e kira scan." — sem backtick no trecho).
    assert!(out.contains("e kira scan."), "{out}");
    assert!(out.contains("o guia (https://exemplo.com/guia)"), "{out}");
    assert!(out.contains("│ uma citação"), "{out}");
    assert!(out.contains("────"), "{out}");
    // Nenhum marcador cru sobrevive ao renderizador: a ênfase `**com
    // fofura**` virou texto sem marcador e `#` em início de linha NUNCA
    // aparece como markdown (o `Aa2x****` do PAT é máscara, não markdown).
    assert!(!out.contains("**com fofura"), "ênfase desligada: {out}");
    assert!(!out.contains("\n# "), "nenhum título cru: {out}");
    // E a prova de read-only: nada escreve, byte a byte.
    let after = sandbox.store_hash();
    assert_eq!(
        before, after,
        "dashboard --snapshot com painel de wiki mutou o store.db — violação do contrato read-only"
    );
}

#[test]
fn wiki_absent_index_shows_the_reindex_cause() {
    // Store legível, índice `wiki_page` vazio: `—` com a causa que conserta
    // (rode kira wiki reindex) — nunca lista inventada.
    let sandbox = Sandbox::new("no-wiki");
    sandbox.seed_store_with_scan(true);
    let (code, out, _) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("wiki: — (wiki vazia — rode kira wiki reindex)"),
        "{out}"
    );
    assert!(!out.contains("painel wiki — "), "{out}");
}

#[test]
fn empty_wiki_section_still_exists_in_the_snapshot_with_the_cause() {
    // Bug 3 no modo texto: mesmo com a wiki VAZIA, o `--snapshot` inclui a
    // seção da wiki com a causa honesta — a prova de que o painel existe
    // mesmo vazio (e na ordem dos painéis: depois das sessões).
    let sandbox = Sandbox::new("empty-wiki-section");
    sandbox.seed_store_with_scan(false);
    let (code, out, _) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}");
    let wiki_pos = out
        .find("wiki: — (wiki vazia — rode kira wiki reindex)")
        .unwrap_or(usize::MAX);
    assert!(
        wiki_pos < usize::MAX,
        "a seção da wiki existe mesmo com o índice vazio: {out}"
    );
    let sessions_pos = out
        .find("sessões: — (store sem sessões)")
        .unwrap_or(usize::MAX);
    assert!(
        sessions_pos < wiki_pos,
        "a wiki aparece DEPOIS das sessões (ordem dos painéis do ciclo): {out}"
    );
    // E o rodapé segue com `q sair` como primeiro item das teclas.
    assert!(out.contains("teclas: q sair"), "{out}");
}

#[test]
fn wiki_readonly_hash_holds_without_snapshot_too() {
    // A prova de read-only também vale no modo não-TTY (texto) com wiki:
    // o dashboard NUNCA escreve — nem o índice que ele acabou de ler.
    let sandbox = Sandbox::new("wiki-plain");
    sandbox.seed_store_with_scan(true);
    sandbox.seed_wiki_fixture();
    let before = sandbox.store_hash();
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    assert!(out.contains("painel wiki — 2 páginas"), "{out}");
    let after = sandbox.store_hash();
    assert_eq!(
        before, after,
        "dashboard (não-TTY) mutou o store.db — violação do contrato read-only"
    );
}
