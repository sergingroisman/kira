//! PR39 — repo map determinístico no brief (≤20 arquivos / ≤600 chars).
//!
//! O mapa é derivado do inventory do `kira ingest` (`.kira/ingest/inventory.json`):
//! só caminhos, zero I/O de conteúdo (o inventory já leu tudo) e zero LLM — a
//! escolha é função pura de (caminhos, findings, termos), então mesma entrada
//! ⇒ mesmo mapa byte a byte (gate M1; ADR-0016: determinismo antes de
//! inteligência).
//!
//! Ranking (te-implementação §9, regra vigente dos excerpts estendida pelo
//! inventory): `citado` (path citado em `findings.json`) > `termo` (termo do
//! work item no caminho) > `entrada` (entrypoint) > `config` > `topo`, com
//! desempate por profundidade (rasa primeiro) e depois caminho — ordenação
//! total e estável: embaralhar a ordem de entrada do inventory não muda o mapa.
//!
//! Todo o falhar é fail-open: inventory ausente/corrompido vira nota no pack,
//! nunca erro do comando.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use crate::ingest::inventory::{Inventory, InventoryFile};

/// Teto de arquivos no mapa (D8, proposto no plano TE).
pub const MAX_MAP_FILES: usize = 20;

/// Teto de caracteres do bloco inteiro (mesma régua de `MAX_EXCERPT_CHARS`).
pub const MAX_MAP_CHARS: usize = 600;

/// Papel do arquivo no mapa — uma palavra, no prompt e no desempate do ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    /// Path citado em `findings.json` (regra vigente: findings primeiro).
    Citado,
    /// Um termo do work item aparece no caminho.
    Termo,
    /// Entry point do projeto (`main.rs`, `index.ts`, `Program.cs`, ...).
    Entrada,
    /// Config/manifest da stack (`Cargo.toml`, `package.json`, ...).
    Config,
    /// Sem sinal — entra pela profundidade da árvore (rasa primeiro).
    Topo,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Citado => "citado",
            Role::Termo => "termo",
            Role::Entrada => "entrada",
            Role::Config => "config",
            Role::Topo => "topo",
        }
    }

    fn score(self) -> u8 {
        match self {
            Role::Citado => 4,
            Role::Termo => 3,
            Role::Entrada => 2,
            Role::Config => 1,
            Role::Topo => 0,
        }
    }
}

/// Entry points reconhecidos pelo basename (lowercase). Lista fechada de
/// propósito: o mapa tem que ser estável entre execuções e não vale a pena
/// ler conteúdo para descobrir entry point (o inventory é a fonte).
const ENTRYPOINT_BASENAMES: [&str; 15] = [
    "main.rs",
    "lib.rs",
    "main.py",
    "app.py",
    "main.go",
    "main.ts",
    "main.js",
    "main.tsx",
    "index.ts",
    "index.js",
    "index.tsx",
    "index.jsx",
    "program.cs",
    "startup.cs",
    "app.cs",
];

/// Configs/manifests reconhecidos pelo basename (lowercase).
const CONFIG_BASENAMES: [&str; 11] = [
    "cargo.toml",
    "package.json",
    "pnpm-workspace.yaml",
    "pyproject.toml",
    "go.mod",
    "tsconfig.json",
    "appsettings.json",
    "requirements.txt",
    "dockerfile",
    "docker-compose.yml",
    "makefile",
];

/// Sufixos de config do ecossistema JS (`vite.config.ts`, `next.config.js`, ...).
const CONFIG_SUFFIXES: [&str; 3] = [".config.ts", ".config.js", ".config.mjs"];

