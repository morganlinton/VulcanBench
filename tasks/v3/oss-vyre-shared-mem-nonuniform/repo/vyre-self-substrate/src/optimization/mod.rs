//! Compatibility facade for optimizer contract modules.
//!
//! The implementation lives under `optimizer::contracts` so the optimizer has
//! one owning domain. This module preserves the historic
//! `vyre_self_substrate::optimization::*` import path for downstream crates.

pub use crate::optimizer::contracts::{
    cross_crate_perf_contracts, optimization_composition_contracts, optimization_pass_selection,
    optimization_registry, optimization_release_passes,
};
