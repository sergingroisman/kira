//! `.gitignore` aninhado (M7/P5): matcher gitwildmatch **subset**, puro, sem
//! dependência nova (ADR-0023: Rust puro — a mesma escolha do denylist).
//!
//! O que este subset cobre (e o que NÃO cobre, de propósito):
//! - **cobre**: comentários `#`, linhas em branco, negação `!` (última regra
//!   que casa vence), `/` inicial (ancora no diretório do arquivo), `/` final
//!   (só diretório), padrões com `/` no meio (ancorados no diretório do
//!   arquivo), padrões sem `/` (casam o basename em qualquer nível), `*`, `?`
//!   e `**` (segmento inteiro: zero ou mais diretórios).
//! - **cobre**: aninhamento — um `.gitignore` vale para o seu subdiretório, e
//!   regras de diretórios MAIS FUNDOS vencem as dos mais rasos (ordem do git).
//! - **cobre**: a regra do git de que NÃO dá para re-incluir um arquivo cujo
//!   diretório pai foi excluído (o ancestral excluído decide antes do filho).
//! - **não cobre**: escapes (`\!`, espaços escapados), classes `[a-z]`,
//!   `core.ignorecase` do Windows (o matcher é case-sensitive, determinístico
//!   nos 3 SOs), e `.git/info/exclude`/ignores globais. São raros em repo de
//!   código e cada um é uma linha a mais de parser para quase nenhum ganho.
//!
//! Determinismo: as regras são ordenadas por (profundidade do diretório,
//! diretório, número da linha) — a mesma árvore produz sempre a mesma
//! decisão, independente da ordem em que o filesystem devolve os arquivos.

use std::path::Path;

use walkdir::WalkDir;

/// Uma regra já parseada de um `.gitignore`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    /// Diretório (relativo à raiz, `""` = raiz) onde o `.gitignore` vive.
    dir: String,
    /// Padrão limpo (sem `!`, sem `/` inicial/final).
    pattern: String,
    /// `!` na frente: re-inclui em vez de excluir.
    negated: bool,
    /// `/` no final: casa só diretório.
    dir_only: bool,
    /// Padrão com `/` (ou `/` inicial): casa o caminho relativo ao `dir`,
    /// não o basename em qualquer nível.
    anchored: bool,
    /// Profundidade do `dir` — regras mais fundas vencem as mais rasas.
    depth: usize,
    /// Número da linha no arquivo — dentro do mesmo arquivo, a última vence.
    line: usize,
}

/// Conjunto de regras de todos os `.gitignore` da árvore, pronto para
/// responder `is_ignored(rel, is_dir)`.
#[derive(Debug, Default)]
pub struct GitIgnore {
    rules: Vec<Rule>,
}

impl GitIgnore {
    /// Carrega todos os `.gitignore` da árvore a partir de `root`, na ordem
    /// canônica (diretórios mais rasos primeiro; dentro de cada arquivo,
    /// ordem de linha). Os diretórios do denylist interno (`.git`, `target`,
    /// `node_modules`, `.kira`…) nunca são descidos — o git também não aplica
    /// ignore dentro de `.git`.
    #[must_use]
    pub fn load(root: &Path) -> Self {
        let walker = WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| {
                if entry.file_type().is_dir() {
                    !super::repo::is_skipped_dir(entry.path(), entry.depth())
                } else {
                    true
                }
            });
        let mut rules = Vec::new();
        for entry in walker.flatten() {
            if !entry.file_type().is_file() || entry.file_name() != ".gitignore" {
                continue;
            }
            // O diretório do arquivo, relativo à raiz e com `/` (o matcher
            // trabalha sempre em POSIX — caminho de Windows é normalizado).
            let dir = super::repo::relative_path(root, entry.path().parent().unwrap_or(root))
                .unwrap_or_default()
                .replace('\\', "/");
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue; // ilegível: ignora o arquivo, nunca quebra o comando
            };
            for (index, line) in text.lines().enumerate() {
                if let Some(rule) = parse_line(line, &dir, index) {
                    rules.push(rule);
                }
            }
        }
        // Ordem canônica: (profundidade, diretório, linha). O sort garante o
        // determinismo mesmo que a caminhada devolva outra ordem.
        rules.sort_by(|a, b| {
            a.depth
                .cmp(&b.depth)
                .then_with(|| a.dir.cmp(&b.dir))
                .then_with(|| a.line.cmp(&b.line))
        });
        Self { rules }
    }

    /// Constrói direto de pares `(dir, conteúdo)` — os testes unitários não
    /// tocam disco.
    #[must_use]
    pub fn from_texts(texts: &[(&str, &str)]) -> Self {
        let mut rules = Vec::new();
        for (dir, text) in texts {
            let dir = dir.trim_matches('/');
            for (index, line) in text.lines().enumerate() {
                if let Some(rule) = parse_line(line, dir, index) {
                    rules.push(rule);
                }
            }
        }
        rules.sort_by(|a, b| {
            a.depth
                .cmp(&b.depth)
                .then_with(|| a.dir.cmp(&b.dir))
                .then_with(|| a.line.cmp(&b.line))
        });
        Self { rules }
    }

    /// Sem regra nenhuma (repo sem `.gitignore`): nada é ignorado — o
    /// comportamento antigo fica intacto.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Decide um caminho relativo à raiz (sempre com `/`). `is_dir` importa
    /// para regras `dir-only`.
    ///
    /// Ancestrais primeiro: diretório pai excluído não é re-incluível por
    /// regra do filho (comportamento documentado do git).
    #[must_use]
    pub fn is_ignored(&self, rel: &str, is_dir: bool) -> bool {
        if self.rules.is_empty() || rel.is_empty() {
            return false;
        }
        let rel = rel.trim_start_matches("./");
        let components: Vec<&str> = rel.split('/').collect();
        // Todos os prefixos de diretório (ancestrais) decidem antes do alvo.
        for index in 0..components.len().saturating_sub(1) {
            let prefix = components[..=index].join("/");
            if self.match_path(&prefix, true) {
                return true;
            }
        }
        self.match_path(rel, is_dir)
    }

    /// Última regra que casa vence (`!` re-inclui, plain exclui).
    fn match_path(&self, rel: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for rule in &self.rules {
            if rule.dir_only && !is_dir {
                continue;
            }
            let Some(rest) = strip_dir(rel, &rule.dir) else {
                continue;
            };
            if rule_matches(rule, rest) {
                ignored = !rule.negated;
            }
        }
        ignored
    }
}

