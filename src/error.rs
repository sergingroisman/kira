use thiserror::Error;

/// Errors produced by KIRA. Every message says what to do next.
#[derive(Debug, Error)]
pub enum Error {
    /// `AZURE_DEVOPS_PAT` is unset or empty.
    #[error(
        "defina a variável de ambiente AZURE_DEVOPS_PAT com um Personal Access Token que tenha permissão de leitura de Work Items (ex.: export AZURE_DEVOPS_PAT=...)\npara diagnosticar a configuração desta máquina, rode `kira doctor`"
    )]
    MissingPat,

    /// `AZURE_DEVOPS_ORG` is unset or empty.
    #[error(
        "defina a variável de ambiente AZURE_DEVOPS_ORG com o nome da organização (ex.: export AZURE_DEVOPS_ORG=minha-org)\npara diagnosticar a configuração desta máquina, rode `kira doctor`"
    )]
    MissingOrg,

    /// Neither `--project` nor `AZURE_DEVOPS_PROJECT` provided a project name.
    #[error(
        "informe o projeto com --project <nome> ou defina a variável de ambiente AZURE_DEVOPS_PROJECT\npara diagnosticar a configuração desta máquina, rode `kira doctor`"
    )]
    MissingProject,

    /// Azure DevOps request failed (network, authentication, not found, ...).
    #[error("falha ao consultar o Azure DevOps: {0}")]
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

    /// Azure DevOps answered with an unexpected JSON shape.
    #[error("resposta inesperada do Azure DevOps: {0}")]
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
    #[error("falha na chamada ao Ollama: {0}")]
    LlmRequest(String),

    /// A local work-item file could not be read (`kira triage --file`).
    #[error("não foi possível ler o arquivo '{path}' — confira o caminho e tente de novo")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
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
}
