//! `kira model` — catálogo local de modelos via Ollama (HF-2; ADR-0040).
//!
//! Hugging Face é CATÁLOGO, nunca runtime: quem baixa e executa o modelo é o
//! Ollama, sempre (binário único e leve é decisão selada — ADR-0030). Este
//! módulo orquestra `ollama list` (leitura, sempre livre) e `ollama pull`
//! (escrita no disco do dev — só com consentimento explícito `--yes` OU
//! prompt interativo em TTY, com disco checado antes; nunca download
//! silencioso).
//!
//! Regras que o módulo respeita:
//! - **Resolução antes do spawn** (ADR-0016): `ollama` é resolvido
//!   `which`-style no PATH (via [`crate::binresolve`]) ou pela override
//!   `KIRA_MODEL_OLLAMA_BIN` (mesmo contrato do `KIRA_SCAN_*_BIN`: valor
//!   presente é contrato do usuário — inexistente é o mesmo "instale o X").
//! - **Consentimento é estado, não texto**: sem `--yes` e sem TTY o pull
//!   NEM COMEÇA — erro acionável, nenhum spawn do ollama. Em TTY, o prompt
//!   mostra modelo/destino/espaço livre e qualquer resposta que não seja
//!   y/yes/s/sim recusa (EOF idem — Enter vazio nunca baixa).
//! - **Disco checado, nunca inventado**: o espaço livre do destino é medido
//!   (`OLLAMA_MODELS` > `~/.ollama/models`); sem medição, a linha diz "—"
//!   com a causa — número fabricado é proibido aqui como em toda tela.
//!   O tamanho do download em si o Ollama mostra durante o pull (o KIRA não
//!   fala com a API do Hugging Face — ADR-0040).
//! - **Pull não é chamada de LLM**: o budget diário (`llm_calls`) mede
//!   inferência, não trabalho — baixar modelo não consome o contador.
//! - **Trilha automática**: o `Outcome` do comando grava a sessão no store
//!   (log de auditoria do pull), com o mesmo fail-open de sempre.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Env override do binário `ollama` (shim dos testes e2e; embalagem própria).
/// Envs novas existem só com justificativa no PR: mesma conduta do
/// `KIRA_SCAN_*_BIN` e do `KIRA_COPILOT_BIN`.
pub const OLLAMA_BIN_ENV: &str = "KIRA_MODEL_OLLAMA_BIN";

/// Erros do `kira model`: todos acionáveis — nenhum body cru, nenhum texto
/// de OS classificado por conteúdo.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    /// `ollama` não existe no PATH nem via override — nada foi spawnado.
    #[error(
        "o comando exige o ollama — instale em https://ollama.com e rode de novo \
         (para apontar outro binário, use a env {OLLAMA_BIN_ENV})"
    )]
    OllamaMissing,

    /// `ollama list` rodou e saiu != 0: dica compacta (1ª linha do stderr),
    /// nunca o body cru.
    #[error(
        "o `ollama list` falhou (exit {exit}) — confira versão/instalação e rode de novo{hint}"
    )]
    ListFailed {
        exit: i32,
        /// ` (dica: ...)` já formatado, ou vazio.
        hint: String,
    },

    /// Saída do `ollama list` não parseou (formato inesperado de versão nova).
    #[error(
        "saída do `ollama list` ilegível — confira rodando `ollama list` direto; \
         se o formato mudou, é atualização do KIRA"
    )]
    ListUnreadable,

    /// Pull sem `--yes` e sem TTY: consentimento é pré-requisito, não cortesia.
    #[error(
        "download de modelo é escrita no disco — confirme com `kira model pull {repo} --yes` \
         ou rode em um terminal interativo (a pergunta mostra destino e espaço livre)"
    )]
    ConsentRequired { repo: String },

    /// O dev recusou no prompt (ou deu Enter vazio / EOF): nada baixado.
    #[error("download cancelado — nada foi baixado")]
    Refused,

    /// `ollama pull` rodou e saiu != 0. Com stdio herdado (barras de progresso
    /// vivas) não há stderr capturável — o conserto é rodar o pull direto.
    #[error(
        "o `ollama pull` falhou (exit {exit}) — rode `ollama pull {repo}` direto \
         para ver o erro completo e rode `kira model pull` de novo"
    )]
    PullFailed { exit: i32, repo: String },

    // ------------------------------------------------------------------
    // HF-5 — gate de admissão (ADR-0040): recusas acionáveis, nenhuma é
    // "inválido" — cada uma diz o critério que faltou e o que fazer.
    // ------------------------------------------------------------------
    /// Política de quantização recusou (Q2 proibido, abaixo do piso, quant
    /// desconhecido, tag do Hub sem quant) — mensagem vem da função pura.
    #[error("política de quantização recusou: {refusal}")]
    QuantRefused {
        /// Recusa já formatada em texto acionável (do módulo de catálogo).
        refusal: String,
    },

    /// Proveniência desconhecida: só tags oficiais do Ollama e quantizers
    /// conhecidos (bartowski, unsloth) entram no catálogo.
    #[error(
        "proveniência desconhecida ({org}) — só entram quantizers conhecidos \
         (bartowski, unsloth) e tags oficiais do Ollama (ADR-0040: a HF não \
         escaneia binário de tensor, a procedência é o primeiro filtro)"
    )]
    UnknownProvenance { org: String },

    /// SHA-256 ausente ou malformado — é conferido PELO DONO contra o model
    /// card (a HF não escaneia o binário; o número vem de conferência humana).
    #[error(
        "informe o SHA-256 conferido contra o model card: `--sha256 <64-hex>`. \
         Confira no card do repositório (`https://hf.co/<repo>` — campo SHA256 \
         dos arquivos GGUF) antes de admitir"
    )]
    Sha256Required,

    /// Probes não registrados: o gate exige probes rodados NO GGUF quantizado
    /// que vai pro ar (quantização quebra safety de formato — medir no FP16
    /// não vale).
    #[error(
        "probes pendentes — rode o `security_probe` e o `--eval` do kind NO GGUF \
         quantizado que vai pro ar e repita com `--probes-ok` (ADR-0040: medir no \
         FP16 não prova o quant)"
    )]
    ProbesRequired,

    /// `ollama show <tag> --template` falhou — sem template conferido, sem
    /// admissão (backdoors de chat-template são determinísticos e inspecionáveis).
    #[error(
        "o `ollama show {tag} --template` falhou (exit {exit}) — confira o modelo \
         com `ollama show {tag}` direto e rode `kira model admit` de novo{hint}"
    )]
    ShowFailed {
        exit: i32,
        tag: String,
        /// ` (dica: ...)` já formatado, ou vazio.
        hint: String,
    },

    /// O template devolvido está vazio: nada a conferir, nada admitido.
    #[error(
        "template vazio — o `ollama show {tag} --template` não devolveu template; \
         confira o modelo direto no Ollama antes de admitir"
    )]
    TemplateEmpty { tag: String },

    /// O template da tag MUDOU desde a admissão anterior: reconfira contra o
    /// model card e, confirmando a mudança, re-admita com `--force`.
    #[error(
        "o template de {tag} mudou desde a admissão anterior — reconfira contra o \
         model card; se a mudança foi conferida e é legítima, rode de novo com `--force`"
    )]
    TemplateDrift { tag: String },

    /// A trilha (store) não abriu: sem registro, sem admissão — a causa vai
    /// na mensagem, nunca falha silenciosa.
    #[error(
        "trilha indisponível para registrar a admissão de {tag} ({cause}) — \
         confira o disco/permissões de {path} e rode `kira model admit` de novo"
    )]
    StoreUnavailable {
        tag: String,
        cause: String,
        /// Caminho do store quando dá para saber (senão, vazio).
        path: String,
    },
}

