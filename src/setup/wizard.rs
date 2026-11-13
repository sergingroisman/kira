//! Wizard de tela cheia do `kira setup` (casca do statui; layout inspirado no
//! onboarding do opencode: trilha de etapas à esquerda, painel à direita,
//! rodapé com teclas e nenhuma pergunta "cega").
//!
//! Divisão de responsabilidade deliberada:
//! - o wizard COLETA respostas locais (nome, projeto/raiz, backend de IA) e
//!   devolve um [`SetupOptions`] com `interactive: false`;
//! - quem grava config/keyring e checa a IA continua sendo [`super::run`] —
//!   uma única fonte de verdade para efeitos e mensagens.
//!
//! Local-first (ADR-0030): o wizard NÃO pede credencial — PAT só nasce em
//! `kira connect` (MVP-3). Etapas (M6): Ambiente → Projeto (raiz detectada +
//! idioma de saída) → Ingestão (preview read-only; Enter ingere, `n` pula,
//! `l` muda o idioma) → IA (4 opções, incluindo "nenhuma por agora") →
//! Resumo. A etapa Ingestão só MOSTRA e COLETA: quem executa é `setup::run`
//! (ADR-0034 — fonte única de efeitos e mensagens).
//!
//! Por que isso importa no Windows: em raw mode o terminal NÃO ecoa por
//! construção (crossterm desliga `ENABLE_ECHO_INPUT`), então o segredo fica
//! oculto nos três SOs sem `stty` e sem dependência nova. A tela mostra uma
//! máscara fixa — o valor nunca entra em [`WizardState::masked`] como texto.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::layout as tui_layout;
use crate::tui::theme::{self, Palette};
use crate::tui::widgets::footer::{self, Key};
use crate::tui::widgets::panel;

use super::{FoundryChoice, SetupOptions, validate_lang};

/// Largura da trilha de etapas (opencode usa 42 para a sidebar de sessão; aqui
/// são cinco rótulos curtos).
const RAIL_WIDTH: u16 = 24;

/// Abaixo desta largura a trilha lateral sai de cena (vira contador na linha
/// de abas): com 80 colunas, 24 de trilha deixariam 54 para a instrução — e
/// instrução truncada é instrução que mente.
const NARROW_WIDTH: u16 = 100;

/// Quantas `•` a máscara mostra (o comprimento real não é revelado).
const MASK_LEN: usize = 12;

/// Como o `kira setup` deve rodar nesta máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Wizard,
    Plain,
}

/// Decide o modo — PURA, para a regra ser testável sem terminal.
///
/// Wizard só quando: TTY nos dois lados, terminal grande o bastante, nenhuma
/// flag explícita e `--plain` ausente. Qualquer dúvida cai no modo de texto,
/// que é o que o CI e os testes conhecem.
pub fn mode(tty: bool, width: u16, height: u16, plain_flag: bool, explicit_flags: bool) -> Mode {
    if !tty || plain_flag || explicit_flags {
        return Mode::Plain;
    }
    if tui_layout::is_degraded(Rect::new(0, 0, width, height)) {
        return Mode::Plain;
    }
    Mode::Wizard
}

/// Etapas do wizard, na ordem em que aparecem na trilha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Ambiente,
    Projeto,
    /// M6 (ADR-0038 D1): preview read-only da ingestão + escolha de idioma.
    Ingestao,
    Ia,
    Resumo,
}

impl Step {
    pub const ALL: [Step; 5] = [
        Step::Ambiente,
        Step::Projeto,
        Step::Ingestao,
        Step::Ia,
        Step::Resumo,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Step::Ambiente => "Ambiente",
            Step::Projeto => "Projeto",
            Step::Ingestao => "Ingestão",
            Step::Ia => "IA",
            Step::Resumo => "Resumo",
        }
    }

    pub fn from_index(index: usize) -> Step {
        Step::ALL.get(index).copied().unwrap_or(Step::Resumo)
    }
}

/// Formulário do Foundry está utilizável? Devolve o campo culpado e a causa
/// acionável (PURA: a mesma regra vale para a tela e para `into_options`).
///
/// As recusas existem para o dono não descobrir o problema só no `triage`: o
/// endpoint de projeto (`/api/projects/`) não serve chat completions, o
/// serviço quer o endpoint do recurso (v1 termina em `/openai/v1`), e o
/// deployment é o nome exato do portal — com espaço não existe.
pub fn foundry_problem(
    endpoint: &str,
    deployment: &str,
    key: &str,
) -> Option<(Field, &'static str)> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Some((
            Field::FoundryEndpoint,
            "endpoint é obrigatório (ex.: https://recurso.openai.azure.com/openai/v1)",
        ));
    }
    if !endpoint.starts_with("https://") && !endpoint.starts_with("http://") {
        return Some((
            Field::FoundryEndpoint,
            "endpoint começa com https:// (na superfície v1, termina em /openai/v1)",
        ));
    }
    if endpoint.contains("/api/projects/") {
        return Some((
            Field::FoundryEndpoint,
            "endpoint de projeto não serve para chat — use o do recurso, terminando em /openai/v1",
        ));
    }
    let deployment = deployment.trim();
    if deployment.is_empty() {
        return Some((
            Field::FoundryDeployment,
            "deployment é obrigatório (ex.: gpt-5) — é o nome que aparece no portal",
        ));
    }
    if deployment.split_whitespace().count() > 1 {
        return Some((
            Field::FoundryDeployment,
            "deployment não tem espaços — copie o nome exato do portal",
        ));
    }
    if key.trim().is_empty() {
        return Some((
            Field::FoundryKey,
            "chave é obrigatória — ela vai para o keyring do OS, nunca para o config",
        ));
    }
    None
}

/// Campo em edição na etapa atual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Formulário inline do Foundry (ADR-0035): endpoint do recurso.
    FoundryEndpoint,
    /// Formulário inline do Foundry: deployment/modelo.
    FoundryDeployment,
    /// Formulário inline do Foundry: chave (segredo, sempre mascarada).
    FoundryKey,
}

impl Field {
    /// Campos do formulário do Foundry, na ordem do Tab.
    pub const FOUNDRY: [Field; 3] = [
        Field::FoundryEndpoint,
        Field::FoundryDeployment,
        Field::FoundryKey,
    ];

    /// O campo guarda segredo? (nunca é renderizado como texto)
    pub fn is_secret(self) -> bool {
        matches!(self, Field::FoundryKey)
    }

    /// Rótulo do campo no painel.
    pub fn label(self) -> &'static str {
        match self {
            Field::FoundryEndpoint => "endpoint",
            Field::FoundryDeployment => "deployment",
            Field::FoundryKey => "chave",
        }
    }
}

/// De onde veio o Foundry que já está pronto para uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundrySource {
    /// `KIRA_FOUNDRY_*` no ambiente.
    Env,
    /// `config.toml` (endpoint/deployment) + chave no keyring.
    Stored,
}

/// O que já está salvo nesta máquina: `config.toml` (valores públicos) e
/// PRESENÇA dos segredos no keyring — o valor nunca é lido para a tela.
///
/// Existe para o wizard responder "o que já está configurado?" sem perguntar
/// de novo o que ele mesmo gravou (ADR-0035).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SavedConfig {
    pub org: Option<String>,
    pub project: Option<String>,
    pub name: Option<String>,
    /// Idioma de saída salvo pelo setup (ADR-0038 D4) — pré-preenche a etapa
    /// Ingestão em vez de perguntar de novo o que a máquina já sabe.
    pub lang: Option<String>,
    pub foundry_endpoint: Option<String>,
    pub foundry_deployment: Option<String>,
    /// A chave do Foundry existe no keyring (só o fato).
    pub foundry_key: bool,
    /// O PAT está no keyring.
    pub pat: bool,
}

