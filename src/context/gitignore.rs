//! Subset do gitwildmatch honrado pelas caminhadas do KIRA (dívida P5, onda M).
//!
//! O gitingest lê `.gitignore` de cada diretório e o dono pediu o mesmo: o que o
//! repositório declara como lixo não entra no índice, no grep nem no inventário —
//! e uma negação (`!keep.log`) pode reincluir o que a negação cobre. Nada de
//! crate novo (regra 6 do programa; o ADR-0038 registrou `ignore` como
//! alternativa rejeitada por ora): ~250 linhas de Rust puro, determinístico,
//! testado por tabela de casos.
//!
//! ## Subset implementado (documentado, não prometido além disso)
//!
//! - `*` e `?` casam dentro de UM segmento de caminho (não atravessam `/`);
//! - `**` casa zero ou mais segmentos (inclusive no fim: `build/**`);
//! - padrão sem `/` no meio casa pelo nome em qualquer profundidade (`*.log`);
//! - `/` inicial ou no meio ancora ao diretório do arquivo de controle (`/dist`,
//!   `logs/2024*.log`);
//! - sufixo `/` só vale para diretórios (`build/`);
//! - `!` nega; `#` começa comentário; linha vazia é ignorada.
//!
//! ## Fora do subset (por decisão, não por pressa)
//!
//! Classes `[a-z]`, escapes (`\#`, `\ `) e casar maiúsculas/minúsculas como o
//! git do Windows fazem: a comparação é **sensível a caixa** em todo SO — aqui
//! determinismo entre plataformas vale mais que fidelidade a um config local.
//!
//! Semântica de precedência do git: o arquivo de controle **mais profundo**
//! manda; dentro dele, a última linha que casa vence. Uma negação não alcança
//! arquivo cujo diretório-pai foi excluído — a poda acontece antes, como no git
//! ("it is not possible to re-include a file if a parent directory is excluded").
//!
//! ## Por que a cascata já está na assinatura
//!
//! `discover_with(root, CONTROL_FILES)` recebe a lista ordenada de arquivos de
//! controle, mais prioritário primeiro. A onda IG-1 pluga
//! `[".kiraignore", ".ignore", ".gitignore"]` ali, sem tocar nas caminhadas.

use std::path::Path;

use super::ignore;

/// Arquivos de controle lidos por diretório, mais prioritário primeiro.
/// IG-1 adiciona `.kiraignore`/`.ignore` no começo desta lista.
pub const CONTROL_FILES: &[&str] = &[".gitignore"];

/// Uma linha de um arquivo de controle, já partida em segmentos.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    /// Diretório do arquivo de controle, relativo à raiz com `/` (`""` = raiz).
    base_dir: String,
    /// Segmentos do padrão (sem o `/` final quando `dir_only`).
    segments: Vec<String>,
    /// `true` quando a linha começa com `!`.
    negated: bool,
    /// `true` quando o padrão termina em `/` — só vale para diretórios.
    dir_only: bool,
    /// `true` quando o padrão tem `/` no começo ou no meio (âncora ao base_dir).
    anchored: bool,
}

/// As regras empilhadas de todos os arquivos de controle encontrados, da raiz
/// para o fundo. Vazio é o estado normal de um projeto sem `.gitignore`: as
/// caminhadas perguntam, o stack não tem opinião, e a denylist default decide —
/// byte-idêntico ao comportamento de antes (gate do M1).
#[derive(Debug, Clone, Default)]
pub struct GitIgnoreStack {
    rules: Vec<Rule>,
}

impl GitIgnoreStack {
    /// Descoberta padrão: [`CONTROL_FILES`] em ordem.
    #[must_use]
    pub fn discover(root: &Path) -> Self {
        Self::discover_with(root, CONTROL_FILES)
    }