/// Uma linha do catálogo local (`ollama list`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRow {
    /// Tag do modelo (`qwen3.5:4b`, `hf.co/<repo>:Q4_K_M`).
    pub name: String,
    /// Tamanho em disco como o Ollama imprime (`3.3 GB`).
    pub size: String,
    /// Idade relativa como o Ollama imprime (`3 days ago`).
    pub modified: String,
}

/// Parse puro da saída de `ollama list` (colunas por whitespace: NAME ID
/// SIZE MODIFIED — nomes de modelo não têm espaço). Header e linhas em
/// branco viram nada; linha curta demais é ignorada (uma linha estranha
/// não mata o catálogo). Retorno VAZIO só quando não há modelo nenhum.
#[must_use]
pub fn parse_ollama_list(stdout: &str) -> Vec<ModelRow> {
    let mut rows = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("NAME") {
            continue;
        }
        // name | id | size | unit | modified...  (7 tokens no mínimo)
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 5 {
            continue;
        }
        rows.push(ModelRow {
            name: tokens[0].to_string(),
            size: format!("{} {}", tokens[2], tokens[3]),
            modified: tokens[4..].join(" "),
        });
    }
    rows
}

/// Espaço livre em bytes no volume que hospeda `dir`; `None` = não foi
/// possível medir (nunca inventa número). CLI-first (ADR-0016): usa o
/// comando de linha do próprio SO — `df -k` no Unix (colunas POSIX,
/// independente de locale) e um one-liner do PowerShell no Windows cuja
/// saída é um inteiro puro (invariante de cultura).
pub fn free_disk_bytes(dir: &Path) -> Option<u64> {
    if cfg!(windows) {
        let drive = drive_of(dir)?;
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!("[long](Get-PSDrive {drive}).Free"))
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_ps_free(&stdout)
    } else {
        let output = Command::new("df").arg("-k").arg(dir).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_df_k(&stdout).map(|kb| kb.saturating_mul(1024))
    }
}

/// Letra do volume de `dir` no Windows (`C:\x` => `C`); `None` fora do
/// formato drive ou fora do Windows (não é chamada).
#[cfg(windows)]
fn drive_of(dir: &Path) -> Option<char> {
    let text = dir.to_string_lossy();
    let mut chars = text.chars();
    let drive = chars.next()?;
    let colon = chars.next()?;
    if colon == ':' && drive.is_ascii_alphabetic() {
        Some(drive.to_ascii_uppercase())
    } else {
        None
    }
}

#[cfg(not(windows))]
fn drive_of(_dir: &Path) -> Option<char> {
    None
}

