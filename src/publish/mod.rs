//! `kira publish` — publica os rascunhos do `digest` no wiki (ADR-0038, M5).
//!
//! O `digest` escreve em `.kira/drafts/`; este comando é o único que escreve em
//! `.kira/wiki/`, onde o dev lê. A regra que importa: **nunca sobrescrever o que o
//! dev escreveu**. Como o wiki é markdown versionável, a única forma honesta de
//! saber se um arquivo foi editado à mão é comparar o conteúdo com o que publicamos
//! da última vez — por isso cada post publicado carrega `published_hash` no
//! front-matter. Se o hash do arquivo atual não bate, o `publish` recusa e diz o que
//! fazer (`--force` é decisão explícita do dono).
//!
//! O índice (`index.md`) é **derivado** do conjunto de posts: nunca é gerado por
//! modelo, senão mente quando a lista muda.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::post::plan::fnv1a64_hex;
use crate::store::Store;
use crate::store::posts::PostState;

/// Prefixo do bloco de front-matter que o `publish` gerencia.
const FRONT_MATTER_OPEN: &str = "---\n";

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("store indisponível: {0}")]
    Store(String),
    #[error("não consegui ler {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("não consegui gravar {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Opções do `publish`.
#[derive(Debug, Clone)]
pub struct PublishOptions {
    pub root: PathBuf,
    /// Sobrescreve post editado à mão (decisão explícita do dono).
    pub force: bool,
}

impl PublishOptions {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            force: false,
        }
    }
}

/// O que aconteceu em cada post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishOutcome {
    Published,
    Unchanged,
    /// Recusado porque o arquivo em `.kira/wiki/` foi editado depois da publicação.
    Edited,
    /// Ainda não tem rascunho (o `digest` não rodou para ele).
    NoDraft,
}

#[derive(Debug, Clone, Default)]
pub struct PublishReport {
    pub published: usize,
    pub unchanged: usize,
    pub edited: Vec<String>,
    pub missing_drafts: Vec<String>,
    pub index_path: String,
    pub notes: Vec<String>,
}

impl PublishReport {
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "publish: {} publicado(s), {} sem mudança\n",
            self.published, self.unchanged
        ));
        if !self.edited.is_empty() {
            out.push_str(&format!(
                "recusado(s) por edição manual ({}): rode com --force para sobrescrever ou renomeie o arquivo\n  {}\n",
                self.edited.len(),
                self.edited.join(", ")
            ));
        }
        if !self.missing_drafts.is_empty() {
            out.push_str(&format!(
                "sem rascunho (rode kira digest): {}\n",
                self.missing_drafts.join(", ")
            ));
        }
        for note in &self.notes {
            out.push_str(&format!("nota: {note}\n"));
        }
        out.push_str(&format!("índice: {}\n", self.index_path));
        out
    }
}

/// Publica todo post entregue que tenha rascunho.
pub fn run(options: &PublishOptions) -> Result<PublishReport, PublishError> {
    let root = &options.root;
    let store = Store::open_default().map_err(|err| PublishError::Store(err.to_string()))?;
    let project_id = crate::store::project_id_for_root(root)
        .map_err(|err| PublishError::Store(err.to_string()))?;
    let posts = store
        .posts_of(&project_id)
        .map_err(|err| PublishError::Store(err.to_string()))?;

    let mut report = PublishReport::default();
    let mut index_rows: Vec<(String, String, String, String)> = Vec::new();

    for post in posts.iter().filter(|p| p.state == PostState::Delivered) {
        let draft = root
            .join(".kira")
            .join("drafts")
            .join(format!("{}.md", post.slug));
        let body = match std::fs::read_to_string(&draft) {
            Ok(body) => body,
            Err(_) => {
                report.missing_drafts.push(post.slug.clone());
                continue;
            }
        };
        let target = root
            .join(".kira")
            .join("wiki")
            .join(format!("{}.md", post.slug));
        let existing = std::fs::read_to_string(&target).ok();
        let published_hash = fnv1a64_hex(&body);

        let outcome = match &existing {
            None => PublishOutcome::Published,
            Some(current) => {
                let previous = front_matter_value(current, "published_hash");
                if previous.as_deref() == Some(published_hash.as_str()) {
                    PublishOutcome::Unchanged
                } else if previous.is_some() && !options.force {
                    // Publicamos este arquivo antes e o conteúdo mudou desde então:
                    // ou o dev editou, ou o digest gerou texto novo. O front-matter
                    // guarda o hash do corpo publicado, então dá para distinguir.
                    if body_hash_of(current).as_deref() == previous.as_deref() {
                        PublishOutcome::Published
                    } else {
                        PublishOutcome::Edited
                    }
                } else {
                    PublishOutcome::Published
                }
            }
        };

        match outcome {
            PublishOutcome::Published => {
                let text = with_front_matter(post, &body, &published_hash);
                write_text(&target, &text)?;
                report.published += 1;
            }
            PublishOutcome::Unchanged => report.unchanged += 1,
            PublishOutcome::Edited => report.edited.push(post.slug.clone()),
            PublishOutcome::NoDraft => {}
        }
        index_rows.push((
            post.slug.clone(),
            post.kind.clone(),
            post.lang.clone(),
            post.read_at.is_none().to_string(),
        ));
    }

    let index_path = root.join(".kira").join("wiki").join("index.md");
    let index = render_index(&index_rows, root);
    write_text(&index_path, &index)?;
    report.index_path = relative(root, &index_path);
    Ok(report)
}