fn role_of(path: &str, priority: &BTreeSet<String>, terms: &[String]) -> Role {
    let lowered = path.to_lowercase();
    if priority.contains(path) {
        // Regra vigente (te-implementação §9): arquivo citado em findings
        // outrank tudo — é a evidência mais específica do pack.
        return Role::Citado;
    }
    // Termos já saem lowercased de `keywords`; o casamento é no CAMINHO (o
    // mapa não lê conteúdo — é derivado do inventory, que só tem paths).
    if terms.iter().any(|term| lowered.contains(term.as_str())) {
        return Role::Termo;
    }
    let basename = lowered.rsplit('/').next().unwrap_or("");
    if ENTRYPOINT_BASENAMES.contains(&basename) {
        return Role::Entrada;
    }
    if CONFIG_BASENAMES.contains(&basename)
        || CONFIG_SUFFIXES
            .iter()
            .any(|suffix| basename.ends_with(suffix))
    {
        return Role::Config;
    }
    Role::Topo
}

/// Um arquivo já ranqueado: profundidade é o nº de componentes do caminho
/// (rasa primeiro — o topo da árvore é o que mais parece "mapa" para o modelo).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ranked {
    path: String,
    role: Role,
    depth: usize,
}

/// Ranking TOTAL e estável: (score desc, profundidade asc, caminho asc). O
/// desempate final pelo caminho é o que garante o aceite "shuffle-in, mapa
/// byte-idêntico-out" — nenhuma informação de ordem de entrada sobrevive.
fn ranked(
    files: &[InventoryFile],
    priority: &BTreeSet<String>,
    terms: &[String],
    skip: &HashSet<String>,
) -> Vec<Ranked> {
    let mut ranked: Vec<Ranked> = files
        .iter()
        .filter(|file| !skip.contains(&file.path))
        .map(|file| Ranked {
            path: file.path.clone(),
            role: role_of(&file.path, priority, terms),
            depth: file.path.split('/').count(),
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.role
            .score()
            .cmp(&a.role.score())
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| a.path.cmp(&b.path))
    });
    ranked
}

/// Resultado da montagem do mapa — cada falha tem a nota certa no caller.
#[derive(Debug, PartialEq, Eq)]
pub enum MapOutcome {
    /// Mapa pronto (bloco já truncado dentro dos tetos).
    Ready(RepoMap),
    /// `.kira/ingest/inventory.json` não existe — nota manda rodar `kira ingest`.
    NoInventory,
    /// Existe, mas não parseia — nota manda rodar `kira ingest` de novo.
    BadInventory,
    /// Repo vazio: bloco omitido, pack byte-idêntico ao de hoje (aceite §9),
    /// sem nota — silêncio aqui é o comportamento pedido.
    EmptyRepo,
}

/// O mapa montado: bloco pronto para o prompt + a flag da nota (c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoMap {
    /// Bloco completo (`MAPA ...` + linhas + marcador), com `\n` de sobra.
    pub block: String,
    /// Total de arquivos no inventory (o número do header).
    pub total_files: usize,
    /// Arquivos que ficaram de fora do teto de 20 (o `(+N arquivos)`).
    pub omitted: usize,
    /// `true` quando NEM findings citados NEM entry point existem no repo
    /// inteiro — a nota (c) do aceite: o mapa é o topo da árvore e isso
    /// precisa ser dito, não silenciado.
    pub only_top_of_tree: bool,
}

/// Monta o mapa a partir do inventory do `kira ingest` na raiz.
///
/// `priority` são os paths citados em `findings.json` (já normalizados),
/// `terms` os termos do work item e `skip` os caminhos que o caller já mandou
/// inteiros no prompt (o work item file) — não faz sentido mapeá-los.
#[must_use]
pub fn build(
    root: &Path,
    priority: &BTreeSet<String>,
    terms: &[String],
    skip: &HashSet<String>,
) -> MapOutcome {
    let path = root.join(".kira").join("ingest").join("inventory.json");
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        // Qualquer falha de leitura que não seja "não existe" também cai na
        // mesma nota de ausência: fail-open, o mapa nunca quebra o comando.
        Err(_) => return MapOutcome::NoInventory,
    };
    let inventory: Inventory = match serde_json::from_str(&raw) {
        Ok(inventory) => inventory,
        Err(_) => return MapOutcome::BadInventory,
    };
    if inventory.files.is_empty() {
        return MapOutcome::EmptyRepo;
    }

    // A nota (c) olha o repo INTEIRO, não só o que coube no teto de 20.
    let has_anchor = inventory.files.iter().any(|file| {
        matches!(
            role_of(&file.path, priority, terms),
            Role::Citado | Role::Entrada
        )
    });
    let total_files = inventory.files.len();
    let ranked = ranked(&inventory.files, priority, terms, skip);
    let block = assemble(&ranked, total_files);
    MapOutcome::Ready(RepoMap {
        block,
        total_files,
        omitted: ranked.len().saturating_sub(MAX_MAP_FILES),
        only_top_of_tree: !has_anchor,
    })
}

