//! Credential and configuration resolution.
//!
//! The PAT lives in exactly two places: the `AZURE_DEVOPS_PAT` environment
//! variable or the OS keyring (saved by `kira setup`, service `"kira"`).
//! Never in a plain file, log, `Debug` output or test.
//!
//! `AZURE_DEVOPS_ORG` / `AZURE_DEVOPS_PROJECT` are resolved with the priority
//! env > `$HOME/.config/kira/config.toml` (written by `kira setup` with mode
//! 0600).
//!
//! The OS keyring is only used when it is actually usable: on machines without
//! a keyring daemon (Linux CI, headless boxes) [`resolve_pat`] degrades to the
//! env var with an actionable error — it never panics, and tests never touch
//! the OS keyring (only [`InMemory`]).

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::Error;

/// Environment variable carrying the Personal Access Token.
pub const PAT_ENV: &str = "AZURE_DEVOPS_PAT";
/// Environment variable carrying the Azure DevOps organization.
pub const ORG_ENV: &str = "AZURE_DEVOPS_ORG";
/// Environment variable carrying the default project name.
pub const PROJECT_ENV: &str = "AZURE_DEVOPS_PROJECT";

/// Keyring service name used by `kira setup`.
pub const KEYRING_SERVICE: &str = "kira";
/// Key stored in the credential store for the PAT.
pub const CRED_KEY_PAT: &str = "azure-devops-pat";

/// Config keys stored in `config.toml`.
pub const CONFIG_KEY_ORG: &str = "organization";
pub const CONFIG_KEY_PROJECT: &str = "project";

/// Where a resolved value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Environment variable.
    Env,
    /// OS keyring (service `"kira"`).
    Keyring,
    /// `$HOME/.config/kira/config.toml`.
    ConfigFile,
    /// A command-line flag (`--project`).
    Flag,
}

impl Source {
    /// Short human-readable label for ✓ lines.
    pub fn label(self) -> &'static str {
        match self {
            Source::Env => "env",
            Source::Keyring => "keyring do OS",
            Source::ConfigFile => "config.toml",
            Source::Flag => "linha de comando",
        }
    }
}

/// A resolved setting: its value and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub value: String,
    pub source: Source,
}

/// A store of secrets. Implementations must never log, `Debug`-print or
/// otherwise expose the stored values.
pub trait CredStore: Send + Sync {
    /// Saves (or overwrites) `key` with `value`.
    fn set(&self, key: &str, value: &str) -> Result<(), Error>;

    /// Returns the stored value for `key`, or `None` when absent.
    fn get(&self, key: &str) -> Result<Option<String>, Error>;

    /// Removes `key`; removing an absent key succeeds (idempotent).
    fn remove(&self, key: &str) -> Result<(), Error>;
}

/// In-memory store for tests only: no OS keyring is ever touched in tests.
#[derive(Default)]
pub struct InMemory {
    entries: Mutex<BTreeMap<String, String>>,
}

impl InMemory {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl CredStore for InMemory {
    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        let mut entries = self.entries.lock().map_err(|_| Error::KeyringUnavailable {
            action: "salvar",
            source: "o store em memória foi corrompido".into(),
        })?;
        entries.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        let entries = self.entries.lock().map_err(|_| Error::KeyringUnavailable {
            action: "ler",
            source: "o store em memória foi corrompido".into(),
        })?;
        Ok(entries.get(key).cloned())
    }

    fn remove(&self, key: &str) -> Result<(), Error> {
        let mut entries = self.entries.lock().map_err(|_| Error::KeyringUnavailable {
            action: "remover",
            source: "o store em memória foi corrompido".into(),
        })?;
        entries.remove(key);
        Ok(())
    }
}

/// OS keyring store backed by the `keyring` crate (Windows Credential
/// Manager, macOS Keychain, secret service on Linux).
pub struct OsKeyring {
    service: String,
}

impl OsKeyring {
    /// Opens the OS keyring, failing with an actionable error when the
    /// platform has no usable credential store (e.g. Linux CI without a
    /// secret-service daemon). The error never contains any secret.
    pub fn open() -> Result<Self, Error> {
        // Probing with a throwaway entry initializes the platform store; on
        // unsupported/daemon-less machines this is where it fails.
        let probe = keyring::Entry::new(KEYRING_SERVICE, CRED_KEY_PAT);
        match probe {
            Ok(_) => Ok(Self {
                service: KEYRING_SERVICE.to_string(),
            }),
            Err(err) => Err(Error::KeyringUnavailable {
                action: "abrir o keyring do OS",
                source: Box::new(err),
            }),
        }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, Error> {
        keyring::Entry::new(&self.service, key).map_err(|err| Error::KeyringUnavailable {
            action: "abrir o keyring do OS",
            source: Box::new(err),
        })
    }
}

impl CredStore for OsKeyring {
    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        let entry = self.entry(key)?;
        entry
            .set_password(value)
            .map_err(|err| Error::KeyringUnavailable {
                action: "salvar",
                source: Box::new(err),
            })
    }

    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            // Absent entry is not an error for our callers.
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(Error::KeyringUnavailable {
                action: "ler",
                source: Box::new(err),
            }),
        }
    }

    fn remove(&self, key: &str) -> Result<(), Error> {
        let entry = self.entry(key)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::KeyringUnavailable {
                action: "remover",
                source: Box::new(err),
            }),
        }
    }
}