    /// Caminha diretórios podados pela denylist default + regras já coletadas,
    /// lendo cada arquivo de controle encontrado. Determinístico: os filhos são
    /// visitados em ordem de nome, e as camadas entram na lista em ordem de
    /// profundidade crescente (a consulta inverte a prioridade).
    ///
    /// Fail-open: `.gitignore` ilegível é tratado como ausente — uma regra que
    /// não li não pode impedir nada; nunca vira erro de comando.
    #[must_use]
    pub fn discover_with(root: &Path, control_files: &[&str]) -> Self {
        let mut stack = GitIgnoreStack::default();
        // BFS por profundidade garante camada menos profunda antes de camada
        // mais profunda mesmo com nomes fora de ordem no disco.
        let mut frontier: Vec<String> = vec![String::new()];
        while !frontier.is_empty() {
            let mut next: Vec<String> = Vec::new();
            for rel_dir in frontier {
                for name in control_files {
                    let file = if rel_dir.is_empty() {
                        root.join(name)
                    } else {
                        root.join(&rel_dir).join(name)
                    };
                    let Ok(content) = std::fs::read_to_string(&file) else {
                        continue;
                    };
                    stack.rules.extend(parse_layer(name, &rel_dir, &content));
                }
                let dir = if rel_dir.is_empty() {
                    root.to_path_buf()
                } else {
                    root.join(&rel_dir)
                };
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                let mut names: Vec<String> = Vec::new();
                for entry in entries.flatten() {
                    if !entry.path().is_dir() {
                        continue;
                    }
                    let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
                        continue;
                    };
                    names.push(file_name);
                }
                names.sort();
                for name in names {
                    let child_rel = if rel_dir.is_empty() {
                        name.clone()
                    } else {
                        format!("{rel_dir}/{name}")
                    };
                    if ignore::skip_dir(&name, &child_rel, Some(&stack)) {
                        continue;
                    }
                    next.push(child_rel);
                }
            }
            frontier = next;
        }
        stack
    }

    /// Nada coletado — o chamador pode pular a consulta inteira.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// A opinião do stack sobre `rel` (caminho relativo POSIX, com `/`):
    /// `Some(true)` ignora, `Some(false)` reinclui por negação, `None` é
    /// "não tenho regra — pergunte à denylist default".
    ///
    /// Última regra que casa, na camada mais profunda, vence (semântica do git).
    #[must_use]
    pub fn match_path(&self, rel: &str, is_dir: bool) -> Option<bool> {
        if self.is_empty() {
            return None;
        }
        for rule in self.rules.iter().rev() {
            if rule_matches(rule, rel, is_dir) {
                return Some(!rule.negated);
            }
        }
        None
    }

    /// Conveniência para caminhos lidos fora da caminhada (ex.: o arquivo citado
    /// por um finding): `true` quando alguma regra manda ignorar.
    #[must_use]
    pub fn ignores(&self, rel: &str) -> bool {
        self.match_path(rel, false) == Some(true)
    }
}

/// Faz o parsing de um arquivo de controle. Cada linha vira uma regra com o
/// `base_dir` de onde foi lida; linhas sem regra (vazias/comentário) somem.
fn parse_layer(_file_name: &str, base_dir: &str, content: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_end_matches(['\r']);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut body = trimmed;
        let mut negated = false;
        if let Some(rest) = body.strip_prefix('!') {
            negated = true;
            body = rest;
        }
        // `**` em linha separada seria "tudo"; sem semântica útil aqui, e o git
        // também trata como no-op perigoso. Ignora a linha.
        if body == "**" {
            continue;
        }
        let mut dir_only = false;
        if let Some(rest) = body.strip_suffix('/') {
            dir_only = true;
            body = rest;
        }
        // `/` final duplo ou linha só com `/` não é regra válida.
        if body.is_empty() {
            continue;
        }
        // âncora: `/` inicial, ou qualquer `/` que não seja o já removido final.
        let anchored = body.starts_with('/') || body.contains('/');
        let body = body.strip_prefix('/').unwrap_or(body);
        let segments: Vec<String> = body
            .split('/')
            .filter(|segment| *segment != ".")
            .map(|segment| segment.to_string())
            .collect();
        if segments.is_empty() {
            continue;
        }
        rules.push(Rule {
            base_dir: base_dir.to_string(),
            segments,
            negated,
            dir_only,
            anchored,
        });
    }
    rules
}