/// Monta o bloco: header + até 20 linhas + marcador `... (+N arquivos)`, tudo
/// dentro de `MAX_MAP_CHARS`. Quando o teto de chars não cabe, linhas saem DO
/// FIM até caber — o marcador nunca é engolido, porque é a informação mais
/// valiosa depois do mapa em si (diz que o mapa é truncado e por quanto).
fn assemble(ranked: &[Ranked], total_files: usize) -> String {
    let header = format!("MAPA do repo (ingest, {total_files} arquivos):\n");
    let candidates: Vec<String> = ranked
        .iter()
        .take(MAX_MAP_FILES)
        .map(|entry| format!("- {} ({})\n", entry.path, entry.role.as_str()))
        .collect();
    let mut kept = candidates.len();
    loop {
        let mut block = header.clone();
        for line in candidates.iter().take(kept) {
            block.push_str(line);
        }
        let omitted = ranked.len() - kept;
        if omitted > 0 {
            block.push_str(&format!("... (+{omitted} arquivos)\n"));
        }
        if block.chars().count() <= MAX_MAP_CHARS || kept == 0 {
            return block;
        }
        kept -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;

    fn file(path: &str) -> InventoryFile {
        InventoryFile {
            path: path.to_string(),
            bytes: 10,
            truncated: false,
        }
    }

    fn inventory(paths: &[&str]) -> Inventory {
        Inventory {
            project_name: "teste".to_string(),
            files: paths.iter().map(|path| file(path)).collect(),
            by_extension: BTreeMap::new(),
            total_bytes: 10 * paths.len() as u64,
            notes: Vec::new(),
        }
    }

    fn terms_of(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    /// (b) do aceite: embaralhar a ordem de entrada do inventory tem que
    /// produzir o MESMO ranking — o desempate final é o caminho.
    #[test]
    fn shuffled_input_produces_the_identical_ranking() {
        let paths = [
            "Cargo.toml",
            "src/main.rs",
            "src/lib/cool.rs",
            "a.txt",
            "dir/b.txt",
            "dir/sub/c.txt",
        ];
        let inv = inventory(&paths);
        let priority: BTreeSet<String> = ["src/lib/cool.rs".to_string()].into_iter().collect();
        let terms = terms_of(&["cooldown"]);
        let skip = HashSet::new();

        let first = ranked(&inv.files, &priority, &terms, &skip);
        // Rotação determinística no lugar de um RNG: provar que nenhuma ordem
        // de entrada sobrevive ao ranking.
        let mut shuffled = inv.files.clone();
        shuffled.rotate_left(3);
        let second = ranked(&shuffled, &priority, &terms, &skip);
        assert_eq!(first, second, "shuffle mudou o ranking");
        // Ordem esperada: citado, entrada, config, topo por profundidade —
        // o termo "cooldown" não casa com caminho nenhum.
        assert_eq!(first[0].path, "src/lib/cool.rs");
        assert_eq!(first[0].role, Role::Citado);
        assert_eq!(first[1].path, "src/main.rs");
        assert_eq!(first[1].role, Role::Entrada);
        assert_eq!(first[2].path, "Cargo.toml");
        assert_eq!(first[2].role, Role::Config);
        assert_eq!(first[3].path, "a.txt");
        assert_eq!(first[4].path, "dir/b.txt");
        assert_eq!(first[5].path, "dir/sub/c.txt");
    }

    /// Termo do work item no caminho outrank entrada/config.
    #[test]
    fn a_term_in_the_path_outranks_the_entrypoint() {
        let paths = ["src/main.rs", "src/cooldown/fila.rs"];
        let inv = inventory(&paths);
        let priority = BTreeSet::new();
        let terms = terms_of(&["cooldown"]);
        let ranked_list = ranked(&inv.files, &priority, &terms, &HashSet::new());
        assert_eq!(ranked_list[0].path, "src/cooldown/fila.rs");
        assert_eq!(ranked_list[0].role, Role::Termo);
        assert_eq!(ranked_list[1].path, "src/main.rs");
    }

    /// (a) do aceite em forma pura: o bloco honra os dois tetos e fecha o
    /// mapa com o marcador contável.
    #[test]
    fn the_block_honors_both_caps_and_counts_the_omitted() {
        let paths: Vec<String> = (0..30).map(|n| format!("p{n:02}/file.rs")).collect();
        let owned: Vec<&str> = paths.iter().map(|path| path.as_str()).collect();
        let inv = inventory(&owned);
        let priority = BTreeSet::new();
        let terms: Vec<String> = Vec::new();
        let skip = HashSet::new();

        let ranked_list = ranked(&inv.files, &priority, &terms, &skip);
        let block = assemble(&ranked_list, inv.files.len());
        assert!(
            block.chars().count() <= MAX_MAP_CHARS,
            "teto de chars: {block}"
        );
        let files_in_block = block.lines().filter(|line| line.starts_with("- ")).count();
        assert_eq!(files_in_block, MAX_MAP_FILES, "bloco: {block}");
        assert!(block.ends_with("... (+10 arquivos)\n"), "bloco: {block}");
        // Header conta o total, não o que coube.
        assert!(block.starts_with("MAPA do repo (ingest, 30 arquivos):\n"));
    }

    /// Mesma entrada ⇒ mesmo bloco byte a byte (extensão do gate M1).
    #[test]
    fn the_same_input_yields_the_byte_identical_block() {
        let paths: Vec<String> = (0..25).map(|n| format!("p{n:02}/file.rs")).collect();
        let owned: Vec<&str> = paths.iter().map(|path| path.as_str()).collect();
        let inv = inventory(&owned);
        let ranked_list = ranked(&inv.files, &BTreeSet::new(), &[], &HashSet::new());
        let first = assemble(&ranked_list, inv.files.len());
        let second = assemble(&ranked_list, inv.files.len());
        assert_eq!(first, second);
    }

    /// Repo vazio ⇒ `EmptyRepo`, sem nota (pack byte-idêntico ao de hoje).
    #[test]
    fn an_empty_repo_yields_the_empty_outcome() {
        let dir = std::env::temp_dir().join(format!("kira-repo-map-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".kira/ingest")).unwrap_or_else(|err| panic!("mkdir: {err}"));
        let inv = inventory(&[]);
        fs::write(
            dir.join(".kira/ingest/inventory.json"),
            serde_json::to_string_pretty(&inv).unwrap_or_else(|err| panic!("serializar: {err}")),
        )
        .unwrap_or_else(|err| panic!("escrever: {err}"));

        let priority = BTreeSet::new();
        let outcome = build(&dir, &priority, &[], &HashSet::new());
        assert_eq!(outcome, MapOutcome::EmptyRepo);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Inventory ausente/corrompido ⇒ nota acionável, nunca erro.
    #[test]
    fn a_missing_or_corrupt_inventory_fails_open() {
        let dir = std::env::temp_dir().join(format!("kira-repo-map-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let priority = BTreeSet::new();
        // Ausente:
        assert_eq!(
            build(&dir, &priority, &[], &HashSet::new()),
            MapOutcome::NoInventory
        );
        // Corrompido:
        fs::create_dir_all(dir.join(".kira/ingest")).unwrap_or_else(|err| panic!("mkdir: {err}"));
        fs::write(dir.join(".kira/ingest/inventory.json"), "{ não é json")
            .unwrap_or_else(|err| panic!("escrever: {err}"));
        assert_eq!(
            build(&dir, &priority, &[], &HashSet::new()),
            MapOutcome::BadInventory
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// (c) do aceite em forma pura: sem findings e sem entrypoint, a flag da
    /// nota vem ligada e o ranking é o topo da árvore (rasa primeiro).
    #[test]
    fn without_findings_or_entrypoint_the_map_is_top_of_tree_and_says_so() {
        let paths = ["src/deep/leaf.rs", "readme.md", "lib/other.rs"];
        let inv = inventory(&paths);
        let priority = BTreeSet::new();
        let terms = terms_of(&["token"]);
        let skip = HashSet::new();

        let ranked_list = ranked(&inv.files, &priority, &terms, &skip);
        // "token" casa com NENHUM caminho; sem findings, sem entrypoint —
        // todos ficam `topo`, ordenados por profundidade.
        assert!(ranked_list.iter().all(|entry| entry.role == Role::Topo));
        assert_eq!(ranked_list[0].path, "readme.md");
        assert_eq!(ranked_list[1].path, "lib/other.rs");
        assert_eq!(ranked_list[2].path, "src/deep/leaf.rs");

        // A flag da nota (c) olha o repo inteiro via `build` (com disco).
        let dir = std::env::temp_dir().join(format!("kira-repo-map-topo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".kira/ingest")).unwrap_or_else(|err| panic!("mkdir: {err}"));
        fs::write(
            dir.join(".kira/ingest/inventory.json"),
            serde_json::to_string_pretty(&inventory(&paths))
                .unwrap_or_else(|err| panic!("serializar: {err}")),
        )
        .unwrap_or_else(|err| panic!("escrever: {err}"));
        let outcome = build(&dir, &priority, &terms, &skip);
        let map = match outcome {
            MapOutcome::Ready(map) => map,
            other => panic!("esperava Ready, veio {other:?}"),
        };
        assert!(map.only_top_of_tree, "nota (c) precisa vir ligada");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Um entrypoint no repo desliga a nota (c) — o mapa tem âncora.
    #[test]
    fn an_entrypoint_disables_the_top_of_tree_note() {
        let paths = ["src/main.rs", "readme.md"];
        let dir = std::env::temp_dir().join(format!("kira-repo-map-entry-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".kira/ingest")).unwrap_or_else(|err| panic!("mkdir: {err}"));
        fs::write(
            dir.join(".kira/ingest/inventory.json"),
            serde_json::to_string_pretty(&inventory(&paths))
                .unwrap_or_else(|err| panic!("serializar: {err}")),
        )
        .unwrap_or_else(|err| panic!("escrever: {err}"));
        let outcome = build(&dir, &BTreeSet::new(), &[], &HashSet::new());
        let map = match outcome {
            MapOutcome::Ready(map) => map,
            other => panic!("esperava Ready, veio {other:?}"),
        };
        assert!(!map.only_top_of_tree);
        assert!(map.block.contains("src/main.rs (entrada)"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// skip_paths (o work item file) nunca entram no mapa.
    #[test]
    fn skipped_paths_never_enter_the_map() {
        let paths = ["work-item.json", "src/main.rs"];
        let inv = inventory(&paths);
        let priority = BTreeSet::new();
        let mut skip = HashSet::new();
        skip.insert("work-item.json".to_string());
        let ranked_list = ranked(&inv.files, &priority, &[], &skip);
        assert_eq!(ranked_list.len(), 1);
        assert_eq!(ranked_list[0].path, "src/main.rs");
    }
}