/// Opens the default secret store: the OS keyring when available.
///
/// Tests must not call this — they construct [`InMemory`] instead.
pub fn open_default_store() -> Result<Box<dyn CredStore>, Error> {
    OsKeyring::open().map(|store| Box::new(store) as Box<dyn CredStore>)
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Path of the KIRA config file: `$HOME/.config/kira/config.toml`
/// (`USERPROFILE` fallback on Windows), or the `KIRA_CONFIG` override.
pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = non_empty_env("KIRA_CONFIG") {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()))?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("kira")
            .join("config.toml"),
    )
}

/// Parses the config file into ordered `key = value` pairs.
///
/// Accepted format (minimal, dependency-free): `key = "value"` lines, `#`
/// comments, blank lines. A malformed line is an error naming the line
/// number. A missing file yields an empty list.
pub fn read_config_file(path: &Path) -> Result<Vec<(String, String)>, Error> {
    let display = path.display().to_string();
    let body = match fs::read_to_string(path) {
        Ok(body) => body,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(Error::ConfigRead {
                path: display,
                source,
            });
        }
    };
    let mut pairs = Vec::new();
    for (index, raw) in body.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(Error::ConfigParse {
                path: display,
                line: index + 1,
            });
        };
        let key = key.trim();
        let mut value = value.trim();
        if let Some(stripped) = value.strip_prefix('"') {
            match stripped.strip_suffix('"') {
                Some(inner) => value = inner,
                None => {
                    return Err(Error::ConfigParse {
                        path: display,
                        line: index + 1,
                    });
                }
            }
        }
        if key.is_empty() || value.is_empty() {
            return Err(Error::ConfigParse {
                path: display,
                line: index + 1,
            });
        }
        pairs.push((key.to_string(), value.to_string()));
    }
    Ok(pairs)
}

/// Reads the KIRA config file, returning an empty list when there is no file
/// or no home directory.
pub fn read_config() -> Result<Vec<(String, String)>, Error> {
    match config_path() {
        Some(path) => read_config_file(&path),
        None => Ok(Vec::new()),
    }
}