/// Parse puro do stdout do PowerShell: primeira linha que é um inteiro puro
/// (o `[long]` imprime sem separadores de cultura). Outra coisa = `None`.
#[must_use]
pub fn parse_ps_free(stdout: &str) -> Option<u64> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.chars().all(|ch| ch.is_ascii_digit()))
        .and_then(|line| line.parse::<u64>().ok())
}

/// Parse puro de `df -k <dir>`: coluna 4 (avail, blocos de 1K) da ÚLTIMA
/// linha — números POSIX sem separador; a coluna de header é localizada e
/// NUNCA é lida por texto. Formato estranho = `None`.
#[must_use]
pub fn parse_df_k(stdout: &str) -> Option<u64> {
    let last = stdout.lines().rev().find(|line| !line.trim().is_empty())?;
    let tokens: Vec<&str> = last.split_whitespace().collect();
    let avail = tokens.get(3)?;
    avail.parse::<u64>().ok()
}

/// Destino dos modelos do Ollama: `OLLAMA_MODELS` (contrato do usuário)
/// precede o default `~/.ollama/models`. Sem HOME, o destino é o cwd (a
/// medição segue honesta para onde o ollama efetivamente vai — a env é a
/// fonte).
#[must_use]
pub fn ollama_models_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("OLLAMA_MODELS")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return dir;
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()));
    home.map_or_else(
        || PathBuf::from("."),
        |home| PathBuf::from(home).join(".ollama").join("models"),
    )
}

/// `kira model list`: parse de `ollama list` — leitura pura, sem rede fora
/// do localhost (o ollama CLI é quem toca o servidor local). Retorna o
/// texto pronto para stdout.
pub fn list() -> Result<String, ModelError> {
    let mut command = ollama_command(&["list"])?;
    let output = command.output().map_err(|_| ModelError::OllamaMissing)?; // binário sumiu entre resolve e spawn
    if !output.status.success() {
        let exit = output.status.code().unwrap_or(-1);
        let hint = crate::binresolve::first_stderr_line(&String::from_utf8_lossy(&output.stderr));
        let hint = if hint.is_empty() {
            String::new()
        } else {
            format!(" (dica: {hint})")
        };
        return Err(ModelError::ListFailed { exit, hint });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let rows = parse_ollama_list(&stdout);
    if rows.is_empty() && !stdout.trim().is_empty() {
        // Saída não vazia que não parseou: formato desconhecido é informação,
        // não catálogo vazio.
        return Err(ModelError::ListUnreadable);
    }
    Ok(render_rows(&rows))
}

/// Render determinístico do catálogo: alinhado por nome, na ORDEM do
/// ollama (o dono leu a lista dele; reordenar aqui não acrescentaria nada).
fn render_rows(rows: &[ModelRow]) -> String {
    if rows.is_empty() {
        return "nenhum modelo local — baixe com `kira model pull hf.co/<repo>:Q4_K_M` \
                (ou `ollama pull <tag>`)\n"
            .to_string();
    }
    let width = rows.iter().map(|row| row.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for row in rows {
        out.push_str(&format!(
            "{:<width$}  {:>8}  {}\n",
            row.name, row.size, row.modified
        ));
    }
    out
}

/// Modo de consentimento do pull (função PURA para teste): `--yes` vence
/// sempre (nunca bloqueia); sem `--yes`, TTY pergunta e sem TTY recusa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    /// Pergunta em TTY.
    Prompt,
    /// `--yes`: segue sem pergunta.
    Proceed,
    /// Sem `--yes` e sem TTY: erro, nada spawnado.
    Refuse,
}

#[must_use]
pub fn consent_mode(yes: bool, is_tty: bool) -> Consent {
    if yes {
        Consent::Proceed
    } else if is_tty {
        Consent::Prompt
    } else {
        Consent::Refuse
    }
}

/// `kira model pull <repo>`: baixa o modelo GGUF via `ollama pull` com
/// consentimento e disco checado. `yes` = `--yes`. stdio do ollama é
/// herdado — as barras de progresso ficam vivas (é o tamanho/timing que o
/// dono vê durante o download; o KIRA não fala com a API da HF).
pub fn pull(repo: &str, yes: bool) -> Result<String, ModelError> {
    // 1. Resolução PRIMEIRO: sem ollama, nada acontece (nem prompt).
    let mut command = ollama_command(&["pull", repo])?;

    // 2. Disco checado antes de qualquer consentimento — o dev decide vendo
    //    o número. Sem medição, a linha diz a causa (nunca número inventado).
    let dest = ollama_models_dir();
    let free = free_disk_bytes(&dest);
    let free_text = match free {
        Some(bytes) => format!("{:.1} GB", bytes as f64 / 1_073_741_824.0),
        None => "— (não consegui medir; nada é inventado)".to_string(),
    };

    // 3. Consentimento (estado, não texto): sem --yes e sem TTY, NEM COMEÇA.
    let is_tty = std::io::stdin().is_terminal();
    match consent_mode(yes, is_tty) {
        Consent::Refuse => {
            return Err(ModelError::ConsentRequired {
                repo: repo.to_string(),
            });
        }
        Consent::Prompt => {
            println!("baixar modelo do Hugging Face? {repo}");
            println!("  destino: {}", dest.display());
            println!("  espaço livre: {free_text}");
            println!("  o Ollama mostra o tamanho durante o download");
            print!("confirmar [y/N]: ");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let mut answer = String::new();
            // EOF/Enter vazio = recusa (mesmo contrato do connect: nunca grava).
            if std::io::stdin().read_line(&mut answer).unwrap_or(0) == 0 {
                return Err(ModelError::Refused);
            }
            let answer = answer.trim().to_ascii_lowercase();
            if !matches!(answer.as_str(), "y" | "yes" | "s" | "sim") {
                return Err(ModelError::Refused);
            }
        }
        Consent::Proceed => {
            // `--yes`: a medição segue visível no stdout para a trilha.
            if free.is_none() {
                eprintln!("nota: espaço livre não medido — sigo com o download (--yes)");
            }
        }
    }

    // 4. Spawn com stdio herdado (barras de progresso vivas). Exit != 0 é
    //    falha do pull — o conserto é rodar o pull direto (erro acionável).
    let status = command.status().map_err(|_| ModelError::OllamaMissing)?;
    if !status.success() {
        return Err(ModelError::PullFailed {
            exit: status.code().unwrap_or(-1),
            repo: repo.to_string(),
        });
    }
    Ok(format!(
        "modelo baixado: {repo} (destino {})",
        dest.display()
    ))
}

/// Recusa da política de quantização (ADR-0040: piso Q4_K_M, Q2_K proibido).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuantRefusal {
    /// Família Q2 — proibida por política (EMNLP 2026: Q2_K eleva o sucesso
    /// de ataque em 1–2 B; nunca entra no catálogo).
    ForbiddenQ2 { quant: String },
    /// Abaixo do piso Q4_K_M (ex.: Q3_K_S).
    BelowFloor { quant: String },
    /// Quant que a casa não reconhece — o dono confere no model card.
    Unknown { quant: String },
    /// Tag do Hub (`hf.co/...`) sem quant — o piso não é verificável.
    HubTagWithoutQuant,
}

