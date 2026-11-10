//! Guards that stand between KIRA and the filesystem (ADR-0011).
//!
//! Layer 1: [`path::Guard`] — deny-list and root confinement, decided without
//! touching the disk. Later layers (jail: canonicalisation + rlimits) are PR27.

pub mod path;
