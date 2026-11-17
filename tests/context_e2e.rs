//! End-to-end for `kira context` (W2-bis p1, ADR-0021).
//!
//! Drives the real binary: the contract at the CLI boundary is that the
//! command prints the pack that would reach the LLM prompt — no model, no
//! network — with the retrieval source stamped (`index` or `grep`), each
//! excerpt anchored `path:linha`, and `--explain`/`--json` views. Everything
//! runs offline: the work item comes from a JSON file, the store lives in a
//! temp dir via `KIRA_STORE_DIR`, and the repository is a temp fixture.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    base: PathBuf,
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
            "kira-context-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        Self { base, cwd, store }
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.cwd.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| panic!("criar {parent:?}: {err}"));
        }
        fs::write(&path, body).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
            .env_remove("AZURE_DEVOPS_PAT")
            .env("KIRA_SECRET_STORE", "off")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_BOARD")
            .env_remove("KIRA_INDEX_MAX_FILES")
            .env_remove("KIRA_NUM_CTX")
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// Work item fixture whose title carries the rare term the test asserts on.
/// Keywords need ≥5 chars, so "cooldown" (8) works with the default extractor.
fn write_item(sandbox: &Sandbox) {
    sandbox.write(
        "item.json",
        r#"{
  "id": 4821,
  "rev": 1,
  "fields": {
    "System.Id": 4821,
    "System.WorkItemType": "Bug",
    "System.Title": "Cooldown do reprocesso expirou",
    "System.State": "Active",
    "System.Description": "O cooldown da fila nunca recomeça depois do erro."
  }
}"#,
    );
}

/// Same work item, saved with a NON-source extension: the file is parsed the
/// same way, but it never enters the repository index — so a repo with only
/// this file has an empty index and the pack must fall back to the grep.
fn write_item_without_source_extension(sandbox: &Sandbox) {
    sandbox.write(
        "work-item.txt",
        r#"{
  "id": 4821,
  "rev": 1,
  "fields": {
    "System.Id": 4821,
    "System.WorkItemType": "Bug",
    "System.Title": "Cooldown do reprocesso expirou",
    "System.State": "Active"
  }
}"#,
    );
}

