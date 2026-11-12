//! Prova de fumaça do MVP-5 (ADR-0030 local-first): o CICLO CRU do KIRA, de
//! ponta a ponta, com o BINÁRIO REAL — `setup --yes` → `scan` → `triage --file`
//! (backend falso) → `pr draft` → `dashboard --snapshot` — num tempdir
//! isolado, SEM rede e SEM credencial nenhuma.
//!
//! É um teste-documento: quem lê deve entender o ciclo do produto inteiro em
//! um arquivo. Cada etapa confere exit code E o ARTEFATO que ela grava
//! (config, findings.json, store.db, wiki, markdown do PR) — exit code sozinho
//! não prova nada.
//!
//! COMO SE DETECTA UMA TENTATIVA DE REDE (prova por ausência — não há servidor
//! neste teste):
//! 1. NENHUM listener é criado (nem em localhost): não existe servidor — se
//!    algum passo conectar, conecta no vazio e falha;
//! 2. o ambiente do filho perde TODA variável `KIRA_*`/`AZURE_*` antes de ser
//!    reconstruído: não há PAT/org/projeto para autenticar nada;
//! 3. o PATH do filho é SÓ o diretório dos shims: nenhum `ollama`, `curl` ou
//!    `wget` existe para o binário — sonda de rede por CLI morre na hora;
//! 4. `KIRA_OLLAMA_URL` aponta para a porta discard 127.0.0.1:9 (nada escuta):
//!    qualquer tentativa de backend de IA por HTTP falha em milissegundos e o
//!    passo sai com exit != 0, o que o teste acusa. O julgamento usa o shim
//!    LOCAL do Copilot (`--llm copilot` + `KIRA_COPILOT_BIN`), nunca rede.
//!
//! Ou seja: qualquer chamada externa derruba uma etapa com exit code != 0 (ou
//! com artefato ausente) e o teste falha apontando o passo.

// Só roda em Unix HOJE: os shims do backend e da ferramenta de audit são scripts
// shell (o cenário mais barato de montar). No Windows o spawn passa por
// `cmd /C` e o shim teria que ser `.cmd` — fazer isso às cegas, sem uma máquina
// Windows para validar, é exatamente como nasceram os últimos dois bugs de
// plataforma. Enquanto essa variante não existir, o ciclo fica coberto no
// Windows pelos e2e de `capabilities` e `dashboard`, que rodam o binário real.
//
// Lacuna registrada (não é teste quebrado): smoke cross-platform dos 5 passos.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Saída falsa de `npm audit` (o MESMO fixture dos testes do scan): o scan
/// nunca roda npm de verdade — o shim só imprime este JSON, determinístico em
/// qualquer máquina (não depende de npm/lockfile reais nem do PATH do host).
const NPM_AUDIT_FIXTURE: &str = include_str!("fixtures/scan/scan_npm_audit.json");

/// Work item do board (fixture dos outros e2e): id 4821, "Sessão expirada com
/// token válido". É o item que o `--file` leva ao triage e ao `pr draft`.
const ITEM_FIXTURE: &str = include_str!("fixtures/work_item_4821.json");