impl std::fmt::Display for QuantRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForbiddenQ2 { quant } => write!(
                f,
                "{quant} é proibido por política (ADR-0040): quantização Q2 degrada a \
                 segurança do modelo — o piso da casa é Q4_K_M; escolha \
                 `hf.co/<repo>:Q4_K_M` ou superior"
            ),
            Self::BelowFloor { quant } => write!(
                f,
                "{quant} fica abaixo do piso Q4_K_M (ADR-0040) — escolha Q4_K_M ou superior"
            ),
            Self::Unknown { quant } => write!(
                f,
                "quant {quant} não reconhecido — confira o tag no model card \
                 (formato `hf.co/<repo>:<QUANT>`) e, confirmando o piso, registre \
                 aqui no KIRA"
            ),
            Self::HubTagWithoutQuant => write!(
                f,
                "tag do Hub sem quant — use `hf.co/<repo>:<QUANT>` (ex.: :Q4_K_M) \
                 para o piso Q4_K_M ser verificável (ADR-0040)"
            ),
        }
    }
}

/// Extrai o segmento de quant do final do tag (`...:Q4_K_M`); `None` quando o
/// último segmento não é quant (tags oficiais do Ollama: `qwen3.5:4b`,
/// `qwen3:1.7b` — o `4b`/`1.7b` é tamanho, não quant).
#[must_use]
pub fn quant_of_tag(tag: &str) -> Option<&str> {
    let last = tag.rsplit(':').next().unwrap_or("");
    // Formato reconhecido: Q<dígitos>[_LETRAS] — a família i-matrix (IQ4_XS)
    // entra pelo mesmo caminho, prefixo `IQ` seguido de dígito.
    let digits_part = last.strip_prefix("IQ").or_else(|| last.strip_prefix('Q'))?;
    if digits_part.is_empty() || !digits_part.as_bytes()[0].is_ascii_digit() {
        return None;
    }
    // O resto só pode ter dígitos, underscore e maiúsculas (Q4_K_M, Q8_0,
    // IQ3_XS) — minúscula/misturada não é quant.
    if !digits_part
        .bytes()
        .all(|b| b.is_ascii_digit() || b == b'_' || b.is_ascii_uppercase())
    {
        return None;
    }
    Some(last)
}

/// Política de quantização (função PURA): o piso é Q4_K_M e a família Q2 é
/// proibida — a recusa é acionável e acontece ANTES de qualquer spawn.
///
/// Tag sem quant (oficial do Ollama) é permitida: os defaults da casa são
/// oficiais e conferidos por testes; o gate de quant existe para o catálogo
/// de GGUF do Hub.
///
/// # Erros
/// [`QuantRefusal`] com a mensagem já acionável.
pub fn check_quant_policy(tag: &str) -> Result<(), QuantRefusal> {
    let Some(quant) = quant_of_tag(tag) else {
        // Tag do Hub sem quant não permite verificar o piso — recusa honesta.
        if tag.starts_with("hf.co/") {
            return Err(QuantRefusal::HubTagWithoutQuant);
        }
        return Ok(());
    };
    // Tier = primeiro grupo de dígitos do quant (Q4_K_M → 4, IQ3_XS → 3,
    // Q2_K → 2). Tier 2 = proibido por política; tier < 4 = abaixo do piso.
    let body = quant.strip_prefix("IQ").unwrap_or(&quant[1..]);
    let digits: String = body.chars().take_while(char::is_ascii_digit).collect();
    let Ok(tier) = digits.parse::<u8>() else {
        return Err(QuantRefusal::Unknown {
            quant: quant.to_string(),
        });
    };
    if tier == 2 {
        return Err(QuantRefusal::ForbiddenQ2 {
            quant: quant.to_string(),
        });
    }
    if tier < 4 {
        return Err(QuantRefusal::BelowFloor {
            quant: quant.to_string(),
        });
    }
    Ok(())
}