fn with_front_matter(
    post: &crate::store::posts::PostRow,
    body: &str,
    published_hash: &str,
) -> String {
    format!(
        "---\nslug: {}\nkind: {}\nlang: {}\nmodel: {}\nprompt_version: {}\nsource_hash: {}\npublished_hash: {}\ncreated_at: {}\n---\n\n{}",
        post.slug,
        post.kind,
        post.lang,
        post.model,
        post.prompt_version,
        post.source_hash,
        published_hash,
        post.created_at,
        body
    )
}

/// Lê um campo do bloco `---` no topo do arquivo.
fn front_matter_value(text: &str, key: &str) -> Option<String> {
    let rest = text.strip_prefix(FRONT_MATTER_OPEN)?;
    let block = rest.split("\n---").next()?;
    for line in block.lines() {
        if let Some((k, v)) = line.split_once(':')
            && k.trim() == key
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

/// Hash do corpo do arquivo (tudo depois do front-matter).
fn body_hash_of(text: &str) -> Option<String> {
    let rest = text.strip_prefix(FRONT_MATTER_OPEN)?;
    let (_, body) = rest.split_once("\n---\n")?;
    Some(fnv1a64_hex(body.trim_start_matches('\n')))
}

/// Índice derivado: nenhuma chamada de modelo, ordem fixa por slug.
fn render_index(rows: &[(String, String, String, String)], root: &Path) -> String {
    let mut out = String::new();
    out.push_str("# Posts\n\n");
    out.push_str(
        "<!-- índice DERIVADO do conjunto de posts por `kira publish`. Não edite à mão. -->\n\n",
    );
    if rows.is_empty() {
        out.push_str(
            "_nenhum post publicado ainda — rode `kira digest` e depois `kira publish`_\n",
        );
        return out;
    }
    let mut sorted: BTreeMap<&str, (&str, &str, &str)> = BTreeMap::new();
    for (slug, kind, lang, unread) in rows {
        sorted.insert(slug, (kind, lang, unread));
    }
    out.push_str("| post | kind | idioma | não lido |\n|---|---|---|---|\n");
    for (slug, (kind, lang, unread)) in sorted {
        let mark = if unread == "true" { "•" } else { "" };
        out.push_str(&format!(
            "| [{slug}]({slug}.md) | {kind} | {lang} | {mark} |\n"
        ));
    }
    out.push('\n');
    out.push_str(&format!("_raiz: {}_\n", root.display()));
    out
}

fn write_text(path: &Path, body: &str) -> Result<(), PublishError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| PublishError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(path, body).map_err(|source| PublishError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rest| rest.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_round_trips_through_the_parser() {
        let text = "---\nslug: intro\npublished_hash: abc\n---\n\ncorpo\n";
        assert_eq!(front_matter_value(text, "slug").as_deref(), Some("intro"));
        assert_eq!(
            front_matter_value(text, "published_hash").as_deref(),
            Some("abc")
        );
        assert_eq!(front_matter_value(text, "nao_existe"), None);
    }

    #[test]
    fn body_hash_ignores_the_front_matter() {
        let a = "---\npublished_hash: 1\n---\n\ncorpo\n";
        let b = "---\npublished_hash: 2\n---\n\ncorpo\n";
        assert_eq!(body_hash_of(a), body_hash_of(b));
    }

    #[test]
    fn body_hash_changes_when_the_body_changes() {
        let a = "---\nx: 1\n---\n\ncorpo A\n";
        let b = "---\nx: 1\n---\n\ncorpo B\n";
        assert_ne!(body_hash_of(a), body_hash_of(b));
    }

    #[test]
    fn index_is_sorted_and_marks_unread() {
        let rows = vec![
            (
                "intro".to_string(),
                "intro".to_string(),
                "pt-BR".to_string(),
                "true".to_string(),
            ),
            (
                "c4-container".to_string(),
                "c4-container".to_string(),
                "pt-BR".to_string(),
                "false".to_string(),
            ),
        ];
        let text = render_index(&rows, Path::new("/repo"));
        let c4 = text.find("c4-container").unwrap();
        let intro = text.find("[intro]").unwrap();
        assert!(c4 < intro, "ordem alfabética por slug:\n{text}");
        assert!(text.contains("| • |"), "não lido marcado:\n{text}");
    }

    #[test]
    fn empty_index_says_what_to_run() {
        let text = render_index(&[], Path::new("/repo"));
        assert!(text.contains("kira digest"), "{text}");
        assert!(text.contains("kira publish"), "{text}");
    }
}
