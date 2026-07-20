//! Self-substrate dispatch wrappers for quantized packing primitives.
//!
//! Quantized low-bit layouts are now executable primitives, not just spec
//! variants. This wrapper lets optimizer/self-hosted paths unpack packed INT4
//! tensors through the same backend seam as every other primitive.

mod shapes;

use crate::dispatch_buffers::{
    ceil_div_u32, decode_f32_output_exact, decode_i32_output_exact, decode_u32_output_exact,
    ensure_input_slots, write_f32_slice_le_bytes, write_u32_slice_le_bytes, write_zero_bytes,
};
use crate::hardware::dispatch_program_cache::ProgramCache;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use shapes::{expect_one_output, expect_two_outputs, validate_batched_packed_matmul_shape};
use vyre_foundation::ir::Program;
use vyre_primitives::math::quantized::{
    i4_packed_words, i4x8_batched_matmul_f32_scaled, i4x8_batched_matmul_top1_f32_scaled,
    i4x8_batched_matvec_f32_scaled, i4x8_dot_f32_scaled, i4x8_matvec_f32_scaled, unpack_i4x8,
};

#[cfg(test)]
use vyre_primitives::math::quantized::{
    i4x8_batched_matmul_f32_scaled_cpu, i4x8_batched_matmul_top1_f32_scaled_cpu,
    i4x8_batched_matvec_f32_scaled_cpu, i4x8_dot_f32_scaled_cpu, i4x8_matvec_f32_scaled_cpu,
    pack_i4x8_cpu, unpack_i4x8_cpu_into,
};

/// Caller-owned dispatch scratch for quantized INT4 unpacking.
#[derive(Debug, Default)]
pub struct QuantizedUnpackGpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<u32, Program>,
}

/// Caller-owned dispatch scratch for packed INT4 scaled dot products.
#[derive(Debug, Default)]
pub struct QuantizedDotGpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<u32, Program>,
}

/// Caller-owned dispatch scratch for packed INT4 row-scaled matvecs.
#[derive(Debug, Default)]
pub struct QuantizedMatvecGpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<(u32, u32), Program>,
}

/// Caller-owned dispatch scratch for packed INT4 batched row-scaled matvecs.
#[derive(Debug, Default)]
pub struct QuantizedBatchedMatvecGpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<(u32, u32, u32), Program>,
}

/// Caller-owned dispatch scratch for packed INT4 batched packed-activation matmuls.
#[derive(Debug, Default)]
pub struct QuantizedBatchedMatmulGpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<(u32, u32, u32), Program>,
}

/// Caller-owned dispatch scratch for packed INT4 batched matmul top-1 routing.
#[derive(Debug, Default)]
pub struct QuantizedBatchedMatmulTop1GpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<(u32, u32, u32), Program>,
}

mod batched_matmul;
mod batched_matvec;
mod dot;
mod matvec;
mod top1;
mod unpack;

pub use batched_matmul::{
    i4x8_batched_matmul_f32_scaled_via, i4x8_batched_matmul_f32_scaled_via_with_scratch_into,
};
pub use batched_matvec::{
    i4x8_batched_matvec_f32_scaled_via, i4x8_batched_matvec_f32_scaled_via_with_scratch_into,
};
pub use dot::{i4x8_dot_f32_scaled_via, i4x8_dot_f32_scaled_via_with_scratch_into};
pub use matvec::{i4x8_matvec_f32_scaled_via, i4x8_matvec_f32_scaled_via_with_scratch_into};
pub use top1::{
    i4x8_batched_matmul_top1_f32_scaled_via,
    i4x8_batched_matmul_top1_f32_scaled_via_with_scratch_into,
};
pub use unpack::{unpack_i4x8_via, unpack_i4x8_via_with_scratch_into};

#[cfg(test)]
mod tests;
