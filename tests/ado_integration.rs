//! Real integration test against Azure DevOps.
//!
//! Ignored by default: it needs real credentials.
//!
//! Run with: `cargo test -- --ignored`
//!
//! Required env: `AZURE_DEVOPS_PAT`, `AZURE_DEVOPS_ORG`, `AZURE_DEVOPS_PROJECT`.
//! Optional env: `KIRA_TEST_WORK_ITEM_ID` (default: 1).

use kira::ado;

fn env_or_panic(name: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => panic!("defina {name} para rodar o teste de integração"),
    }
}

#[tokio::test]
#[ignore = "requires real Azure DevOps credentials; run with: cargo test -- --ignored"]
async fn fetches_real_work_item() {
    let pat = env_or_panic("AZURE_DEVOPS_PAT");
    let organization = env_or_panic("AZURE_DEVOPS_ORG");
    let project = env_or_panic("AZURE_DEVOPS_PROJECT");
    let id = std::env::var("KIRA_TEST_WORK_ITEM_ID")
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or(1);

    let item = ado::get_work_item(&pat, &organization, &project, id)
        .await
        .unwrap_or_else(|err| panic!("falha ao consultar work item {id}: {err}"));

    assert_eq!(item.id, id);
    assert!(!item.title.is_empty());
}
