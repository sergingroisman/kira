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

    /// Azure DevOps answered with an unexpected JSON shape.
    #[error("resposta inesperada do Azure DevOps: {0}")]
    Api(#[from] serde_json::Error),
}
