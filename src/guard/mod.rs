//! Guards that stand between KIRA and the filesystem (ADR-0011).
//!
//! Layer 1: [`path::Guard`] — deny-list and root confinement, decided without
//! touching the disk (PR3).
//! Layer 2: [`jail::Jail`] — confinamento por canonicalização + rlimits no
//! processo filho (PR27, ADR-0011).

pub mod jail;
pub mod path;
