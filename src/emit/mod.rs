//! `kira emit` — publica os rascunhos do `digest` no wiki (ADR-0038, M5).
//!
//! O `digest` escreve em `.kira/digest/`; este comando é o único que escreve em
//! `.kira/wiki/`, onde o dev lê. A regra que importa: **nunca sobrescrever o que o
//! dev escreveu**. Como o wiki é markdown versionável, a única forma honesta de
//! saber se um arquivo foi editado à mão é comparar o conteúdo com o que publicamos
//! da última vez — por isso cada post publicado carrega `published_hash` no
//! front-matter. Se o hash do arquivo atual não bate, o `emit` recusa e diz o que
//! fazer (`--force` é decisão explícita do dono). Arquivo que existe SEM o nosso
//! hash foi escrito do zero pelo dev: recusa igual, porta igual.
//!
//! O índice (`index.md`) é **derivado** do conjunto de posts: nunca é gerado por
//! modelo, senão mente quando a lista muda.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::guard::path::Guard;
use crate::post::plan::fnv1a64_hex;
use crate::store::Store;
use crate::store::posts::PostState;

/// Prefixo do bloco de front-matter que o `emit` gerencia.
const FRONT_MATTER_OPEN: &str = "---\n";

#[derive(Debug, thiserror::Error)]
pub enum EmitError {
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
    /// O guard de escrita recusou o destino (ADR-0011): travessia, deny-list ou
    /// confinamento na raiz. A mensagem do guard já é acionável, passa inteira.
    #[error(transparent)]
    Guard(#[from] crate::guard::path::GuardError),
}

/// Opções do `emit`.
#[derive(Debug, Clone)]
pub struct EmitOptions {
    pub root: PathBuf,
    /// Sobrescreve post editado à mão (decisão explícita do dono).
    pub force: bool,
}

impl EmitOptions {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            force: false,
        }
    }
}

/// O que aconteceu em cada post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitOutcome {
    Published,
    Unchanged,
    /// Recusado: o arquivo em `.kira/wiki/` foi editado à mão depois da
    /// publicação, ou foi escrito do zero pelo dev (sem o nosso `published_hash`).
    Edited,
}

#[derive(Debug, Clone, Default)]
pub struct EmitReport {
    pub published: usize,
    pub unchanged: usize,
    /// Caminhos relativos (POSIX) dos arquivos recusados: o dev precisa do nome
    /// do arquivo, não só do slug.
    pub edited: Vec<String>,
    pub missing_drafts: Vec<String>,
    pub index_path: String,
    pub notes: Vec<String>,
}

