//! Shared f32 parity policy for reusable conform lenses.
//!
//! Integer and boolean buffers compare byte-identical. F32 buffers compare
//! under the program's ULP policy because GPU backends are allowed to use
//! native transcendental approximations while the CPU reference uses a
//! deterministic libm oracle.

pub use vyre_harness::fp_contract::{
    compare_output_buffers, f32_buffer_matches, f32_ulp_tolerance, ulp_distance,
    BACKEND_ELEMENTARY_F32_ULP_BUDGET, BACKEND_TRANSCENDENTAL_ULP_BUDGET,
    REFERENCE_TRANSCENDENTAL_ULP_BUDGET,
};

/// Re-export of the buffer parity enum for lens consumers.
pub use vyre_harness::fp_contract::BufferParity;
