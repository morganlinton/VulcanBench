//! Shader-snapshot migration entries collected by inventory.
//!
//! Each dialect op that generates a target-text kernel submits a
//! `MigrationEntry` so the pre-sweep snapshot tool can dump every shader
//! to disk and compare future runs byte-for-byte against the locked
//! snapshot. The entry carries the op id, the destination snapshot path,
//! and a closure that emits the target-text source on demand.

/// Snapshot migration entry for a single op.
#[allow(dead_code)]
pub(crate) struct MigrationEntry {
    /// Stable op identifier (e.g. `"workgroup.visitor"`).
    pub(crate) op_id: &'static str,
    /// On-disk path relative to the repo root where the snapshot lives.
    pub(crate) snapshot_path: &'static str,
    /// Emits the target-text source this op generates.
    pub(crate) emit: fn() -> String,
}

inventory::collect!(MigrationEntry);