impl SavedConfig {
    /// Lê o `config.toml` e pergunta ao keyring só "existe?" (nunca o valor).
    pub fn detect() -> Self {
        let pairs = crate::credentials::read_config().unwrap_or_default();
        let value = |key: &str| {
            pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        // Uma abertura só: keyring sem daemon degrada para "ausente" em vez de
        // derrubar o wizard.
        let store = crate::credentials::open_default_store().ok();
        let has = |key: &str| {
            store
                .as_ref()
                .and_then(|store| store.get(key).ok().flatten())
                .is_some_and(|value| !value.trim().is_empty())
        };
        Self {
            org: value(crate::credentials::CONFIG_KEY_ORG),
            project: value(crate::credentials::CONFIG_KEY_PROJECT),
            name: value(crate::credentials::CONFIG_KEY_DEV_NAME),
            lang: value(crate::credentials::CONFIG_KEY_OUTPUT_LANG),
            foundry_endpoint: value(crate::credentials::CONFIG_KEY_FOUNDRY_ENDPOINT),
            foundry_deployment: value(crate::credentials::CONFIG_KEY_FOUNDRY_DEPLOYMENT),
            foundry_key: has(crate::credentials::CRED_KEY_FOUNDRY_API_KEY),
            pat: has(crate::credentials::CRED_KEY_PAT),
        }
    }

    /// Nada salvo: primeiro setup nesta máquina.
    pub fn is_empty(&self) -> bool {
        self.org.is_none()
            && self.project.is_none()
            && self.name.is_none()
            && self.lang.is_none()
            && self.foundry_endpoint.is_none()
            && self.foundry_deployment.is_none()
            && !self.foundry_key
            && !self.pat
    }
}

/// O que a etapa Ingestão mostra ANTES de decidir: os números da MESMA
/// caminhada do `kira ingest` (read-only — sem store, sem escrita, sem rede).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub files: usize,
    pub bytes: u64,
    pub stacks: Vec<String>,
    pub notes: Vec<String>,
}

/// Caminha a raiz detectada para o preview. `None` quando a pasta não existe
/// (máquina de teste, raiz removida) — a tela diz isso em vez de inventar.
fn preview_of(root: &str) -> Option<Preview> {
    let path = std::path::Path::new(root);
    if !path.is_dir() {
        return None;
    }
    let inv = crate::ingest::inventory::walk(path, crate::ingest::inventory::Budget::default());
    let profile = crate::ingest::profile_of(path);
    let mut notes = inv.notes.clone();
    notes.extend(profile.notes.iter().cloned());
    Some(Preview {
        files: inv.files.len(),
        bytes: inv.total_bytes,
        stacks: profile
            .all
            .iter()
            .map(|stack| stack.as_str().to_string())
            .collect(),
        notes,
    })
}

/// O que a tela mostra sobre a máquina — detectado uma vez, sem rede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvReport {
    pub os: String,
    pub arch: String,
    pub shell: String,
    pub keyring: String,
    pub config_path: String,
    pub store_path: String,
    /// Raiz do projeto detectada (mesma regra do store) — o passo "Projeto"
    /// mostra e confirma; nunca é inventada pela tela.
    pub root: String,
    pub tools: Vec<(String, bool)>,
    /// O que já está gravado (config.toml + presença no keyring).
    pub saved: SavedConfig,
}

impl EnvReport {
    /// Detecta o ambiente real (só leituras locais).
    pub fn detect() -> Self {
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let shell = detect_shell(&|name| std::env::var(name).ok());
        let keyring = match os.as_str() {
            "windows" => "Credential Manager".to_string(),
            "macos" => "Keychain".to_string(),
            _ => "secret-service (D-Bus)".to_string(),
        };
        let path_of = |value: Option<std::path::PathBuf>| {
            value
                .map(|path| crate::store::display_path(&path))
                .unwrap_or_else(|| "— (sem diretório)".to_string())
        };
        let root = crate::store::detect_project_root()
            .map(|path| crate::store::display_path(&path))
            .unwrap_or_else(|| ".".to_string());
        let tools = ["dotnet", "pnpm", "ollama", "copilot"]
            .iter()
            .map(|name| {
                let found =
                    crate::binresolve::find_on_path(std::env::var_os("PATH").as_deref(), name)
                        .is_some();
                ((*name).to_string(), found)
            })
            .collect();
        Self {
            os,
            arch,
            shell,
            keyring,
            config_path: path_of(crate::credentials::config_path()),
            store_path: path_of(crate::store::db_path()),
            root,
            tools,
            saved: SavedConfig::detect(),
        }
    }

    /// Fixture para teste de render (sem tocar no SO).
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self {
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
            shell: "PowerShell".to_string(),
            keyring: "Credential Manager".to_string(),
            config_path: r"C:\Users\dev\.config\kira\config.toml".to_string(),
            store_path: r"C:\Users\dev\.config\kira\store.db".to_string(),
            root: r"C:\repo\kira".to_string(),
            tools: vec![
                ("dotnet".to_string(), true),
                ("pnpm".to_string(), false),
                // Ollama presente: o default honesto do Enter na IA é o barato.
                ("ollama".to_string(), true),
            ],
            saved: SavedConfig::default(),
        }
    }
}

/// Nome do shell do dono — sem parsear texto localizado do SO.
pub fn detect_shell(env: &dyn Fn(&str) -> Option<String>) -> String {
    if let Some(shell) = env("SHELL").filter(|value| !value.trim().is_empty()) {
        let name = shell
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(shell.as_str())
            .to_string();
        return match name.as_str() {
            "pwsh" | "powershell" => "PowerShell".to_string(),
            "bash" => "bash".to_string(),
            "zsh" => "zsh".to_string(),
            "fish" => "fish".to_string(),
            other => other.to_string(),
        };
    }
    if env("PSModulePath").is_some() {
        "PowerShell".to_string()
    } else if cfg!(windows) {
        "cmd".to_string()
    } else {
        "sh".to_string()
    }
}

/// Estado do wizard — puro, sem I/O (o terminal só converte tecla em [`Input`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardState {
    pub step: usize,
    pub name: String,
    pub backend: Option<String>,
    pub field: Field,
    /// As três envs do Foundry estão presentes? Com elas o wizard não abre o
    /// formulário: o motor lê do ambiente, como sempre.
    pub foundry_ready: bool,
    /// De onde vem o Foundry já pronto (`None` = ainda não está utilizável).
    pub foundry_source: Option<FoundrySource>,
    /// Endpoint do Foundry (público; pré-preenchido da env ou do config).
    pub foundry_endpoint: String,
    /// Deployment/modelo do Foundry (público; pré-preenchido da env).
    pub foundry_deployment: String,
    /// Chave do Foundry digitada no formulário. NUNCA é renderizada.
    pub foundry_key: String,
    /// O formulário inline do Foundry está aberto?
    pub foundry_form: bool,
    /// Etapa Ingestão (M6): `true` = ingerir ao gravar (Enter), `false` =
    /// pular (`n`). O wizard só COLETA; quem executa é `setup::run`.
    pub ingest: bool,
    /// Idioma de saída confirmado (ADR-0038 D4): `en`, `pt-BR`, …
    pub lang: String,
    /// Rascunho do campo de idioma (só existe com o formulário aberto).
    pub lang_draft: String,
    /// O campo de idioma está em edição?
    pub lang_form: bool,
    /// Preview read-only da ingestão (números da caminhada; `None` = raiz
    /// inexistente — a tela diz isso, nunca inventa).
    pub preview: Option<Preview>,
    /// Aviso de uma linha (validação do próprio wizard, nunca de rede).
    pub notice: Option<String>,
    pub env: EnvReport,
}

impl WizardState {
    pub fn new(
        defaults: &SetupOptions,
        env: EnvReport,
        env_lookup: &dyn Fn(&str) -> Option<String>,
    ) -> Self {
        // `--name` > nome salvo > padrão do SO capitalizado > "dev".
        let name = defaults
            .name
            .clone()
            .or_else(|| env.saved.name.clone())
            .unwrap_or_else(|| super::default_dev_name_from_env(env_lookup));
        // Detecção antes de perguntar: env > config.toml salvo. A chave do
        // Foundry NUNCA é pré-preenchida (nem mascarada do valor real).
        let env_endpoint = ["KIRA_FOUNDRY_ENDPOINT", "KIRA_FOUNDRY_URL"]
            .iter()
            .find_map(|name| env_lookup(name).filter(|value| !value.trim().is_empty()));
        let env_deployment =
            env_lookup("KIRA_FOUNDRY_DEPLOYMENT").filter(|value| !value.trim().is_empty());
        let env_key =
            env_lookup("KIRA_FOUNDRY_API_KEY").is_some_and(|value| !value.trim().is_empty());
        // "Já definido" = o RUNTIME consegue resolver: env completa, ou
        // config.toml (endpoint + deployment) com a chave no keyring.
        let env_complete = env_endpoint.is_some() && env_deployment.is_some() && env_key;
        let stored_complete = env.saved.foundry_endpoint.is_some()
            && env.saved.foundry_deployment.is_some()
            && env.saved.foundry_key;
        let foundry_source = if env_complete {
            Some(FoundrySource::Env)
        } else if stored_complete {
            Some(FoundrySource::Stored)
        } else {
            None
        };
        let foundry_endpoint = env_endpoint
            .or_else(|| env.saved.foundry_endpoint.clone())
            .unwrap_or_default();
        let foundry_deployment = env_deployment
            .or_else(|| env.saved.foundry_deployment.clone())
            .unwrap_or_default();
        // Idioma: o salvo pré-preenche (nunca perguntar de novo o que a
        // máquina já sabe); sem nada salvo, `en` é o contrato interno.
        let lang = defaults
            .lang
            .clone()
            .or_else(|| env.saved.lang.clone())
            .unwrap_or_else(|| "en".to_string());
        let preview = preview_of(&env.root);
        Self {
            step: 0,
            name,
            backend: None,
            field: Field::FoundryEndpoint,
            foundry_ready: foundry_source.is_some(),
            foundry_source,
            foundry_endpoint,
            foundry_deployment,
            foundry_key: String::new(),
            foundry_form: false,
            ingest: true,
            lang,
            lang_draft: String::new(),
            lang_form: false,
            preview,
            notice: None,
            env,
        }
    }

