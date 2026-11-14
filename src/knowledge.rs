//! `kira knowledge` — os textos do dono (onda SK, SK-2).
//!
//! Fonte de verdade = markdown no disco em `<store_dir>/knowledge/*.md`
//! (normalmente `~/.config/kira/knowledge/`). O índice `knowledge`/
//! `knowledge_fts` no store é DERIVADO e descartável: reconstruir nunca perde
//! conhecimento. Todo texto passa por `crate::redact` ANTES de entrar no
//! índice — segredo nunca chega ao store nem ao prompt. O diretório é do
//! dono: leitura livre, escrita NUNCA (nada aqui cria, edita ou apaga
//! arquivo). Escopo global: o conhecimento vale para qualquer projeto.

use std::path::{Path, PathBuf};

use crate::store::{KnowledgeHit, KnowledgeRow, Store};

/// Um documento do dono já lido e redigido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeDoc {
    /// Stem do arquivo (âncora estável na saída).
    pub id: String,
    /// Primeira linha `# ...` (ou o stem quando o texto não tem cabeçalho).
    pub title: String,
    /// Texto completo já redigido.
    pub body: String,
}

/// Erros do conhecimento — todos acionáveis.
#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error(
        "sem diretório para o store — defina HOME (ou USERPROFILE no Windows) ou a variável KIRA_STORE_DIR"
    )]
    NoStoreDir,
    #[error("store indisponível: {0}")]
    Store(String),
}

/// Diretório do conhecimento do dono: `<store_dir>/knowledge/` — segue
/// `KIRA_STORE_DIR`, então teste e execução portável nunca tocam o HOME real.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    crate::store::store_dir().map(|store| store.join("knowledge"))
}

/// Lê `*.md` do diretório do dono (ordem alfabética — determinismo já na
/// fonte). Arquivo ilegível vira nota (fail-open), nunca erro do comando;
/// sem diretório = sem conhecimento (não é erro).
#[must_use]
pub fn load() -> (Vec<KnowledgeDoc>, Vec<String>) {
    match dir() {
        Some(dir) => load_from(&dir),
        None => (Vec::new(), vec![KnowledgeError::NoStoreDir.to_string()]),
    }
}

/// Leitura real de um diretório (seam para testes sem tocar env).
#[must_use]
pub fn load_from(dir: &Path) -> (Vec<KnowledgeDoc>, Vec<String>) {
    let mut docs = Vec::new();
    let mut notes = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (docs, notes); // sem diretório = sem conhecimento do dono
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();
    for path in paths {
        // Stem não-UTF-8 não tem âncora estável: vira nota, nunca pânico.
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            notes.push(format!(
                "arquivo com nome não-UTF-8 ignorado: {}",
                path.display()
            ));
            continue;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                // CRLF do Windows não pode virar lixo no índice: normaliza.
                let text = text.replace("\r\n", "\n");
                docs.push(KnowledgeDoc {
                    id: name.to_string(),
                    title: crate::redact::redact(&title_of(&text, name)),
                    body: crate::redact::redact(text.trim()),
                });
            }
            Err(source) => notes.push(format!("arquivo ilegível em {}: {source}", path.display())),
        }
    }
    (docs, notes)
}

/// Título do documento: a primeira linha `# ...` (ou o nome do arquivo quando
/// o texto não tem cabeçalho — determinístico dos dois jeitos).
fn title_of(text: &str, fallback: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map_or_else(|| fallback.to_string(), ToOwned::to_owned)
}

/// Reconstrói o índice derivado do ZERO (delete-then-insert no store): o
/// disco manda, então arquivo que saiu sai do índice também.
pub fn reindex(store: &Store) -> Result<usize, KnowledgeError> {
    let dir = dir().ok_or(KnowledgeError::NoStoreDir)?;
    reindex_from(&dir, store)
}

/// Reindex real (seam para testes sem tocar env).
pub fn reindex_from(dir: &Path, store: &Store) -> Result<usize, KnowledgeError> {
    let (docs, _notes) = load_from(dir);
    let rows: Vec<KnowledgeRow> = docs
        .into_iter()
        .map(|doc| KnowledgeRow {
            id: doc.id,
            title: doc.title,
            body: doc.body,
        })
        .collect();
    store
        .knowledge_replace(&rows)
        .map_err(|err| KnowledgeError::Store(err.to_string()))
}

/// Busca no índice derivado, top-k determinístico.
pub fn search(
    store: &Store,
    query: &str,
    limit: usize,
) -> Result<Vec<KnowledgeHit>, KnowledgeError> {
    store
        .knowledge_search(query, limit)
        .map_err(|err| KnowledgeError::Store(err.to_string()))
}

/// Listagem: uma linha por documento — âncora `knowledge:<id>` + título +
/// tamanho do corpo. Notas de arquivo ilegível entram como linha própria
/// (nunca silêncio); catálogo vazio devolve a dica acionável.
#[must_use]
pub fn list() -> String {
    let (docs, notes) = load();
    let mut out = String::new();
    for doc in &docs {
        out.push_str(&format!(
            "knowledge:{}  {}  ({} bytes)\n",
            doc.id,
            doc.title,
            doc.body.len()
        ));
    }
    for note in &notes {
        out.push_str(&format!("nota: {note}\n"));
    }
    if docs.is_empty() && notes.is_empty() {
        let path = dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(|| "<store_dir>/knowledge".to_string());
        out.push_str(&format!(
            "nenhum texto em {path} — crie o diretório, coloque arquivos *.md e rode `kira knowledge reindex`\n"
        ));
    }
    out
}