fn config_get<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// Writes `updates` into the config file with mode 0600, preserving the other
/// keys and their order. Creates parent directories as needed.
pub fn write_config_file(path: &Path, updates: &[(&str, &str)]) -> Result<(), Error> {
    let mut pairs = read_config_file(path)?;
    for (key, value) in updates {
        match pairs.iter_mut().find(|(name, _)| name == key) {
            Some(pair) => pair.1 = value.to_string(),
            None => pairs.push((key.to_string(), value.to_string())),
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::ConfigRead {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let mut body = String::from("# kira — configuração local (escrita por `kira setup`)\n");
    for (key, value) in &pairs {
        body.push_str(&format!("{key} = \"{value}\"\n"));
    }
    fs::write(path, body).map_err(|source| Error::ConfigRead {
        path: path.display().to_string(),
        source,
    })?;
    restrict_permissions(path);
    Ok(())
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {
    // On Windows the file lives under the user profile, which already
    // restricts access to the user; no extra mode bits exist.
}

/// Core of the PAT resolution: env wins over the store; a missing store or an
/// empty store degrades to the actionable `MissingPat` error — never a panic.
fn resolve_pat_parts(
    env_value: Option<String>,
    store: Option<&dyn CredStore>,
) -> Result<Resolved, Error> {
    if let Some(value) = env_value.filter(|value| !value.trim().is_empty()) {
        return Ok(Resolved {
            value,
            source: Source::Env,
        });
    }
    match store {
        Some(store) => match store.get(CRED_KEY_PAT)? {
            Some(value) if !value.trim().is_empty() => Ok(Resolved {
                value,
                source: Source::Keyring,
            }),
            _ => Err(Error::MissingPat),
        },
        None => Err(Error::MissingPat),
    }
}

/// Resolves the PAT: `AZURE_DEVOPS_PAT` env > OS keyring.
pub fn resolve_pat() -> Result<String, Error> {
    resolve_pat_with_source().map(|resolved| resolved.value)
}

/// [`resolve_pat`] keeping the source (for `kira doctor` display).
pub fn resolve_pat_with_source() -> Result<Resolved, Error> {
    let env_value = non_empty_env(PAT_ENV);
    let store = open_default_store().ok();
    resolve_pat_parts(env_value, store.as_deref())
}

/// Core of a plain setting resolution: flag > env > config.toml.
fn resolve_setting_parts(
    flag: Option<String>,
    env_value: Option<String>,
    config_value: Option<String>,
    missing: Error,
) -> Result<Resolved, Error> {
    if let Some(value) = flag.filter(|value| !value.trim().is_empty()) {
        return Ok(Resolved {
            value,
            source: Source::Flag,
        });
    }
    if let Some(value) = env_value.filter(|value| !value.trim().is_empty()) {
        return Ok(Resolved {
            value,
            source: Source::Env,
        });
    }
    match config_value.filter(|value| !value.trim().is_empty()) {
        Some(value) => Ok(Resolved {
            value,
            source: Source::ConfigFile,
        }),
        None => Err(missing),
    }
}

/// Resolves the Azure DevOps organization: env > config.toml.
pub fn resolve_org() -> Result<String, Error> {
    resolve_org_with_source().map(|resolved| resolved.value)
}

/// [`resolve_org`] keeping the source (for `kira doctor` display).
pub fn resolve_org_with_source() -> Result<Resolved, Error> {
    let config = read_config()?;
    resolve_setting_parts(
        None,
        non_empty_env(ORG_ENV),
        config_get(&config, CONFIG_KEY_ORG).map(str::to_string),
        Error::MissingOrg,
    )
}

/// Resolves the default project: flag > env > config.toml.
pub fn resolve_project(flag: Option<&str>) -> Result<String, Error> {
    resolve_project_with_source(flag).map(|resolved| resolved.value)
}

/// [`resolve_project`] keeping the source (for `kira doctor` display).
pub fn resolve_project_with_source(flag: Option<&str>) -> Result<Resolved, Error> {
    let config = read_config()?;
    resolve_setting_parts(
        flag.map(str::to_string),
        non_empty_env(PROJECT_ENV),
        config_get(&config, CONFIG_KEY_PROJECT).map(str::to_string),
        Error::MissingProject,
    )
}

/// Everything the live commands need to talk to Azure DevOps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzureConfig {
    pub pat: String,
    pub organization: String,
    pub project: String,
}

/// Resolves org, project (with `--project` override) and PAT in priority
/// order, so live commands share one code path.
pub fn resolve_azure_config(project_override: Option<&str>) -> Result<AzureConfig, Error> {
    let organization = resolve_org()?;
    let project = resolve_project(project_override)?;
    let pat = resolve_pat()?;
    Ok(AzureConfig {
        pat,
        organization,
        project,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_round_trips_values() {
        let store = InMemory::new();
        assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
        store.set(CRED_KEY_PAT, "abc").expect("set");
        assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), Some("abc".into()));
        // Overwrite.
        store.set(CRED_KEY_PAT, "def").expect("set");
        assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), Some("def".into()));
        store.remove(CRED_KEY_PAT).expect("remove");
        assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
    }

    #[test]
    fn in_memory_remove_is_idempotent() {
        let store = InMemory::new();
        store.remove(CRED_KEY_PAT).expect("remover chave ausente");
    }

    #[test]
    fn keys_are_isolated_in_the_store() {
        let store = InMemory::new();
        store.set("a", "1").expect("set");
        store.set("b", "2").expect("set");
        assert_eq!(store.get("a").ok().flatten(), Some("1".into()));
        assert_eq!(store.get("b").ok().flatten(), Some("2".into()));
    }

    #[test]
    fn pat_resolution_prefers_env_over_the_store() {
        let store = InMemory::new();
        store.set(CRED_KEY_PAT, "do-keyring").expect("set");
        let resolved = resolve_pat_parts(Some("da-env".into()), Some(&store))
            .expect("env presente deve resolver");
        assert_eq!(resolved.value, "da-env");
        assert_eq!(resolved.source, Source::Env);
    }

    #[test]
    fn pat_resolution_falls_back_to_the_store() {
        let store = InMemory::new();
        store.set(CRED_KEY_PAT, "do-keyring").expect("set");
        let resolved = resolve_pat_parts(None, Some(&store)).expect("keyring presente");
        assert_eq!(resolved.value, "do-keyring");
        assert_eq!(resolved.source, Source::Keyring);
    }

    #[test]
    fn pat_resolution_without_env_or_store_is_missing_pat() {
        let err = resolve_pat_parts(None, None).expect_err("sem origens deve falhar");
        assert!(matches!(err, Error::MissingPat));
        let message = err.to_string();
        assert!(message.contains("kira setup"), "mensagem: {message}");
        assert!(message.contains("AZURE_DEVOPS_PAT"), "mensagem: {message}");
    }

    #[test]
    fn pat_resolution_with_empty_entry_is_missing_pat() {
        let store = InMemory::new();
        let err = resolve_pat_parts(None, Some(&store)).expect_err("store vazio deve falhar");
        assert!(matches!(err, Error::MissingPat));
    }

    #[test]
    fn blank_env_value_is_ignored() {
        let store = InMemory::new();
        store.set(CRED_KEY_PAT, "do-keyring").expect("set");
        let resolved = resolve_pat_parts(Some("   ".into()), Some(&store))
            .expect("env em branco deve ser ignorada");
        assert_eq!(resolved.value, "do-keyring");
        assert_eq!(resolved.source, Source::Keyring);
    }

    #[test]
    fn setting_resolution_is_flag_then_env_then_config() {
        let resolved = resolve_setting_parts(
            Some("flag".into()),
            Some("env".into()),
            Some("config".into()),
            Error::MissingOrg,
        )
        .expect("flag vence");
        assert_eq!(resolved.value, "flag");
        assert_eq!(resolved.source, Source::Flag);

        let resolved = resolve_setting_parts(
            None,
            Some("env".into()),
            Some("config".into()),
            Error::MissingOrg,
        )
        .expect("env vence");
        assert_eq!(resolved.value, "env");
        assert_eq!(resolved.source, Source::Env);

        let resolved = resolve_setting_parts(None, None, Some("config".into()), Error::MissingOrg)
            .expect("config vence por último");
        assert_eq!(resolved.value, "config");
        assert_eq!(resolved.source, Source::ConfigFile);

        let err = resolve_setting_parts(None, None, None, Error::MissingOrg).expect_err("nada");
        assert!(matches!(err, Error::MissingOrg));
    }

    #[test]
    fn blank_values_do_not_shadow_lower_priority_sources() {
        let resolved = resolve_setting_parts(
            Some("  ".into()),
            Some(" ".into()),
            Some("config".into()),
            Error::MissingOrg,
        )
        .expect("config deve vencer quando flag/env são brancas");
        assert_eq!(resolved.value, "config");
    }

    #[test]
    fn config_file_round_trips_and_preserves_unknown_keys() {
        let dir = std::env::temp_dir().join(format!("kira-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("config.toml");
        write_config_file(&path, &[(CONFIG_KEY_ORG, "minha-org")]).expect("escrever");
        write_config_file(&path, &[(CONFIG_KEY_PROJECT, "MeuProjeto")]).expect("atualizar");

        let pairs = read_config_file(&path).expect("ler");
        assert_eq!(config_get(&pairs, CONFIG_KEY_ORG), Some("minha-org"));
        assert_eq!(config_get(&pairs, CONFIG_KEY_PROJECT), Some("MeuProjeto"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path)
                .map(|meta| meta.permissions())
                .map(|perm| perm.mode() & 0o777)
                .unwrap_or(0);
            assert_eq!(mode, 0o600, "config.toml deve ter modo 0600");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_file_is_empty_not_an_error() {
        let pairs = read_config_file(Path::new("/kira/não/existe/config.toml"))
            .expect("arquivo ausente não é erro");
        assert!(pairs.is_empty());
    }

    #[test]
    fn malformed_config_line_names_the_line() {
        let dir = std::env::temp_dir().join(format!("kira-test-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("criar diretório temporário");
        let path = dir.join("config.toml");
        fs::write(&path, "organization = \"ok\"\nisto quebra tudo\n").expect("escrever");
        let err = read_config_file(&path).expect_err("linha inválida deve falhar");
        let message = err.to_string();
        assert!(message.contains("linha 2"), "mensagem: {message}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn comments_and_sections_are_skipped_when_parsing() {
        let dir = std::env::temp_dir().join(format!("kira-test-cmt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("criar diretório temporário");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "# comentário\norganization = \"org-a\"\n[secao]\nproject = \"proj-b\"\n",
        )
        .expect("escrever");
        let pairs = read_config_file(&path).expect("ler");
        assert_eq!(config_get(&pairs, CONFIG_KEY_ORG), Some("org-a"));
        assert_eq!(config_get(&pairs, CONFIG_KEY_PROJECT), Some("proj-b"));
        let _ = fs::remove_dir_all(&dir);
    }
}