    pub fn current(&self) -> Step {
        Step::from_index(self.step)
    }

    /// Máscara fixa: o tamanho do segredo não vaza pela tela.
    pub fn masked(&self) -> String {
        "•".repeat(MASK_LEN)
    }

    /// Situação de cada etapa, para a trilha (`✓`, `›`, `·`).
    ///
    /// `✓` só em etapa já VISITADA: valor pré-preenchido não é valor
    /// confirmado, e marcar `✓ Projeto` enquanto o dono está no Ambiente
    /// prometeria algo que ele ainda não viu.
    pub fn step_status(&self, step: Step) -> char {
        if step == self.current() {
            return '›';
        }
        let index = Step::ALL
            .iter()
            .position(|candidate| *candidate == step)
            .unwrap_or(0);
        if index > self.step {
            return '·';
        }
        let done = match step {
            Step::Ambiente => true,
            Step::Projeto => !self.env.root.trim().is_empty() && self.env.root != ".",
            // Ingestão: qualquer saída da etapa é uma decisão (Enter ingere,
            // `n` pula) — visitada é decidida.
            Step::Ingestao => true,
            // Foundry só conta como escolhido quando dá para usar: env
            // completa, config+keyring ou o formulário validado (ADR-0035).
            Step::Ia => match self.backend.as_deref() {
                Some("foundry") => self.foundry_ready || self.foundry_choice().is_some(),
                Some(_) => true,
                None => false,
            },
            Step::Resumo => false,
        };
        if done { '✓' } else { '·' }
    }

    /// Aplica uma tecla. Devolve `true` quando o wizard deve ENCERRAR com as
    /// respostas coletadas (Enter no Resumo).
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        self.notice = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                // Esc dentro de um campo fecha o campo (não sai da etapa):
                // o dono quer voltar ao menu, não perder a etapa.
                if self.lang_form {
                    self.lang_form = false;
                    self.lang_draft.clear();
                    return false;
                }
                if self.foundry_form {
                    self.foundry_form = false;
                    self.field = Field::FoundryEndpoint;
                    return false;
                }
                if self.step == 0 {
                    return false;
                }
                self.step -= 1;
                self.field = Field::FoundryEndpoint;
                return false;
            }
            KeyCode::BackTab => {
                if self.step > 0 {
                    self.step -= 1;
                }
                return false;
            }
            _ => {}
        }
        match self.current() {
            Step::Ambiente => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Tab) {
                    self.step = 1;
                }
            }
            Step::Projeto => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Tab) {
                    self.step = 2;
                }
            }
            Step::Ingestao => self.handle_ingest(key),
            Step::Ia => self.handle_ia(key),
            Step::Resumo => {
                if key.code == KeyCode::Enter {
                    return self.finish().is_some();
                }
            }
        }
        false
    }

    /// Ingestão (M6): Enter = ingerir, `n` = pular, `l` = editar o idioma de
    /// saída. Nada aqui escreve — a coleta vira `SetupOptions` e o motor
    /// executa (ADR-0034).
    fn handle_ingest(&mut self, key: KeyEvent) {
        if self.lang_form {
            match key.code {
                KeyCode::Esc => {
                    // Esc DESCARTA o rascunho: o idioma confirmado é o que
                    // estava antes, nunca o que foi rejeitado ou abandonado.
                    self.lang_form = false;
                    self.lang_draft.clear();
                }
                KeyCode::Enter => match validate_lang(&self.lang_draft) {
                    Some(value) => {
                        self.lang = value;
                        self.lang_form = false;
                        self.lang_draft.clear();
                    }
                    None => {
                        self.notice = Some(
                            "idioma inválido — use algo como en ou pt-BR (letras, dígitos, hífen)."
                                .to_string(),
                        );
                    }
                },
                KeyCode::Char(value) if !value.is_control() => {
                    self.lang_draft.push(value);
                }
                KeyCode::Backspace => {
                    self.lang_draft.pop();
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Tab => {
                self.ingest = true;
                self.step = 3;
            }
            KeyCode::Char('n') => {
                self.ingest = false;
                self.step = 3;
            }
            KeyCode::Char('l') => {
                self.lang_form = true;
                self.lang_draft.clear();
            }
            _ => {}
        }
    }

    /// IA: as teclas `1..4` escolhem (a 4 = "nenhuma por agora"); o Enter
    /// assume o default HONESTO da máquina — Ollama presente ⇒ `ollama`,
    /// ausente ⇒ `nenhuma` (o caro nunca é default).
    fn handle_ia(&mut self, key: KeyEvent) {
        if self.foundry_form {
            self.handle_foundry_form(key);
            return;
        }
        match key.code {
            KeyCode::Char('1') => {
                self.backend = Some("ollama".to_string());
                self.step = 4;
            }
            KeyCode::Char('2') => {
                self.backend = Some("copilot".to_string());
                self.step = 4;
            }
            KeyCode::Char('3') => {
                self.backend = Some("foundry".to_string());
                if self.foundry_ready {
                    // Tudo já está no ambiente: nada a digitar.
                    self.step = 4;
                } else {
                    // Formulário inline: endpoint/deployment pré-preenchidos
                    // da env; a chave nasce vazia (segredo só vem do teclado).
                    self.foundry_form = true;
                    self.field = Field::FoundryEndpoint;
                }
            }
            KeyCode::Char('4') => {
                self.backend = Some("nenhuma".to_string());
                self.step = 4;
            }
            KeyCode::Enter => {
                // Enter com foundry escolhido e formulário incompleto reabre o
                // formulário em vez de avançar com dado faltando.
                if self.backend.as_deref() == Some("foundry") && !self.foundry_ready {
                    self.foundry_form = true;
                    self.field = Field::FoundryEndpoint;
                    return;
                }
                if self.backend.is_none() {
                    // Default honesto: o que a máquina TEM, não o que seria
                    // ideal — sem Ollama, "nenhuma" é a escolha real.
                    let ollama_found = self
                        .env
                        .tools
                        .iter()
                        .any(|(name, found)| name == "ollama" && *found);
                    self.backend = Some(if ollama_found {
                        "ollama".to_string()
                    } else {
                        "nenhuma".to_string()
                    });
                }
                self.step = 4;
            }
            _ => {}
        }
    }

    /// Formulário inline do Foundry: digitar vai para o campo focado, Tab/↑/↓
    /// andam entre os três e Enter valida tudo (só avança com os três
    /// utilizáveis).
    fn handle_foundry_form(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.focus_foundry(1),
            KeyCode::Up | KeyCode::BackTab => self.focus_foundry(-1),
            KeyCode::Enter => match foundry_problem(
                &self.foundry_endpoint,
                &self.foundry_deployment,
                &self.foundry_key,
            ) {
                Some((field, message)) => {
                    self.field = field;
                    self.notice = Some(message.to_string());
                }
                None => {
                    self.step = 4;
                    self.foundry_form = false;
                }
            },
            KeyCode::Char(value) if !value.is_control() => {
                self.foundry_text_mut().push(value);
            }
            KeyCode::Backspace => {
                self.foundry_text_mut().pop();
            }
            _ => {}
        }
    }

    /// Anda o foco entre os campos do formulário, com wrap.
    fn focus_foundry(&mut self, direction: isize) {
        let index = Field::FOUNDRY
            .iter()
            .position(|field| *field == self.field)
            .unwrap_or(0) as isize;
        self.field =
            Field::FOUNDRY[(index + direction).rem_euclid(Field::FOUNDRY.len() as isize) as usize];
    }

    /// Texto do campo focado do formulário (o único destino da digitação
    /// enquanto ele está aberto).
    fn foundry_text_mut(&mut self) -> &mut String {
        match self.field {
            Field::FoundryDeployment => &mut self.foundry_deployment,
            Field::FoundryKey => &mut self.foundry_key,
            _ => &mut self.foundry_endpoint,
        }
    }

    /// O que o formulário coletou, quando o backend é foundry e os três campos
    /// estão válidos. Com a env completa devolve `None` de propósito: aí o
    /// motor lê do ambiente (`KIRA_FOUNDRY_*`), como sempre.
    pub fn foundry_choice(&self) -> Option<FoundryChoice> {
        if self.backend.as_deref() != Some("foundry") || self.foundry_ready {
            return None;
        }
        if foundry_problem(
            &self.foundry_endpoint,
            &self.foundry_deployment,
            &self.foundry_key,
        )
        .is_some()
        {
            return None;
        }
        Some(FoundryChoice {
            endpoint: self.foundry_endpoint.trim().to_string(),
            deployment: self.foundry_deployment.trim().to_string(),
            api_key: self.foundry_key.clone(),
        })
    }

    /// Devolve as opções — o "Enter" do Resumo. Local-first não tem campo
    /// obrigatório: org/projeto/credencial não são pedidos (ADR-0030 #3).
    pub fn finish(&self) -> Option<SetupOptions> {
        Some(self.into_options())
    }

    /// Converte o que foi coletado em `SetupOptions` para o motor de texto.
    ///
    /// `interactive: false` de propósito: o wizard já perguntou tudo, então o
    /// motor não reabre menu nem prompt (e não roda `stty`).
    pub fn into_options(&self) -> SetupOptions {
        let mut options = SetupOptions {
            name: Some(self.name.trim().to_string()).filter(|value| !value.is_empty()),
            // O wizard já perguntou tudo: o motor de texto não reabre menu,
            // não pergunta nome e não chama `stty`.
            interactive: false,
            // A raiz DETECTADA vai ao motor: o passo "projeto" do fluxo
            // local-first imprime/registra o que a tela confirmou.
            project_root: Some(self.env.root.clone()),
            ..SetupOptions::default()
        };
        options.backend = self.backend.clone();
        // M6: a decisão da etapa Ingestão e o idioma chegam ao motor — que é
        // o ÚNICO que executa e grava (ADR-0034).
        options.ingest = Some(self.ingest);
        options.lang = Some(self.lang.clone());
        // Só o formulário (não o ambiente) vira `FoundryChoice`: assim o motor
        // testa exatamente o que foi digitado e grava depois do teste.
        options.foundry = self.foundry_choice();
        options
    }
}