/// Proveniência de um tag (gate de admissão item 1, ADR-0040): só entram
/// tags oficiais do Ollama e quantizers conhecidos (bartowski, unsloth) —
/// a HF não escaneia binário de tensor (Pillar, ICLR 2026: GGUF envenenado
/// passou em todo scan automático da plataforma), então a procedência é o
/// primeiro filtro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Tag oficial do Ollama (sem prefixo `hf.co/`).
    Official,
    /// `hf.co/bartowski/...` — quantizer conhecido.
    Bartowski,
    /// `hf.co/unsloth/...` — quantizer conhecido.
    Unsloth,
    /// Qualquer outro org do Hub — recusado no `model admit`.
    Unknown,
}

impl Provenance {
    /// Nome registrado no store (`model_admission.provenance`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::Bartowski => "bartowski",
            Self::Unsloth => "unsloth",
            Self::Unknown => "unknown",
        }
    }
}

#[must_use]
pub fn provenance_of_tag(tag: &str) -> Provenance {
    let Some(rest) = tag.strip_prefix("hf.co/") else {
        return Provenance::Official;
    };
    let org = rest.split('/').next().unwrap_or("");
    match org {
        "bartowski" => Provenance::Bartowski,
        "unsloth" => Provenance::Unsloth,
        _ => Provenance::Unknown,
    }
}

/// Versão mínima do Ollama exigida pelo `kira doctor` (HF-5, ADR-0040):
/// CVE-2026-7482 "Bleeding Llama" — GGUF craftado vazava heap via `/api/create`.
/// O KIRA nunca sobe blob por essa rota (só `ollama pull hf.co/...`), mas o
/// piso vale para o servidor que executa os modelos do catálogo.
pub const MIN_OLLAMA_VERSION: (u64, u64, u64) = (0, 17, 1);

/// Parse puro da saída de `ollama --version`: a primeira linha cujo último
/// token é `x.y.z` (o Ollama imprime `ollama version is 0.17.1`). Formato
/// estranho = `None` (nunca inventa versão).
#[must_use]
pub fn parse_ollama_version(stdout: &str) -> Option<(u64, u64, u64)> {
    stdout.lines().find_map(|line| {
        let last = line.trim().rsplit(' ').next().unwrap_or("");
        let mut parts = last.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        let patch = parts.next()?.parse::<u64>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some((major, minor, patch))
    })
}

/// Comparação semver simplificada: `(major, minor, patch) >= mínimo`.
#[must_use]
pub fn version_at_least(version: (u64, u64, u64), min: (u64, u64, u64)) -> bool {
    version >= min
}

/// Env seam do e2e para o template do `model admit` (mesmo papel do
/// `KIRA_ADO_URL`): quando presente, o valor vira a saída do
/// `ollama show <tag> --template` — nenhum fluxo de produto a define.
pub const TEMPLATE_ENV: &str = "KIRA_MODEL_TEMPLATE";

