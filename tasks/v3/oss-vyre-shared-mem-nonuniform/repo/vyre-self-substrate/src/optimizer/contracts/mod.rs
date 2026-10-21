//! Optimizer contract metadata and pass-selection policy.
//!
//! These modules describe the optimizer pass universe, release-path pass
//! ordering, composition laws, and benchmark-driven pass selection. Keeping
//! them under `optimizer` prevents a second root-level optimization domain from
//! drifting away from the executable optimizer pipeline.

pub mod cross_crate_perf_contracts;
pub mod optimization_composition_contracts;
pub mod optimization_pass_selection;
pub mod optimization_registry;
pub mod optimization_release_passes;
