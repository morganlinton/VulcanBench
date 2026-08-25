#![forbid(unsafe_code)]
//! vyre-aot  -  Ahead-of-time compilation for vyre Programs.
//!
//! Vyre is a JIT-friendly substrate: at runtime, a `Program` is lowered
//! through `vyre-driver` to a backend-specific module loaded by the GPU
//! driver. That works when the consumer can ship the entire vyre runtime
//! alongside the artifact.
//!
//! For embedded targets and code-budget-constrained submissions
//! (parameter-golf is the motivating case: `code_bytes + compressed_model_bytes
//! ≤ 16,000,000`) we need the compiler to disappear at runtime. Only the
//! pre-emitted target bytes plus a tiny backend loader should ship.
//!
//! ## Public API
//!
//! Three functions:
//!
//! - [`mod@compile`]: lower a `Program` to a [`CompiledArtifact`] containing
//!   target-specific kernel bytes, the buffer-binding
//!   table, and the dispatch config.
//! - [`emit_launcher_rust`]: generate a self-contained Rust launcher crate
//!   source tree that loads the artifact via raw driver-API FFI and
//!   dispatches it.
//! - [`mod@bundle`]: package an artifact + weight bytes + launcher source into
//!   the final on-disk submission tree.
//!
//! Concrete driver crates register AOT emitters through `vyre-driver`.

#![deny(missing_docs)]

pub mod artifact;
pub mod bundle;
/// Runtime-cache compatibility for AOT-emitted artifacts (audit P0 #26).
pub mod cache;
pub mod compile;
pub mod launcher;
pub mod manifest;

pub use artifact::{
    BufferAccessKind, BufferEntry, BufferMemoryKind, CompiledArtifact, DispatchGeometry, Target,
};
pub use bundle::{bundle, Bundle, BundleError};
pub use compile::{compile, CompileError};
pub use launcher::{emit_launcher_rust, LauncherError, LauncherOpts};
pub use manifest::Manifest;

/// Crate version surfaced into emitted artifacts and manifests.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Snapshot the driver-tier observability surface from inside vyre-aot.
/// Exposes substrate counters + decision histograms so callers
/// emitting AOT bundles can include them in their build provenance.
#[must_use]
pub fn observability_snapshot() -> vyre_driver::observability::DriverObservability {
    vyre_driver::observability::DriverObservability::snapshot()
}

/// VSA-fingerprint a Program with the same key family AOT artifacts
/// persist on `CompiledArtifact::vsa_fingerprint`. Producers use this
/// to pre-check whether the artifact already exists before paying
/// the compile cost.
#[must_use]
pub fn program_fingerprint(program: &vyre_foundation::ir::Program) -> Vec<u32> {
    vyre_driver::program_vsa_fingerprint(program)
}