/// Saída de `ollama --version` (binário resolvido no PATH/override, ADR-0016).
pub fn version() -> Result<String, ModelError> {
    let mut command = ollama_command(&["--version"])?;
    let output = command.output().map_err(|_| ModelError::OllamaMissing)?;
    if !output.status.success() {
        let exit = output.status.code().unwrap_or(-1);
        let hint = crate::binresolve::first_stderr_line(&String::from_utf8_lossy(&output.stderr));
        let hint = if hint.is_empty() {
            String::new()
        } else {
            format!(" (dica: {hint})")
        };
        return Err(ModelError::ListFailed { exit, hint });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Template de chat da tag via `ollama show <tag> --template` — o gate de
/// admissão item 2 (backdoors de chat-template são determinísticos e
/// inspecionáveis). O seam [`TEMPLATE_ENV`] substitui o spawn nos e2e.
pub fn show_template(tag: &str) -> Result<String, ModelError> {
    if let Some(seam) = std::env::var(TEMPLATE_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Ok(seam);
    }
    let mut command = ollama_command(&["show", tag, "--template"])?;
    let output = command.output().map_err(|_| ModelError::OllamaMissing)?;
    if !output.status.success() {
        let exit = output.status.code().unwrap_or(-1);
        let hint = crate::binresolve::first_stderr_line(&String::from_utf8_lossy(&output.stderr));
        let hint = if hint.is_empty() {
            String::new()
        } else {
            format!(" (dica: {hint})")
        };
        return Err(ModelError::ShowFailed {
            exit,
            tag: tag.to_string(),
            hint,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// SHA-256 é hex de 64 caracteres — a conferência contra o model card é
/// HUMANA; aqui só o formato é verificado (nada de hash inventado).
fn sha256_well_formed(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `kira model admit <tag>` (HF-5, ADR-0040): registra no store que a tag
/// passou NO GATE DE ADMISSÃO e, só depois disso, ela entra no routing de
/// especialistas (`model_role`). Gate, nesta ordem de custo:
///
/// 1. **Política de quantização** (pura, local): piso Q4_K_M, família Q2
///    proibida — recusa ANTES de qualquer spawn;
/// 2. **Proveniência**: só tags oficiais do Ollama e quantizers conhecidos
///    (bartowski, unsloth);
/// 3. **SHA-256**: `--sha256 <64-hex>` conferido pelo dono contra o model
///    card (a HF não escaneia binário de tensor — Pillar, ICLR 2026);
/// 4. **Probes**: `--probes-ok` atesta `security_probe` + `--eval` do kind
///    rodados NO GGUF quantizado que vai pro ar;
/// 5. **Template**: `ollama show <tag> --template` precisa responder e o
///    hash dele não pode ter mudado desde a admissão anterior (drift →
///    reconfirmação com `--force`).
///
/// O budget (item 6 do gate) já é estrutural: toda chamada de LLM de
/// qualquer modelo entra no mesmo `llm_calls` (P0, schema v9).
///
/// Admissão é escrita no store local (trilha), nunca no mundo — sem
/// `--apply`. Re-admissão com template idêntico é idempotente.
pub fn admit(
    tag: &str,
    sha256: Option<&str>,
    probes_ok: bool,
    force: bool,
) -> Result<String, ModelError> {
    // 1. Política de quantização: recusa barata e local, nunca spawn.
    if let Err(refusal) = check_quant_policy(tag) {
        return Err(ModelError::QuantRefused {
            refusal: refusal.to_string(),
        });
    }
    // 2. Proveniência: o org do Hub precisa ser conhecido.
    if let Provenance::Unknown = provenance_of_tag(tag) {
        let org = tag
            .strip_prefix("hf.co/")
            .and_then(|rest| rest.split('/').next())
            .unwrap_or(tag);
        return Err(ModelError::UnknownProvenance {
            org: org.to_string(),
        });
    }
    // 3. SHA-256 conferido pelo dono (formato verificado, valor é dele).
    let Some(sha) = sha256.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(ModelError::Sha256Required);
    };
    if !sha256_well_formed(sha) {
        return Err(ModelError::Sha256Required);
    }
    // 4. Probes no GGUF quantizado: atestado do dono, exigido sempre.
    if !probes_ok {
        return Err(ModelError::ProbesRequired);
    }
    // 5. Template conferido: `ollama show` precisa responder e o hash não
    //    pode ter driftado desde a admissão anterior.
    let template = show_template(tag)?;
    if template.trim().is_empty() {
        return Err(ModelError::TemplateEmpty {
            tag: tag.to_string(),
        });
    }
    let template_hash = crate::post::plan::fnv1a64_hex(&template);

    let store =
        crate::store::Store::open_default().map_err(|err| ModelError::StoreUnavailable {
            tag: tag.to_string(),
            cause: err.to_string(),
            path: crate::store::store_dir()
                .map(|dir| dir.join("store.db"))
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        })?;
    let previous = store
        .model_admission_get(tag)
        .map_err(|err| ModelError::StoreUnavailable {
            tag: tag.to_string(),
            cause: err.to_string(),
            path: String::new(),
        })?;
    if let Some(previous) = previous
        && previous.template_hash != template_hash
        && !force
    {
        return Err(ModelError::TemplateDrift {
            tag: tag.to_string(),
        });
    }

    let quant = quant_of_tag(tag).unwrap_or("").to_string();
    let row = crate::store::admission::AdmissionRow {
        tag: tag.to_string(),
        quant: quant.clone(),
        provenance: provenance_of_tag(tag).name().to_string(),
        sha256: sha.to_ascii_lowercase(),
        template_hash,
        probes_ok: true,
        admitted_at: crate::store::now_rfc3339_utc(),
    };
    store
        .model_admission_put(&row)
        .map_err(|err| ModelError::StoreUnavailable {
            tag: tag.to_string(),
            cause: err.to_string(),
            path: String::new(),
        })?;
    Ok(format!(
        "admitido: {tag} (quant {}, proveniência {}, template ok, probes ok) — \
         agora entra no routing por `model_role.<kind>` no config.toml",
        if quant.is_empty() { "oficial" } else { &quant },
        provenance_of_tag(tag).name()
    ))
}

/// Command do ollama com o caminho RESOLVIDO (nunca o nome nu) e o wrapper
/// `cmd /C chcp 65001` para `.cmd`/`.bat` no Windows — mesma conduta do scan
/// (o std não executa script como binário; o chcp corrige o mojibake REAL).
fn ollama_command(args: &[&str]) -> Result<Command, ModelError> {
    let path_env = std::env::var_os("PATH");
    // Override presente é contrato do usuário: o arquivo apontado spawna
    // como veio; inexistente é o mesmo "instale o X" (ADR do scan).
    let bin = match crate::binresolve::bin_from_env(OLLAMA_BIN_ENV) {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_file() {
                return Err(ModelError::OllamaMissing);
            }
            path
        }
        None => crate::binresolve::find_on_path(path_env.as_deref(), "ollama")
            .ok_or(ModelError::OllamaMissing)?,
    };
    let via_cmd = cfg!(windows) && crate::binresolve::is_cmd_script(&bin.to_string_lossy());
    if via_cmd {
        let owned: Vec<std::ffi::OsString> = args.iter().map(std::ffi::OsString::from).collect();
        let line = crate::shellline::cmd_command_line(&bin, &owned);
        let mut command = Command::new("cmd");
        command.arg("/C");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.raw_arg(line);
        }
        #[cfg(not(windows))]
        {
            // Inalcançável (via_cmd só é true no Windows); mantém o build honesto.
            let _ = line;
            command.arg(&bin);
            command.args(args);
        }
        Ok(command)
    } else {
        let mut command = Command::new(&bin);
        command.args(args);
        Ok(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Header + 3 linhas reais (uma delas hf.co — formato do dono).
    const LIST_OUTPUT: &str = "\
NAME                                                                                             ID              SIZE      MODIFIED
qwen3.5:4b                                                                                       d8b0f5e9760c    3.3 GB    3 days ago
hf.co/mlabonne/Llama-3.1-8B-GGUF:Q4_K_M                                                          a26faf0135b2    4.9 GB    4 days ago
dolphin3:8b                                                                                      d5ab9ae8e1f2    4.9 GB    4 days ago
";

    #[test]
    fn parse_ollama_list_reads_name_size_and_modified() {
        let rows = parse_ollama_list(LIST_OUTPUT);
        assert_eq!(rows.len(), 3, "header não vira linha: {rows:?}");
        assert_eq!(rows[0].name, "qwen3.5:4b");
        assert_eq!(rows[0].size, "3.3 GB");
        assert_eq!(rows[0].modified, "3 days ago");
        assert_eq!(
            rows[1].name, "hf.co/mlabonne/Llama-3.1-8B-GGUF:Q4_K_M",
            "tag hf.co entra inteira (tem pontos e barra, nunca espaço)"
        );
    }

    #[test]
    fn parse_ollama_list_skips_blank_and_short_lines() {
        let rows = parse_ollama_list("\n  \nNAME ID SIZE MODIFIED\nlinha-curta\nqwen x 1 GB\n");
        assert!(
            rows.is_empty(),
            "linhas de menos de 5 tokens não são modelos: {rows:?}"
        );
    }

    #[test]
    fn render_rows_empty_catalog_tells_how_to_pull() {
        let text = render_rows(&[]);
        assert!(
            text.contains("kira model pull"),
            "catálogo vazio ensina o pull: {text}"
        );
    }

    #[test]
    fn render_rows_is_aligned_and_deterministic() {
        let rows = parse_ollama_list(LIST_OUTPUT);
        let text = render_rows(&rows);
        // Duas execuções = mesmo texto (determinismo do render).
        assert_eq!(text, render_rows(&parse_ollama_list(LIST_OUTPUT)));
        assert!(text.contains("qwen3.5:4b"));
        assert!(text.contains("3.3 GB"));
    }

    #[test]
    fn consent_mode_yes_wins_and_no_tty_refuses() {
        assert_eq!(consent_mode(true, false), Consent::Proceed);
        assert_eq!(
            consent_mode(true, true),
            Consent::Proceed,
            "--yes vence em TTY"
        );
        assert_eq!(consent_mode(false, true), Consent::Prompt);
        assert_eq!(consent_mode(false, false), Consent::Refuse);
    }

    #[test]
    fn parse_ps_free_reads_a_plain_integer_line() {
        assert_eq!(parse_ps_free("123456789012\n"), Some(123_456_789_012));
        assert_eq!(parse_ps_free("\n  42  \n"), Some(42));
        assert!(
            parse_ps_free("1.234.567\nabc\n").is_none(),
            "separador de cultura NÃO é inteiro puro — nunca parseia errado"
        );
        assert!(parse_ps_free("").is_none());
    }

    #[test]
    fn parse_df_k_reads_the_avail_column_of_the_last_line() {
        // POSIX df: header + uma linha de dados; a coluna 4 é o avail em 1K.
        let df = "Filesystem  1K-blocks     Used   Available Use% Mounted on\n\
                  /dev/sda1   26214400 10485760    15728640  40% /\n";
        assert_eq!(parse_df_k(df), Some(15_728_640));
        assert_eq!(parse_df_k("nada\n"), None, "linha curta = None, não 0");
        assert_eq!(parse_df_k(""), None);
    }

    #[test]
    fn ollama_models_dir_honors_the_env() {
        // Exceção única da casa: edition 2024 torna set/remove_var unsafe e
        // este teste é test-only (nenhum outro teste lê OLLAMA_MODELS). A
        // precedência é o contrato: a env ganha do padrão ~/{.ollama/models}.
        let marker = std::env::temp_dir().join("kira-model-env-marker");
        let saved = std::env::var_os("OLLAMA_MODELS");
        unsafe { std::env::set_var("OLLAMA_MODELS", &marker) };
        assert_eq!(ollama_models_dir(), marker);
        match saved {
            Some(value) => unsafe { std::env::set_var("OLLAMA_MODELS", value) },
            None => unsafe { std::env::remove_var("OLLAMA_MODELS") },
        }
    }

    // -------------------------------------------------------------------
    // HF-5 — gate de admissão: política de quantização (pura, sem spawn)
    // -------------------------------------------------------------------

    #[test]
    fn quant_detection_reads_the_tag_tail() {
        assert_eq!(
            quant_of_tag("hf.co/bartowski/X-GGUF:Q4_K_M"),
            Some("Q4_K_M")
        );
        assert_eq!(quant_of_tag("hf.co/unsloth/X-GGUF:Q2_K"), Some("Q2_K"));
        assert_eq!(
            quant_of_tag("hf.co/bartowski/X-GGUF:IQ3_XS"),
            Some("IQ3_XS")
        );
        // Tamanho não é quant: tags oficiais ficam sem.
        assert_eq!(quant_of_tag("qwen3.5:4b"), None);
        assert_eq!(quant_of_tag("qwen3:1.7b"), None);
        assert_eq!(quant_of_tag("dolphin3:8b"), None);
    }

    #[test]
    fn q2_family_is_refused_by_policy_with_an_actionable_note() {
        // O aceite literal do item: Q2_K recusado com nota acionável.
        let err = check_quant_policy("hf.co/bartowski/Qwen2.5-1.5B-GGUF:Q2_K")
            .expect_err("Q2_K proibido");
        assert!(matches!(err, QuantRefusal::ForbiddenQ2 { .. }), "{err:?}");
        assert!(
            err.to_string().contains("proibido por política") && err.to_string().contains("Q4_K_M"),
            "mensagem acionável: {err}"
        );
        // A família inteira Q2 (IQ2_XS inclusive) está fora.
        assert!(matches!(
            check_quant_policy("hf.co/bartowski/X-GGUF:IQ2_XS"),
            Err(QuantRefusal::ForbiddenQ2 { .. })
        ));
    }

    #[test]
    fn quants_below_the_floor_are_refused() {
        let err = check_quant_policy("hf.co/bartowski/X-GGUF:Q3_K_S").expect_err("abaixo do piso");
        assert!(matches!(err, QuantRefusal::BelowFloor { .. }), "{err:?}");
        assert!(
            err.to_string().contains("piso Q4_K_M"),
            "mensagem acionável: {err}"
        );
    }

    #[test]
    fn floor_and_above_pass() {
        for quant in ["Q4_K_M", "Q4_K_S", "Q5_K_M", "Q6_K", "Q8_0", "IQ4_XS"] {
            let tag = format!("hf.co/bartowski/X-GGUF:{quant}");
            assert_eq!(check_quant_policy(&tag), Ok(()), "{tag}");
        }
        // Tags oficiais sem quant passam (defaults da casa).
        assert_eq!(check_quant_policy("qwen3.5:4b"), Ok(()));
        assert_eq!(check_quant_policy("qwen3:1.7b"), Ok(()));
    }

    #[test]
    fn hub_tag_without_quant_cannot_prove_the_floor() {
        let err = check_quant_policy("hf.co/bartowski/X-GGUF").expect_err("precisa de quant");
        assert!(
            err.to_string().contains("hf.co/<repo>:<QUANT>"),
            "mensagem acionável: {err}"
        );
    }

    #[test]
    fn provenance_only_trusts_known_quantizers_and_official() {
        assert_eq!(provenance_of_tag("qwen3.5:4b"), Provenance::Official);
        assert_eq!(
            provenance_of_tag("hf.co/bartowski/X-GGUF:Q4_K_M"),
            Provenance::Bartowski
        );
        assert_eq!(
            provenance_of_tag("hf.co/unsloth/X-GGUF:Q4_K_M"),
            Provenance::Unsloth
        );
        assert_eq!(
            provenance_of_tag("hf.co/mlabonne/X-GGUF:Q4_K_M"),
            Provenance::Unknown
        );
        assert_eq!(Provenance::Bartowski.name(), "bartowski");
    }

    // -------------------------------------------------------------------
    // HF-5 — doctor: piso do Ollama (CVE-2026-7482)
    // -------------------------------------------------------------------

    #[test]
    fn ollama_version_parses_the_canned_output() {
        assert_eq!(
            parse_ollama_version("ollama version is 0.17.1"),
            Some((0, 17, 1))
        );
        assert_eq!(
            parse_ollama_version("ollama version is 0.24.3\naviso qualquer\n"),
            Some((0, 24, 3))
        );
        // Formato estranho nunca vira versão inventada.
        assert_eq!(parse_ollama_version(""), None);
        assert_eq!(parse_ollama_version("versão desconhecida"), None);
        assert_eq!(parse_ollama_version("1.2"), None);
        assert_eq!(parse_ollama_version("a.b.c"), None);
    }

    #[test]
    fn ollama_minimum_is_checked_against_the_cve_piso() {
        assert!(version_at_least((0, 17, 1), MIN_OLLAMA_VERSION));
        assert!(version_at_least((0, 24, 3), MIN_OLLAMA_VERSION));
        assert!(version_at_least((1, 0, 0), MIN_OLLAMA_VERSION));
        assert!(!version_at_least((0, 17, 0), MIN_OLLAMA_VERSION));
        assert!(!version_at_least((0, 16, 9), MIN_OLLAMA_VERSION));
    }
}
