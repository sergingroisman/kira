use serde::Deserialize;

use crate::error::Error;

/// Domain view of an Azure DevOps work item, free of API specifics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItem {
    pub id: i32,
    pub title: String,
    pub state: String,
    pub work_item_type: String,
    pub assigned_to: Option<String>,
    pub description: Option<String>,
    pub area_path: Option<String>,
}

/// Wire shape of `GET {organization}/{project}/_apis/wit/workitems/{id}`.
#[derive(Debug, Deserialize)]
struct WorkItemApi {
    id: i32,
    fields: FieldsApi,
}

#[derive(Debug, Deserialize)]
struct FieldsApi {
    #[serde(rename = "System.Title")]
    title: String,
    #[serde(rename = "System.State")]
    state: String,
    #[serde(rename = "System.WorkItemType")]
    work_item_type: String,
    #[serde(rename = "System.AssignedTo", default)]
    assigned_to: Option<IdentityRefApi>,
    #[serde(rename = "System.Description", default)]
    description: Option<String>,
    #[serde(rename = "System.AreaPath", default)]
    area_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdentityRefApi {
    #[serde(rename = "displayName", default)]
    display_name: Option<String>,
}

impl WorkItem {
    /// Parses the JSON body returned by the Azure DevOps REST API.
    pub fn from_api_json(body: &str) -> Result<Self, Error> {
        let api: WorkItemApi = serde_json::from_str(body)?;
        Ok(Self {
            id: api.id,
            title: api.fields.title,
            state: api.fields.state,
            work_item_type: api.fields.work_item_type,
            assigned_to: api
                .fields
                .assigned_to
                .and_then(|reference| reference.display_name),
            description: api
                .fields
                .description
                .filter(|text| !text.trim().is_empty()),
            area_path: api.fields.area_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_FULL: &str = include_str!("../tests/fixtures/work_item_4821.json");
    const FIXTURE_MINIMAL: &str = include_str!("../tests/fixtures/work_item_minimal.json");

    fn parse(body: &str) -> WorkItem {
        WorkItem::from_api_json(body).unwrap_or_else(|err| panic!("fixture inválida: {err}"))
    }

    #[test]
    fn parses_full_fixture() {
        let item = parse(FIXTURE_FULL);
        assert_eq!(item.id, 4821);
        assert_eq!(item.title, "Sessão expirada com token válido");
        assert_eq!(item.state, "Active");
        assert_eq!(item.work_item_type, "Bug");
        assert_eq!(item.assigned_to.as_deref(), Some("Maria Souza"));
        assert_eq!(item.area_path.as_deref(), Some("Kira\\Backend"));
        let description = item
            .description
            .unwrap_or_else(|| panic!("descrição deve existir na fixture completa"));
        assert!(
            description.contains("<b>"),
            "descrição bruta mantém HTML: {description}"
        );
    }

    #[test]
    fn parses_minimal_fixture() {
        let item = parse(FIXTURE_MINIMAL);
        assert_eq!(item.id, 42);
        assert_eq!(item.title, "Somente o essencial");
        assert_eq!(item.state, "New");
        assert_eq!(item.work_item_type, "Task");
        assert_eq!(item.assigned_to, None);
        assert_eq!(item.description, None);
        assert_eq!(item.area_path, None);
    }

    #[test]
    fn missing_required_field_is_an_error_naming_the_field() {
        let body = r#"{"id": 1, "fields": {"System.State": "New"}}"#;
        let Err(err) = WorkItem::from_api_json(body) else {
            panic!("campo obrigatório ausente deve produzir erro");
        };
        assert!(
            err.to_string().contains("System.Title"),
            "mensagem deve citar o campo ausente: {err}"
        );
    }
}