/// Render de um hit: âncora `knowledge:<id>` + título + trecho.
#[must_use]
pub fn render_hits(hits: &[KnowledgeHit]) -> String {
    let mut out = String::new();
    for hit in hits {
        out.push_str(&format!(
            "knowledge:{}  [{}]  {}\n",
            hit.id, hit.title, hit.snippet
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Diretório temporário único por teste (sem depender de env — `env::set_var`
    // é unsafe na edition 2024 e o store aqui é aberto direto pelo caminho).
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-knowledge-unit-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar diretório de teste");
        dir
    }

    #[test]
    fn title_comes_from_the_first_heading_or_falls_back_to_the_stem() {
        assert_eq!(title_of("# PBM\n\n- regra\n", "arquivo"), "PBM");
        assert_eq!(title_of("## só subtítulo\n", "arquivo"), "arquivo");
        assert_eq!(title_of("", "arquivo"), "arquivo");
        assert_eq!(title_of("#   \n\n corpo\n", "arquivo"), "arquivo");
    }

    #[test]
    fn load_is_sorted_deterministic_and_redacted() {
        let dir = temp_dir("ordenado");
        std::fs::write(dir.join("zz-ultimo.md"), "# Último\n- conteúdo z\n").expect("escrever");
        std::fs::write(
            dir.join("aa-primeiro.md"),
            "# Primeiro\n- configuração: token=AbC123XyZ789QqQ\n",
        )
        .expect("escrever");
        std::fs::write(dir.join("meio.txt"), "não é md, fica fora\n").expect("escrever");

        let (docs, notes) = load_from(&dir);
        assert!(notes.is_empty(), "{notes:?}");
        let ids: Vec<&str> = docs.iter().map(|doc| doc.id.as_str()).collect();
        assert_eq!(
            ids,
            ["aa-primeiro", "zz-ultimo"],
            "ordem alfabética: {ids:?}"
        );
        assert_eq!(docs[0].title, "Primeiro");
        assert!(
            !docs[0].body.contains("AbC123XyZ789QqQ"),
            "segredo vazou para o índice: {}",
            docs[0].body
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_directory_is_not_an_error_and_unreadable_file_is_a_note() {
        let dir = temp_dir("ausente");
        std::fs::remove_dir(&dir).expect("tirar o diretório vazio");
        let (docs, notes) = load_from(&dir);
        assert!(docs.is_empty());
        assert!(notes.is_empty(), "sem diretório não é erro: {notes:?}");

        let dir = temp_dir("ilegivel");
        std::fs::write(dir.join("quebrado.md"), "conteúdo\n").expect("escrever");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(dir.join("quebrado.md"))
                .expect("metadata")
                .permissions();
            perms.set_mode(0o000);
            std::fs::set_permissions(dir.join("quebrado.md"), perms).expect("permissão 000");
        }
        let (docs, notes) = load_from(&dir);
        assert!(docs.is_empty());
        // No Windows não há chmod 000 efetivo, então a nota só é exigível no
        // unix; a leitura ter batido não é defeito ali, é semântica do FS.
        #[cfg(unix)]
        assert!(
            notes.len() == 1 && notes[0].contains("ilegível"),
            "falha de leitura vira nota: {notes:?}"
        );
        #[cfg(not(unix))]
        let _ = notes;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reindex_from_scratch_and_search_round_trip() {
        let store_dir = temp_dir("store");
        let store = Store::open_in_dir(&store_dir).expect("store de teste");
        assert!(store.knowledge_fts5_enabled(), "FTS5 presente no bundled");

        let knowledge_dir = store_dir.join("knowledge");
        std::fs::create_dir_all(&knowledge_dir).expect("criar knowledge");
        std::fs::write(
            knowledge_dir.join("pbm.md"),
            "# Pague Menos PBM\n\n- backend .NET 10 no cliente\n",
        )
        .expect("escrever");
        std::fs::write(knowledge_dir.join("viajem.md"), "texto sem cabeçalho\n").expect("escrever");

        let count = reindex_from(&knowledge_dir, &store).expect("reindex");
        assert_eq!(count, 2, "os dois documentos entraram");

        let hits = search(&store, "backend", 5).expect("busca");
        assert_eq!(hits.len(), 1, "um hit: {:?}", hits);
        assert_eq!(hits[0].id, "pbm", "âncora é o stem do arquivo");
        assert_eq!(hits[0].title, "Pague Menos PBM");
        assert!(hits[0].snippet.contains("backend"), "{:?}", hits[0].snippet);

        // Reindex do zero: arquivo que saiu do disco sai do índice também.
        std::fs::remove_file(knowledge_dir.join("pbm.md")).expect("remover");
        let count = reindex_from(&knowledge_dir, &store).expect("reindex 2");
        assert_eq!(count, 1, "só o que resta no disco");
        let hits = search(&store, "backend", 5).expect("busca 2");
        assert!(
            hits.is_empty(),
            "documento removido não pode casar: {hits:?}"
        );

        let _ = std::fs::remove_dir_all(&store_dir);
    }

    #[test]
    fn empty_query_returns_nothing_and_determinism_holds() {
        let store_dir = temp_dir("vazio");
        let store = Store::open_in_dir(&store_dir).expect("store de teste");
        let hits = search(&store, "   ", 5).expect("busca vazia");
        assert!(hits.is_empty(), "consulta vazia nunca casa nada");

        let knowledge_dir = store_dir.join("knowledge");
        std::fs::create_dir_all(&knowledge_dir).expect("criar knowledge");
        std::fs::write(knowledge_dir.join("a.md"), "# A\n- digitação\n").expect("escrever");
        reindex_from(&knowledge_dir, &store).expect("reindex");
        let first = search(&store, "digitação", 5).expect("busca");
        let second = search(&store, "digitação", 5).expect("busca 2");
        assert_eq!(first, second, "mesmo estado = mesmo resultado");
        let _ = std::fs::remove_dir_all(&store_dir);
    }
}