#[test]
fn explain_shows_the_index_source_and_the_anchored_excerpts() {
    let sandbox = Sandbox::new("explain-index");
    write_item(&sandbox);
    sandbox.write("src/queue/fila.rs", "pub fn cooldown() -> u32 { 30 }\n");
    // Um segundo arquivo, sem o termo — o termo raro só existe em fila.rs.
    sandbox.write("README.md", "projeto de exemplo\n");

    let (code, stdout, stderr) = sandbox.run(&["context", "--file", "item.json", "--explain"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    // Fonte = index: o packer usou o FTS5 (o reindex roda antes do pack).
    assert!(
        stdout.contains("fonte do repositório: index"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("src/queue/fila.rs:1 — pub fn cooldown()"),
        "excerto ancorado: {stdout}"
    );
    assert!(
        stdout.contains("[fonte=index, rank="),
        "explain mostra o rank: {stdout}"
    );
    assert!(
        stdout.contains("prioridade=não"),
        "sem findings, sem prioridade: {stdout}"
    );
    assert!(
        stdout.contains("chars (~") && stdout.contains("tokens, 3 chars/token)"),
        "resumo com orçamento e tokens: {stdout}"
    );
}

#[test]
fn json_report_is_parseable_and_marks_the_source() {
    let sandbox = Sandbox::new("json");
    write_item(&sandbox);
    sandbox.write("src/queue/fila.rs", "pub fn cooldown() -> u32 { 30 }\n");
    let (code, stdout, stderr) = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("JSON inválido: {err}"));
    assert_eq!(
        parsed["pack"]["repo_source"], "index",
        "fonte no JSON: {parsed}"
    );
    assert_eq!(parsed["item_id"], 4821, "{parsed}");
    assert_eq!(parsed["chars_per_token"], 3, "{parsed}");
    let code_excerpts = parsed["pack"]["code"]
        .as_array()
        .unwrap_or_else(|| panic!("code deve ser lista: {parsed}"));
    assert!(
        code_excerpts
            .iter()
            .any(|excerpt| excerpt["path"] == "src/queue/fila.rs"),
        "{parsed}"
    );
}

#[test]
fn grep_is_the_fallback_for_a_repo_without_indexable_files() {
    let sandbox = Sandbox::new("fallback-grep");
    // Depois do ADR-0038 a decisão é **denylist**, então `.txt` passou a ser
    // indexável. Para o repositório ficar sem nenhum arquivo indexável, o work
    // item mora num diretório que a caminhada ignora (dot-dir) e o único conteúdo
    // do repo é um binário — o fallback para o grep continua sendo o contrato.
    write_item_without_source_extension(&sandbox);
    std::fs::create_dir_all(sandbox.cwd.join(".tmp")).expect("mkdir .tmp");
    std::fs::rename(
        sandbox.cwd.join("work-item.txt"),
        sandbox.cwd.join(".tmp/work-item.txt"),
    )
    .expect("mover work item");
    sandbox.write(".tmp/leia.bin", "não é fonte\n");
    let (code, stdout, stderr) = sandbox.run(&["context", "--file", ".tmp/work-item.txt"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("fonte do repositório: grep"),
        "fallback declarado: {stdout}"
    );
    assert!(
        stdout.contains("contexto: 0 trecho(s)"),
        "sem trechos, sem erro: {stdout}"
    );
}

#[test]
fn the_live_id_is_refused_with_an_actionable_message() {
    let sandbox = Sandbox::new("no-network");
    write_item(&sandbox);
    let (code, stdout, stderr) = sandbox.run(&["context", "4821"]);
    assert_ne!(code, 0, "o caminho vivo exige rede — não pode passar");
    assert!(stderr.contains("100% local"), "erro acionável: {stderr}");
    assert!(
        stderr.contains("--file"),
        "erro aponta o caminho offline: {stderr}"
    );
    assert!(stdout.is_empty(), "sem LLM, sem rede, sem saída: {stdout}");
}

#[test]
fn the_store_never_keeps_the_secret_from_the_indexed_lines() {
    let sandbox = Sandbox::new("no-secrets");
    write_item(&sandbox);
    sandbox.write(
        "src/config.rs",
        "let token = \"AbC123XyZ789QqQ\";\ncontato serjota@proton.me\n",
    );
    let (code, _stdout, stderr) = sandbox.run(&["context", "--file", "item.json", "--explain"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    // Abre o store real do sandbox e garante que nada secreto foi gravado.
    let db_path = sandbox.store.join("store.db");
    let conn = rusqlite::Connection::open(&db_path)
        .unwrap_or_else(|err| panic!("abrir {db_path:?}: {err}"));
    let dump: Vec<String> = {
        let mut statement = conn
            .prepare("SELECT text FROM repo_fts")
            .unwrap_or_else(|err| panic!("repo_fts: {err}"));
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap_or_else(|err| panic!("query: {err}"));
        rows.filter_map(|row| row.ok()).collect()
    };
    assert!(
        !dump.is_empty(),
        "o índice deveria ter linhas de src/config.rs"
    );
    for text in dump {
        assert!(!text.contains("AbC123XyZ789QqQ"), "PAT vazou: {text}");
        assert!(!text.contains("serjota@proton.me"), "e-mail vazou: {text}");
    }
}

#[test]
fn the_pack_is_deterministic_across_runs() {
    let sandbox = Sandbox::new("determinism");
    write_item(&sandbox);
    sandbox.write("src/queue/fila.rs", "pub fn cooldown() -> u32 { 30 }\n");
    let first = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(first.0, 0, "{}", first.2);
    let second = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(second.0, 0, "{}", second.2);
    // O JSON precisa ser byte-idêntico entre duas execuções no mesmo estado.
    assert_eq!(first.1, second.1, "mesmo estado ⇒ mesmo pack");
}

// PR39 — repo map determinístico no brief (docs/te-implementacao.md §9).
//
// Fixture do aceite (a): 30 arquivos no inventory do `kira ingest` — um
// citado em findings, um entrypoint, um config e 27 do topo da árvore. O mapa
// tem que ter EXATAMENTE os 20 esperados, na ordem do ranking, byte a byte,
// dentro do teto de 600 chars.

/// Inventory com 30 arquivos: 3 com papel + 27 do topo (p00..p26).
fn write_map_fixture(sandbox: &Sandbox) {
    sandbox.write(".kira/ingest/inventory.json", &map_inventory_json());
    sandbox.write(
        ".kira/findings.json",
        r#"{"findings":[{"tool":"dotnet","rule":"vulnerable","package":"Newtonsoft.Json","version":"12.0.1","severity":"High","path":"src/lib/cool.rs","message":"advisory","fingerprint":"0000000000000009"}],"stacks":[]}"#,
    );
    write_item(sandbox);
    // Os arquivos existem no repo também — o `context` indexa o repositório
    // e o pack traz trechos; o mapa é derivado só do inventory.
    for n in 0..27 {
        sandbox.write(&format!("p{n:02}/file.rs"), "pub fn nada() {}\n");
    }
    sandbox.write("src/lib/cool.rs", "pub fn cool() {}\n");
    sandbox.write("src/main.rs", "fn main() {}\n");
    sandbox.write("Cargo.toml", "[package]\nname = \"x\"\n");
}

fn map_inventory_json() -> String {
    let mut paths: Vec<String> = vec![
        "Cargo.toml".to_string(),
        "src/main.rs".to_string(),
        "src/lib/cool.rs".to_string(),
    ];
    paths.extend((0..27).map(|n| format!("p{n:02}/file.rs")));
    let files: Vec<String> = paths
        .iter()
        .map(|path| format!("  {{\n    \"path\": \"{path}\",\n    \"bytes\": 10,\n    \"truncated\": false\n  }}"))
        .collect();
    format!(
        "{{\n  \"project_name\": \"x\",\n  \"files\": [\n{}\n  ],\n  \"by_extension\": {{\"rs\": 29, \"toml\": 1}},\n  \"total_bytes\": 300,\n  \"notes\": []\n}}",
        files.join(",\n")
    )
}

/// O mapa esperado do fixture: citado → entrada → config → topo (rasa
/// primeiro), 20 linhas, marcador com os 10 de fora, ≤600 chars.
fn expected_map_block() -> String {
    let mut block = String::from("MAPA do repo (ingest, 30 arquivos):\n");
    block.push_str("- src/lib/cool.rs (citado)\n");
    block.push_str("- src/main.rs (entrada)\n");
    block.push_str("- Cargo.toml (config)\n");
    for n in 0..17 {
        block.push_str(&format!("- p{n:02}/file.rs (topo)\n"));
    }
    block.push_str("... (+10 arquivos)\n");
    block
}

#[test]
fn the_repo_map_has_exactly_the_expected_files_and_honors_the_caps() {
    let sandbox = Sandbox::new("repo-map");
    write_map_fixture(&sandbox);
    let first = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(first.0, 0, "{}", first.2);
    let parsed: serde_json::Value =
        serde_json::from_str(&first.1).unwrap_or_else(|err| panic!("JSON inválido: {err}"));
    let map = parsed["pack"]["repo_map"]
        .as_str()
        .unwrap_or_else(|| panic!("repo_map ausente: {parsed}"));
    // (a) byte a byte: os 20 esperados, na ordem do ranking.
    assert_eq!(map, expected_map_block(), "mapa: {map}");
    assert!(
        map.chars().count() <= 600,
        "teto de 600 chars: {} chars",
        map.chars().count()
    );
    // (d) o teto do orçamento é honrado: o mapa é cobrado no used_chars.
    let used = parsed["pack"]["used_chars"].as_u64().unwrap_or(0);
    assert!(used >= map.chars().count() as u64, "used_chars: {used}");
    // Mesmo estado ⇒ mesmo mapa byte a byte (extensão do gate M1).
    let second = sandbox.run(&["context", "--file", "item.json", "--json"]);
    let parsed2: serde_json::Value =
        serde_json::from_str(&second.1).unwrap_or_else(|err| panic!("JSON inválido: {err}"));
    assert_eq!(
        parsed2["pack"]["repo_map"], parsed["pack"]["repo_map"],
        "mesmo estado ⇒ mesmo mapa"
    );
}

#[test]
fn without_findings_or_entrypoint_the_map_says_top_of_tree() {
    let sandbox = Sandbox::new("repo-map-topo");
    write_item(&sandbox);
    // Inventory sem citado e sem entrypoint — só arquivos rasos/profundos.
    sandbox.write(
        ".kira/ingest/inventory.json",
        r#"{
  "project_name": "x",
  "files": [
    {"path": "readme.md", "bytes": 10, "truncated": false},
    {"path": "lib/other.rs", "bytes": 10, "truncated": false},
    {"path": "src/deep/leaf.rs", "bytes": 10, "truncated": false}
  ],
  "by_extension": {"md": 1, "rs": 2},
  "total_bytes": 30,
  "notes": []
}"#,
    );
    sandbox.write("readme.md", "projeto\n");
    sandbox.write("lib/other.rs", "pub fn other() {}\n");
    sandbox.write("src/deep/leaf.rs", "pub fn leaf() {}\n");
    let (code, stdout, stderr) = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("JSON inválido: {err}"));
    let map = parsed["pack"]["repo_map"]
        .as_str()
        .unwrap_or_else(|| panic!("repo_map ausente: {parsed}"));
    // (c) não é vazio silencioso: nota + mapa do topo da árvore (rasa
    // primeiro), todos com papel `topo`.
    let notes: Vec<String> = parsed["pack"]["notes"]
        .as_array()
        .unwrap_or_else(|| panic!("notes: {parsed}"))
        .iter()
        .filter_map(|note| note.as_str().map(String::from))
        .collect();
    assert!(
        notes
            .iter()
            .any(|note| note.contains("sem findings e sem entrypoint")),
        "nota (c): {notes:?}"
    );
    assert_eq!(
        map,
        "MAPA do repo (ingest, 3 arquivos):\n- readme.md (topo)\n- lib/other.rs (topo)\n- src/deep/leaf.rs (topo)\n",
        "mapa do topo: {map}"
    );
}

#[test]
fn a_missing_inventory_degrades_to_an_actionable_note() {
    let sandbox = Sandbox::new("repo-map-missing");
    write_item(&sandbox);
    sandbox.write("src/queue/fila.rs", "pub fn cooldown() -> u32 { 30 }\n");
    let (code, stdout, stderr) = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("JSON inválido: {err}"));
    assert!(
        parsed["pack"]["repo_map"].is_null(),
        "sem inventory, sem mapa: {parsed}"
    );
    let notes: Vec<String> = parsed["pack"]["notes"]
        .as_array()
        .unwrap_or_else(|| panic!("notes: {parsed}"))
        .iter()
        .filter_map(|note| note.as_str().map(String::from))
        .collect();
    assert!(
        notes.iter().any(|note| note.contains("rode `kira ingest`")),
        "nota acionável: {notes:?}"
    );
}
// ===========================================================================
// PR40 — token_profile em config.toml (docs/te-implementacao.md §10, D10).
// ===========================================================================

/// Estado determinístico do fixture: work item + 4 findings (severidades
/// distintas) + 13 arquivos com o termo + 5 páginas de wiki (conteúdo longo,
/// para o cap de 400/600/800 chars aparecer). Skills bundled não casam com os
/// termos (checado) e a sandbox não tem knowledge — o pack é só findings +
/// mapa ausente (nota) + wiki + repositório.
fn write_profile_fixture(sandbox: &Sandbox) {
    write_item(sandbox);
    sandbox.write(
        ".kira/findings.json",
        r#"{"findings":[
{"tool":"dotnet","rule":"vulnerable","package":"Pkg.One","version":"1.0.0","severity":"critical","path":"src/mod_00.rs","message":"advisory","fingerprint":"0000000000000001"},
{"tool":"dotnet","rule":"vulnerable","package":"Pkg.Two","version":"2.0.0","severity":"high","path":"src/mod_01.rs","message":"advisory","fingerprint":"0000000000000002"},
{"tool":"dotnet","rule":"vulnerable","package":"Pkg.Three","version":"3.0.0","severity":"moderate","path":"src/mod_02.rs","message":"advisory","fingerprint":"0000000000000003"},
{"tool":"audit","rule":"vulnerable","package":"Pkg.Four","version":"4.0.0","severity":"low","path":"src/mod_03.rs","message":"advisory","fingerprint":"0000000000000004"}],"stacks":[]}"#,
    );
    for n in 0..13 {
        sandbox.write(
            &format!("src/mod_{n:02}.rs"),
            &format!("pub fn cooldown_{n}() -> u32 {{ {n} }}\n"),
        );
    }
    // Wiki: 5 páginas com conteúdo longo (o cap do excerpt é o que muda entre
    // perfis). Grava pelo Wiki::write REAL (header `updated_at` = agora ⇒
    // idade 0 na corrida seguinte — determinística) e reindexa o FTS5 na
    // store do sandbox, porque o comando `context` não reindexa wiki.
    let store = kira::store::Store::open_in_dir(&sandbox.store)
        .unwrap_or_else(|err| panic!("store: {err}"));
    let wiki = kira::wiki::Wiki::open(&sandbox.cwd).unwrap_or_else(|err| panic!("wiki: {err}"));
    for n in 0..5 {
        let mut content = String::from("# Cooldown do reprocesso\n");
        content
            .push_str("A fila do reprocesso nunca recomeça depois do erro; o cooldown expirou. ");
        content.push_str("O reprocesso de cada item da fila depende do cooldown anterior; ");
        content.push_str("sem o recomeço do cooldown o reprocesso para e a fila cresce. ");
        content.push_str("Fila cresce até o cooldown voltar; o reprocesso recomeça tarde. ");
        content.push_str("Cooldown longo atrasa o reprocesso e a fila inteira do dia. ");
        content.push_str("Reprocesso em lote quebra quando o cooldown da fila não recomeça. ");
        content.push_str("O cooldown expira; o reprocesso recomeça só no dia seguinte. ");
        content.push_str("Fila de reprocesso empacada: cooldown sem recomeço é o defeito. ");
        wiki.write(&format!("bugs/p{n}"), &content)
            .unwrap_or_else(|err| panic!("wiki {n}: {err}"));
    }
    wiki.reindex(&store)
        .unwrap_or_else(|err| panic!("reindex wiki: {err}"));
}

