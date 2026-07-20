//! Tier 2.5 byte/text scan primitives (DFA, substring, filters).
//!
//! The path IS the interface. Callers write
//! `vyre_primitives::matching::bracket_match::bracket_match(...)`  -
//! explicit paths; no wildcard re-exports.
//!
//! See `docs/primitives-tier.md` and `docs/lego-block-rule.md`.

/// Back-compat module tree for older `matching::ops::*` imports.
pub mod ops;

/// Bounded-stack bracket-pair detector.
pub mod bracket_match;
/// Anchor-DFA plan shared by software, SPIR-V, and accelerator experiments.
pub mod anchor_dfa;

mod region_programs;

/// Span-region dedup primitive. Collapses same-pid overlapping or
/// touching `(pid, start, end)` triples into a representative span.
/// Every multimatch consumer in the workspace was reimplementing this
///  -  one primitive replaces all of them.
pub mod region;
#[cfg(test)]
mod region_tests;

mod dfa_compile;

/// NFA → CompiledDfa subset construction. Composes with
/// `dfa_compile`'s output type so any consumer of the dense AC kernel
/// (`vyre_libs::scan::classic_ac_bounded_ranges_program`) can scan
/// regex pattern sets too - not just literal AC.
pub mod nfa_to_dfa;

#[cfg(any(test, feature = "cpu-parity"))]
pub use bracket_match::cpu_ref as bracket_match_cpu_ref;
#[cfg(any(test, feature = "cpu-parity"))]
pub use bracket_match::cpu_ref_into as bracket_match_cpu_ref_into;
pub use bracket_match::{
    bracket_match, bracket_match_dispatch_grid, pack_u32 as pack_bracket_u32,
    BRACKET_MATCH_PARALLEL_WORKGROUP_SIZE, CLOSE_BRACE, MATCH_NONE, OPEN_BRACE, OTHER,
};
pub use anchor_dfa::{
    build_anchor_dfa_plan, AnchorDfaCandidate, AnchorDfaLiteral, AnchorDfaPlan,
    AnchorDfaPlanError, ANCHOR_DFA_PLAN_SCHEMA_VERSION,
};
pub use dfa_compile::{
    dfa_compile, dfa_compile_with_budget, CompiledDfa, DfaCompileError, DfaWireError,
    DEFAULT_DFA_BUDGET_BYTES,
};
pub use nfa_to_dfa::{
    dfa_fingerprint, dfa_wire_bytes, nfa_to_dfa, DfaDedupBatch, DfaDedupResult, DfaDedupStats,
    DfaDedupTable, NfaTables, NfaToDfaError,
};
#[cfg(any(test, feature = "cpu-parity"))]
pub use region::dedup_regions_cpu;
#[cfg(any(test, feature = "cpu-parity"))]
pub use region::dedup_regions_inplace;
pub use region::{
    dedup_regions_cluster_program, dedup_regions_flag_program, region_dedup_dispatch_grid,
    RegionTriple, REGION_DEDUP_WORKGROUP_SIZE,
};
