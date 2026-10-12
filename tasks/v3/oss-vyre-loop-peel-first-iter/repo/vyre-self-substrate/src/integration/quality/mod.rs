//! Quality gates, public API boundaries, and release-readiness contracts.

pub mod allocation_regression;
pub mod architecture_boundary_map;
pub mod contributor_module_map;
#[cfg(any(test, feature = "cpu-parity"))]
pub mod cpu_fallback_reachability;
pub mod crate_metadata_readiness;
pub mod deep_review_gate;
pub mod paradigm_shift_plan_audit;
pub mod public_api_boundary;
pub mod public_api_doctest_gate;
