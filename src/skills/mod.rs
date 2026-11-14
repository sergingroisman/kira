//! `kira skills` — catálogo de cartas de instrução (onda SK, SK-1).
//!
//! Formato (fonte de verdade = markdown, como a wiki): frontmatter obrigatório
//! (`name`, `description`, `license`, `author`, `version`) + corpo. O
//! `description` carrega trigger ("Use when…") e anti-trigger ("Do NOT use…") —
//! é o que o FTS5 casa; `version`+`author` dão a cadeia de custódia (mesmo
//! espírito do `TRIAGE_PROMPT_VERSION`).
//!
//! Onde mora:
//! - **bundled**: embutido no binário via `include_str!` (zero instalação,
//!   determinístico) — `skills/*.md` no repo do KIRA;
//! - **usuário**: `<projeto>/.kira/skills/*.md` (local, nunca commitado) —
//!   estende/substitui o bundled pelo `name` (usuário vence o bundled).
//!
//! Regras da casa: o índice `skill`/`skill_fts` é DERIVADO e descartável
//! (reconstruir nunca perde conhecimento); todo corpo e descrição passa por
//! `crate::redact` ANTES de indexar e ANTES de gravar (segredo nunca chega ao
//! disco nem ao prompt); falha de leitura de um skill vira nota, nunca erro do
//! comando.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::store::{SkillHit, SkillRow, Store};

/// `project_id` reservado para o catálogo bundled no índice (nunca colide com
/// o id de projeto, que é hash).
pub const BUNDLED_PROJECT: &str = "__bundled__";

/// De onde o skill veio (cadeia de custódia na listagem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillOrigin {
    Bundled,
    User,
}

impl SkillOrigin {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::User => "user",
        }
    }
}

/// Um skill já validado e redigido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub license: String,
    pub author: String,
    pub version: String,
    pub body: String,
    pub origin: SkillOrigin,
}

/// Erros do catálogo — todos acionáveis.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error(
        "frontmatter ausente — o skill precisa começar com o bloco --- (name, description, license, author, version)"
    )]
    MissingFrontMatter,
    #[error(
        "skill sem o campo obrigatório `{0}` — o frontmatter precisa de name, description, license, author e version"
    )]
    MissingField(&'static str),
    #[error(
        "nome de skill inválido `{0}` — use só minúsculas, dígitos, '-' e '_' (vira nome de arquivo em .kira/skills/)"
    )]
    InvalidName(String),
    #[error("versão inválida `{0}` no skill `{1}` — use semver MAJOR.MINOR.PATCH (ex.: 1.0.0)")]
    InvalidVersion(String, String),
    #[error("skill `{0}` não encontrado — confira `kira skills list`")]
    NotFound(String),
    #[error("não consegui acessar {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("store indisponível: {0}")]
    Store(String),
}

/// Parse + validação de UM skill (função pura sobre o texto; `redact` aplicado
/// aqui — quem chama nunca vê segredo).
pub fn parse_skill(text: &str, origin: SkillOrigin) -> Result<Skill, SkillError> {
    // CRLF do Windows não pode quebrar o parser: normaliza antes.
    let text = text.replace("\r\n", "\n");
    let rest = text
        .strip_prefix("---\n")
        .ok_or(SkillError::MissingFrontMatter)?;
    let close = rest.find("\n---").ok_or(SkillError::MissingFrontMatter)?;
    let block = &rest[..close];
    let body = rest[close + "\n---".len()..].trim_start_matches('\n');

    let mut fields: std::collections::BTreeMap<&str, String> = std::collections::BTreeMap::new();
    for line in block.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        fields.insert(key.trim(), value.trim().to_string());
    }
    let field = |key: &'static str| -> Result<String, SkillError> {
        fields
            .get(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or(SkillError::MissingField(key))
    };

    let name = field("name")?;
    if !name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(SkillError::InvalidName(name));
    }
    let version = field("version")?;
    if !is_semver(&version) {
        return Err(SkillError::InvalidVersion(version, name));
    }
    Ok(Skill {
        name,
        description: crate::redact::redact(&field("description")?),
        license: field("license")?,
        author: field("author")?,
        version,
        body: crate::redact::redact(body.trim()),
        origin,
    })
}

/// `MAJOR.MINOR.PATCH`, cada parte numérica (subset do semver que basta para
/// a cadeia de custódia — pré-release fica de fora de propósito).
fn is_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

/// Texto de skill canônico (frontmatter + corpo), para gravar no `add`.
fn render_skill(skill: &Skill) -> String {
    format!(
        "---\nname: {}\ndescription: {}\nlicense: {}\nauthor: {}\nversion: {}\n---\n\n{}\n",
        skill.name, skill.description, skill.license, skill.author, skill.version, skill.body
    )
}

/// Catálogo bundled embutido no binário (`skills/*.md`, zero instalação).
/// Ordem alfabética por nome — determinismo já na fonte.
const BUNDLED_RAW: &[&str] = &[
    include_str!("../../skills/drawio.md"),
    include_str!("../../skills/react.md"),
    include_str!("../../skills/shadcn.md"),
    include_str!("../../skills/tailwind.md"),
    include_str!("../../skills/tanstack.md"),
];

