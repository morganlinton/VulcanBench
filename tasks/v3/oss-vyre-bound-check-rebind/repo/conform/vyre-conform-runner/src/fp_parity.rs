//! Shared floating-point parity helpers.
//!
//! The parity-matrix test and `prove`'s cross-backend oracle both need
//! the same rule: integer/bool buffers compare byte-identical, but F32
//! buffers compare under a bounded-ULP window because target-text transcendentals
//! are not correctly-rounded. Keeping one implementation means `prove`
//! and `parity_matrix` cannot disagree about what "parity" means.
//!
//! Reference-oracle contract: vyre-reference uses the deterministic workspace
//! libm path for f32 transcendentals. The conform contract budgets that oracle
//! at `REFERENCE_TRANSCENDENTAL_ULP_BUDGET` ULP against a correctly-rounded
//! mathematical result, then budgets native backends at
//! `BACKEND_TRANSCENDENTAL_ULP_BUDGET` ULP against the reference. That makes
//! the full backend-vs-correct envelope explicit instead of hiding the
//! reference oracle's own approximation behind backend tolerance.

pub use vyre_test_harness::fp_parity::{
    compare_output_buffers, f32_buffer_matches, f32_ulp_tolerance, ulp_distance,
    BACKEND_ELEMENTARY_F32_ULP_BUDGET, BACKEND_TRANSCENDENTAL_ULP_BUDGET,
    REFERENCE_TRANSCENDENTAL_ULP_BUDGET,
};

/// Re-export of the buffer parity enum for lens consumers.
pub use vyre_test_harness::fp_parity::BufferParity;
