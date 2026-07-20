//! SPEC waivers for `vyre-driver-cuda` resident dispatch hot-path scan findings.
//!
//! `cargo xtask hot-path-scan` flags every `format!` and allocation pattern in
//! files listed under `docs/optimization/HOT_PATHS.toml`. Resident dispatch
//! keeps contextual `InvalidProgram` messages on cold validation paths; the
//! success-path fixes live in `stream.rs` and `staging_reserve.rs`.
//!
//! This contract is a **red test**: bump the budget only with a SPEC note and
//! evidence that the new pattern is cold-path or allocation-free.

const RESIDENT_DISPATCH_SOURCE: &str =
    include_str!("../../vyre-driver-cuda/src/backend/resident_dispatch.rs");

// The resident-batch async path was split out of the monolithic
// resident_dispatch.rs into the `resident_dispatch/batch.rs` submodule (LAW7
// module split), so the batch-pending delegation contract is verified against
// that file. The format!/Vec::new budget below still tracks the parent file.
const RESIDENT_DISPATCH_BATCH_SOURCE: &str =
    include_str!("../../vyre-driver-cuda/src/backend/resident_dispatch/batch.rs");

fn production_source() -> &'static str {
    RESIDENT_DISPATCH_SOURCE
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: resident_dispatch.rs production source must precede tests.")
}

fn batch_production_source() -> &'static str {
    RESIDENT_DISPATCH_BATCH_SOURCE
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: resident_dispatch/batch.rs production source must precede tests.")
}

fn count_pattern(source: &str, pattern: &str) -> usize {
    source.matches(pattern).count()
}

/// Cold-path `format!` diagnostics for contextual `InvalidProgram` fixes.
const MAX_PRODUCTION_FORMAT_MACRO: usize = 27;

/// Borrowed-fallback staging only (debug: `VYRE_CUDA_RESIDENT_BORROWED_FALLBACK`;
/// release also requires `VYRE_CUDA_ALLOW_BORROWED_FALLBACK=1`).
const MAX_PRODUCTION_VEC_NEW: usize = 1;

#[test]
fn cuda_resident_dispatch_hot_path_waiver_budget_is_not_exceeded() {
    let production = production_source();
    let format_count = count_pattern(production, "format!(");
    assert!(
        format_count <= MAX_PRODUCTION_FORMAT_MACRO,
        "Fix: resident_dispatch production gained {format_count} format! site(s) (budget {MAX_PRODUCTION_FORMAT_MACRO}). \
         Either move cold diagnostics out of the hot-path file or update this SPEC waiver with Command→Output evidence."
    );
    let vec_new_count = count_pattern(production, "Vec::new()");
    assert!(
        vec_new_count <= MAX_PRODUCTION_VEC_NEW,
        "Fix: resident_dispatch production gained {vec_new_count} Vec::new() site(s) (budget {MAX_PRODUCTION_VEC_NEW}). \
         Reuse staging_reserve helpers or extend this waiver with measured cold-path proof."
    );
}

#[test]
fn cuda_resident_batch_pending_does_not_construct_outputs_in_hot_path_file() {
    let production = batch_production_source();
    assert!(
        production.contains("CudaPendingDispatch::new_resident_batch_pending("),
        "Fix: CUDA resident batch async must delegate empty host-output construction to stream.rs."
    );
    assert!(
        !production.contains("CudaPendingDispatch::new(\n            Arc::clone(&self.ctx),\n            Arc::clone(&self.launch_resources),\n            event,\n            stream,\n            allocations,\n            Some(resident_use),\n            Some(host_transfers),\n            Vec::new(),"),
        "Fix: resident_dispatch.rs must not inline Vec::new() for resident batch pending outputs."
    );
}