/// Guard local do terminal: raw mode + alternate screen, desfeitos no Drop
/// (inclusive quando o wizard sai por erro).
struct ScreenGuard;

impl ScreenGuard {
    fn enter() -> std::io::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

/// Roda o wizard de tela cheia e devolve as respostas (`None` = cancelado).
pub fn run(
    defaults: &SetupOptions,
    env_lookup: &dyn Fn(&str) -> Option<String>,
) -> Option<SetupOptions> {
    let _guard = ScreenGuard::enter().ok()?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout())).ok()?;
    let palette = theme::palette_from_env();
    let mut state = WizardState::new(defaults, EnvReport::detect(), env_lookup);

    loop {
        terminal.draw(|frame| render(frame, &state, palette)).ok()?;
        let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(200)) else {
            continue;
        };
        let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() else {
            continue;
        };
        if key.kind != crossterm::event::KeyEventKind::Press {
            continue;
        }
        // `q` cancela — menos onde ele é texto: no formulário do Foundry
        // (endpoint/chave podem conter "q") e no campo de idioma.
        if key.code == KeyCode::Char('q') && !state.foundry_form && !state.lang_form {
            return None;
        }
        if state.handle(key) {
            return state.finish();
        }
    }
}

/// Desenha um frame do wizard (casca do dashboard + trilha de etapas).
pub fn render(frame: &mut Frame, state: &WizardState, palette: Palette) {
    let area = frame.area();
    let shell = tui_layout::shell(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(
                "kira setup · {} · {} · {} · config {}",
                state.env.os, state.env.arch, state.env.shell, state.env.config_path
            ),
            Style::default().fg(palette.footer),
        ))),
        shell.status,
    );
    // Abaixo de 100 colunas a trilha lateral rouba 24 de largura e o texto da
    // etapa começa a truncar instrução — exatamente o que não pode acontecer.
    // Nessas telas a trilha vira um contador na linha de abas e o painel ocupa
    // a largura toda.
    let narrow = area.width < NARROW_WIDTH;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            if narrow {
                format!(
                    "etapa {}/{} · {} · Enter Avança · Esc Volta · q Cancelar",
                    state.step + 1,
                    Step::ALL.len(),
                    state.current().title()
                )
            } else {
                "configure o ambiente · Enter Avança · Esc Volta · q Cancelar".to_string()
            },
            Style::default().fg(palette.footer),
        ))),
        shell.tabs,
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(RAIL_WIDTH), Constraint::Min(20)])
        .split(shell.workspace);

    if !narrow {
        let mut rail: Vec<Line> = vec![Line::from(Span::styled(
            "ETAPAS",
            Style::default().fg(palette.missing),
        ))];
        for step in Step::ALL {
            let glyph = state.step_status(step);
            let active = step == state.current();
            let style = if active {
                Style::default()
                    .fg(palette.title)
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().fg(palette.help)
            };
            rail.push(Line::from(Span::styled(
                format!(" {glyph} {}", step.title()),
                style,
            )));
        }
        frame.render_widget(
            Paragraph::new(rail).block(panel::block("passo", true, palette)),
            cols[0],
        );
    }

    let mut body: Vec<Line> = Vec::new();
    match state.current() {
        Step::Ambiente => {
            body.push(Line::from(Span::styled(
                "detectado nesta máquina (sem rede)",
                Style::default().fg(palette.missing),
            )));
            body.push(Line::from(format!(
                " so          {} · {}",
                state.env.os, state.env.arch
            )));
            body.push(Line::from(format!(" shell       {}", state.env.shell)));
            body.push(Line::from(format!(" keyring     {}", state.env.keyring)));
            body.push(Line::from(format!(
                " config      {}",
                state.env.config_path
            )));
            body.push(Line::from(format!(" store       {}", state.env.store_path)));
            body.push(Line::from(""));
            body.push(Line::from(Span::styled(
                " ferramentas",
                Style::default().fg(palette.missing),
            )));
            for (name, found) in &state.env.tools {
                let (glyph, style) = if *found {
                    ('✓', Style::default().fg(palette.ok))
                } else {
                    ('—', Style::default().fg(palette.missing))
                };
                body.push(Line::from(Span::styled(format!("  {glyph} {name}"), style)));
            }
            body.push(Line::from(""));
            let saved = &state.env.saved;
            if saved.is_empty() {
                body.push(Line::from(Span::styled(
                    " nada salvo ainda — este é o primeiro setup nesta máquina",
                    Style::default().fg(palette.missing),
                )));
            } else {
                body.push(Line::from(Span::styled(
                    " já configurado (config.toml + keyring)",
                    Style::default().fg(palette.missing),
                )));
                if let Some(name) = &saved.name {
                    body.push(Line::from(format!("  dev          {name}")));
                }
                let foundry = match (&saved.foundry_endpoint, &saved.foundry_deployment) {
                    (Some(endpoint), Some(deployment)) => format!("{endpoint} · {deployment}"),
                    (Some(endpoint), None) => endpoint.clone(),
                    (None, Some(deployment)) => format!("(sem endpoint) · {deployment}"),
                    (None, None) => "—".to_string(),
                };
                body.push(Line::from(format!("  foundry      {foundry}")));
                // O PAT não é assunto do wizard (ADR-0030 #4): a presença da
                // credencial de board aparece no `kira doctor` e no `connect`.
                body.push(Line::from(format!(
                    "  chaves       Foundry {}",
                    mark(saved.foundry_key)
                )));
            }
        }
        Step::Projeto => {
            // Raiz DETECTADA, nunca perguntada do zero: o dono só confirma
            // (ou vê que a detecção acertou).
            body.push(Line::from(Span::styled(
                "a raiz do projeto foi detectada nesta máquina",
                Style::default().fg(palette.missing),
            )));
            body.push(Line::from(""));
            body.push(Line::from(format!(" raiz         {}", state.env.root)));
            body.push(Line::from(""));
            body.push(Line::from(Span::styled(
                " Enter confirma e avança — o store registra o projeto por esta raiz",
                Style::default().fg(palette.help),
            )));
        }
        Step::Ingestao => {
            body.push(Line::from(Span::styled(
                "conhecer o projeto: índice + digest em .kira/ — sem rede, sem modelo",
                Style::default().fg(palette.missing),
            )));
            body.push(Line::from(""));
            match &state.preview {
                Some(preview) => {
                    body.push(Line::from(format!(" arquivos     {}", preview.files)));
                    body.push(Line::from(format!(
                        " bytes        {}",
                        preview.bytes / 1024
                    )));
                    let stacks = if preview.stacks.is_empty() {
                        "—".to_string()
                    } else {
                        preview.stacks.join(", ")
                    };
                    body.push(Line::from(format!(" stacks       {stacks}")));
                    for note in preview.notes.iter().take(3) {
                        body.push(Line::from(format!(" nota         {note}")));
                    }
                }
                None => {
                    body.push(Line::from(Span::styled(
                        " raiz não legível — o preview aparece quando a pasta existe",
                        Style::default().fg(palette.missing),
                    )));
                }
            }
            body.push(Line::from(""));
            let lang_shown = if state.lang_form {
                format!("{}▌", state.lang_draft)
            } else {
                state.lang.clone()
            };
            body.push(Line::from(format!(
                " idioma       {lang_shown} (posts, cards e PR)"
            )));
            body.push(Line::from(Span::styled(
                " Enter ingere · n pula · l muda o idioma — nada roda antes do Enter no Resumo",
                Style::default().fg(palette.help),
            )));
        }
        Step::Ia => {
            body.push(Line::from(Span::styled(
                "o caro nunca é default nem fallback silencioso",
                Style::default().fg(palette.missing),
            )));
            body.push(Line::from(""));
            body.push(menu_line(
                state.backend.as_deref() == Some("ollama"),
                "1) Ollama local (0 custo, nada sai da máquina)",
            ));
            body.push(menu_line(
                state.backend.as_deref() == Some("copilot"),
                "2) Copilot CLI (usa sua assinatura)",
            ));
            body.push(menu_line(
                state.backend.as_deref() == Some("foundry"),
                "3) Microsoft Foundry (BYOK: custo por token)",
            ));
            body.push(menu_line(
                state.backend.as_deref() == Some("nenhuma"),
                "4) nenhuma por agora (scan, wiki, context, pr draft e dashboard funcionam sem IA)",
            ));
            if state.foundry_form {
                body.push(Line::from(""));
                body.push(Line::from(Span::styled(
                    "endpoint/deployment vão para config.toml; a chave vai para o keyring do OS",
                    Style::default().fg(palette.help),
                )));
                if state.env.saved.foundry_endpoint.as_deref()
                    == Some(state.foundry_endpoint.as_str())
                    && !state.foundry_endpoint.is_empty()
                {
                    body.push(Line::from(Span::styled(
                        " endpoint/deployment vieram do config.toml salvo",
                        Style::default().fg(palette.help),
                    )));
                }
                body.push(Line::from(""));
                field_line(
                    &mut body,
                    "endpoint",
                    &state.foundry_endpoint,
                    state.field,
                    Field::FoundryEndpoint,
                    palette,
                );
                field_line(
                    &mut body,
                    "deployment",
                    &state.foundry_deployment,
                    state.field,
                    Field::FoundryDeployment,
                    palette,
                );
                masked_line(
                    &mut body,
                    &state.foundry_key,
                    state.field,
                    Field::FoundryKey,
                    palette,
                );
                body.push(Line::from(""));
                body.push(Line::from(Span::styled(
                    " Tab anda entre os campos · Enter valida e avança · Esc fecha o formulário",
                    Style::default().fg(palette.help),
                )));
            } else if state.foundry_ready {
                // Diz ONDE está configurado: "já definido" sem origem é
                // exatamente o tipo de mensagem que não ensina nada.
                let origem = match state.foundry_source {
                    Some(FoundrySource::Stored) => "config.toml + keyring do OS",
                    _ => "ambiente (KIRA_FOUNDRY_*)",
                };
                body.push(Line::from(""));
                body.push(Line::from(Span::styled(
                    format!("foundry já definido em {origem}"),
                    Style::default().fg(palette.ok),
                )));
                body.push(Line::from(Span::styled(
                    "3 avança sem pedir nada; Esc volta",
                    Style::default().fg(palette.help),
                )));
            }
        }
        Step::Resumo => {
            body.push(Line::from(Span::styled(
                "o que será gravado ao continuar",
                Style::default().fg(palette.missing),
            )));
            body.push(Line::from(""));
            body.push(Line::from(format!(" raiz         {}", state.env.root)));
            body.push(Line::from(format!(" dev          {}", state.name)));
            body.push(Line::from(format!(
                " ingestão     {}",
                if state.ingest {
                    "sim (índice + digest, sem scan)"
                } else {
                    "não (ativar depois: kira ingest)"
                }
            )));
            body.push(Line::from(format!(
                " idioma       {} (posts, cards e PR)",
                state.lang
            )));
            let ia = match state.backend.as_deref() {
                Some(backend) => backend.to_string(),
                None => {
                    // O default do Enter é honesto: o que a máquina tem.
                    let ollama_found = state
                        .env
                        .tools
                        .iter()
                        .any(|(name, found)| name == "ollama" && *found);
                    if ollama_found {
                        "ollama (default)".to_string()
                    } else {
                        "nenhuma (default)".to_string()
                    }
                }
            };
            body.push(Line::from(format!(" IA           {ia}")));
            // Foundry coletado no formulário: o resumo mostra o que será
            // gravado e ONDE — endpoint/deployment são públicos (config),
            // a chave é segredo (keyring).
            if let Some(choice) = state.foundry_choice() {
                body.push(Line::from(format!(" endpoint     {}", choice.endpoint)));
                body.push(Line::from(format!(" deployment   {}", choice.deployment)));
                body.push(Line::from(format!(
                    " chave        {} (keyring do OS)",
                    mask_for(&state.foundry_key)
                )));
            } else if state.backend.as_deref() == Some("foundry") {
                // A origem é a mesma regra que o runtime aplica: dizer "do
                // ambiente" quando veio do config seria mentira.
                let origem = match state.foundry_source {
                    Some(FoundrySource::Stored) => "do config.toml + keyring do OS",
                    _ => "do ambiente (KIRA_FOUNDRY_*)",
                };
                body.push(Line::from(Span::styled(
                    format!(" endpoint/deployment  {origem}"),
                    Style::default().fg(palette.help),
                )));
            }
            body.push(Line::from(""));
            body.push(Line::from(Span::styled(
                " nada é gravado antes do Enter; a credencial do board nasce em kira connect",
                Style::default().fg(palette.help),
            )));
        }
    }
    if let Some(notice) = &state.notice {
        body.push(Line::from(""));
        body.push(Line::from(Span::styled(
            format!(" {notice}"),
            Style::default().fg(palette.failed),
        )));
    }
    let body_area = if narrow { shell.workspace } else { cols[1] };
    frame.render_widget(
        Paragraph::new(body).block(panel::block(state.current().title(), true, palette)),
        body_area,
    );

    let keys: Vec<Key> = match state.current() {
        Step::Resumo => vec![
            Key {
                key: "Enter",
                desc: "Gravar",
            },
            Key {
                key: "Esc",
                desc: "Voltar",
            },
            Key {
                key: "q",
                desc: "Cancelar",
            },
        ],
        // Acesso removido (ADR-0030 #4): o PAT não é assunto do wizard —
        // o rodapé da IA cobre as 4 opções do menu.
        Step::Ingestao if state.lang_form => vec![
            Key {
                key: "Enter",
                desc: "Validar",
            },
            Key {
                key: "Esc",
                desc: "Descartar",
            },
        ],
        Step::Ingestao => vec![
            Key {
                key: "Enter",
                desc: "Ingerir",
            },
            Key {
                key: "n",
                desc: "Pular",
            },
            Key {
                key: "l",
                desc: "Idioma",
            },
            Key {
                key: "Esc",
                desc: "Voltar",
            },
            Key {
                key: "q",
                desc: "Cancelar",
            },
        ],
        Step::Ia if state.foundry_form => vec![
            Key {
                key: "Tab",
                desc: "Campo",
            },
            Key {
                key: "Enter",
                desc: "Validar",
            },
            Key {
                key: "Esc",
                desc: "Fechar",
            },
        ],
        Step::Ia => vec![
            Key {
                key: "1-4",
                desc: "Escolher",
            },
            Key {
                key: "Enter",
                desc: "Avançar",
            },
            Key {
                key: "Esc",
                desc: "Voltar",
            },
            Key {
                key: "q",
                desc: "Cancelar",
            },
        ],
        _ => vec![
            Key {
                key: "Enter",
                desc: "Avançar",
            },
            Key {
                key: "Tab",
                desc: "Campo",
            },
            Key {
                key: "Esc",
                desc: "Voltar",
            },
            Key {
                key: "q",
                desc: "Cancelar",
            },
        ],
    };
    frame.render_widget(
        Paragraph::new(Line::from(footer::spans(&keys, palette))),
        shell.footer,
    );
}