/// Os contados do pack por perfil (D10): 4/8/12 trechos, 2/3/4 páginas,
/// 2/3/4 achados detalhados — três packs DISTINTOS e reprodutíveis (M1 ×3).
#[test]
fn the_three_profiles_produce_three_distinct_deterministic_packs() {
    let sandbox = Sandbox::new("profiles-caps");
    write_profile_fixture(&sandbox);
    let mut per_profile: std::collections::HashMap<&str, (usize, usize, usize)> =
        std::collections::HashMap::new();
    for profile in ["budget", "balanced", "quality"] {
        let first = sandbox.run(&[
            "context",
            "--file",
            "item.json",
            "--json",
            "--profile",
            profile,
        ]);
        assert_eq!(first.0, 0, "{}: {}", profile, first.2);
        let second = sandbox.run(&[
            "context",
            "--file",
            "item.json",
            "--json",
            "--profile",
            profile,
        ]);
        assert_eq!(second.0, 0, "{}: {}", profile, second.2);
        // Mesmo estado ⇒ mesmo pack byte a byte (gate M1, por perfil).
        assert_eq!(first.1, second.1, "determinismo do perfil {profile}");
        let parsed: serde_json::Value = serde_json::from_str(&first.1)
            .unwrap_or_else(|err| panic!("JSON inválido ({profile}): {err}"));
        assert_eq!(
            parsed["token_profile"], profile,
            "o JSON nomeia o perfil: {parsed}"
        );
        // Os caps aparecem como DESIGUALDADE monotônica aqui — o pool do
        // índice é cortado antes do filtro do `--file` (quirk pré-existente
        // do packer, fora do escopo do PR40), então o cap exato é provado no
        // teste unitário `the_profile_caps_govern_the_pack_per_profile`.
        let code_len = parsed["pack"]["code"]
            .as_array()
            .unwrap_or_else(|| panic!("code: {parsed}"))
            .len();
        let pages_len = parsed["pack"]["sections"]
            .as_array()
            .unwrap_or_else(|| panic!("sections: {parsed}"))
            .len();
        let findings_len = parsed["pack"]["findings"]
            .as_array()
            .unwrap_or_else(|| panic!("findings: {parsed}"))
            .len();
        per_profile.insert(profile, (code_len, pages_len, findings_len));
    }
    // Os três packs são DIFERENTES entre si (perfis distintos ⇒ packs
    // distintos): mesmo estado, três saídas diferentes.
    let mut outputs = Vec::new();
    for profile in ["budget", "balanced", "quality"] {
        let run = sandbox.run(&[
            "context",
            "--file",
            "item.json",
            "--json",
            "--profile",
            profile,
        ]);
        outputs.push(run.1);
    }
    assert_ne!(outputs[0], outputs[1], "budget != balanced");
    assert_ne!(outputs[1], outputs[2], "balanced != quality");
    assert_ne!(outputs[0], outputs[2], "budget != quality");
    // D10 na prática: quality ≥ balanced ≥ budget em TODAS as três contagens.
    let (b, m, q) = (
        per_profile["budget"],
        per_profile["balanced"],
        per_profile["quality"],
    );
    assert!(
        q.0 >= m.0 && m.0 >= b.0 && q.1 >= m.1 && m.1 >= b.1 && q.2 >= m.2 && m.2 >= b.2,
        "caps monotônicos violados: budget={b:?} balanced={m:?} quality={q:?}"
    );
}

