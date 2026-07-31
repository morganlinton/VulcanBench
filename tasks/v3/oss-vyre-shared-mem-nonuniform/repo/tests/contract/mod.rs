//! Workspace-level contract tests (VYRE-TASK-000003).
//!
//! These modules are wired into `vyre-foundation` via
//! `tests/contract_workspace.rs` so `cargo test -p vyre-foundation contract`
//! executes cross-crate invariants without a dedicated workspace test crate.

mod claims_inventory_smoke;
mod public_api_surface;
mod xtask_help_smoke;

/// Workspace root (`vyre/` directory).
pub(crate) fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("contract tests are included from vyre-foundation under the workspace root")
        .to_path_buf()
}
