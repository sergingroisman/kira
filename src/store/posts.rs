//! `posts` — a fila que o `Courier` consome (ADR-0038, M2).
//!
//! A unidade de trabalho **é** o artigo: uma linha em `posts` guarda identidade
//! (`slug`), procedência (`source_hash`, `prompt_version`, `model`, `lang`), estado e
//! o lease (`postage_until`, o "selo"). A view `outbox` é só a leitura dos pendentes.
//!
//! Por que tabela e não broker: o Kira já embarca SQLite `bundled`, e o que a fila
//! precisa (retomar, dormir, não duplicar trabalho) é `SELECT`/`UPDATE` com lease.
//! Redis/Postgres furariam o local-first (ADR-0030) e não rodam nativos no Windows.
//!
//! Garantias provadas pelos testes deste arquivo:
//! - dois `claim` concorrentes **não** pegam a mesma linha;
//! - lease vencida devolve a linha à fila sem código de recuperação;
//! - a mesma identidade + mesmo `source_hash` **não** reenfileira (idempotência);
//! - `attempts` >= 3 vira `dead` (carta extraviada), não loop infinito.

use rusqlite::{OptionalExtension, params};

use super::{Store, StoreError, now_rfc3339_utc};

/// Estado da carta no correio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostState {
    /// Selada, esperando retirada (aparece na view `outbox`).
    Sealed,
    /// Retirada por um `Courier` até `postage_until`.
    Picked,
    /// Entregue: o `.md` existe no destino.
    Delivered,
    /// Falhou 3 vezes: carta extraviada.
    Dead,
}

impl PostState {
    pub fn as_str(self) -> &'static str {
        match self {
            PostState::Sealed => "sealed",
            PostState::Picked => "picked",
            PostState::Delivered => "delivered",
            PostState::Dead => "dead",
        }
    }

    fn from_stored(text: &str) -> Self {
        match text {
            "picked" => PostState::Picked,
            "delivered" => PostState::Delivered,
            "dead" => PostState::Dead,
            _ => PostState::Sealed,
        }
    }
}

/// Tentativas antes de a carta ser declarada extraviada.
pub const MAX_ATTEMPTS: i64 = 3;

/// Uma linha de `posts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostRow {
    pub id: String,
    pub project_id: String,
    pub slug: String,
    pub kind: String,
    pub lang: String,
    pub source_hash: String,
    pub prompt_version: String,
    pub model: String,
    pub state: PostState,
    /// Epoch em que o lease vence (`None` = livre).
    pub postage_until: Option<i64>,
    pub attempts: i64,
    /// Quando o dev leu no dashboard (formato RFC3339). `None` = não lido.
    pub read_at: Option<String>,
    /// Destino do `.md` publicado (`None` antes do `emit`).
    pub path: Option<String>,
    pub created_at: String,
}

/// O que se sabe ao enfileirar um post.
#[derive(Debug, Clone)]
pub struct NewPost {
    pub id: String,
    pub slug: String,
    pub kind: String,
    pub lang: String,
    pub source_hash: String,
    pub prompt_version: String,
    pub model: String,
}

/// Resultado de um `post_enqueue`: distingue trabalho novo de repetição.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueOutcome {
    /// Linha nova, pronta para retirada.
    New,
    /// Já existia com a mesma procedência: nada a fazer (idempotência).
    Unchanged,
    /// Existia, mas o repositório/prompt/modelo/idioma mudou: voltou para a fila.
    Requeued,
}

