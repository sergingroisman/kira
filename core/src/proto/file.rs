//! File tool wire types.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct FileReadRequest {
    pub path: String,
    pub base_dir: String,
    #[serde(default = "default_max_read_bytes")]
    pub max_bytes: usize,
}

fn default_max_read_bytes() -> usize {
    crate::safety::FILE_READ_MAX_BYTES_DEFAULT
}

#[derive(Serialize)]
pub struct FileReadResponse {
    pub content: String,
    pub bytes_read: usize,
    pub truncated: bool,
    pub canonical_path: String,
}

#[derive(Deserialize, Debug)]
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

#[derive(Serialize)]
pub struct FileWriteResponse {
    pub bytes_written: usize,
    pub canonical_path: String,
    pub created: bool,
}