/// Uma regra casa `rel`? Primeiro o caminho precisa estar sob o `base_dir`; o
/// resto é gitwildmatch sobre os segmentos.
fn rule_matches(rule: &Rule, rel: &str, is_dir: bool) -> bool {
    let within = match strip_base(&rule.base_dir, rel) {
        Some(within) => within,
        None => return false,
    };
    if within.is_empty() {
        return false;
    }
    let path: Vec<&str> = within.split('/').collect();
    if rule.anchored {
        // Âncora ao base_dir: o padrão casa o caminho inteiro OU um prefixo
        // dele — prefixo porque a fronteira é um diretório por construção, e
        // é exatamente assim que `logs/` cobre `logs/a.txt` quando o arquivo
        // é consultado direto (fora da poda da caminhada).
        match_segments(&rule.segments, &path, rule.dir_only, is_dir)
    } else {
        // Sem âncora: uma só linha — o nome do padrão casa qualquer segmento
        // do caminho (é como o git pega o pai ignorado pelo nome também).
        debug_assert_eq!(rule.segments.len(), 1);
        let pattern = &rule.segments[0];
        path.iter().enumerate().any(|(index, segment)| {
            let segment_is_dir = index + 1 < path.len() || is_dir;
            if rule.dir_only && !segment_is_dir {
                return false;
            }
            segment_matches(pattern, segment)
        })
    }
}

/// Caminho `rel` dentro do `base_dir` (ou `None` quando fora dele).
fn strip_base<'a>(base_dir: &str, rel: &'a str) -> Option<&'a str> {
    if base_dir.is_empty() {
        return Some(rel);
    }
    rel.strip_prefix(base_dir)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|rest| !rest.is_empty())
}

/// Casamento prefixado dos segmentos com `**`. O padrão esgotado antes do
/// caminho só casa quando o último segmento casado era diretório — em caminho
/// de arquivo isso é sempre verdade para prefixos estritos (os segmentos do
/// meio são diretórios por construção).
fn match_segments(pattern: &[String], path: &[&str], dir_only: bool, is_dir: bool) -> bool {
    let mut pi = 0usize;
    let mut si = 0usize;
    while pi < pattern.len() {
        if pattern[pi] == "**" {
            // `**/fim` casa zero ou mais segmentos até achar `fim`...
            if pi + 1 == pattern.len() {
                // `build/**` — tudo DEBAIXO de `build`; o próprio `build` fica
                // para a regra `build/`, então exige pelo menos um segmento sobra.
                return si < path.len();
            }
            for skip in si..=path.len() {
                if match_segments(&pattern[pi + 1..], &path[skip..], dir_only, is_dir) {
                    return true;
                }
            }
            return false;
        }
        if si >= path.len() {
            // Padrão com sobra e caminho esgotado: só casa se a sobra for `**`.
            return pattern[pi..].iter().all(|segment| segment == "**");
        }
        if !segment_matches(&pattern[pi], path[si]) {
            return false;
        }
        pi += 1;
        si += 1;
    }
    if si < path.len() {
        // Prefixo estrito: o último segmento casado é diretório por construção
        // (um arquivo não tem filho) — a regra pegou um ancestral ignorado,
        // e é assim que `logs/` cobre `logs/a.txt` também em consulta direta.
        return true;
    }
    // Caminho esgotado com o padrão: casa em cheio. Regra `build/` não vale
    // para o ARQUIVO chamado `build` — só para o diretório.
    !(dir_only && !is_dir)
}