/// Sandbox do ciclo: projeto (repo de mentira com stack npm), store isolado,
/// diretório de shims (ferramenta de scan + backend de IA falsos) e o JSON de
/// audit em arquivo (o shim do Windows usa `@type`, que é builtin do cmd e não
/// depende de PATH). Tudo removido no Drop.
struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
    shims: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Sandbox {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-mvp5-e2e-{tag}-{}-{nanos}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("proj");
        let store = base.join("store");
        let shims = base.join("shims");
        std::fs::create_dir_all(cwd.join(".git"))
            .unwrap_or_else(|err| panic!("criar {cwd:?}/.git: {err}"));
        std::fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        std::fs::create_dir_all(&shims).unwrap_or_else(|err| panic!("criar {shims:?}: {err}"));

        // Stack npm de mentira: os dois markers que o scan procura. O npm de
        // verdade NUNCA roda (o shim vem pelo `KIRA_SCAN_NPM_BIN`).
        std::fs::write(cwd.join("package.json"), "{\"name\":\"ciclo-cru\"}\n")
            .unwrap_or_else(|err| panic!("escrever package.json: {err}"));
        std::fs::write(cwd.join("package-lock.json"), "{\"lockfileVersion\":3}\n")
            .unwrap_or_else(|err| panic!("escrever package-lock: {err}"));

        // O item do board como arquivo local: é o `--file` do ciclo offline
        // (nenhum board é consultado — nem existe credencial para isso).
        std::fs::write(cwd.join("item.json"), ITEM_FIXTURE)
            .unwrap_or_else(|err| panic!("escrever item.json: {err}"));

        let audit_json = shims.join("npm_audit.json");
        std::fs::write(&audit_json, NPM_AUDIT_FIXTURE)
            .unwrap_or_else(|err| panic!("escrever audit.json: {err}"));

        Sandbox {
            base,
            cwd,
            store,
            shims,
        }
    }

    /// Shim da ferramenta de scan: imprime o JSON de audit fixo. No Unix o
    /// JSON vai EMBUTIDO no script (`printf`) — o PATH do filho é só o dir dos
    /// shims, então o shim não pode depender de `cat`/`sed` externos.
    fn install_npm_shim(&self, audit_json: &Path) -> PathBuf {
        let flat = NPM_AUDIT_FIXTURE.lines().collect::<Vec<_>>().join(" ");
        let windows = format!("@type \"{}\"\r\n", audit_json.display());
        let unix = format!("#!/bin/sh\nprintf '%s' '{flat}'\n");
        self.write_shim("npm", &windows, &unix)
    }

    /// Shim do backend de IA ("Copilot CLI"): captura o argv (prova de que o
    /// PROMPT chegou ao binário — nada de rede) e responde com o formato das
    /// 3 seções do contrato do triage (`Causa raiz provável` / `Arquivos
    /// prováveis` / `Confiança`). Resposta fixa = determinismo.
    fn install_copilot_shim(&self, argv_capture: &Path) -> PathBuf {
        let cap = argv_capture.display().to_string();
        let windows = format!(
            "@echo %*>\"{cap}\"\r\n\
             @echo Causa raiz provável:\r\n\
             @echo TTL do token muito curto.\r\n\
             @echo Arquivos prováveis:\r\n\
             @echo src/auth/session.rs\r\n\
             @echo Confiança:\r\n\
             @echo média\r\n\
             @exit /b 0\r\n"
        );
        let unix = format!(
            "#!/bin/sh\n\
             printf '%s\\n' \"$@\" > '{cap}'\n\
             printf 'Causa raiz provável:\\nTTL do token muito curto.\\nArquivos prováveis:\\nsrc/auth/session.rs\\nConfiança:\\nmédia\\n'\n"
        );
        self.write_shim("copilot", &windows, &unix)
    }

    /// Grava o script em `<nome>.staging-<pid>` e RENAME para o nome final.
    ///
    /// Por que o desvio: o shim é EXECUTADO pelo KIRA; se ele for reescrito
    /// (outro teste rodando em paralelo no mesmo processo) enquanto um filho
    /// o mantém mapeado, o Linux falha a escrita com `ETXTBSY` ("text file
    /// busy"). Escrever num arquivo novo e renomear por cima é atômico e
    /// nunca toca no arquivo que está em execução.
    fn write_shim(&self, name: &str, windows_script: &str, unix_script: &str) -> PathBuf {
        let script = if cfg!(windows) {
            windows_script
        } else {
            unix_script
        };
        let final_path = self.shims.join(if cfg!(windows) {
            format!("{name}.cmd")
        } else {
            name.to_string()
        });
        let staging = self
            .shims
            .join(format!("{name}.staging-{}", std::process::id()));
        std::fs::write(&staging, script)
            .unwrap_or_else(|err| panic!("escrever shim em staging {staging:?}: {err}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|err| panic!("chmod shim {staging:?}: {err}"));
        }
        std::fs::rename(&staging, &final_path)
            .unwrap_or_else(|err| panic!("renomear shim {staging:?} -> {final_path:?}: {err}"));
        final_path
    }

    /// Roda o binário REAL do KIRA com o ambiente do ciclo cru:
    /// - remove SÓ as variáveis `KIRA_*`/`AZURE_*` do ambiente herdado. NUNCA
    ///   `env_clear()`: no Windows isso apaga `SystemRoot`/`PATHEXT` e o spawn
    ///   do processo falha por um motivo alheio ao contrato (o que precisa ser
    ///   provado é a ausência de credencial, não a ausência de sistema);
    /// - reconstrói o mínimo do produto apontando para o tempdir.
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command.args(args).current_dir(&self.cwd);
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy().to_ascii_uppercase();
            if name.starts_with("KIRA_") || name.starts_with("AZURE_") {
                command.env_remove(&key);
            }
        }
        command
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("config.toml"))
            // Sem keyring do sistema: o ciclo cru não grava nem lê segredo.
            .env("KIRA_SECRET_STORE", "off")
            // Ferramenta de scan e backend de IA por override ABSOLUTO: não
            // dependem do PATH do host (CI sem npm/Copilot instalados).
            .env("KIRA_SCAN_NPM_BIN", self.npm_shim())
            .env("KIRA_COPILOT_BIN", self.copilot_shim())
            // Porta discard: nada escuta; tentativa de Ollama falha em ms.
            .env("KIRA_OLLAMA_URL", "http://127.0.0.1:9")
            // PATH = só os shims: nenhum binário de rede nem o `ollama` existe
            // para o filho (ver a prova por ausência no cabeçalho do arquivo).
            .env(path_key(), &self.shims);
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn npm_shim(&self) -> PathBuf {
        self.shims
            .join(if cfg!(windows) { "npm.cmd" } else { "npm" })
    }

    fn copilot_shim(&self) -> PathBuf {
        self.shims.join(if cfg!(windows) {
            "copilot.cmd"
        } else {
            "copilot"
        })
    }

    /// Sessão mais recente de um comando na trilha automática (store.db).
    fn ultima_sessao(&self, command: &str) -> String {
        let connection = rusqlite::Connection::open(self.store.join("store.db"))
            .unwrap_or_else(|err| panic!("abrir store.db: {err}"));
        connection
            .query_row(
                "SELECT summary FROM sessions WHERE command = ?1 ORDER BY id DESC LIMIT 1",
                [command],
                |row| row.get(0),
            )
            .unwrap_or_else(|err| panic!("sessão '{command}' na trilha: {err}"))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Chave do PATH no Windows é `Path` (o `std` resolve case, mas o costume dos
/// outros e2e é o cfg explícito).
fn path_key() -> &'static str {
    if cfg!(windows) { "Path" } else { "PATH" }
}

/// O ciclo cru inteiro, na ordem que o dev roda após instalar o KIRA sem
/// nenhuma credencial: setup → scan → triage (--file, backend falso) →
/// pr draft → dashboard. Cada etapa valida exit code E artefato.
#[test]
fn ciclo_cru_offline_do_setup_ao_dashboard() {
    let sandbox = Sandbox::new("ciclo-cru");
    let argv_capture = sandbox.base.join("argv-capture.txt");
    let _npm = sandbox.install_npm_shim(&sandbox.shims.join("npm_audit.json"));
    let _copilot = sandbox.install_copilot_shim(&argv_capture);

    // ======================================================================
    // ETAPA 1 — `kira setup --yes`: não-interativo, sem PAT, sem Ollama.
    // Contrato do MVP-2: escolhe "nenhuma" e IMPRIME o pulado (nunca
    // bloqueia, nunca menciona PAT). Artefato: config.toml escrito no
    // tempdir — é a configuração que os próximos passos leem.
    // ======================================================================
    let (code, stdout, stderr) = sandbox.run(&["setup", "--yes"]);
    let saida = format!("{stdout}{stderr}");
    assert_eq!(code, 0, "setup --yes deve sair 0 — saída: {saida}");
    assert!(
        saida.contains("funciona agora:  scan · wiki · context · pr draft · dashboard"),
        "o bloco 'funciona agora' lista a jornada local inteira: {saida}"
    );
    assert!(
        saida.contains("desativado:      judge de item do board · read (board) · publicar PR"),
        "o que precisa de capacidade fica EXPLÍCITO como desativado: {saida}"
    );
    assert!(
        saida.contains("próximo passo:   kira scan"),
        "o resumo termina apontando o próximo passo real: {saida}"
    );
    assert!(
        saida.contains("ia:  nenhuma") && saida.contains("ollama não encontrado nesta máquina"),
        "sem Ollama a escolha é 'nenhuma' E o pulado é impresso com a causa: {saida}"
    );
    assert!(
        !saida.contains("PAT"),
        "o setup local-first NUNCA pede nem menciona PAT: {saida}"
    );
    let config = std::fs::read_to_string(sandbox.store.join("config.toml"))
        .unwrap_or_else(|err| panic!("ler config.toml escrito pelo setup: {err}"));
    assert!(
        config.contains("dev_name ="),
        "o setup grava a config local que os próximos passos leem: {config}"
    );

    // ======================================================================
    // ETAPA 2 — `kira scan`: a percepção. Zero rede, zero credencial; a
    // ferramenta de audit é o shim (`KIRA_SCAN_NPM_BIN`). Artefatos:
    // stdout determinístico, `.kira/findings.json` com os achados parseados
    // e a sessão na trilha automática do store.
    // ======================================================================
    let (code, stdout, stderr) = sandbox.run(&["scan"]);
    let saida = format!("{stdout}{stderr}");
    assert_eq!(code, 0, "scan deve sair 0 — saída: {saida}");
    assert!(
        stdout.contains("stacks: npm") && stdout.contains("achados: 2"),
        "scan detecta a stack e conta os achados: {stdout}"
    );
    assert!(
        stdout.contains("high: audit 1") && stdout.contains("moderate: audit 1"),
        "contagem por severidade com counts por ferramenta: {stdout}"
    );
    #[derive(serde::Deserialize)]
    struct Findings {
        #[serde(default)]
        findings: Vec<Finding>,
    }
    #[derive(serde::Deserialize)]
    struct Finding {
        fingerprint: String,
        package: String,
        severity: String,
        message: String,
    }
    let texto = std::fs::read_to_string(sandbox.cwd.join(".kira").join("findings.json"))
        .unwrap_or_else(|err| panic!("ler .kira/findings.json escrito pelo scan: {err}"));
    let documento: Findings = serde_json::from_str(&texto)
        .unwrap_or_else(|err| panic!("findings.json deve ser JSON de achados: {err} — {texto}"));
    assert_eq!(
        documento.findings.len(),
        2,
        "o fixture de audit tem 2 achados: {texto}"
    );
    assert!(
        documento.findings.iter().any(|finding| {
            finding.package == "minimist"
                && finding.severity == "high"
                && finding.message == "Prototype Pollution"
                && !finding.fingerprint.is_empty()
        }),
        "o achado minimist/high entra com fingerprint: {texto}"
    );
    assert!(
        documento
            .findings
            .iter()
            .any(|finding| finding.package == "lodash" && finding.severity == "moderate"),
        "o segundo achado (lodash/moderate) também: {texto}"
    );
    let sessao_scan = sandbox.ultima_sessao("scan");
    assert!(
        sessao_scan.starts_with("kira scan em '"),
        "a trilha automática registra a sessão de scan: {sessao_scan}"
    );

    // ======================================================================
    // ETAPA 3 — `kira triage --file item.json` com BACKEND FALSO: o julga-
    // mento offline (o coração do MVP). O `--llm copilot` + `KIRA_COPILOT_BIN`
    // apontam para o shim local — NENHUMA rede, NENHUM fallback (a porta do
    // Ollama está morta: um fallback falharia e o passo sairia != 0).
    // Artefatos: o relatório das 3 seções, o episódio de wiki do item e a
    // sessão de triage no store (que o `pr draft` consome depois).
    // ======================================================================
    let (code, stdout, stderr) =
        sandbox.run(&["triage", "--file", "item.json", "--llm", "copilot"]);
    let saida = format!("{stdout}{stderr}");
    assert_eq!(
        code, 0,
        "triage --file com shim deve sair 0 — saída: {saida}"
    );
    for secao in [
        "Causa raiz provável:",
        "TTL do token muito curto.",
        "Arquivos prováveis:",
        "src/auth/session.rs",
        "Confiança:",
        "média",
    ] {
        assert!(
            stdout.contains(secao),
            "a resposta das 3 seções do contrato no stdout: '{secao}' ausente — {stdout}"
        );
    }
    assert!(
        !stderr.contains("Ollama não está rodando"),
        "nenhum fallback ao Ollama (porta morta) — o shim local respondeu: {stderr}"
    );
    // O prompt REAL (título do item) chegou ao binário do backend — prova de
    // que o fluxo `--file` → prompt → CLI funciona sem rede.
    #[cfg(unix)]
    {
        let argv = std::fs::read_to_string(&argv_capture)
            .unwrap_or_else(|err| panic!("ler argv capturado pelo shim: {err}"));
        assert!(
            argv.starts_with("-p\n"),
            "o prompt vai ao CLI via -p: {argv}"
        );
        assert!(
            argv.trim_end().ends_with("\n-s"),
            "a flag silenciosa acompanha: {argv}"
        );
        assert!(
            argv.contains("Sessão expirada com token válido"),
            "o item do --file chegou ao prompt: {argv}"
        );
    }
    let wiki = std::fs::read_to_string(sandbox.cwd.join(".kira/wiki/bugs/4821.md"))
        .unwrap_or_else(|err| panic!("ler episódio de wiki escrito pelo triage: {err}"));
    assert!(
        wiki.contains("Causa raiz prevista: TTL do token muito curto.")
            && wiki.contains("Arquivos prováveis: src/auth/session.rs"),
        "o episódio do item 4821 registra o julgamento: {wiki}"
    );
    assert!(
        wiki.contains("Achados do scan: high 1"),
        "o episódio liga o julgamento aos achados do scan (etapa 2): {wiki}"
    );
    let sessao_triage = sandbox.ultima_sessao("triage");
    for secao in ["Causa raiz provável", "Arquivos prováveis", "Confiança"] {
        assert!(
            sessao_triage.contains(secao),
            "a sessão de triage grava o resumo das 3 seções: '{secao}' ausente — {sessao_triage}"
        );
    }

    // ======================================================================
    // ETAPA 4 — `kira pr draft 4821 --file item.json`: a escrita. Sem LLM,
    // sem rede: o markdown é montado SÓ com o que o KIRA já sabe, com `—` onde
    // não houver dado — NUNCA texto inventado.
    //
    // Até a issue #78 este passo saía OCO: o `scribe` procurava o triage
    // casando o primeiro token de `args` com o id, e o `triage --file` gravava
    // `--file` ali. Agora o item é DADO da sessão (`sessions.item_id`, migração
    // v5), então o rascunho recupera o julgamento da etapa 3 seja qual for a
    // forma de invocação — que é o modo padrão de quem não conectou board.
    // ======================================================================
    let (code, markdown, stderr) = sandbox.run(&["pr", "draft", "4821", "--file", "item.json"]);
    assert_eq!(code, 0, "pr draft deve sair 0 — stderr: {stderr}");
    assert!(
        markdown.contains("# Sessão expirada com token válido\n"),
        "o título vem do item do --file (dado real, não inventado): {markdown}"
    );
    assert!(
        markdown.contains("## Contexto\nItem #4821 do tipo Bug no estado Active."),
        "o contexto vem do item do --file: {markdown}"
    );
    for secao in [
        "## Causa raiz provável",
        "## Mudança proposta",
        "## Evidências",
        "## Riscos",
        "## Checklist",
    ] {
        assert!(
            markdown.contains(secao),
            "seção obrigatória do template: '{secao}' ausente — {markdown}"
        );
    }
    // O julgamento da ETAPA 3 chega aqui. Antes da #78 este bloco esperava `—`.
    assert!(
        markdown.contains("TTL do token muito curto."),
        "a causa do triage (etapa 3) é recuperada pelo item, não por argumento: {markdown}"
    );
    assert!(
        markdown.contains("src/auth/session.rs"),
        "os arquivos citados pelo triage entram no rascunho: {markdown}"
    );
    // O que continua honesto: não há evidência ligada a `src/auth/session.rs`
    // (os achados do scan são npm), então a seção diz que não há dado e manda
    // rodar os dois passos — em vez de inventar relação.
    assert!(
        markdown.contains(
            "## Evidências\n— (não há dado; rode kira triage 4821 antes ou rode kira scan)"
        ),
        "evidência ausente vira `—` com a causa, nunca invenção: {markdown}"
    );
    assert!(
        !markdown.contains("Prototype Pollution"),
        "achado de scan não relacionado não é usado como evidência: {markdown}"
    );
    assert!(
        markdown.contains("o merge é sempre humano"),
        "o checklist fixo (gates locais + merge humano) está no template: {markdown}"
    );
    // DETERMINISMO: mesma entrada, mesma saída, byte a byte.
    let (code_novamente, markdown_novamente, _) =
        sandbox.run(&["pr", "draft", "4821", "--file", "item.json"]);
    assert_eq!(code_novamente, 0);
    assert_eq!(
        markdown, markdown_novamente,
        "mesma entrada = mesmo markdown (rascunho determinístico)"
    );

    // ======================================================================
    // ETAPA 5 — `kira dashboard --snapshot`: a vigilância. Read-only total,
    // zero rede (a porta do Ollama está morta e o snapshot nem chama modelo).
    // Conteúdo: os painéis com os dados das etapas anteriores — achados do
    // scan e a trilha (scan + triage) — e o rodapé de teclas.
    // ======================================================================
    let (code, stdout, stderr) = sandbox.run(&["dashboard", "--snapshot"]);
    let saida = format!("{stdout}{stderr}");
    assert_eq!(code, 0, "dashboard --snapshot deve sair 0 — saída: {saida}");
    assert!(
        stdout.contains("kira dashboard (read-only — snapshot de texto)"),
        "o cabeçalho declara o modo read-only: {stdout}"
    );
    assert!(
        stdout.contains("projeto: proj"),
        "o projeto é o da raiz detectada: {stdout}"
    );
    assert!(
        stdout.contains("achados: 2")
            && stdout.contains("painel achados — critical 0 · high 1 · medium 0 · low 0")
            && stdout.contains("ferramentas: audit 2"),
        "o painel de achados mostra o resultado do scan (etapa 2): {stdout}"
    );
    assert!(
        stdout.contains("minimist 1.2.0") && stdout.contains("Prototype Pollution"),
        "o achado aparece com pacote/versão/resumo: {stdout}"
    );
    assert!(
        stdout.contains("painel sessões") && stdout.contains("scan") && stdout.contains("triage"),
        "o painel de sessões mostra a trilha das etapas: {stdout}"
    );
    assert!(
        stdout.contains("último scan: há"),
        "a idade do último scan vem da trilha (nunca de relógio inventado): {stdout}"
    );
    assert!(
        stdout.contains("teclas: q sair"),
        "o rodapé de teclas fecha o snapshot: {stdout}"
    );
}
