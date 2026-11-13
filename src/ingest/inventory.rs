//! Caminhada única que produz os fatos do projeto (ADR-0038, M1).
//!
//! Uma passada só, determinística: mesma árvore ⇒ mesma lista, mesma ordem, mesmos
//! bytes. O resultado alimenta o `.kira/ingest/inventory.json`, a árvore do digest e
//! a decisão de quais arquivos são lidos para o resumo.
//!
//! Regras que vieram do uso real (ADR-0038):
//! - a decisão de pular vem de [`crate::context::ignore`], não de uma lista local;
//! - a **raiz** nunca é pulada, mesmo chamando `build` ou começando com ponto;
//! - binário e arquivo acima do teto entram como **nota**, nunca desaparecem calados;
//! - profundidade e número de arquivos têm orçamento com nota, sem erro.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::context::ignore::{self, MAX_FILE_BYTES};

/// Bytes lidos para decidir se o arquivo é binário (mesmo probe do grep).
const BINARY_PROBE_BYTES: usize = 2_048;

/// Orçamento da caminhada. Constantes fixas: env só com justificativa (ADR-0038).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub max_depth: usize,
    pub max_files: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_depth: 20,
            max_files: 5_000,
        }
    }
}

/// Um arquivo ingerido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryFile {
    /// Caminho relativo à raiz, sempre com `/`.
    pub path: String,
    pub bytes: u64,
    /// `true` quando passou de [`MAX_FILE_BYTES`] e será lido só até o teto.
    pub truncated: bool,
}

/// Fatos do projeto. Tudo aqui é derivado, ordenado e serializável.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub project_name: String,
    pub files: Vec<InventoryFile>,
    /// Contagem por extensão (`<sem extensão>` quando não houver), ordem alfabética.
    pub by_extension: BTreeMap<String, usize>,
    pub total_bytes: u64,
    /// Tudo que o usuário precisa saber e não caberia numa contagem.
    pub notes: Vec<String>,
}

impl Inventory {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Árvore determinística a partir dos caminhos já ordenados.
    ///
    /// Sai em indentação de dois espaços por nível, diretório antes dos filhos —
    /// legível para humano e estável byte a byte (é o que o digest publica).
    pub fn tree(&self) -> String {
        let mut out = String::new();
        let mut current: Vec<&str> = Vec::new();
        for file in &self.files {
            let parts: Vec<&str> = file.path.split('/').collect();
            let dirs = &parts[..parts.len() - 1];
            // Emite só os níveis de diretório que mudaram, na ordem.
            let common = current
                .iter()
                .zip(dirs.iter())
                .take_while(|(a, b)| a == b)
                .count();
            for (depth, dir) in dirs.iter().enumerate().skip(common) {
                out.push_str(&"  ".repeat(depth));
                out.push_str(dir);
                out.push_str("/\n");
            }
            for _ in 0..dirs.len() {
                out.push_str("  ");
            }
            out.push_str(parts[parts.len() - 1]);
            out.push('\n');
            current = dirs.to_vec();
        }
        out
    }
}

