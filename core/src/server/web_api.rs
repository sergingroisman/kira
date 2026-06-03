//! `/v1/web/*` — sovereign search + safe fetch (P0 + soberania).

use axum::{extract::State, Json};
use serde::Deserialize;
use tracing::instrument;

use crate::config::WebToolsConfig;
use crate::server::{error::AppError, AppState};
use crate::web::{
    fetch::{render_md as render_fetch_md, FetchResult},
    search::{render_results_md, SearchOpts, SearchResult},
};

fn web_cfg(state: &AppState) -> WebToolsConfig {
    state
        .tokens
        .as_ref()
        .map(|t| t.web_tools.clone())
        .unwrap_or_default()
}

#[derive(Deserialize, Debug)]
pub struct WebSearchReq {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(serde::Serialize, Debug)]
pub struct WebSearchResp {
    pub results: Vec<SearchResult>,
    pub markdown: String,
}

#[instrument(skip(state), fields(q = %req.query))]
pub async fn search(
    State(state): State<AppState>,
    Json(req): Json<WebSearchReq>,
) -> Result<Json<WebSearchResp>, AppError> {
    let cfg = web_cfg(&state);
    let opts = SearchOpts {
        max_results: req.max_results,
        language: req.language,
    };
    let results = crate::web::search(&cfg, &req.query, &opts).await?;
    let markdown = render_results_md(&results);
    Ok(Json(WebSearchResp { results, markdown }))
}

#[derive(Deserialize, Debug)]
pub struct WebFetchReq {
    pub url: String,
}

#[derive(serde::Serialize, Debug)]
pub struct WebFetchResp {
    pub result: FetchResult,
    pub markdown: String,
}

#[instrument(skip(state), fields(url = %req.url))]
pub async fn fetch(
    State(state): State<AppState>,
    Json(req): Json<WebFetchReq>,
) -> Result<Json<WebFetchResp>, AppError> {
    let cfg = web_cfg(&state);
    let result = crate::web::fetch(&cfg, &req.url).await?;
    let markdown = render_fetch_md(&result, cfg.fetch_output_max_chars);
    Ok(Json(WebFetchResp { result, markdown }))
}