/// Aceite (d): sem chave e com `token_profile = "balanced"` explícito ⇒
/// packs BYTE-IDÊNTICOS — ninguém muda de comportamento sem querer.
#[test]
fn the_default_matches_an_explicit_balanced_config_byte_for_byte() {
    let sandbox = Sandbox::new("default-balanced");
    write_profile_fixture(&sandbox);
    let absent = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(absent.0, 0, "{}", absent.2);
    let config = sandbox.store.join("balanced.toml");
    fs::write(&config, "token_profile = \"balanced\"\n")
        .unwrap_or_else(|err| panic!("escrever config: {err}"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(["context", "--file", "item.json", "--json"])
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env("KIRA_CONFIG", &config)
        .env("KIRA_SECRET_STORE", "off")
        .env_remove("KIRA_NUM_CTX");
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("executar: {err}"));
    assert_eq!(output.status.code().unwrap_or(-1), 0);
    let explicit = String::from_utf8_lossy(&output.stdout).into_owned();
    assert_eq!(
        absent.1, explicit,
        "default == balanced explícito, byte a byte"
    );
}

/// Aceite (c): perfil inválido ⇒ erro acionável com a lista dos válidos,
/// vindo do flag OU do config.toml — nunca fallback silencioso.
#[test]
fn an_invalid_profile_fails_with_the_actionable_list() {
    let sandbox = Sandbox::new("profile-invalid");
    write_profile_fixture(&sandbox);
    let (code, stdout, stderr) = sandbox.run(&[
        "context",
        "--file",
        "item.json",
        "--json",
        "--profile",
        "esquisito",
    ]);
    assert_ne!(code, 0, "perfil inválido não pode passar: {stdout}");
    assert!(stdout.is_empty(), "sem pack inválido: {stdout}");
    assert!(stderr.contains("esquisito"), "erro acionável: {stderr}");
    assert!(
        stderr.contains("budget, balanced, quality"),
        "lista dos válidos: {stderr}"
    );
    // Via config.toml: mesmo contrato, sem flag.
    let config = sandbox.store.join("bad.toml");
    fs::write(&config, "token_profile = \"esquisito\"\n")
        .unwrap_or_else(|err| panic!("escrever config: {err}"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(["context", "--file", "item.json", "--json"])
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env("KIRA_CONFIG", &config)
        .env("KIRA_SECRET_STORE", "off")
        .env_remove("KIRA_NUM_CTX");
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("executar: {err}"));
    assert_ne!(
        output.status.code().unwrap_or(-1),
        0,
        "config inválido não passa"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("esquisito") && stderr.contains("budget, balanced, quality"),
        "erro via config: {stderr}"
    );
}

/// Raiz temporária do sandbox vira `<ROOT>` e carimbo de escrita da wiki
/// (`updated_at=...`) vira `<STAMP>` — os dois são dados de MÁQUINA/relógio,
/// não de pack. Os contadores `tokens_estimate`/`used_chars` também: são
/// calculados sobre os caminhos ABSOLUTOS crus do pack, e o root do sandbox
/// tem comprimento diferente por runner (VPS ≠ CI). Conteúdo continua byte a
/// byte; a monotonicidade dos caps é coberta pelo teste dos três estados.
fn normalize_golden(output: &str, roots: &[String]) -> String {
    // Uma raiz por forma em que o store pode imprimir o caminho do projeto:
    // a direta (como o sandbox criou) e a canônica (`/private/var/...` no
    // macOS). No Windows/Linux as duas coincidem e o loop é no-op.
    let mut out = output.to_string();
    for root in roots {
        out = out.replace(root, "<ROOT>");
    }
    // cursor avança para não re-achar o `updated_at=<STAMP>` que acabou de
    // entrar: antes, o loop re-findava a própria substituição e o `break`
    // deixava a 2ª ocorrência crua (bugs/p1 divergia do dourado).
    let mut cursor = 0;
    while let Some(rel) = out[cursor..].find("updated_at=") {
        let start = cursor + rel;
        let end = start + "updated_at=".len();
        let rest = &out[end..];
        let stamp_len = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == ':' || c == 'T' || c == 'Z'))
            .unwrap_or(rest.len());
        if stamp_len >= 19 {
            out.replace_range(start..end + stamp_len, "updated_at=<STAMP>");
            cursor = start + "updated_at=<STAMP>".len();
        } else {
            cursor = end;
        }
    }
    for field in ["tokens_estimate", "used_chars"] {
        let needle = format!("\"{field}\": ");
        let mut cursor = 0;
        while let Some(rel) = out[cursor..].find(&needle) {
            let start = cursor + rel + needle.len();
            let digits = out[start..]
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(out[start..].len());
            if digits > 0 {
                out.replace_range(start..start + digits, "<N>");
            }
            cursor = start + "<N>".len();
        }
    }
    out
}