/// Linha de campo com rótulo alinhado e cursor quando em edição.
fn field_line(
    body: &mut Vec<Line>,
    label: &str,
    value: &str,
    field: Field,
    target: Field,
    palette: Palette,
) {
    let active = field == target;
    let shown = if value.is_empty() && !active {
        "— (vazio)".to_string()
    } else if active {
        format!("{value}▌")
    } else {
        value.to_string()
    };
    let style = if active {
        Style::default().fg(palette.title)
    } else {
        Style::default().fg(palette.value)
    };
    body.push(Line::from(Span::styled(
        format!(" {label:<12} {shown}"),
        style,
    )));
}

fn menu_line(selected: bool, text: &str) -> Line<'static> {
    let marker = if selected { "➤ " } else { "  " };
    Line::from(format!(" {marker}{text}"))
}

/// Glifo de presença para a tela (`✓` existe, `—` não existe).
fn mark(present: bool) -> &'static str {
    if present { "✓" } else { "—" }
}

/// Máscara de tamanho FIXO: o comprimento do segredo não vaza pela tela.
fn mask_for(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        "•".repeat(MASK_LEN)
    }
}

/// Linha de campo de SEGREDO: nunca mostra o valor, só a máscara.
fn masked_line(body: &mut Vec<Line>, value: &str, field: Field, target: Field, palette: Palette) {
    let active = field == target;
    let shown = match (value.is_empty(), active) {
        (true, true) => "▌".to_string(),
        (true, false) => "— (vazio)".to_string(),
        (false, true) => format!("{}▌", mask_for(value)),
        (false, false) => mask_for(value),
    };
    let style = if active {
        Style::default().fg(palette.title)
    } else {
        Style::default().fg(palette.value)
    };
    body.push(Line::from(Span::styled(
        format!(" {:<12} {shown}", target.label()),
        style,
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |name: &str| {
            owned
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn state() -> WizardState {
        WizardState::new(
            &SetupOptions::default(),
            EnvReport::fixture(),
            &env_of(&[("USER", "sergin")]),
        )
    }

    /// Ambiente com tudo já salvo (config.toml + keyring).
    fn env_with_saved() -> EnvReport {
        let mut env = EnvReport::fixture();
        env.saved = SavedConfig {
            org: Some("minha-org".to_string()),
            project: Some("Kira".to_string()),
            name: Some("Serginho".to_string()),
            lang: Some("pt-BR".to_string()),
            foundry_endpoint: Some("https://recurso.openai.azure.com/openai/v1".to_string()),
            foundry_deployment: Some("gpt-5".to_string()),
            foundry_key: true,
            pat: true,
        };
        env
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn mode_only_opens_the_wizard_in_a_big_tty_without_explicit_flags() {
        assert_eq!(mode(true, 120, 40, false, false), Mode::Wizard);
        assert_eq!(mode(false, 120, 40, false, false), Mode::Plain, "sem TTY");
        assert_eq!(
            mode(true, 79, 40, false, false),
            Mode::Plain,
            "terminal pequeno"
        );
        assert_eq!(
            mode(true, 120, 23, false, false),
            Mode::Plain,
            "terminal baixo"
        );
        assert_eq!(mode(true, 120, 40, true, false), Mode::Plain, "--plain");
        assert_eq!(
            mode(true, 120, 40, false, true),
            Mode::Plain,
            "flag explícita vence: o dono já disse o que quer"
        );
    }

    #[test]
    fn shell_detection_prefers_the_posix_shell_and_falls_back_to_powershell() {
        assert_eq!(detect_shell(&env_of(&[("SHELL", "/usr/bin/zsh")])), "zsh");
        assert_eq!(
            detect_shell(&env_of(&[(
                "SHELL",
                "C:\\Program Files\\PowerShell\\7\\pwsh.exe"
            )])),
            "pwsh.exe"
        );
        let windows_like = env_of(&[("PSModulePath", "C:\\...")]);
        let detected = detect_shell(&windows_like);
        assert!(
            detected == "PowerShell" || detected == "sh",
            "sem SHELL, o melhor palpite é PSModulePath (ou sh fora do Windows): {detected}"
        );
    }

    #[test]
    fn detected_environment_feeds_the_rail_and_walks_to_the_project_step() {
        let mut state = state();
        assert_eq!(state.current(), Step::Ambiente);
        assert_eq!(state.step_status(Step::Ambiente), '›');
        assert_eq!(state.step_status(Step::Projeto), '·');
        // Enter no Ambiente leva ao Projeto; o wizard NÃO coleta org/projeto
        // nem credencial (ADR-0030 #3): o nome já vem do SO, a raiz é
        // detectada.
        assert!(!state.handle(key(KeyCode::Enter)));
        assert_eq!(state.current(), Step::Projeto);
    }

    #[test]
    fn project_step_confirms_the_detected_root_and_advances() {
        let mut state = state();
        state.handle(key(KeyCode::Enter)); // Ambiente → Projeto
        // A raiz mostrada é a DETECTADA (injetada no EnvReport) — nunca
        // perguntada do zero.
        assert_eq!(state.env.root, r"C:\repo\kira");
        assert_eq!(state.step_status(Step::Projeto), '›');
        state.handle(key(KeyCode::Enter));
        assert_eq!(
            state.current(),
            Step::Ingestao,
            "Enter confirma a raiz e avança para a ingestão"
        );
        // Depois de visitar, a etapa aparece como concluída na trilha.
        assert_eq!(state.step_status(Step::Projeto), '✓');
    }

    #[test]
    fn ingestao_step_collects_the_choice_and_the_language() {
        // Enter = ingerir; a decisão chega ao motor como Some(true).
        let mut enter = state();
        enter.step = 2;
        assert!(!enter.handle(key(KeyCode::Enter)));
        assert_eq!(enter.current(), Step::Ia);
        assert!(enter.ingest);
        assert_eq!(enter.into_options().ingest, Some(true));

        // `n` = pular.
        let mut skip = state();
        skip.step = 2;
        skip.handle(key(KeyCode::Char('n')));
        assert!(!skip.ingest);
        assert_eq!(skip.current(), Step::Ia);
        assert_eq!(skip.into_options().ingest, Some(false));

        // `l` abre o campo; só código válido fecha com Enter.
        let mut lang = state();
        lang.step = 2;
        lang.handle(key(KeyCode::Char('l')));
        assert!(lang.lang_form);
        for value in "pt BR".chars() {
            lang.handle(key(KeyCode::Char(value)));
        }
        lang.handle(key(KeyCode::Enter));
        assert!(lang.lang_form, "espaço não é código de idioma");
        assert!(lang.notice.is_some(), "a recusa diz o que usar");
        // Esc descarta o rascunho: o confirmado é o anterior.
        lang.handle(key(KeyCode::Esc));
        assert!(!lang.lang_form);
        assert_eq!(lang.lang, "en");
        // Agora o valor válido entra.
        lang.handle(key(KeyCode::Char('l')));
        for value in "pt-BR".chars() {
            lang.handle(key(KeyCode::Char(value)));
        }
        lang.handle(key(KeyCode::Enter));
        assert!(!lang.lang_form);
        assert_eq!(lang.lang, "pt-BR");
        let options = lang.into_options();
        assert_eq!(options.lang.as_deref(), Some("pt-BR"));
        assert_eq!(
            options.ingest,
            Some(true),
            "a escolha do Enter chega ao motor"
        );
    }

    #[test]
    fn ingestao_screen_shows_preview_or_the_honest_absence_and_never_runs_anything() {
        // Raiz do fixture não existe: o preview diz "não legível" em vez de
        // inventar número (a regra de dado ausente com causa).
        let mut state = state();
        state.step = 2;
        let text = screen_of(&state, 120, 40);
        assert!(text.contains("Ingestão"), "{text}");
        assert!(text.contains("raiz não legível"), "{text}");
        assert!(text.contains("idioma"), "{text}");
        assert!(text.contains("Enter ingere"), "{text}");
        assert!(!text.contains("PAT"), "{text}");

        // Com preview, os números aparecem.
        let mut shown = state.clone();
        shown.preview = Some(Preview {
            files: 181,
            bytes: 3_500_000,
            stacks: vec!["rust".to_string()],
            notes: vec![],
        });
        let text = screen_of(&shown, 120, 40);
        assert!(text.contains("181"), "{text}");
        assert!(text.contains("rust"), "{text}");
    }

    #[test]
    fn ia_choices_are_explicit_and_the_enter_default_is_honest() {
        let mut with_ollama = state();
        with_ollama.step = 3; // IA
        // Ollama PRESENTE no fixture: Enter = ollama (o barato da máquina).
        with_ollama.handle(key(KeyCode::Enter));
        assert_eq!(
            with_ollama.backend.as_deref(),
            Some("ollama"),
            "com Ollama, Enter = barato"
        );
        assert_eq!(with_ollama.current(), Step::Resumo);

        // Escolhas explícitas: 2 = copilot, 4 = nenhuma por agora.
        let mut none = with_ollama.clone();
        none.step = 3;
        none.backend = None;
        none.handle(key(KeyCode::Char('4')));
        assert_eq!(none.backend.as_deref(), Some("nenhuma"));
        assert_eq!(
            none.into_options().backend.as_deref(),
            Some("nenhuma"),
            "a 4ª opção chega ao motor como escolha explícita"
        );

        // Sem Ollama na máquina, o Enter NÃO escolhe um pré-requisito
        // inexistente: cai em "nenhuma" (default honesto).
        let mut sem_ollama = state();
        sem_ollama.env.tools = vec![("dotnet".to_string(), true), ("ollama".to_string(), false)];
        sem_ollama.step = 3;
        sem_ollama.handle(key(KeyCode::Enter));
        assert_eq!(
            sem_ollama.backend.as_deref(),
            Some("nenhuma"),
            "sem Ollama, o default é 'nenhuma por agora'"
        );
    }

    #[test]
    fn esc_walks_back_one_step_and_never_writes() {
        let mut state = state();
        state.step = 4;
        assert!(!state.handle(key(KeyCode::Esc)));
        assert_eq!(state.step, 3);
        state.step = 0;
        assert!(
            !state.handle(key(KeyCode::Esc)),
            "Esc no primeiro passo não encerra"
        );
        assert_eq!(state.step, 0);
    }

    #[test]
    fn foundry_opens_the_inline_form_and_keeps_the_key_masked() {
        let mut state = state();
        state.step = 3;
        state.handle(key(KeyCode::Char('3')));
        assert_eq!(state.current(), Step::Ia, "não avança sem os três valores");
        assert!(state.foundry_form, "o formulário abre inline");
        assert_eq!(state.field, Field::FoundryEndpoint);
        // Os três campos, na ordem do Tab.
        for value in "https://r.openai.azure.com/openai/v1".chars() {
            state.handle(key(KeyCode::Char(value)));
        }
        state.handle(key(KeyCode::Tab));
        assert_eq!(state.field, Field::FoundryDeployment);
        for value in "gpt-5".chars() {
            state.handle(key(KeyCode::Char(value)));
        }
        state.handle(key(KeyCode::Tab));
        assert_eq!(state.field, Field::FoundryKey);
        for value in "«redacted:sk-…»".chars() {
            state.handle(key(KeyCode::Char(value)));
        }
        assert_eq!(state.foundry_key, "«redacted:sk-…»");
        assert!(
            !mask_for(&state.foundry_key).contains("segredo"),
            "a máscara nunca contém o valor"
        );
        assert_eq!(
            mask_for(&state.foundry_key).chars().count(),
            MASK_LEN,
            "o comprimento real não vaza"
        );

        // Enter valida os três e avança; a escolha chega ao motor.
        state.handle(key(KeyCode::Enter));
        assert_eq!(state.step, 4, "com os três válidos o wizard avança");
        assert!(!state.foundry_form, "o formulário fecha ao avançar");
        let choice = state
            .foundry_choice()
            .unwrap_or_else(|| panic!("formulário válido deve virar choice"));
        assert_eq!(choice.endpoint, "https://r.openai.azure.com/openai/v1");
        assert_eq!(choice.deployment, "gpt-5");
        assert_eq!(choice.api_key, "«redacted:sk-…»");
        assert_eq!(state.step_status(Step::Ia), '✓', "etapa IA concluída");
        let options = state.into_options();
        assert_eq!(
            options
                .foundry
                .as_ref()
                .map(|foundry| foundry.deployment.as_str()),
            Some("gpt-5"),
            "o motor recebe exatamente o que foi digitado"
        );
    }

    #[test]
    fn foundry_form_rejects_a_project_endpoint_and_empty_fields() {
        assert_eq!(
            foundry_problem("", "gpt-5", "k").map(|(_, cause)| cause.contains("endpoint")),
            Some(true)
        );
        assert_eq!(
            foundry_problem("recurso.openai.azure.com", "gpt-5", "k").map(|(field, _)| field),
            Some(Field::FoundryEndpoint),
            "sem esquema não é endpoint"
        );
        let project = "https://x.services.ai.azure.com/api/projects/p";
        assert_eq!(
            foundry_problem(project, "gpt-5", "k").map(|(_, msg)| msg.contains("/openai/v1")),
            Some(true),
            "endpoint de projeto é recusado com a saída"
        );
        assert_eq!(
            foundry_problem("https://x.openai.azure.com/openai/v1", "", "k").map(|(f, _)| f),
            Some(Field::FoundryDeployment)
        );
        assert_eq!(
            foundry_problem("https://x.openai.azure.com/openai/v1", "gpt 5", "k")
                .map(|(field, _)| field),
            Some(Field::FoundryDeployment),
            "nome de deployment não tem espaço"
        );
        assert_eq!(
            foundry_problem("https://x.openai/v1", "gpt-5", "  ").map(|(f, _)| f),
            Some(Field::FoundryKey)
        );
        assert!(
            foundry_problem("https://x.openai.azure.com/openai/v1", "gpt-5", "k").is_none(),
            "os três válidos não têm problema"
        );
    }

    #[test]
    fn foundry_form_prefills_public_values_from_the_env_but_never_the_key() {
        let mut state = WizardState::new(
            &SetupOptions::default(),
            EnvReport::fixture(),
            &env_of(&[
                (
                    "KIRA_FOUNDRY_ENDPOINT",
                    "https://detectado.openai.azure.com/openai/v1",
                ),
                ("KIRA_FOUNDRY_DEPLOYMENT", "gpt-4o-mini"),
            ]),
        );
        state.step = 3;
        state.handle(key(KeyCode::Char('3')));
        assert!(state.foundry_form, "sem chave, o formulário abre");
        assert_eq!(
            state.foundry_endpoint, "https://detectado.openai.azure.com/openai/v1",
            "endpoint detectado é pré-preenchido"
        );
        assert_eq!(state.foundry_deployment, "gpt-4o-mini");
        assert!(state.foundry_key.is_empty(), "segredo nunca vem de fora");

        // Com as TRÊS envs, não há o que digitar: avança direto e o motor lê
        // do ambiente (choice = None).
        let mut ready = WizardState::new(
            &SetupOptions::default(),
            EnvReport::fixture(),
            &env_of(&[
                (
                    "KIRA_FOUNDRY_ENDPOINT",
                    "https://x.openai.azure.com/openai/v1",
                ),
                ("KIRA_FOUNDRY_DEPLOYMENT", "gpt-5"),
                ("KIRA_FOUNDRY_API_KEY", "k"),
            ]),
        );
        ready.step = 3;
        ready.handle(key(KeyCode::Char('3')));
        assert_eq!(ready.current(), Step::Resumo, "env completa avança");
        assert_eq!(ready.backend.as_deref(), Some("foundry"));
        assert!(
            ready.foundry_choice().is_none(),
            "nada é gravado sem formulário"
        );
    }

    #[test]
    fn esc_closes_only_the_foundry_form_not_the_step() {
        let mut state = state();
        state.step = 3;
        state.handle(key(KeyCode::Char('3')));
        assert!(state.foundry_form);
        state.handle(key(KeyCode::Esc));
        assert!(!state.foundry_form, "Esc fecha o formulário");
        assert_eq!(state.current(), Step::Ia, "sem sair da etapa");
        // Enter com foundry escolhido reabre (nada de avançar sem os dados).
        state.handle(key(KeyCode::Enter));
        assert!(state.foundry_form);
    }

    #[test]
    fn finish_has_no_required_field_and_the_resumo_enter_returns_options() {
        let mut state = state();
        state.step = 4;
        // Local-first não tem campo obrigatório: org/projeto/credencial não
        // são pedidos (ADR-0030 #3), então o Resumo sempre entrega opções.
        let options = state.finish().unwrap_or_else(|| panic!("resumo válido"));
        assert_eq!(options.org, None, "org nunca é coletada pelo wizard");
        assert_eq!(options.project, None, "projeto nunca é coletado");
        assert!(options.pat.is_none(), "PAT nunca é coletado");
        assert!(options.skip.is_empty(), "nada é pulado por coleta");
        assert_eq!(
            options.project_root.as_deref(),
            Some(r"C:\repo\kira"),
            "a raiz detectada vai ao motor"
        );
        assert!(!options.interactive, "o wizard já perguntou tudo");
        assert_eq!(options.name.as_deref(), Some("Sergin"));
        // M6: ingestão e idioma chegam decididos ao motor (defaults: ingerir, en).
        assert_eq!(options.ingest, Some(true));
        assert_eq!(options.lang.as_deref(), Some("en"));
        // Enter no resumo encerra com respostas.
        assert!(state.handle(key(KeyCode::Enter)));
    }

    /// Desenha um frame do wizard em `TestBackend` e devolve o texto da tela.
    fn screen_of(state: &WizardState, width: u16, height: u16) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                .unwrap_or_else(|err| panic!("{err}"));
        terminal
            .draw(|frame| render(frame, state, theme::mono()))
            .unwrap_or_else(|err| panic!("{err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..height {
            for x in 0..width {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn render_shows_the_rail_and_never_a_credential_question() {
        let mut state = state();
        // Passa por TODAS as etapas e confirma que nenhuma pergunta de
        // credencial aparece (ADR-0030 #3: o setup não pede PAT).
        let mut text = String::new();
        for step in 0..Step::ALL.len() {
            state.step = step;
            text.push_str(&screen_of(&state, 120, 40));
        }
        assert!(text.contains("ETAPAS"), "{text}");
        assert!(text.contains("Ambiente"), "{text}");
        assert!(text.contains("Projeto"), "{text}");
        assert!(text.contains("raiz"), "{text}");
        assert!(
            !text.contains("PAT"),
            "a palavra PAT não pode aparecer em nenhuma etapa do wizard: {text}"
        );
        assert!(
            !text.contains("Work Items"),
            "aula do PAT não é do setup: {text}"
        );
        // O formulário do Foundry mostra o rótulo "chave", nunca o valor.
        let mut foundry = state.clone();
        foundry.step = 3;
        foundry.handle(key(KeyCode::Char('3')));
        for value in "https://r.openai.azure.com/openai/v1".chars() {
            foundry.handle(key(KeyCode::Char(value)));
        }
        foundry.handle(key(KeyCode::Tab));
        for value in "gpt-5".chars() {
            foundry.handle(key(KeyCode::Char(value)));
        }
        foundry.handle(key(KeyCode::Tab));
        for value in "segredo-brutal".chars() {
            foundry.handle(key(KeyCode::Char(value)));
        }
        let text = screen_of(&foundry, 120, 40);
        assert!(text.contains("chave"), "{text}");
        assert!(
            !text.contains("segredo-brutal"),
            "a chave digitada NUNCA na tela: {text}"
        );
        assert!(text.contains("•"), "máscara visível: {text}");
    }

    #[test]
    fn ambiente_lists_what_is_already_configured_without_pat() {
        let state: WizardState =
            WizardState::new(&SetupOptions::default(), env_with_saved(), &no_env);
        let text = screen_of(&state, 120, 40);
        assert!(text.contains("já configurado"), "{text}");
        assert!(text.contains("Serginho"), "{text}");
        assert!(
            text.contains("https://recurso.openai.azure.com/openai/v1 · gpt-5"),
            "endpoint e deployment salvos aparecem: {text}"
        );
        assert!(text.contains("Foundry ✓"), "presença da chave: {text}");
        assert!(
            !text.contains("PAT"),
            "a presença do PAT não é assunto do wizard: {text}"
        );

        // Máquina limpa: a mesma seção diz que não há nada, em vez de sumir.
        let fresh = WizardState::new(&SetupOptions::default(), EnvReport::fixture(), &no_env);
        let text = screen_of(&fresh, 120, 40);
        assert!(text.contains("nada salvo ainda"), "{text}");
    }

    #[test]
    fn rail_marks_only_visited_steps_as_done() {
        let mut state = WizardState::new(&SetupOptions::default(), env_with_saved(), &no_env);
        // Etapas à frente ainda não foram visitadas: nunca '✓' adiantado.
        assert_eq!(state.step_status(Step::Projeto), '·');
        assert_eq!(state.step_status(Step::Ingestao), '·');
        assert_eq!(state.step_status(Step::Ia), '·');
        state.step = 1;
        assert_eq!(state.step_status(Step::Ambiente), '✓');
        assert_eq!(state.step_status(Step::Ia), '·');
        // Depois de passar pela IA, as etapas anteriores mostram seu estado —
        // inclusive a Ingestão, onde qualquer saída é uma decisão.
        state.step = 3;
        assert_eq!(state.step_status(Step::Projeto), '✓');
        assert_eq!(state.step_status(Step::Ingestao), '✓');
    }

    #[test]
    fn saved_foundry_with_the_key_in_the_keyring_skips_the_form_and_says_where() {
        let mut state = WizardState::new(&SetupOptions::default(), env_with_saved(), &no_env);
        assert_eq!(state.foundry_source, Some(FoundrySource::Stored));
        state.step = 3;
        let text = screen_of(&state, 120, 40);
        assert!(
            text.contains("foundry já definido em config.toml + keyring do OS"),
            "a origem aparece: {text}"
        );
        // `3` avança sem abrir formulário e sem regravar nada (o motor usa o
        // que já está salvo).
        state.handle(key(KeyCode::Char('3')));
        assert_eq!(state.current(), Step::Resumo);
        assert!(!state.foundry_form);
        assert!(
            state.into_options().foundry.is_none(),
            "nada a gravar de novo quando veio do config + keyring"
        );
    }

    #[test]
    fn saved_public_values_prefill_the_foundry_form() {
        let state = WizardState::new(&SetupOptions::default(), env_with_saved(), &no_env);
        assert_eq!(
            state.name, "Serginho",
            "dev_name salvo vence o usuário do SO"
        );
        assert_eq!(
            state.foundry_endpoint,
            "https://recurso.openai.azure.com/openai/v1"
        );
        assert!(
            state.foundry_key.is_empty(),
            "segredo nunca é pré-preenchido"
        );

        // Sem a chave no keyring o formulário abre — já com os valores
        // públicos salvos e dizendo de onde vieram.
        let mut partial = env_with_saved();
        partial.saved.foundry_key = false;
        let mut state = WizardState::new(&SetupOptions::default(), partial, &no_env);
        assert_eq!(state.foundry_source, None);
        state.step = 3;
        state.handle(key(KeyCode::Char('3')));
        assert!(state.foundry_form, "sem chave, o formulário abre");
        assert_eq!(
            state.foundry_endpoint, "https://recurso.openai.azure.com/openai/v1",
            "endpoint salvo é reaproveitado"
        );
        let text = screen_of(&state, 120, 40);
        assert!(
            text.contains("vieram do config.toml salvo"),
            "diz a origem do preenchimento: {text}"
        );
    }

    #[test]
    fn narrow_terminal_trades_the_rail_for_the_step_counter_and_keeps_the_help() {
        let mut state = state();
        state.step = 3;
        state.handle(key(KeyCode::Char('3')));

        let text = screen_of(&state, 80, 24);
        assert!(
            text.contains("etapa 4/5"),
            "a trilha vira contador em 80 colunas: {text}"
        );
        assert!(text.contains("IA"), "etapa nomeada: {text}");
        assert!(
            !text.contains("ETAPAS"),
            "sem trilha lateral em tela estreita: {text}"
        );
        // A instrução do formulário cabe inteira (nada de corte por largura).
        assert!(
            text.contains("/openai/v1") || text.contains("endpoint"),
            "instrução do formulário presente: {text}"
        );
    }
}
