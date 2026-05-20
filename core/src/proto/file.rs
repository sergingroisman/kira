//! File tool wire types — exported to TypeScript via `ts-rs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/file.ts")]
pub struct FileReadRequest {
    pub path: String,
    pub base_dir: String,
    #[serde(default = "default_max_read_bytes")]
    pub max_bytes: usize,
}

fn default_max_read_bytes() -> usize {
    crate::safety::FILE_READ_MAX_BYTES_DEFAULT
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/file.ts")]
pub struct FileReadResponse {
    pub content: String,
    pub bytes_read: usize,
    pub truncated: bool,
    pub canonical_path: String,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/file.ts")]
pub struct FileWriteRequest {
    pub path: String,
    pub base_dir: String,
    pub content: String,
    #[serde(default)]
    pub create_dirs: bool,
    #[serde(default = "default_true")]
    pub overwrite: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/file.ts")]
pub struct FileWriteResponse {
    pub bytes_written: usize,
    pub canonical_path: String,
    pub created: bool,
}