/// Skills bundled (ordem do `BUNDLED_RAW`; determinístico).
#[must_use]
pub fn bundled() -> Vec<Skill> {
    BUNDLED_RAW
        .iter()
        .filter_map(|text| parse_skill(text, SkillOrigin::Bundled).ok())
        .collect()
}

/// Skills do usuário em `<projeto>/.kira/skills/*.md`; arquivo inválido vira
/// nota (fail-open), nunca erro do comando.
#[must_use]
pub fn load_user(root: &Path) -> (Vec<Skill>, Vec<String>) {
    let dir = root.join(".kira").join("skills");
    let mut skills = Vec::new();
    let mut notes = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (skills, notes); // sem diretório = sem skills do usuário
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();
    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(text) => match parse_skill(&text, SkillOrigin::User) {
                Ok(skill) => skills.push(skill),
                Err(err) => notes.push(format!("skill inválido em {}: {err}", path.display())),
            },
            Err(source) => notes.push(format!("skill ilegível em {}: {source}", path.display())),
        }
    }
    (skills, notes)
}

/// Catálogo efetivo: bundled + usuário, com o usuário substituindo o bundled
/// pelo `name` (workspace > bundled). Ordenado por nome.
#[must_use]
pub fn all(root: &Path) -> (Vec<Skill>, Vec<String>) {
    let (user, notes) = load_user(root);
    let user_names: BTreeSet<&str> = user.iter().map(|skill| skill.name.as_str()).collect();
    let mut merged: Vec<Skill> = bundled()
        .into_iter()
        .filter(|skill| !user_names.contains(skill.name.as_str()))
        .collect();
    merged.extend(user);
    merged.sort_by(|a, b| a.name.cmp(&b.name));
    (merged, notes)
}

/// Reconstrói o índice derivado: bundled sob [`BUNDLED_PROJECT`], usuário sob
/// o projeto atual. Devolve quantas linhas indexou.
pub fn reindex(root: &Path, store: &Store, project_id: &str) -> Result<usize, SkillError> {
    let bundled_rows: Vec<SkillRow> = bundled()
        .into_iter()
        .map(|skill| row_for(&skill, BUNDLED_PROJECT))
        .collect();
    let (user, _notes) = load_user(root);
    let user_rows: Vec<SkillRow> = user
        .into_iter()
        .map(|skill| row_for(&skill, project_id))
        .collect();
    store
        .skill_replace(BUNDLED_PROJECT, &bundled_rows)
        .map_err(|err| SkillError::Store(err.to_string()))?;
    store
        .skill_replace(project_id, &user_rows)
        .map_err(|err| SkillError::Store(err.to_string()))?;
    Ok(bundled_rows.len() + user_rows.len())
}

fn row_for(skill: &Skill, project_id: &str) -> SkillRow {
    SkillRow {
        project_id: project_id.to_string(),
        name: skill.name.clone(),
        origin: skill.origin.as_str().to_string(),
        version: skill.version.clone(),
        description: skill.description.clone(),
        body: skill.body.clone(),
    }
}

/// Uma linha por skill: `name@version  [origem]  description`. Notas de
/// arquivo inválido entram como linha própria (nunca silêncio).
#[must_use]
pub fn list(root: &Path) -> String {
    let (skills, notes) = all(root);
    let mut out = String::new();
    for skill in &skills {
        out.push_str(&format!(
            "{}@{}  [{}]  {}\n",
            skill.name,
            skill.version,
            skill.origin.as_str(),
            skill.description
        ));
    }
    for note in &notes {
        out.push_str(&format!("nota: {note}\n"));
    }
    out
}

/// Corpo do skill com a âncora de procedência.
pub fn show(root: &Path, name: &str) -> Result<String, SkillError> {
    let (skills, _notes) = all(root);
    let skill = skills
        .into_iter()
        .find(|skill| skill.name == name)
        .ok_or_else(|| SkillError::NotFound(name.to_string()))?;
    Ok(format!(
        "skill: {}@{} [{}]\n\n{}\n",
        skill.name,
        skill.version,
        skill.origin.as_str(),
        skill.body
    ))
}

/// `kira skills add <arquivo>`: valida frontmatter, REDIGE e grava em
/// `.kira/skills/<name>.md` (local — nunca commitado).
pub fn add(root: &Path, file: &Path) -> Result<Skill, SkillError> {
    let text = std::fs::read_to_string(file).map_err(|source| SkillError::Io {
        path: file.display().to_string(),
        source,
    })?;
    let skill = parse_skill(&text, SkillOrigin::User)?;
    let dir = root.join(".kira").join("skills");
    std::fs::create_dir_all(&dir).map_err(|source| SkillError::Io {
        path: dir.display().to_string(),
        source,
    })?;
    let target = dir.join(format!("{}.md", skill.name));
    std::fs::write(&target, render_skill(&skill)).map_err(|source| SkillError::Io {
        path: target.display().to_string(),
        source,
    })?;
    Ok(skill)
}

