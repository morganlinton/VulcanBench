//! Self-hosted optimizer keystone.
//!
//! The encoder turns a `vyre_foundation::ir::Program` into the canonical
//! 5-buffer `ProgramGraph` ABI shared by every Tier 2.5 graph primitive.
//! Once the IR lives in that shape, optimizer passes are *graph primitives
//! reused as compiler passes*: DCE is `persistent_bfs` reachability, CSE is
//! `union_find` over a structural-hash key, const-fold is `level_wave`
//! bottom-up evaluation. The compiler runs on the same substrate it
//! ships to users.
//!
//! V1 scope: flat-entry Programs only (no nested `If`/`Loop`/`Block`/
//! `Region` scoping) and DCE only. Nested scopes and the CSE/const-fold
//! passes land in V2 against the same encoding.
//!
//! GPU dispatch sits one layer above this crate (driver layer). V1 uses
//! `vyre_primitives::graph::persistent_bfs::cpu_ref` so the encoding can
//! be proven sound against the existing `vyre_foundation` DCE pass before
//! any backend is wired.

pub mod canonicalize_via_encoded;
pub mod const_fold_via_encoded;
pub mod const_prop;
pub mod contracts;
pub mod cross_scope_cse;
pub mod cse_via_encoded;
pub mod dce_program;
pub mod dce_via_encoded;
pub mod dead_branch;
pub mod dispatcher;
pub mod encode;
pub mod expr_arena;
pub mod licm;
pub mod pattern_match_via_encoded;
pub mod pipeline;
pub mod pipeline_resident;
pub mod pipeline_resident_decode;
mod rewrite_walk;
pub mod validate_via_encoded;

pub use contracts::{
    cross_crate_perf_contracts, optimization_composition_contracts, optimization_pass_selection,
    optimization_registry, optimization_release_passes,
};