/// Resto do caminho relativo ao diretório da regra; `None` quando a regra não
/// se aplica a este caminho (diretório diferente).
fn strip_dir<'a>(rel: &'a str, dir: &str) -> Option<&'a str> {
    if dir.is_empty() {
        return Some(rel);
    }
    let prefix = format!("{dir}/");
    rel.strip_prefix(prefix.as_str())
}

/// Casa o resto do caminho contra o padrão da regra.
fn rule_matches(rule: &Rule, rest: &str) -> bool {
    if rule.anchored {
        path_match(&rule.pattern, rest)
    } else {
        // Sem `/`: casa o basename em qualquer nível.
        let basename = rest.rsplit('/').next().unwrap_or(rest);
        glob_segment(&rule.pattern, basename)
    }
}

/// Parse de UMA linha de `.gitignore` (função pura).
fn parse_line(line: &str, dir: &str, index: usize) -> Option<Rule> {
    // Trailing whitespace é ignorado (git faz o mesmo; escapes não são
    // suportados neste subset — documentado no cabeçalho).
    let line = line.trim_end_matches([' ', '\t']);
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (negated, line) = match line.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let (anchored_head, line) = match line.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let (dir_only, pattern) = match line.strip_suffix('/') {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    if pattern.is_empty() {
        return None;
    }
    // `/` no meio do padrão ancora no diretório do arquivo (regra do git).
    let anchored = anchored_head || pattern.contains('/');
    Some(Rule {
        dir: dir.to_string(),
        pattern: pattern.to_string(),
        negated,
        dir_only,
        anchored,
        depth: if dir.is_empty() {
            0
        } else {
            dir.split('/').count()
        },
        line: index,
    })
}

/// Glob por SEGMENTOS de caminho: `*`/`?` não atravessam `/`, `**` como
/// segmento inteiro atravessa zero ou mais diretórios.
fn path_match(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    segment_match(&pattern, &path)
}

fn segment_match(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.first() {
        None => path.is_empty(),
        Some(&"**") => {
            // `**` casa zero ou mais segmentos: tenta cada corte do caminho.
            (0..=path.len()).any(|cut| segment_match(&pattern[1..], &path[cut..]))
        }
        Some(head) => {
            !path.is_empty()
                && glob_segment(head, path[0])
                && segment_match(&pattern[1..], &path[1..])
        }
    }
}

/// Glob de um segmento: `*` (zero ou mais chars) e `?` (um char),
/// case-sensitive (determinismo nos 3 SOs — ver cabeçalho do módulo).
fn glob_segment(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    // DP por linha: reach[j] = p[..i] casou com t[..j].
    let mut reach = vec![false; t.len() + 1];
    reach[0] = true;
    for pc in &p {
        let mut next = vec![false; t.len() + 1];
        match pc {
            '*' => {
                let mut any = false;
                for j in 0..=t.len() {
                    any = any || reach[j];
                    next[j] = any;
                }
            }
            '?' => {
                next[1..=t.len()].copy_from_slice(&reach[0..t.len()]);
            }
            literal => {
                for j in 1..=t.len() {
                    next[j] = reach[j - 1] && t[j - 1] == *literal;
                }
            }
        }
        reach = next;
    }
    reach[t.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gi(texts: &[(&str, &str)]) -> GitIgnore {
        GitIgnore::from_texts(texts)
    }

    #[test]
    fn empty_ignore_ignores_nothing() {
        let empty = gi(&[]);
        assert!(empty.is_empty());
        assert!(!empty.is_ignored("src/main.rs", false));
        assert!(!empty.is_ignored("build", true));
    }

    #[test]
    fn comments_and_blank_lines_are_not_rules() {
        let g = gi(&[("", "# comentário\n\n   \nbuild/\n")]);
        assert!(!g.is_ignored("#comentário", false));
        assert!(g.is_ignored("build", true));
    }

    #[test]
    fn basename_pattern_matches_at_any_level() {
        let g = gi(&[("", "*.log\n")]);
        assert!(g.is_ignored("app.log", false));
        assert!(g.is_ignored("a/b/deep.log", false));
        assert!(!g.is_ignored("a/b/log.txt", false));
    }

    #[test]
    fn excluded_dir_takes_everything_under_it() {
        let g = gi(&[("", "build\n")]);
        assert!(g.is_ignored("build", true));
        assert!(g.is_ignored("build/out.js", false));
        assert!(g.is_ignored("a/build/out.js", false), "qualquer nível");
        assert!(!g.is_ignored("src/build.rs", false), "basename ≠ sufixo");
    }

    #[test]
    fn dir_only_pattern_does_not_match_files() {
        let g = gi(&[("", "logs/\n")]);
        assert!(g.is_ignored("logs", true));
        assert!(
            g.is_ignored("logs/a.txt", false),
            "conteúdo do dir cai junto"
        );
        assert!(
            !g.is_ignored("logs", false),
            "arquivo chamado logs NÃO é dir-only"
        );
    }

    #[test]
    fn anchored_pattern_matches_from_the_file_dir() {
        let g = gi(&[("", "/config.toml\ndocs/drafts/\n")]);
        assert!(g.is_ignored("config.toml", false));
        assert!(
            !g.is_ignored("sub/config.toml", false),
            "/ inicial ancora na raiz"
        );
        assert!(g.is_ignored("docs/drafts", true));
        assert!(g.is_ignored("docs/drafts/a.md", false));
        assert!(!g.is_ignored("other/drafts", true));
    }

    #[test]
    fn negation_reincludes_last_match_wins() {
        let g = gi(&[("", "*.log\n!keep.log\n")]);
        assert!(g.is_ignored("app.log", false));
        assert!(!g.is_ignored("keep.log", false), "! depois vence");
        let inverted = gi(&[("", "!keep.log\n*.log\n")]);
        assert!(
            inverted.is_ignored("keep.log", false),
            "plain depois do ! vence (ordem de linha)"
        );
    }

    #[test]
    fn nested_gitignore_overrides_parent() {
        // Padrão do git: regras mais fundas vencem as mais rasas.
        let g = gi(&[
            ("", "*.md\ndocs/important.md\n"),
            ("docs", "!important.md\n"),
        ]);
        assert!(g.is_ignored("README.md", false));
        assert!(
            !g.is_ignored("docs/important.md", false),
            "o .gitignore de docs/ re-inclui"
        );
        assert!(g.is_ignored("docs/other.md", false));
    }

    #[test]
    fn child_rule_cannot_reinclude_under_excluded_dir() {
        // Regra documentada do git: pai excluído ⇒ filho não é re-incluível.
        let g = gi(&[("", "build/\n"), ("build", "!keep.txt\n")]);
        assert!(
            g.is_ignored("build/keep.txt", false),
            "build/ excluído decide antes do filho"
        );
        assert!(g.is_ignored("build/out.js", false));
    }

    #[test]
    fn doublestar_matches_across_directories() {
        let g = gi(&[("", "**/generated\na/**/b.txt\n")]);
        assert!(g.is_ignored("x/y/generated", true));
        assert!(g.is_ignored("generated", true));
        assert!(g.is_ignored("a/deep/down/b.txt", false));
        assert!(
            !g.is_ignored("c/deep/b.txt", false),
            "âncora `a/` respeitada"
        );
    }

    #[test]
    fn question_mark_matches_one_char() {
        let g = gi(&[("", "?.tmp\n")]);
        assert!(g.is_ignored("a.tmp", false));
        assert!(!g.is_ignored("ab.tmp", false));
    }

    #[test]
    fn matcher_is_case_sensitive_for_determinism() {
        let g = gi(&[("", "build\n")]);
        assert!(!g.is_ignored("BUILD", true), "case-sensitive nos 3 SOs");
    }
}
