use thiserror::Error;

/// Errors produced by KIRA. Every message says what to do next.
#[derive(Debug, Error)]
pub enum Error {
    /// `AZURE_DEVOPS_PAT` is unset or empty.
    #[error(
        "defina a variável de ambiente AZURE_DEVOPS_PAT com um Personal Access Token que tenha permissão de leitura de Work Items (ex.: export AZURE_DEVOPS_PAT=...)"
    )]
    MissingPat,

    /// `AZURE_DEVOPS_ORG` is unset or empty.
    #[error(
        "defina a variável de ambiente AZURE_DEVOPS_ORG com o nome da organização (ex.: export AZURE_DEVOPS_ORG=minha-org)"
    )]
    MissingOrg,

    /// Neither `--project` nor `AZURE_DEVOPS_PROJECT` provided a project name.
    #[error(
        "informe o projeto com --project <nome> ou defina a variável de ambiente AZURE_DEVOPS_PROJECT"
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
}
