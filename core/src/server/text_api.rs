//! Text pipeline API endpoints (C1 / M17-M20).
//!
//! `POST /v1/text/tokenize` — `{text}` → `{count}`
//! `POST /v1/text/chunk`    — `{text, target_tokens?, overlap_tokens?}` → `{chunks}`
//! `POST /v1/text/compress` — `{text, target_ratio?, strip_comments?}` → result
//! `POST /v1/text/rerank`   — `{query, documents, top_k?}` → ranked list

use axum::Json;
use serde::{Deserialize, Serialize};

use crate::text::{
    chunk, compress,
    compressor::CompressOpts,
    rerank,
    tokenizer::{ApproxTokenizer, Tokenizer},
    Chunk, ChunkOpts, CompressionResult, RankedDoc,
};

// ── tokenize ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TokenizeReq {
    pub text: String,
}

#[derive(Serialize)]
pub struct TokenizeResp {
    pub count: usize,
}

pub async fn tokenize(Json(req): Json<TokenizeReq>) -> Json<TokenizeResp> {
    let t = ApproxTokenizer::default();
    Json(TokenizeResp {
        count: t.count(&req.text),
    })
}

// ── chunk ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChunkReq {
    pub text: String,
    #[serde(default)]
    pub target_tokens: Option<usize>,
    #[serde(default)]
    pub overlap_tokens: Option<usize>,
}

#[derive(Serialize)]
pub struct ChunkResp {
    pub chunks: Vec<Chunk>,
    pub count: usize,
}

pub async fn chunk_endpoint(Json(req): Json<ChunkReq>) -> Json<ChunkResp> {
    let defaults = ChunkOpts::default();
    let opts = ChunkOpts {
        target_tokens: req.target_tokens.unwrap_or(defaults.target_tokens),
        overlap_tokens: req.overlap_tokens.unwrap_or(defaults.overlap_tokens),
    };
    let t = ApproxTokenizer::default();
    let chunks = chunk(&req.text, &t, opts);
    let count = chunks.len();
    Json(ChunkResp { chunks, count })
}

// ── compress ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CompressReq {
    pub text: String,
    #[serde(default)]
    pub target_ratio: Option<f64>,
    #[serde(default)]
    pub strip_comments: Option<bool>,
}

pub async fn compress_endpoint(Json(req): Json<CompressReq>) -> Json<CompressionResult> {
    let defaults = CompressOpts::default();
    let opts = CompressOpts {
        target_ratio: req.target_ratio.unwrap_or(defaults.target_ratio),
        strip_comments: req.strip_comments.unwrap_or(defaults.strip_comments),
    };
    Json(compress(&req.text, opts))
}

// ── rerank ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RerankReq {
    pub query: String,
    pub documents: Vec<String>,
    #[serde(default)]
    pub top_k: Option<usize>,
}

#[derive(Serialize)]
pub struct RerankResp {
    pub ranked: Vec<RankedDoc>,
}

pub async fn rerank_endpoint(Json(req): Json<RerankReq>) -> Json<RerankResp> {
    let ranked = rerank(&req.query, &req.documents, req.top_k.unwrap_or(0));
    Json(RerankResp { ranked })
}
