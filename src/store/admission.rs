//! Gate de admissão de modelo (HF-5, ADR-0040) — ESTADO no store.
//!
//! Nenhuma tag entra no routing de especialistas sem: proveniência
//! conhecida, SHA-256 conferido contra o model card, template conferido e
//! probes rodados NO GGUF quantizado. Cada admissão é uma linha aqui (uma
//! por tag): é estado, não dado derivado — reconstruir o índice/migração
//! nunca apaga admissão; o routing só consulta (nunca escreve).

use super::{Store, StoreError};

/// Uma admissão registrada: a tag e a evidência do gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRow {
    /// Tag do modelo (`qwen3.5:4b`, `hf.co/bartowski/...:Q4_K_M`).
    pub tag: String,
    /// Segmento de quant conferido contra o piso (`Q4_K_M`), vazio em tag oficial.
    pub quant: String,
    /// Proveniência registrada (`official`, `bartowski`, `unsloth`).
    pub provenance: String,
    /// SHA-256 conferido pelo dono contra o model card (hex, 64 chars).
    pub sha256: String,
    /// FNV-1a 64 (hex) do template devolvido pelo `ollama show --template`.
    pub template_hash: String,
    /// Probes de segurança rodados no GGUF quantizado (`security_probe` +
    /// `--eval` do kind) — atestado do dono.
    pub probes_ok: bool,
    /// Momento da admissão (RFC 3339 UTC — padrão do store).
    pub admitted_at: String,
}

impl Store {
    /// Registra (ou reconfirma) a admissão de uma tag. O `model admit` valida
    /// o gate antes de chamar; o store grava exatamente o que chegou.
    pub fn model_admission_put(&self, row: &AdmissionRow) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO model_admission \
                 (tag, quant, provenance, sha256, template_hash, probes_ok, admitted_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(tag) DO UPDATE SET \
                 quant = excluded.quant, provenance = excluded.provenance, \
                 sha256 = excluded.sha256, template_hash = excluded.template_hash, \
                 probes_ok = excluded.probes_ok, admitted_at = excluded.admitted_at",
                rusqlite::params![
                    row.tag,
                    row.quant,
                    row.provenance,
                    row.sha256,
                    row.template_hash,
                    i64::from(row.probes_ok),
                    row.admitted_at,
                ],
            )
            .map_err(StoreError::Sql)?;
        Ok(())
    }

    /// A admissão registrada de uma tag; `None` = nunca admitida (e o
    /// routing, então, não a usa).
    pub fn model_admission_get(&self, tag: &str) -> Result<Option<AdmissionRow>, StoreError> {
        self.conn
            .query_row(
                "SELECT tag, quant, provenance, sha256, template_hash, probes_ok, \
                 admitted_at FROM model_admission WHERE tag = ?1",
                rusqlite::params![tag],
                |row| {
                    Ok(AdmissionRow {
                        tag: row.get(0)?,
                        quant: row.get(1)?,
                        provenance: row.get(2)?,
                        sha256: row.get(3)?,
                        template_hash: row.get(4)?,
                        probes_ok: row.get::<_, i64>(5)? != 0,
                        admitted_at: row.get(6)?,
                    })
                },
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(StoreError::Sql(other)),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let dir = std::env::temp_dir().join(format!(
            "kira-admission-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        Store::open_at(&dir.join("store.db")).expect("store")
    }

    fn row(tag: &str) -> AdmissionRow {
        AdmissionRow {
            tag: tag.to_string(),
            quant: "Q4_K_M".to_string(),
            provenance: "bartowski".to_string(),
            sha256: "a".repeat(64),
            template_hash: "abc123".to_string(),
            probes_ok: true,
            admitted_at: "2026-11-14T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn admission_roundtrip_and_get_unknown_is_none() {
        let store = store();
        assert!(
            store
                .model_admission_get("hf.co/bartowski/X:Q4_K_M")
                .expect("get")
                .is_none(),
            "sem admissão gravada, a consulta é None"
        );
        store
            .model_admission_put(&row("hf.co/bartowski/X:Q4_K_M"))
            .expect("put");
        let got = store
            .model_admission_get("hf.co/bartowski/X:Q4_K_M")
            .expect("get")
            .expect("admitida");
        assert_eq!(got.quant, "Q4_K_M");
        assert!(got.probes_ok);
        assert_eq!(got.template_hash, "abc123");
        assert_eq!(
            store.model_admission_get("outra-tag").expect("get"),
            None,
            "tag sem admissão é None, nunca inventada"
        );
    }

    #[test]
    fn re_admission_updates_the_row() {
        let store = store();
        let mut nova = row("hf.co/bartowski/X:Q4_K_M");
        store.model_admission_put(&nova).expect("put");
        nova.sha256 = "b".repeat(64);
        store.model_admission_put(&nova).expect("put de novo");
        let got = store
            .model_admission_get("hf.co/bartowski/X:Q4_K_M")
            .expect("get")
            .expect("admitida");
        assert_eq!(
            got.sha256,
            "b".repeat(64),
            "re-admissão atualiza, não duplica"
        );
    }
}
