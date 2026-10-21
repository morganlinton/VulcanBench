//! Generated live CUDA-resident/reference differential matrix for release-path semantics.

mod common;

#[path = "generated_resident_cuda_reference_matrix/case_defs.rs"]
mod case_defs;
#[path = "generated_resident_cuda_reference_matrix/case_tables.rs"]
mod case_tables;
#[path = "generated_resident_cuda_reference_matrix/program_builders.rs"]
mod program_builders;
#[path = "generated_resident_cuda_reference_matrix/resident_reference.rs"]
mod resident_reference;
#[path = "generated_resident_cuda_reference_matrix/generated_f32.rs"]
mod generated_f32;

#[path = "generated_resident_cuda_reference_matrix/bool_contracts.rs"]
mod bool_contracts;
#[path = "generated_resident_cuda_reference_matrix/f32_contracts.rs"]
mod f32_contracts;
#[path = "generated_resident_cuda_reference_matrix/atomic_cast_contracts.rs"]
mod atomic_cast_contracts;
#[path = "generated_resident_cuda_reference_matrix/integer_contracts.rs"]
mod integer_contracts;
#[path = "generated_resident_cuda_reference_matrix/memory_contracts.rs"]
mod memory_contracts;
use common::{
    assert_f32_output_lanes, assert_u32_output_lanes, bool_bytes, bool_word, eq_word, f32_bytes,
    ge_word, generated_bool_cast_values, generated_f32_cast_values, generated_f32_fma_values,
    generated_i32_cast_values, generated_mixed_bool_values as generated_bool_values,
    generated_mixed_u32_values as generated_atomic_values, generated_u32_cast_values, gt_word,
    i32_bytes, le_word, live_backend, lt_word, ne_word, reference_outputs,
    resident_cuda_reference_outputs, u32_bytes, GENERATED_LANE_COUNT as LANE_COUNT,
    GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OUTPUT_BYTES: usize = LANE_COUNT * std::mem::size_of::<u32>();
const BUCKET_COUNT: usize = 8;
const BUCKET_MASK: u32 = BUCKET_COUNT as u32 - 1;
const MAX_F32_ULP: u32 = 1;

use case_defs::*;
use case_tables::*;
use generated_f32::*;
use program_builders::*;
use resident_reference::*;
