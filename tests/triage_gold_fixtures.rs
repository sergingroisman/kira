//! O lote de `triage_gold` (gate M2) tem de ser VÁLIDO antes de ser medido.
//!
//! O gate M2 compara a causa raiz prevista com a rotulada e exige ≥70%. Se um
//! `item.json` ou `gold.json` estiver malformado, o eval pode contar um item a
//! menos — e um gate que mede 19 sem avisar é pior que um gate que falha. Este
//! teste é o guarda de forma: todo lote presente em `tests/fixtures/triage_gold`
//! precisa ter os campos que a medição usa.

use std::fs;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/triage_gold")
}

#[test]
fn every_gold_item_has_the_fields_the_gate_reads() {
    let root = fixtures_root();
    let mut pastas: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("ler {root:?}: {err}"))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .collect();
    pastas.sort();

    assert!(
        !pastas.is_empty(),
        "o lote de triage_gold está vazio — o gate M2 não tem o que medir"
    );

    let mut problemas = Vec::new();
    for pasta in &pastas {
        let nome = pasta
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        for arquivo in ["item.json", "gold.json"] {
            if !pasta.join(arquivo).is_file() {
                problemas.push(format!("{nome}: falta {arquivo}"));
            }
        }

        let item: serde_json::Value = match fs::read_to_string(pasta.join("item.json")) {
            Ok(texto) => match serde_json::from_str(&texto) {
                Ok(valor) => valor,
                Err(err) => {
                    problemas.push(format!("{nome}: item.json inválido ({err})"));
                    continue;
                }
            },
            Err(err) => {
                problemas.push(format!("{nome}: item.json ilegível ({err})"));
                continue;
            }
        };
        // O eval usa o id do work item e o texto do título/descrição; item sem
        // id ou sem descrição vira prompt vazio e a medição não significa nada.
        let id = item
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        if id <= 0 {
            problemas.push(format!("{nome}: item.json sem `id` numérico"));
        }
        for campo in ["System.Title", "System.Description"] {
            let valor = item
                .get("fields")
                .and_then(|fields| fields.get(campo))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .trim();
            if valor.is_empty() {
                problemas.push(format!("{nome}: fields.{campo} vazio"));
            }
        }
        if let Some(id_field) = item
            .get("fields")
            .and_then(|fields| fields.get("System.Id"))
            .and_then(serde_json::Value::as_i64)
            && id_field != id
        {
            problemas.push(format!(
                "{nome}: fields.System.Id ({id_field}) diferente de id ({id})"
            ));
        }

        let gold: serde_json::Value = match fs::read_to_string(pasta.join("gold.json")) {
            Ok(texto) => match serde_json::from_str(&texto) {
                Ok(valor) => valor,
                Err(err) => {
                    problemas.push(format!("{nome}: gold.json inválido ({err})"));
                    continue;
                }
            },
            Err(err) => {
                problemas.push(format!("{nome}: gold.json ilegível ({err})"));
                continue;
            }
        };
        let causa = gold
            .get("causa_raiz")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim();
        if causa.is_empty() {
            problemas.push(format!("{nome}: gold.json sem `causa_raiz`"));
        }
        let arquivos = gold
            .get("arquivos")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if arquivos.is_empty() {
            problemas.push(format!("{nome}: gold.json sem `arquivos`"));
        }
    }

    assert!(
        problemas.is_empty(),
        "lote de triage_gold inválido:\n  {}",
        problemas.join("\n  ")
    );
}