impl EmitReport {
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "emit: {} publicado(s), {} sem mudança\n",
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
pub fn run(options: &EmitOptions) -> Result<EmitReport, EmitError> {
    let root = &options.root;
    let store = Store::open_default().map_err(|err| EmitError::Store(err.to_string()))?;
    let project_id =
        crate::store::project_id_for_root(root).map_err(|err| EmitError::Store(err.to_string()))?;
    let posts = store
        .posts_of(&project_id)
        .map_err(|err| EmitError::Store(err.to_string()))?;

    let mut report = EmitReport::default();
    let mut index_rows: Vec<(String, String, String, String)> = Vec::new();
    // Toda escrita do emit passa pelo guard (ADR-0011): confinamento na raiz e
    // deny-list, decididos antes de tocar o disco.
    let guard = Guard::new(root)?;

    for post in posts.iter().filter(|p| p.state == PostState::Delivered) {
        let draft = root
            .join(".kira")
            .join("digest")
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

        let outcome = decide_outcome(existing.as_deref(), &published_hash, options.force);

        match outcome {
            EmitOutcome::Published => {
                let text = with_front_matter(post, &body, &published_hash);
                write_text(&guard, &target, &text)?;
                report.published += 1;
            }
            EmitOutcome::Unchanged => report.unchanged += 1,
            EmitOutcome::Edited => report.edited.push(relative(root, &target)),
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
    write_text(&guard, &index_path, &index)?;
    report.index_path = relative(root, &index_path);
    Ok(report)
}

/// Decisão pura sobre um arquivo que já existe (ou não) no wiki. Três portas:
/// - `--force` ⇒ publica (decisão explícita do dono);
/// - o corpo no disco bate com o que publicamos ⇒ seguro republicar (só o
///   rascunho mudou) ou nada a fazer (rascunho idêntico);
/// - qualquer divergência de texto do dev ⇒ recusa, nomeando o arquivo.
///
/// O caso que o gate antigo deixava passar: arquivo SEM o nosso
/// `published_hash` foi escrito do zero pelo dev — sem `--force`, é recusa.
fn decide_outcome(existing: Option<&str>, published_hash: &str, force: bool) -> EmitOutcome {
    let Some(current) = existing else {
        return EmitOutcome::Published;
    };
    let previous = front_matter_value(current, "published_hash");
    if previous.as_deref() == Some(published_hash) {
        return EmitOutcome::Unchanged;
    }
    if force {
        return EmitOutcome::Published;
    }
    if previous.is_none() {
        return EmitOutcome::Edited;
    }
    if body_hash_of(current).as_deref() == previous.as_deref() {
        EmitOutcome::Published
    } else {
        EmitOutcome::Edited
    }
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
        "<!-- índice DERIVADO do conjunto de posts por `kira emit`. Não edite à mão. -->\n\n",
    );
    if rows.is_empty() {
        out.push_str("_nenhum post publicado ainda — rode `kira digest` e depois `kira emit`_\n");
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

fn write_text(guard: &Guard, path: &Path, body: &str) -> Result<(), EmitError> {
    // O guard decide antes de tocar o disco: travessia e deny-list são recusados
    // mesmo sem o pai existir. Se a única reclamação for o pai ausente, o
    // diretório de artefato é criado e a checagem se repete — o destino final
    // escrito é sempre o canônico devolvido pelo guard.
    let destination = match guard.check_write(path) {
        Ok(destination) => destination,
        Err(crate::guard::path::GuardError::UnresolvableParent { .. }) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|source| EmitError::Write {
                    path: parent.display().to_string(),
                    source,
                })?;
            }
            guard.check_write(path)?
        }
        Err(err) => return Err(err.into()),
    };
    std::fs::write(&destination, body).map_err(|source| EmitError::Write {
        path: destination.display().to_string(),
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
    fn a_file_without_our_hash_is_never_overwritten_without_force() {
        // Regression do aceite M5: arquivo escrito do zero pelo dev (sem
        // front-matter, ou sem `published_hash`) era sobrescrito calado.
        // `--force` é a única porta.
        assert_eq!(
            decide_outcome(Some("# escrito pelo dev\n"), "abc", false),
            EmitOutcome::Edited
        );
        assert_eq!(
            decide_outcome(
                Some("---\nslug: intro\n---\n\ntexto do dev\n"),
                "abc",
                false
            ),
            EmitOutcome::Edited
        );
        assert_eq!(
            decide_outcome(Some("---\nslug: intro\n---\n\ntexto do dev\n"), "abc", true),
            EmitOutcome::Published
        );
    }

    #[test]
    fn our_untouched_publication_is_republished_when_only_the_draft_changed() {
        let old_body = "corpo publicado\n";
        let old_hash = fnv1a64_hex(old_body);
        let file = format!("---\npublished_hash: {old_hash}\n---\n\n{old_body}");
        // Rascunho novo por cima de corpo intacto: republica.
        assert_eq!(
            decide_outcome(Some(&file), "hash-do-rascunho-novo", false),
            EmitOutcome::Published
        );
        // Rascunho idêntico ao publicado: nada a fazer.
        assert_eq!(
            decide_outcome(Some(&file), &old_hash, false),
            EmitOutcome::Unchanged
        );
        // Corpo mexido à mão: recusa, mesmo com front-matter nosso.
        let edited = format!("---\npublished_hash: {old_hash}\n---\n\ncorpo mexido\n");
        assert_eq!(
            decide_outcome(Some(&edited), "novo", false),
            EmitOutcome::Edited
        );
        // Arquivo ausente é só publicar.
        assert_eq!(decide_outcome(None, "abc", false), EmitOutcome::Published);
    }

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
        assert!(text.contains("kira emit"), "{text}");
    }
}