impl Store {
    /// Enfileira (ou atualiza) um post, de forma idempotente.
    ///
    /// A chave é `(project_id, slug)`. Se a procedência completa
    /// (`source_hash`, `prompt_version`, `model`, `lang`) é a mesma, a linha fica
    /// como está — inclusive se já foi entregue. Qualquer mudança na procedência
    /// devolve a linha para `sealed` com `attempts` zerado.
    pub fn post_enqueue(
        &self,
        project_id: &str,
        post: &NewPost,
    ) -> Result<EnqueueOutcome, StoreError> {
        let existing = self
            .conn
            .query_row(
                "SELECT id, source_hash, prompt_version, model, lang FROM posts \
             WHERE project_id = ?1 AND slug = ?2",
                params![project_id, post.slug],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(StoreError::Sql)?;

        match existing {
            None => {
                self.conn
                    .execute(
                        "INSERT INTO posts (id, project_id, slug, kind, lang, source_hash, \
                         prompt_version, model, state, postage_until, attempts, read_at, path, \
                         created_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'sealed', NULL, 0, NULL, NULL, ?9)",
                        params![
                            post.id,
                            project_id,
                            post.slug,
                            post.kind,
                            post.lang,
                            post.source_hash,
                            post.prompt_version,
                            post.model,
                            now_rfc3339_utc(),
                        ],
                    )
                    .map_err(StoreError::Sql)?;
                Ok(EnqueueOutcome::New)
            }
            Some((id, source_hash, prompt_version, model, lang)) => {
                let unchanged = source_hash == post.source_hash
                    && prompt_version == post.prompt_version
                    && model == post.model
                    && lang == post.lang;
                if unchanged {
                    return Ok(EnqueueOutcome::Unchanged);
                }
                self.conn
                    .execute(
                        "UPDATE posts SET source_hash = ?2, prompt_version = ?3, model = ?4, \
                         lang = ?5, kind = ?6, state = 'sealed', postage_until = NULL, \
                         attempts = 0, path = NULL WHERE id = ?1",
                        params![
                            id,
                            post.source_hash,
                            post.prompt_version,
                            post.model,
                            post.lang,
                            post.kind,
                        ],
                    )
                    .map_err(StoreError::Sql)?;
                Ok(EnqueueOutcome::Requeued)
            }
        }
    }

    /// Retira a próxima carta disponível e sela com lease até `now + ttl`.
    ///
    /// Uma única instrução atômica: o `UPDATE` com subselect não pode entregar a
    /// mesma linha para dois chamadores, mesmo sem transação explícita — SQLite
    /// serializa a escrita.
    pub fn post_claim(
        &self,
        project_id: &str,
        now: i64,
        ttl_seconds: i64,
    ) -> Result<Option<PostRow>, StoreError> {
        let claimed: Option<String> = self
            .conn
            .query_row(
                "UPDATE posts SET state = 'picked', postage_until = ?2, attempts = attempts + 1 \
                 WHERE id = ( \
                     SELECT id FROM posts \
                     WHERE project_id = ?1 \
                       AND ( state = 'sealed' \
                             OR (state = 'picked' AND postage_until IS NOT NULL \
                                 AND postage_until < ?3) ) \
                     ORDER BY created_at, slug LIMIT 1 \
                 ) \
                 RETURNING id",
                params![project_id, now + ttl_seconds, now],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::Sql)?;

        let Some(id) = claimed else {
            return Ok(None);
        };
        self.post_by_id(&id)
    }

    /// Marca entrega concluída (o `.md` foi escrito em `path`).
    pub fn post_deliver(&self, id: &str, path: &str) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE posts SET state = 'delivered', postage_until = NULL, path = ?2, \
                 read_at = NULL WHERE id = ?1",
                params![id, path],
            )
            .map_err(StoreError::Sql)?;
        Ok(())
    }

    /// Registra falha: volta para a fila ou vira carta extraviada.
    ///
    /// Devolve o estado resultante para o chamador poder reportar.
    pub fn post_fail(&self, id: &str) -> Result<PostState, StoreError> {
        let attempts: i64 = self
            .conn
            .query_row(
                "SELECT attempts FROM posts WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(StoreError::Sql)?;
        let next = if attempts >= MAX_ATTEMPTS {
            PostState::Dead
        } else {
            PostState::Sealed
        };
        self.conn
            .execute(
                "UPDATE posts SET state = ?2, postage_until = NULL WHERE id = ?1",
                params![id, next.as_str()],
            )
            .map_err(StoreError::Sql)?;
        Ok(next)
    }

    /// Devolve à fila tudo que está `picked` — é o `--pause` do `Courier`.
    pub fn post_release_all(&self) -> Result<usize, StoreError> {
        self.conn
            .execute(
                "UPDATE posts SET state = 'sealed', postage_until = NULL WHERE state = 'picked'",
                [],
            )
            .map_err(StoreError::Sql)
    }

    /// Marca o post como lido (caixa de correio do dashboard).
    pub fn post_mark_read(&self, id: &str) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE posts SET read_at = ?2 WHERE id = ?1 AND read_at IS NULL",
                params![id, now_rfc3339_utc()],
            )
            .map_err(StoreError::Sql)?;
        Ok(())
    }

    /// Posts do projeto, entregues primeiro e mais novos antes (leitura humana).
    pub fn posts_of(&self, project_id: &str) -> Result<Vec<PostRow>, StoreError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, project_id, slug, kind, lang, source_hash, prompt_version, model, \
                 state, postage_until, attempts, read_at, path, created_at \
                 FROM posts WHERE project_id = ?1 ORDER BY created_at DESC, slug",
            )
            .map_err(StoreError::Sql)?;
        let rows = statement
            .query_map(params![project_id], map_row)
            .map_err(StoreError::Sql)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(StoreError::Sql)?);
        }
        Ok(out)
    }

    /// Quantos posts entregues o dev ainda não leu (o "não lido" da caixa).
    pub fn posts_unread(&self, project_id: &str) -> Result<usize, StoreError> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM posts WHERE project_id = ?1 \
                 AND state = 'delivered' AND read_at IS NULL",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(StoreError::Sql)?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    fn post_by_id(&self, id: &str) -> Result<Option<PostRow>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, project_id, slug, kind, lang, source_hash, prompt_version, model, \
                 state, postage_until, attempts, read_at, path, created_at \
                 FROM posts WHERE id = ?1",
                params![id],
                map_row,
            )
            .optional()
            .map_err(StoreError::Sql)
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PostRow> {
    let state: String = row.get(8)?;
    Ok(PostRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        slug: row.get(2)?,
        kind: row.get(3)?,
        lang: row.get(4)?,
        source_hash: row.get(5)?,
        prompt_version: row.get(6)?,
        model: row.get(7)?,
        state: PostState::from_stored(&state),
        postage_until: row.get(9)?,
        attempts: row.get(10)?,
        read_at: row.get(11)?,
        path: row.get(12)?,
        created_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(tag: &str) -> (std::path::PathBuf, Store, String) {
        let dir = std::env::temp_dir().join(format!("kira-posts-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");
        let store = Store::open_in_dir(&dir).expect("store");
        let project = store.ensure_project(&dir).expect("projeto").id;
        (dir, store, project)
    }

    fn post(slug: &str, hash: &str) -> NewPost {
        NewPost {
            id: format!("id-{slug}-{hash}"),
            slug: slug.to_string(),
            kind: "intro".to_string(),
            lang: "pt-BR".to_string(),
            source_hash: hash.to_string(),
            prompt_version: "v1".to_string(),
            model: "ollama:qwen".to_string(),
        }
    }

    #[test]
    fn enqueue_is_idempotent_by_provenance() {
        let (dir, store, project) = sandbox("idem");
        assert_eq!(
            store.post_enqueue(&project, &post("intro", "h1")).unwrap(),
            EnqueueOutcome::New
        );
        assert_eq!(
            store.post_enqueue(&project, &post("intro", "h1")).unwrap(),
            EnqueueOutcome::Unchanged
        );
        assert_eq!(store.posts_of(&project).unwrap().len(), 1, "não duplica");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_provenance_requeues_even_after_delivery() {
        let (dir, store, project) = sandbox("requeue");
        store.post_enqueue(&project, &post("intro", "h1")).unwrap();
        let claimed = store
            .post_claim(&project, 1000, 60)
            .unwrap()
            .expect("claim");
        store
            .post_deliver(&claimed.id, ".kira/wiki/intro.md")
            .unwrap();
        // Mesmo hash: nada muda, continua entregue.
        assert_eq!(
            store.post_enqueue(&project, &post("intro", "h1")).unwrap(),
            EnqueueOutcome::Unchanged
        );
        assert_eq!(
            store.posts_of(&project).unwrap()[0].state,
            PostState::Delivered
        );
        // Hash novo (repositório mudou): volta para a fila e zera tentativas.
        assert_eq!(
            store.post_enqueue(&project, &post("intro", "h2")).unwrap(),
            EnqueueOutcome::Requeued
        );
        let row = &store.posts_of(&project).unwrap()[0];
        assert_eq!(row.state, PostState::Sealed);
        assert_eq!(row.attempts, 0);
        assert_eq!(row.path, None, "destino antigo não sobrevive ao requeue");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_claims_never_take_the_same_post() {
        let (dir, store, project) = sandbox("claim");
        store.post_enqueue(&project, &post("a", "h")).unwrap();
        store.post_enqueue(&project, &post("b", "h")).unwrap();
        let first = store
            .post_claim(&project, 1000, 60)
            .unwrap()
            .expect("primeiro claim");
        let second = store
            .post_claim(&project, 1000, 60)
            .unwrap()
            .expect("segundo claim");
        assert_ne!(
            first.slug, second.slug,
            "cada claim pega uma carta distinta"
        );
        assert!(
            store.post_claim(&project, 1000, 60).unwrap().is_none(),
            "fila vazia"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_expired_lease_returns_the_post_to_the_queue() {
        let (dir, store, project) = sandbox("lease");
        store.post_enqueue(&project, &post("a", "h")).unwrap();
        let claimed = store
            .post_claim(&project, 1000, 60)
            .unwrap()
            .expect("claim");
        assert_eq!(claimed.state, PostState::Picked);
        assert_eq!(claimed.postage_until, Some(1060));
        // Antes de vencer: ninguém pega.
        assert!(store.post_claim(&project, 1059, 60).unwrap().is_none());
        // Depois de vencer: a mesma carta volta, com a tentativa contada.
        let again = store
            .post_claim(&project, 1061, 60)
            .unwrap()
            .expect("vencido volta");
        assert_eq!(again.id, claimed.id);
        assert_eq!(again.attempts, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn three_failures_turn_the_post_into_a_dead_letter() {
        let (dir, store, project) = sandbox("dead");
        store.post_enqueue(&project, &post("a", "h")).unwrap();
        let mut state = PostState::Sealed;
        for attempt in 1..=MAX_ATTEMPTS {
            let claimed = store
                .post_claim(&project, attempt * 1000, 60)
                .unwrap()
                .expect("claim");
            state = store.post_fail(&claimed.id).unwrap();
            if attempt < MAX_ATTEMPTS {
                assert_eq!(
                    state,
                    PostState::Sealed,
                    "tentativa {attempt} volta para a fila"
                );
            }
        }
        assert_eq!(state, PostState::Dead, "3 falhas = carta extraviada");
        assert!(
            store.post_claim(&project, 99_000, 60).unwrap().is_none(),
            "extraviada sai da fila"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pause_releases_everything_picked() {
        let (dir, store, project) = sandbox("pause");
        store.post_enqueue(&project, &post("a", "h")).unwrap();
        store.post_enqueue(&project, &post("b", "h")).unwrap();
        store.post_claim(&project, 1000, 60).unwrap();
        store.post_claim(&project, 1000, 60).unwrap();
        assert_eq!(store.post_release_all().unwrap(), 2, "as duas voltam");
        assert_eq!(
            store
                .post_claim(&project, 1000, 60)
                .unwrap()
                .unwrap()
                .attempts,
            2
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unread_counts_only_delivered_and_unread() {
        let (dir, store, project) = sandbox("unread");
        store.post_enqueue(&project, &post("a", "h")).unwrap();
        store.post_enqueue(&project, &post("b", "h")).unwrap();
        assert_eq!(
            store.posts_unread(&project).unwrap(),
            0,
            "nada entregue ainda"
        );
        let first = store.post_claim(&project, 1000, 60).unwrap().unwrap();
        store.post_deliver(&first.id, ".kira/wiki/a.md").unwrap();
        assert_eq!(store.posts_unread(&project).unwrap(), 1);
        store.post_mark_read(&first.id).unwrap();
        assert_eq!(store.posts_unread(&project).unwrap(), 0, "lido não conta");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn outbox_view_only_exposes_sealed_posts() {
        let (dir, store, project) = sandbox("view");
        store.post_enqueue(&project, &post("a", "h")).unwrap();
        store.post_enqueue(&project, &post("b", "h")).unwrap();
        let claimed = store.post_claim(&project, 1000, 60).unwrap().unwrap();
        let sealed: i64 = store
            .conn
            .query_row(
                "SELECT count(*) FROM outbox WHERE project_id = ?1",
                params![project],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sealed, 1, "a retirada não aparece na caixa de saída");
        store.post_deliver(&claimed.id, ".kira/wiki/a.md").unwrap();
        let sealed: i64 = store
            .conn
            .query_row(
                "SELECT count(*) FROM outbox WHERE project_id = ?1",
                params![project],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sealed, 1, "entregue também não");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