/// Busca no índice (bundled + skills do projeto atual), top-k determinístico.
pub fn search(
    store: &Store,
    project_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SkillHit>, SkillError> {
    store
        .skill_search(BUNDLED_PROJECT, project_id, query, limit)
        .map_err(|err| SkillError::Store(err.to_string()))
}

/// Render de um hit para a saída humana: âncora `skill:<name>@<version>`.
#[must_use]
pub fn render_hits(hits: &[SkillHit]) -> String {
    let mut out = String::new();
    for hit in hits {
        out.push_str(&format!(
            "skill:{}@{} [{}]  {}\n",
            hit.name, hit.version, hit.origin, hit.snippet
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "---\nname: meu-skill\ndescription: Use when testing. Do NOT use in prod.\nlicense: MIT\nauthor: Ana\nversion: 1.2.3\n---\n\n# Meu skill\n- regra 1\n";

    #[test]
    fn parses_a_valid_skill_and_keeps_the_body() {
        let skill = parse_skill(VALID, SkillOrigin::User).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(skill.name, "meu-skill");
        assert_eq!(skill.version, "1.2.3");
        assert_eq!(skill.origin, SkillOrigin::User);
        assert!(skill.body.contains("regra 1"), "corpo: {}", skill.body);
        assert!(
            !skill.body.starts_with('-'),
            "cabeçalho sai do corpo: {}",
            skill.body
        );
    }

    #[test]
    fn crlf_does_not_break_the_parser() {
        let crlf = VALID.replace('\n', "\r\n");
        let skill = parse_skill(&crlf, SkillOrigin::Bundled).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(skill.name, "meu-skill");
        assert!(skill.body.contains("regra 1"), "corpo CRLF: {}", skill.body);
    }

    #[test]
    fn missing_frontmatter_or_field_is_actionable() {
        assert!(matches!(
            parse_skill("sem cabeçalho\n", SkillOrigin::User),
            Err(SkillError::MissingFrontMatter)
        ));
        let no_version = VALID.replace("version: 1.2.3\n", "");
        let err = parse_skill(&no_version, SkillOrigin::User).expect_err("sem version");
        assert!(
            err.to_string().contains("version"),
            "o erro nomeia o campo: {err}"
        );
    }

    #[test]
    fn invalid_name_and_version_are_refused() {
        let bad_name = VALID.replace("name: meu-skill", "name: Meu Skill");
        assert!(matches!(
            parse_skill(&bad_name, SkillOrigin::User),
            Err(SkillError::InvalidName(_))
        ));
        let bad_version = VALID.replace("version: 1.2.3", "version: v1");
        assert!(matches!(
            parse_skill(&bad_version, SkillOrigin::User),
            Err(SkillError::InvalidVersion(_, _))
        ));
    }

    #[test]
    fn secrets_are_redacted_at_parse_time() {
        let leaked = VALID.replace("- regra 1", "- configuração: token=AbC123XyZ789QqQ");
        let skill = parse_skill(&leaked, SkillOrigin::User).unwrap_or_else(|err| panic!("{err}"));
        assert!(
            !skill.body.contains("AbC123XyZ789QqQ"),
            "segredo vazou para o corpo: {}",
            skill.body
        );
        assert!(
            !skill.description.contains("ghp_"),
            "descrição também é redigida"
        );
    }

    #[test]
    fn bundled_catalog_is_valid_and_sorted_by_name() {
        let catalog = bundled();
        assert!(catalog.len() >= 5, "catálogo bundled: {}", catalog.len());
        let names: Vec<&str> = catalog.iter().map(|skill| skill.name.as_str()).collect();
        assert!(names.contains(&"react"), "{names:?}");
        assert!(names.contains(&"tailwind"), "{names:?}");
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "bundled em ordem determinística");
    }

    #[test]
    fn render_round_trips_through_the_parser() {
        let skill = parse_skill(VALID, SkillOrigin::User).unwrap_or_else(|err| panic!("{err}"));
        let rendered = render_skill(&skill);
        let again = parse_skill(&rendered, SkillOrigin::User).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(skill, again, "render → parse é estável");
    }

    #[test]
    fn user_skill_overrides_bundled_by_name() {
        let dir = std::env::temp_dir().join(format!(
            "kira-skills-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(dir.join(".kira/skills")).expect("criar");
        std::fs::write(
            dir.join(".kira/skills/react.md"),
            "---\nname: react\ndescription: override\nlicense: MIT\nauthor: Ana\nversion: 9.0.0\n---\n\ncorpo do usuário\n",
        )
        .expect("escrever");
        let (catalog, notes) = all(&dir);
        assert!(notes.is_empty(), "{notes:?}");
        let react = catalog
            .iter()
            .find(|skill| skill.name == "react")
            .expect("react no catálogo");
        assert_eq!(react.version, "9.0.0", "o do usuário vence o bundled");
        assert_eq!(react.origin, SkillOrigin::User);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