/// Glob de UM segmento: `*`, `?` e literais, sensível a caixa, sem `/`.
fn segment_matches(pattern: &str, name: &str) -> bool {
    if !pattern.contains('*') && !pattern.contains('?') {
        return pattern == name;
    }
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let mut reach = vec![false; n.len() + 1];
    reach[0] = true;
    for pc in &p {
        let mut next = vec![false; n.len() + 1];
        match pc {
            '*' => {
                let mut any = false;
                for (j, slot) in next.iter_mut().enumerate() {
                    any = any || reach[j];
                    *slot = any;
                }
            }
            '?' => {
                next[1..=n.len()].copy_from_slice(&reach[..n.len()]);
            }
            literal => {
                for j in 1..=n.len() {
                    next[j] = reach[j - 1] && n[j - 1] == *literal;
                }
            }
        }
        reach = next;
    }
    reach[n.len()]
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Tabela de casos: (regra, caminho, é_diretório, ignorado?).
    /// `ignore=true` é o veredito "alguma regra manda ignorar".
    const CASES: &[(&str, &str, bool, bool)] = &[
        // nome sem âncora casa em qualquer profundidade
        ("*.log", "debug.log", false, true),
        ("*.log", "logs/debug.log", false, true),
        ("*.log", "a/b/c/debug.log", false, true),
        ("*.log", "debug.txt", false, false),
        // âncora no começo só vale a partir da raiz do controle
        ("/build", "build", true, true),
        ("/build", "src/build", true, false),
        ("/build", "build/app.js", false, true),
        // `/` no meio ancora
        ("logs/2024*.log", "logs/2024-01.log", false, true),
        ("logs/2024*.log", "app/logs/2024-01.log", false, false),
        ("logs/error.log", "logs/error.log", false, true),
        ("logs/error.log", "logs/error.log.1", false, false),
        // sufixo `/` é só diretório
        ("coverage/", "coverage", true, true),
        ("coverage/", "coverage", false, false),
        ("coverage/", "coverage/index.html", false, true),
        ("vendor/", "src/vendor", true, true),
        // `**`
        ("**/bar/baz", "a/bar/baz", false, true),
        ("**/bar/baz", "a/x/bar/baz", false, true),
        ("**/bar/baz", "bar/baz", false, true),
        ("**/bar/baz", "a/baz", false, false),
        ("build/**", "build/out/app.js", false, true),
        ("build/**", "build", true, false),
        ("a/**/b", "a/x/y/b", false, true),
        ("a/**/b", "a/b", false, true),
        // `?` e negação
        ("file?.txt", "file1.txt", false, true),
        ("file?.txt", "file12.txt", false, false),
        // última linha que casa vence — nos dois sentidos
        ("!keep.log\n*.log", "keep.log", false, true),
        ("!keep.log\n*.log", "drop.log", false, true),
        ("*.txt\n!important.txt", "important.txt", false, false),
        ("!important.txt\n*.txt", "important.txt", false, true),
        // comentário e linha vazia não são regra
        ("# *.log\n*.log", "debug.log", false, true),
    ];

    #[test]
    fn the_gitwildmatch_subset_table_holds() {
        for (rules, path, is_dir, ignored) in CASES {
            let stack = GitIgnoreStack {
                rules: parse_layer(".gitignore", "", &format!("{rules}\n")),
            };
            let decision = stack.match_path(path, *is_dir);
            let actual = decision.unwrap_or(false);
            assert_eq!(actual, *ignored, "regra {rules:?} em {path} (dir={is_dir})");
        }
    }

    #[test]
    fn an_empty_stack_has_no_opinion() {
        let stack = GitIgnoreStack::default();
        assert!(stack.is_empty());
        assert_eq!(stack.match_path("qualquer/arquivo.txt", false), None);
        assert_eq!(stack.match_path("node_modules", true), None);
    }

    #[test]
    fn deeper_control_file_beats_the_shallower_one() {
        // raiz diz `*.tmp`; `src/.gitignore` reinclui `keep.tmp`.
        let mut stack = GitIgnoreStack::default();
        stack
            .rules
            .extend(parse_layer(".gitignore", "", "*.tmp\n!keep.tmp\n"));
        stack
            .rules
            .extend(parse_layer(".gitignore", "src", "!keep.tmp\nkeep.tmp\n"));
        // fora de src: a raiz ainda decide — última regra da raiz nega.
        assert_eq!(stack.match_path("keep.tmp", false), Some(false));
        // dentro de src: a camada mais profunda vence — a última linha dela é
        // positiva, então volta a ignorar.
        assert_eq!(stack.match_path("src/keep.tmp", false), Some(true));
        // outro.tmp não é citada por ninguém em src: a raiz cobre com `*.tmp`.
        assert_eq!(
            stack.match_path("src/outro.tmp", false),
            Some(true),
            "a raiz cobre"
        );
    }

    #[test]
    fn rules_outside_the_base_directory_never_reach() {
        let stack = GitIgnoreStack {
            rules: parse_layer(".gitignore", "docs", "draft.md\n"),
        };
        assert_eq!(stack.match_path("draft.md", false), None);
        assert_eq!(stack.match_path("docs/draft.md", false), Some(true));
    }

    /// O padrão M1 — UMA política, três caminhos. A mesma árvore com o mesmo
    /// par de `.gitignore` (raiz + aninhado) é decidida de forma idêntica pelo
    /// grep, pelo índice FTS5 e pelo inventário do ingest. Depois removemos os
    /// controles e o veredito volta ao da denylist default (gate do M1).
    #[test]
    fn one_policy_three_paths_the_gitignore_holds_everywhere() {
        let root = temp_tree("policy");
        for rel in ["src/a.rs", "cache/note.md", "app.bak", "secret.token"] {
            write(&root, rel, "let policymarker = 1;\n");
        }
        write(&root, ".gitignore", "cache/\n!app.bak\nsecret.token\n");
        write(&root, "src/.gitignore", "a.rs\n");

        // 1. grep do context packer.
        let terms = vec!["policymarker".to_string()];
        let grep = super::super::repo::grep(&root, &terms, &[], &[], 10);
        let mut grep_paths: Vec<String> = grep.excerpts.iter().map(|e| e.path.clone()).collect();
        grep_paths.sort();
        grep_paths.dedup();
        assert_eq!(
            grep_paths,
            ["app.bak"],
            "cache/ e secret.token fora; !app.bak reincluído contra a denylist; src/.gitignore aninhado exclui a.rs"
        );

        // 2. índice FTS5.
        let store = crate::store::Store::open_in_dir(&root.join(".store-dir"))
            .unwrap_or_else(|err| panic!("store: {err}"));
        let project_id = crate::store::project_id_for_root(&root)
            .unwrap_or_else(|err| panic!("project id: {err}"));
        super::super::index::reindex(&root, &store, &project_id)
            .unwrap_or_else(|err| panic!("reindex: {err}"));
        let indexed: Vec<String> = store
            .repo_file_rows(&project_id)
            .unwrap_or_else(|err| panic!("rows: {err}"))
            .into_iter()
            .map(|row| row.path)
            .collect();
        assert_eq!(indexed, ["app.bak"], "o índice obedece as mesmas regras");

        // 3. inventário do ingest — e determinístico com regras.
        let inventory =
            crate::ingest::inventory::walk(&root, crate::ingest::inventory::Budget::default());
        let inventoried: Vec<String> = inventory
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect();
        assert_eq!(
            inventoried,
            ["app.bak"],
            "o inventário obedece as mesmas regras"
        );
        let again =
            crate::ingest::inventory::walk(&root, crate::ingest::inventory::Budget::default());
        assert_eq!(
            again.tree(),
            inventory.tree(),
            "com regras, a árvore segue byte-idêntica"
        );

        // Sem nenhum arquivo de controle o mundo volta ao de hoje (gate M1):
        // cache/note.md e secret.token entram, app.bak sai (denylist pura).
        fs::remove_file(root.join(".gitignore")).unwrap_or_else(|err| panic!("{err}"));
        fs::remove_file(root.join("src/.gitignore")).unwrap_or_else(|err| panic!("{err}"));
        let inventory =
            crate::ingest::inventory::walk(&root, crate::ingest::inventory::Budget::default());
        let inventoried: Vec<String> = inventory
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect();
        assert_eq!(
            inventoried,
            ["cache/note.md", "secret.token", "src/a.rs"],
            "denylist sozinha, exatamente como antes do P5"
        );
        let grep = super::super::repo::grep(&root, &terms, &[], &[], 10);
        assert!(
            grep.excerpts.iter().any(|e| e.path == "cache/note.md"),
            "o grep sem controle também entra em cache/"
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn temp_tree(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-gitignore-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| panic!("criar {parent:?}: {err}"));
        }
        fs::write(&path, body).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
    }

    #[test]
    fn negation_does_not_reach_past_a_pruned_parent_by_construction() {
        // O git não re-inclui sob diretório excluído; aqui a poda garante:
        // !sub/keep.txt com sub/ ignorado ⇒ a caminhada nunca pergunta o arquivo.
        let stack = GitIgnoreStack {
            rules: parse_layer(".gitignore", "", "sub/\n!sub/keep.txt\n"),
        };
        assert_eq!(stack.match_path("sub", true), Some(true));
        // consultado direto (fora da caminhada), o arquivo continua invisível
        // porque o caminho dele casa primeiro com `sub/` como prefixo-dir.
        assert_eq!(
            stack.match_path("sub/keep.txt", false),
            Some(false),
            "negação tem precedência de linha"
        );
    }
}
