//! Compatibility re-exports for source provenance.
//!
//! Source fingerprinting is owned by `vyre-driver::evidence` so dispatch,
//! benchmark, conformance, and replay surfaces cannot drift into parallel
//! provenance contracts. This module keeps the historical
//! `vyre_bench::probes::*` imports working.

pub use vyre_driver::{
    capture_git_info, capture_git_info_at, source_fingerprint, source_tree_fingerprint,
    source_tree_fingerprint_at, SourceProvenance,
};
