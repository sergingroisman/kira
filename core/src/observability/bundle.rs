//! `HarnessBundle` (P7) — cryptographic fingerprint of the running harness.
//!
//! Hash inputs (sha256 over concatenation in stable order):
//! 1. `tokens.toml` raw bytes (if present)
//! 2. `.kira/workflows/*.yaml` sorted by filename
//! 3. `core/src/agents/prompts/**/*.md` sorted by path (embedded compile-time)
//! 4. Active lessons snapshot (id + version sorted by id)
//!
//! Use cases:
//! - "Ficou mais lento desde terça" — `SELECT * FROM runs WHERE bundle_hash=...`
//! - Shadow runs (v1): parallel `bundle_B` vs current to A/B harness changes
//! - Promotion (v1): only promote `bundle_B` → main when metrics improve

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Compiled-in prompt artifacts. Add new files here when extending prompts.
const PROMPT_ARTIFACTS: &[(&str, &str)] = &[
    ("agents/prompts/_principles.md", include_str!("../agents/prompts/_principles.md")),
    ("agents/prompts/writer.md", include_str!("../agents/prompts/writer.md")),
    ("agents/prompts/deployer.md", include_str!("../agents/prompts/deployer.md")),
    ("agents/prompts/judge.md", include_str!("../agents/prompts/judge.md")),
    ("agents/prompts/reviewer.md", include_str!("../agents/prompts/reviewer.md")),
    ("agents/prompts/sdd/specify.md", include_str!("../agents/prompts/sdd/specify.md")),
    ("agents/prompts/sdd/design.md", include_str!("../agents/prompts/sdd/design.md")),
    ("agents/prompts/sdd/tasks.md", include_str!("../agents/prompts/sdd/tasks.md")),
];

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/bundle.ts")]
pub struct HarnessBundle {
    /// Full sha256 hex of the concatenated inputs.
    pub hash: String,
    /// Per-component sha256 — pinpoints which input changed between bundles.
    pub components: BundleComponents,
    /// kira-core Cargo package version.
    pub kira_version: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/bundle.ts")]
pub struct BundleComponents {
    pub tokens_toml: String,
    pub workflows: String,
    pub prompts: String,
    pub lessons: String,
}

/// SHA-256 implementation via FIPS 180-4. Inlined to avoid a `sha2` dep.
mod sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    pub fn hash(input: &[u8]) -> [u8; 32] {
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
            0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
        ];
        let bit_len = (input.len() as u64).wrapping_mul(8);
        let mut padded = input.to_vec();
        padded.push(0x80);
        while padded.len() % 64 != 56 {
            padded.push(0);
        }
        padded.extend_from_slice(&bit_len.to_be_bytes());
        for chunk in padded.chunks_exact(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3],
                ]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }
            let mut a = h[0]; let mut b = h[1]; let mut c = h[2]; let mut d = h[3];
            let mut e = h[4]; let mut f = h[5]; let mut g = h[6]; let mut hh = h[7];
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ (!e & g);
                let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                hh = g; g = f; f = e; e = d.wrapping_add(t1);
                d = c; c = b; b = a; a = t1.wrapping_add(t2);
            }
            h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e); h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g); h[7] = h[7].wrapping_add(hh);
        }
        let mut out = [0u8; 32];
        for (i, v) in h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }
        out
    }

    pub fn hex(input: &[u8]) -> String {
        let h = hash(input);
        let mut s = String::with_capacity(64);
        for b in &h {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

pub fn sha256_hex(input: &[u8]) -> String {
    sha256::hex(input)
}

/// Compute hash for the prompts compiled in. Stable across machines for
/// the same kira-core build.
fn prompts_hash() -> String {
    let mut combined = Vec::new();
    for (path, content) in PROMPT_ARTIFACTS {
        combined.extend_from_slice(path.as_bytes());
        combined.push(b'|');
        combined.extend_from_slice(content.as_bytes());
        combined.push(b'\n');
    }
    sha256_hex(&combined)
}

/// Compute hash for `tokens.toml` if present in repo root.
fn tokens_hash(repo_root: &Path) -> String {
    let p = repo_root.join("tokens.toml");
    match std::fs::read(&p) {
        Ok(bytes) => sha256_hex(&bytes),
        Err(_) => "absent".into(),
    }
}

/// Compute hash for all `.kira/workflows/*.yaml` (sorted by name).
fn workflows_hash(repo_root: &Path) -> String {
    let dir = repo_root.join(".kira/workflows");
    let mut entries: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
            .collect(),
        Err(_) => return "absent".into(),
    };
    entries.sort();
    let mut combined = Vec::new();
    for path in entries {
        if let Ok(b) = std::fs::read(&path) {
            combined.extend_from_slice(path.file_name().and_then(|x| x.to_str()).unwrap_or("").as_bytes());
            combined.push(b'|');
            combined.extend_from_slice(&b);
            combined.push(b'\n');
        }
    }
    if combined.is_empty() {
        "empty".into()
    } else {
        sha256_hex(&combined)
    }
}

/// Compute hash for active lessons. `lessons_snapshot` is the JSON list
/// (id + version + priority) sorted by id.
pub fn lessons_hash(snapshot_json: &str) -> String {
    sha256_hex(snapshot_json.as_bytes())
}

/// Compute the full bundle. Caller passes lessons snapshot to avoid
/// crate dep cycles (`observability` → `lessons` would be bad).
pub fn current(repo_root: &Path, lessons_snapshot_json: &str) -> HarnessBundle {
    let tokens = tokens_hash(repo_root);
    let workflows = workflows_hash(repo_root);
    let prompts = prompts_hash();
    let lessons = lessons_hash(lessons_snapshot_json);

    // Final bundle hash = sha256 over component hashes in stable order.
    let combined = format!(
        "tokens:{tokens}|workflows:{workflows}|prompts:{prompts}|lessons:{lessons}"
    );
    let hash = sha256_hex(combined.as_bytes());

    HarnessBundle {
        hash,
        components: BundleComponents {
            tokens_toml: tokens,
            workflows,
            prompts,
            lessons,
        },
        kira_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        // FIPS test: sha256("abc") = ba7816...
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_empty() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn prompts_hash_stable() {
        // Determinism — same build = same hash.
        let a = prompts_hash();
        let b = prompts_hash();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn current_bundle_has_all_components() {
        let tmp = std::env::temp_dir().join("kira-bundle-test");
        std::fs::create_dir_all(&tmp).unwrap();
        let bundle = current(&tmp, "[]");
        assert_eq!(bundle.hash.len(), 64);
        assert_eq!(bundle.components.prompts.len(), 64);
        assert_eq!(bundle.kira_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn lesson_change_alters_bundle() {
        let tmp = std::env::temp_dir().join("kira-bundle-lesson-test");
        std::fs::create_dir_all(&tmp).unwrap();
        let a = current(&tmp, "[]");
        let b = current(&tmp, "[{\"id\":\"L-1\"}]");
        assert_ne!(a.hash, b.hash);
        assert_ne!(a.components.lessons, b.components.lessons);
    }
}