/// Os 3 dourados (aceite a): um por perfil, byte a byte. O JSON carrega
/// caminhos ABSOLUTOS das páginas de wiki (o pack cita `caminho` do disco),
/// então o dourado é comparado depois de trocar a raiz temporária do sandbox
/// por `<ROOT>` — a comparação continua byte a byte no resto, e em DUAS
/// execuções no mesmo sandbox é byte a byte sem normalização (M1 ×3).
/// `KIRA_UPDATE_CONTEXT_GOLDENS=1` regenera os arquivos (padrão PR6).
#[test]
fn the_three_golden_packs_are_reproducible_per_profile() {
    let sandbox = Sandbox::new("profiles-golden");
    write_profile_fixture(&sandbox);
    let manifest = env!("CARGO_MANIFEST_DIR");
    for profile in ["budget", "balanced", "quality"] {
        let first = sandbox.run(&[
            "context",
            "--file",
            "item.json",
            "--json",
            "--profile",
            profile,
        ]);
        assert_eq!(first.0, 0, "{}: {}", profile, first.2);
        let second = sandbox.run(&[
            "context",
            "--file",
            "item.json",
            "--json",
            "--profile",
            profile,
        ]);
        assert_eq!(first.1, second.1, "mesmo estado ⇒ mesmo pack ({profile})");

        let root = sandbox.cwd.to_string_lossy().replace('\\', "/");
        // O store CANONICALIZA a raiz do projeto — no macOS o temp chega por
        // atalho (`/var/folders/...`) e o caminho real é `/private/var/...`:
        // o pack imprime o canônico, então trocar só o `root` deixava um
        // `/private` sobrando e o dourado divergia SÓ no macOS (o e2e-desktop
        // não rodava em PR, então nenhum PR teria pego). Substituir as duas
        // formas devolve a comparação byte a byte nos 3 SOs.
        let canonical = std::fs::canonicalize(&sandbox.cwd)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| root.clone());
        // O excerpt da wiki começa no header da página, e o header carrega o
        // carimbo da ESCRITA (`updated_at=<agora>`) — dado de relógio, não de
        // pack. Normaliza as raízes e o carimbo; o resto é byte a byte.
        let roots = if canonical == root {
            vec![root.clone()]
        } else {
            vec![root, canonical]
        };
        let normalized = normalize_golden(&first.1, &roots);
        let golden_path = std::path::Path::new(manifest)
            .join("tests/fixtures/context/profile")
            .join(format!("{profile}.json"));
        if std::env::var("KIRA_UPDATE_CONTEXT_GOLDENS").is_ok() {
            fs::create_dir_all(
                golden_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
            )
            .unwrap_or_else(|err| panic!("criar fixtures: {err}"));
            fs::write(&golden_path, &normalized)
                .unwrap_or_else(|err| panic!("escrever {}: {err}", golden_path.display()));
            continue;
        }
        let golden = fs::read_to_string(&golden_path).unwrap_or_else(|err| {
            panic!(
                "ler dourado {}: {err} (gere com KIRA_UPDATE_CONTEXT_GOLDENS=1)",
                golden_path.display()
            )
        });
        assert_eq!(
            normalized, golden,
            "dourado do perfil {profile} divergiu — mudança de comportamento não autorizada"
        );
    }
}
