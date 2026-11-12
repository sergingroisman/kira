use thiserror::Error;

/// Errors produced by KIRA. Every message says what to do next.
#[derive(Debug, Error)]
pub enum Error {
    /// `AZURE_DEVOPS_PAT` is unset or empty (and not found in the OS keyring).
    #[error(
        "PAT não encontrado — defina AZURE_DEVOPS_PAT com um Personal Access Token de leitura de Work Items (Unix: export AZURE_DEVOPS_PAT=...; PowerShell: $env:AZURE_DEVOPS_PAT='...'; cmd: setx AZURE_DEVOPS_PAT \"...\"), ou rode `kira setup` para salvar o PAT no keyring do sistema\npara diagnosticar a configuração desta máquina, rode `kira doctor`"
    )]
    MissingPat,

    /// `AZURE_DEVOPS_ORG` is unset or empty (and absent from config.toml).
    #[error(
        "defina AZURE_DEVOPS_ORG com o nome da organização (Unix: export AZURE_DEVOPS_ORG=minha-org; PowerShell: $env:AZURE_DEVOPS_ORG='minha-org'; cmd: setx AZURE_DEVOPS_ORG \"minha-org\"), ou rode `kira setup` para salvar a configuração\npara diagnosticar a configuração desta máquina, rode `kira doctor`"
    )]
    MissingOrg,

    /// Neither `--project` nor `AZURE_DEVOPS_PROJECT` provided a project name.
    #[error(
        "informe o projeto com --project <nome> ou defina a variável de ambiente AZURE_DEVOPS_PROJECT\nou rode `kira setup` para salvar a configuração (rode `kira doctor` para diagnosticar esta máquina)"
    )]
    MissingProject,

    /// The requested board provider is not implemented in this phase.
    #[error(
        "board '{name}' ainda não é suportado — o Azure DevOps é o board desta fase (MVP); use KIRA_BOARD=azure ou remova a variável"
    )]
    UnknownBoard { name: String },

    /// The OS keyring is not usable on this machine (no daemon, unsupported
    /// platform, ...). The keyring error itself never contains the secret.
    #[error(
        "keyring indisponível ({action}) — use a variável de ambiente AZURE_DEVOPS_PAT ou rode `kira setup` de novo"
    )]
    KeyringUnavailable {
        action: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The config file `$HOME/.config/kira/config.toml` could not be read.
    #[error("não foi possível ler o arquivo '{path}' — corrija o arquivo ou remova-o: {source}")]
    ConfigRead {
        path: String,
        source: std::io::Error,
    },

    /// The config file has a line that does not follow `key = "value"`.
    #[error(
        "linha {line} inválida em '{path}' — use o formato chave = \"valor\" (ex.: organization = \"minha-org\")"
    )]
    ConfigParse { path: String, line: usize },

    /// Azure DevOps request failed (network, authentication, not found, ...).
    #[error(
        "falha ao consultar o Azure DevOps: {0} — rode `kira doctor` para diagnosticar credencial e conectividade (a resposta crua do servidor não é exibida)"
    )]
    Ado(#[from] Box<dyn std::error::Error + Send + Sync>),

    /// Azure DevOps rejected the PAT with HTTP 401.
    #[error(
        "PAT inválido ou sem escopo Work Items (Read) — gere um novo token em https://dev.azure.com/{organization}/_usersSettings/tokens (scope: Work Items Read) e rode `kira doctor`"
    )]
    AdoUnauthorized { organization: String },

    /// Azure DevOps answered with an unexpected HTTP status.
    #[error(
        "HTTP {status} ao acessar https://dev.azure.com/{organization} — confira se o nome da organização está correto e se a máquina tem acesso à rede"
    )]
    AdoStatus { status: u16, organization: String },

    /// The work item returned HTTP 404 with `TF401232`/`WorkItemUnauthorizedAccessException`:
    /// it does not exist or the PAT cannot read it. The response body is never echoed.
    #[error(
        "o work item #{id} não existe ou você não pode lê-lo — corrige uma destas hipóteses: \
         (1) o PAT não tem o escopo Work Items (Read) — edite/gere o token em https://dev.azure.com/{organization}/_usersSettings/tokens; \
         (2) o item pertence a outro projeto — rode com --project <nome> (confira no browser em https://dev.azure.com/{organization}/{project}/_workitems/edit/{id}); \
         (3) o id está errado"
    )]
    WorkItemInacessivel {
        id: i32,
        organization: String,
        project: String,
    },

    /// Azure DevOps answered with an unexpected JSON shape.
    #[error(
        "resposta inesperada do Azure DevOps: {0} — rode `kira doctor` e, se persistir, atualize o KIRA (o formato da API pode ter mudado)"
    )]
    Api(#[from] serde_json::Error),

    /// The local Ollama server is not reachable (nothing listening).
    #[error(
        "Ollama não está rodando — instale em https://ollama.com e execute 'ollama serve' (no Windows ele roda como serviço)"
    )]
    LlmNotRunning,

    /// The local Ollama server did not answer within the 120s timeout.
    #[error(
        "Ollama não respondeu em 120s — rode `ollama ps` para conferir se o modelo está carregado (100% GPU), teste com `ollama run <modelo>` e, se continuar lento, reduza o contexto com KIRA_NUM_CTX=2048"
    )]
    LlmTimeout,

    /// Ollama answered with a non-2xx status; the body is never echoed.
    #[error(
        "HTTP {status} do Ollama — confira se o modelo existe com `ollama list` (o corpo da resposta foi omitido)"
    )]
    LlmStatus { status: u16 },

    /// Ollama answered with an unexpected JSON shape.
    #[error(
        "resposta inesperada do Ollama: {0} — confira KIRA_OLLAMA_URL e atualize o Ollama se necessário"
    )]
    LlmBadResponse(String),

    /// The HTTP request to Ollama failed without a response.
    #[error(
        "falha na chamada ao Ollama: {0} — confira se `ollama serve` está no ar e se KIRA_OLLAMA_URL aponta para http://localhost:11434 (rode `kira doctor`)"
    )]
    LlmRequest(String),

    /// `KIRA_FOUNDRY_API_KEY` is unset or empty (PR11b, ADR-0020).
    #[error(
        "chave do Foundry ausente — defina a variável de ambiente KIRA_FOUNDRY_API_KEY com uma das chaves do recurso (portal do Foundry → recurso → Keys and Endpoints; ex.: export KIRA_FOUNDRY_API_KEY=...)"
    )]
    FoundryMissingKey,

    /// `KIRA_FOUNDRY_ENDPOINT` is unset or empty.
    #[error(
        "defina a variável de ambiente KIRA_FOUNDRY_ENDPOINT com o endpoint do recurso Foundry — na superfície v1 ele termina em /openai/v1 (ex.: export KIRA_FOUNDRY_ENDPOINT=https://sdossantosjunior-5474-resource.openai.azure.com/openai/v1)"
    )]
    FoundryMissingEndpoint,

    /// `KIRA_FOUNDRY_DEPLOYMENT` is unset or empty.
    #[error(
        "defina a variável de ambiente KIRA_FOUNDRY_DEPLOYMENT com o nome do deployment/modelo (ex.: export KIRA_FOUNDRY_DEPLOYMENT=gpt-4o-mini — confira a lista de Deployments no portal do Foundry)"
    )]
    FoundryMissingDeployment,

    /// A project endpoint (`*/api/projects/*`) was given — it does not serve
    /// chat completions.
    #[error(
        "endpoint de projeto do Foundry não aceita chat completions — defina a variável de ambiente KIRA_FOUNDRY_ENDPOINT com o endpoint do recurso OpenAI terminando em /openai/v1 (ex.: https://sdossantosjunior-5474-resource.openai.azure.com/openai/v1)"
    )]
    FoundryEndpoint,

    /// Foundry rejected the key with HTTP 401/403. The key itself never
    /// appears here (it lives in env; Entra ID is a later roadmap step).
    #[error(
        "chave recusada pelo Foundry (HTTP {status}) — gere uma nova chave em portal.azure.com (recurso → Keys and Endpoints) e rode de novo; se usar gateway, confira KIRA_FOUNDRY_AUTH=bearer (Entra ID fica para o roadmap)"
    )]
    FoundryAuth { status: u16 },

    /// Foundry answered 404 without mentioning `api-version`.
    #[error(
        "HTTP 404 do Foundry — confira KIRA_FOUNDRY_ENDPOINT e KIRA_FOUNDRY_DEPLOYMENT na lista de Deployments do recurso no portal do Foundry (a chave nunca é exibida)"
    )]
    FoundryDeployment,

    /// The server rejected the `api-version` — the default of the KIRA
    /// (`2024-10-21`) can be overridden for endpoints on another version.
    #[error(
        "a versão da API foi recusada (HTTP {status}) — defina a variável de ambiente KIRA_FOUNDRY_API_VERSION com a versão aceita pelo endpoint (o default do KIRA é 2024-10-21)"
    )]
    FoundryApiVersion { status: u16 },

    /// HTTP 429 — the quota of the license period is exhausted. The String is
    /// the server `Retry-After` (or a default suggestion).
    #[error(
        "HTTP 429 do Foundry — cota do período esgotada; aguarde {0} e rode de novo (o retry-after veio do servidor)"
    )]
    FoundryRateLimited(String),

    /// Other status, network failure or unexpected shape. `detail` is the
    /// first line of the server `error.message`, redacted and bounded to 200
    /// chars — never the raw body, never the key.
    #[error(
        "falha do Foundry: {detail} — rode de novo; se persistir, confira KIRA_FOUNDRY_ENDPOINT e o status do recurso no portal (a chave nunca é exibida)"
    )]
    FoundryFailed { detail: String },

    /// The request exceeded `KIRA_FOUNDRY_TIMEOUT` (default 120s).
    #[error(
        "Foundry não respondeu em {timeout_secs}s — rode de novo ou aumente o limite com KIRA_FOUNDRY_TIMEOUT (o default é 120s)"
    )]
    FoundryTimeout { timeout_secs: u64 },

    /// A `KIRA_FOUNDRY_*` numeric/enum variable holds an invalid value.
    #[error("valor '{value}' inválido para {env} — {hint}")]
    FoundryEnv {
        env: &'static str,
        value: String,
        hint: &'static str,
    },

    /// The server rejected a body parameter (`unsupported_parameter` /
    /// `temperature` / `max_tokens`) — reasoning models (GPT-5, o-series)
    /// refuse the classic fields.
    #[error(
        "o endpoint recusou um parâmetro do corpo (HTTP {status}) — defina a variável de ambiente KIRA_FOUNDRY_BODY_MODE=reasoning (GPT-5/o-series: sem temperature, max_completion_tokens) ou KIRA_FOUNDRY_BODY_MODE=classic para forçar o formato antigo; mensagem do servidor: {detail}"
    )]
    FoundryBodyMode { status: u16, detail: String },

    /// The model answer carried none of the three triage section headers
    /// (W2-fix PR-F2). Silence here used to become three `—` with exit 0; an
    /// answer out of format is an error, not an empty result.
    #[error(
        "resposta do modelo fora do formato das 3 seções (Causa raiz provável / Arquivos prováveis / Confiança) — rode `kira triage` de novo, troque o modelo com --model, ou confira `ollama ps` (o modelo precisa estar carregado)"
    )]
    LlmFormat,

    /// The Copilot CLI (`copilot`) was not found: neither the
    /// `KIRA_COPILOT_BIN` override pointed at an existing file nor was a
    /// `copilot` executable on the PATH.
    #[error(
        "backend Copilot indisponível — instale o Copilot CLI (https://github.com/github/copilot-cli) e rode `copilot auth login` para autenticar, defina KIRA_COPILOT_BIN para um caminho específico, ou use o default local com `--llm ollama`"
    )]
    CopilotMissing,

    /// The Copilot CLI rejected the call for an auth reason (SAML/SSO, 401,
    /// 403, login required). The stderr itself is never echoed — the message
    /// lists the four steps in order instead.
    #[error(
        "o Copilot CLI negou o acesso (auth/SAML/SSO) — faça, nesta ordem: \
(1) rode `copilot auth login`; \
(2) autorize o app do Copilot para a organização em https://github.com/settings/connections/applications (botão 'Grant'/'Authorize' na org Avanade); \
(3) se o admin bloquear apps OAuth, peça a aprovação à TI; \
(4) enquanto isso use `--llm ollama` (o default local)"
    )]
    CopilotAuth,

    /// The Copilot CLI exited non-zero for a non-auth reason. `hint` is the
    /// first non-empty stderr line, capped at 200 chars — the raw body (and
    /// the prompt) are never echoed.
    #[error(
        "o Copilot CLI falhou: {hint} — teste o CLI fora do KIRA com `copilot -p teste` e, se persistir, use `--llm ollama`"
    )]
    CopilotFailed { hint: String },

    /// The Copilot CLI did not finish within the `KIRA_COPILOT_TIMEOUT`
    /// budget (default 180s) and was killed.
    #[error(
        "o Copilot CLI excedeu {secs}s — aumente o limite com KIRA_COPILOT_TIMEOUT=<segundos> ou use `--llm ollama`"
    )]
    CopilotTimeout { secs: u64 },

    /// `--llm`/`KIRA_LLM` named a backend that does not exist.
    #[error(
        "backend LLM '{name}' desconhecido — use `ollama` (o default local), `copilot` (spawn do Copilot CLI, ADR-0019) ou `foundry` (BYOK Microsoft Foundry, ADR-0020; também KIRA_LLM=ollama|copilot|foundry)"
    )]
    UnknownLlm { name: String },

    /// A local work-item file could not be read (`kira triage --file`).
    #[error("não foi possível ler o arquivo '{path}' — confira o caminho e tente de novo")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// A filesystem write was refused by the guard (ADR-0011, layer 1).
    ///
    /// The rendered message is the guard's own actionable text, so callers can
    /// surface it directly: refusing a write must explain how to proceed.
    #[error("{0}")]
    Guard(#[from] crate::guard::path::GuardError),

    /// The `kira context --json` report could not be serialized. Theoretical
    /// path (the payload is a plain struct); the variant exists so no call
    /// site ever needs `unwrap`/`expect` on serialization.
    #[error("não foi possível serializar o relatório do contexto: {0}")]
    ContextReportJson(serde_json::Error),

    /// The `kira triage --eval` measurement could not run (W2-bis, ADR-0018):
    /// the directory/gold is malformed, an item failed, or the JSON report
    /// could not be serialized. The String already carries the actionable
    /// schema/example — the gate never measures with fake data.
    #[error("{0}")]
    Eval(String),
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn missing_config_errors_suggest_kira_doctor() {
        for err in [Error::MissingPat, Error::MissingOrg, Error::MissingProject] {
            assert!(
                err.to_string().contains("kira doctor"),
                "erro deve sugerir `kira doctor`: {err}"
            );
        }
    }

    #[test]
    fn unauthorized_error_points_to_the_org_token_page() {
        let err = Error::AdoUnauthorized {
            organization: "minha-org".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "PAT inválido ou sem escopo Work Items (Read) — \
             gere um novo token em https://dev.azure.com/minha-org/_usersSettings/tokens \
             (scope: Work Items Read) e rode `kira doctor`"
        );
    }

    #[test]
    fn http_status_error_is_compact_and_actionable() {
        let err = Error::AdoStatus {
            status: 404,
            organization: "minha-org".to_string(),
        };
        let message = err.to_string();
        assert!(message.contains("HTTP 404"), "{message}");
        assert!(
            message.contains("https://dev.azure.com/minha-org"),
            "{message}"
        );
        assert!(
            !message.contains('<'),
            "mensagem não pode conter HTML: {message}"
        );
    }

    #[test]
    fn work_item_inacessivel_error_names_all_three_hypotheses_without_body() {
        let err = Error::WorkItemInacessivel {
            id: 523373,
            organization: "minha-org".to_string(),
            project: "Kira".to_string(),
        };
        let message = err.to_string();
        assert!(message.contains("#523373"), "mensagem: {message}");
        assert!(
            message.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"),
            "hipótese (1) deve apontar a página do token: {message}"
        );
        assert!(
            message.contains("--project <nome>"),
            "hipótese (2) deve sugerir --project: {message}"
        );
        assert!(
            message.contains("o id está errado"),
            "hipótese (3) precisa existir: {message}"
        );
        assert!(
            !message.contains('"'),
            "mensagem não pode conter corpo cru de resposta HTTP: {message}"
        );
    }

    #[test]
    fn ollama_not_running_error_tells_how_to_start_it() {
        let message = Error::LlmNotRunning.to_string();
        assert!(
            message.contains("Ollama não está rodando"),
            "mensagem: {message}"
        );
        assert!(message.contains("ollama serve"), "mensagem: {message}");
        assert!(
            message.contains("https://ollama.com"),
            "mensagem deve apontar a instalação: {message}"
        );
    }

    #[test]
    fn ollama_timeout_error_mentions_the_deadline_and_next_step() {
        let message = Error::LlmTimeout.to_string();
        assert!(message.contains("120s"), "mensagem: {message}");
        assert!(message.contains("ollama ps"), "mensagem: {message}");
    }

    #[test]
    fn ollama_status_error_is_compact_without_body() {
        let message = Error::LlmStatus { status: 404 }.to_string();
        assert!(message.contains("HTTP 404"), "mensagem: {message}");
        assert!(
            message.contains("ollama list"),
            "mensagem deve sugerir a checagem do modelo: {message}"
        );
        assert!(
            !message.contains('<'),
            "mensagem não pode conter HTML/corpo: {message}"
        );
    }

    /// Acceptance of W1-PR4: **every** variant tells the user what to do next.
    ///
    /// The check is a cue list rather than exact wording, so improving a message
    /// never breaks the test — but a variant that only says "failed" does.
    #[test]
    fn every_variant_is_actionable() {
        const CUES: [&str; 15] = [
            "defina", "rode", "informe", "use ", "corrija", "remova", "instale", "gere", "confira",
            "crie", "escolha", "atualize", "execute", "recusado", "aumente",
        ];

        let cases: Vec<(&str, Error)> = vec![
            ("MissingPat", Error::MissingPat),
            ("MissingOrg", Error::MissingOrg),
            ("MissingProject", Error::MissingProject),
            (
                "UnknownBoard",
                Error::UnknownBoard {
                    name: "jira".to_string(),
                },
            ),
            (
                "KeyringUnavailable",
                Error::KeyringUnavailable {
                    action: "ler",
                    source: Box::new(std::io::Error::other("sem daemon")),
                },
            ),
            (
                "ConfigRead",
                Error::ConfigRead {
                    path: "/tmp/config.toml".to_string(),
                    source: std::io::Error::other("negado"),
                },
            ),
            (
                "ConfigParse",
                Error::ConfigParse {
                    path: "/tmp/config.toml".to_string(),
                    line: 3,
                },
            ),
            (
                "Ado",
                Error::Ado(Box::new(std::io::Error::other("conexão recusada"))),
            ),
            (
                "AdoUnauthorized",
                Error::AdoUnauthorized {
                    organization: "minha-org".to_string(),
                },
            ),
            (
                "AdoStatus",
                Error::AdoStatus {
                    status: 503,
                    organization: "minha-org".to_string(),
                },
            ),
            (
                "WorkItemInacessivel",
                Error::WorkItemInacessivel {
                    id: 4821,
                    organization: "minha-org".to_string(),
                    project: "Projeto".to_string(),
                },
            ),
            (
                "Api",
                Error::Api(serde_json::from_str::<i32>("não é json").expect_err("json inválido")),
            ),
            ("LlmNotRunning", Error::LlmNotRunning),
            ("LlmTimeout", Error::LlmTimeout),
            ("LlmStatus", Error::LlmStatus { status: 404 }),
            (
                "LlmBadResponse",
                Error::LlmBadResponse("corpo omitido".to_string()),
            ),
            (
                "LlmRequest",
                Error::LlmRequest("conexão recusada".to_string()),
            ),
            ("LlmFormat", Error::LlmFormat),
            ("CopilotMissing", Error::CopilotMissing),
            ("CopilotAuth", Error::CopilotAuth),
            (
                "CopilotFailed",
                Error::CopilotFailed {
                    hint: "segfault do CLI".to_string(),
                },
            ),
            ("CopilotTimeout", Error::CopilotTimeout { secs: 180 }),
            ("FoundryMissingKey", Error::FoundryMissingKey),
            ("FoundryMissingEndpoint", Error::FoundryMissingEndpoint),
            ("FoundryMissingDeployment", Error::FoundryMissingDeployment),
            ("FoundryEndpoint", Error::FoundryEndpoint),
            ("FoundryAuth", Error::FoundryAuth { status: 401 }),
            ("FoundryDeployment", Error::FoundryDeployment),
            (
                "FoundryApiVersion",
                Error::FoundryApiVersion { status: 400 },
            ),
            (
                "FoundryBodyMode",
                Error::FoundryBodyMode {
                    status: 400,
                    detail: "unsupported_parameter: temperature".to_string(),
                },
            ),
            (
                "FoundryRateLimited",
                Error::FoundryRateLimited("42".to_string()),
            ),
            (
                "FoundryFailed",
                Error::FoundryFailed {
                    detail: "campo ausente: choices".to_string(),
                },
            ),
            (
                "FoundryTimeout",
                Error::FoundryTimeout { timeout_secs: 120 },
            ),
            (
                "FoundryEnv",
                Error::FoundryEnv {
                    env: "KIRA_FOUNDRY_MAX_TOKENS",
                    value: "abc".to_string(),
                    hint: "defina um inteiro >= 1 (ex.: export KIRA_FOUNDRY_MAX_TOKENS=1024)",
                },
            ),
            (
                "UnknownLlm",
                Error::UnknownLlm {
                    name: "gpt".to_string(),
                },
            ),
            (
                "Io",
                Error::Io {
                    path: "/tmp/item.json".to_string(),
                    source: std::io::Error::other("ausente"),
                },
            ),
            (
                "Guard",
                Error::Guard(crate::guard::path::GuardError::ParentTraversal {
                    candidate: "../escape.txt".to_string(),
                }),
            ),
        ];

        for (name, err) in cases {
            let message = err.to_string();
            assert!(
                CUES.iter().any(|cue| message.contains(cue)),
                "{name} não diz o que fazer (W1-PR4): {message}"
            );
        }
    }
}
