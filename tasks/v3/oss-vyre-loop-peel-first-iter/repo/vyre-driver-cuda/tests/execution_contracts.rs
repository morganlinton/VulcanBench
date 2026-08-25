//! Live CUDA execution contracts for lane coverage and readback semantics.

mod common;
use common::{
    bytes_f32 as bytes_to_f32, bytes_u32, f32_bytes, i32_bytes, ordered_f32_bits, u16_bytes,
    u32_bytes,
};
use std::sync::Arc;

use vyre_driver::{pipeline, DispatchConfig, VyreBackend};
use vyre_driver_cuda::{cuda_factory, CudaBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

mod execution_contracts_part1 {

    include!("__split/execution_contracts_part1.rs");
}
mod execution_contracts_part2 {
    include!("__split/execution_contracts_part2.rs");
}