/// Caminha a raiz e devolve os fatos, sem tocar em nada do projeto.
pub fn walk(root: &Path, budget: Budget) -> Inventory {
    let project_name = root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );

    let mut files: Vec<InventoryFile> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut binary_skipped = 0usize;
    let mut depth_capped = 0usize;
    let mut file_capped = false;

    // P5: o `.gitignore` do projeto é lido como REGRA (o arquivo em si continua
    // fora do conteúdo ingerido — a denylist o mantém). Mesmo stack, mesmo
    // veredito do índice e do grep: uma política, três caminhos.
    let stack = crate::context::gitignore::GitIgnoreStack::discover(root);

    let walker = WalkDir::new(root)
        .max_depth(budget.max_depth)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            // Regra do ADR-0038: a raiz é sempre ingerida (depth 0).
            if entry.depth() == 0 || !entry.file_type().is_dir() {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            let rel = entry
                .path()
                .strip_prefix(root)
                .ok()
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            !ignore::skip_dir(&name, &rel, Some(&stack))
        });

    for entry in walker {
        let Ok(entry) = entry else {
            continue;
        };
        // Profundidade maior que o orçamento: WalkDir já não desce, mas o diretório
        // no limite é contado para a nota (senão o corte fica invisível).
        if entry.depth() >= budget.max_depth && entry.file_type().is_dir() {
            depth_capped += 1;
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let path = rel.to_string_lossy().replace('\\', "/");
        let name = entry.file_name().to_string_lossy();
        if ignore::skip_file(&name, &path, Some(&stack)) {
            continue;
        }
        if files.len() >= budget.max_files {
            file_capped = true;
            break;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        // Binário: amostra inicial com NUL. Não entra no inventário e vira nota.
        if let Some(sample) = read_probe(entry.path())
            && ignore::looks_binary(&sample)
        {
            binary_skipped += 1;
            continue;
        }
        files.push(InventoryFile {
            path,
            bytes: meta.len(),
            truncated: meta.len() > MAX_FILE_BYTES,
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));

    let mut by_extension: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_bytes = 0u64;
    for file in &files {
        total_bytes = total_bytes.saturating_add(file.bytes);
        let ext = file
            .path
            .rsplit('/')
            .next()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .filter(|ext| !ext.is_empty())
            .unwrap_or_else(|| "<sem extensão>".to_string());
        *by_extension.entry(ext).or_insert(0) += 1;
    }

    if file_capped {
        notes.push(format!(
            "orçamento de arquivos atingido ({}): a árvore do digest está parcial",
            budget.max_files
        ));
    }
    if depth_capped > 0 {
        notes.push(format!(
            "{depth_capped} diretório(s) no limite de profundidade {} não foram percorridos",
            budget.max_depth
        ));
    }
    if binary_skipped > 0 {
        notes.push(format!(
            "{binary_skipped} arquivo(s) binário(s) ignorado(s)"
        ));
    }
    let oversized: Vec<&str> = files
        .iter()
        .filter(|f| f.truncated)
        .map(|f| f.path.as_str())
        .collect();
    if !oversized.is_empty() {
        notes.push(format!(
            "{} arquivo(s) acima de {} KB serão lidos só até o teto: {}",
            oversized.len(),
            MAX_FILE_BYTES / 1024,
            oversized.join(", ")
        ));
    }

    Inventory {
        project_name,
        files,
        by_extension,
        total_bytes,
        notes,
    }
}

/// Primeiros bytes do arquivo, para o sniff de binário. `None` = ilegível.
fn read_probe(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; BINARY_PROBE_BYTES];
    let read = file.read(&mut buffer).ok()?;
    buffer.truncate(read);
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kira-inv-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, bytes).expect("write");
    }

    #[test]
    fn walk_keeps_packages_and_counts_by_extension() {
        let root = temp_repo("packages");
        write(&root, "packages/core/src/lib.rs", b"pub fn a() {}\n");
        write(&root, "packages/core/package.json", b"{}\n");
        write(&root, "node_modules/x/index.js", b"x\n");
        let inv = walk(&root, Budget::default());
        let paths: Vec<&str> = inv.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["packages/core/package.json", "packages/core/src/lib.rs"]
        );
        assert_eq!(inv.by_extension.get("rs"), Some(&1));
        assert_eq!(inv.by_extension.get("json"), Some(&1));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Regressão do ADR-0038: projeto que mora numa pasta chamada `build` era zerado.
    #[test]
    fn root_named_build_is_still_ingested() {
        let parent = temp_repo("rootbuild");
        let root = parent.join("build");
        std::fs::create_dir_all(&root).expect("mkdir");
        write(&root, "src/main.rs", b"fn main() {}\n");
        let inv = walk(&root, Budget::default());
        assert_eq!(inv.files.len(), 1, "raiz homônima não pode sumir");
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn binary_and_oversized_files_are_noted_not_silent() {
        let root = temp_repo("notes");
        write(&root, "logo.png", b"\x89PNG\r\n\x1a\n");
        write(&root, "big.rs", &vec![b'a'; (MAX_FILE_BYTES + 10) as usize]);
        write(&root, "ok.rs", b"fn ok() {}\n");
        let inv = walk(&root, Budget::default());
        assert_eq!(inv.files.len(), 2, "png fora, big e ok dentro");
        assert!(
            inv.notes.iter().any(|n| n.contains("acima de")),
            "notas: {:?}",
            inv.notes
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn file_budget_is_reported_and_deterministic() {
        let root = temp_repo("budget");
        for i in 0..5 {
            write(&root, &format!("f{i}.rs"), b"x\n");
        }
        let budget = Budget {
            max_depth: 20,
            max_files: 3,
        };
        let a = walk(&root, budget);
        let b = walk(&root, budget);
        assert_eq!(a.files.len(), 3);
        assert_eq!(
            a, b,
            "duas caminhadas iguais produzem inventários idênticos"
        );
        assert!(a.notes.iter().any(|n| n.contains("orçamento de arquivos")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tree_renders_directories_once() {
        let root = temp_repo("tree");
        write(&root, "src/a.rs", b"a\n");
        write(&root, "src/b.rs", b"b\n");
        write(&root, "README.md", b"r\n");
        let inv = walk(&root, Budget::default());
        let tree = inv.tree();
        assert_eq!(tree, "README.md\nsrc/\n  a.rs\n  b.rs\n");
        let _ = std::fs::remove_dir_all(&root);
    }
}
