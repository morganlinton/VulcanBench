//! Fixed-arity u32 input/output buffer layouts used by compiler-internal ops.
//!
//! Each `const` describes the exact sequence of `DataType::U32` slots an op
//! consumes or produces. Pack layouts together so the op definitions read
//! cleanly (`U32X4_INPUTS`, `U32_OUTPUTS`) without a file-per-const.

use crate::ir::DataType;

/// Single-word `u32` output.
#[allow(dead_code, reason = "compiler op schema catalog entry")]
pub(crate) const U32_OUTPUTS: &[DataType] = &[DataType::U32];

/// Two-word `u32` output.
#[allow(dead_code, reason = "compiler op schema catalog entry")]
pub(crate) const U32X2_OUTPUTS: &[DataType] = &[DataType::U32, DataType::U32];

/// Four-word `u32` input tuple.
#[allow(dead_code, reason = "compiler op schema catalog entry")]
pub(crate) const U32X4_INPUTS: &[DataType] =
    &[DataType::U32, DataType::U32, DataType::U32, DataType::U32];

/// Five-word `u32` input tuple.
#[allow(dead_code, reason = "compiler op schema catalog entry")]
pub(crate) const U32X5_INPUTS: &[DataType] = &[
    DataType::U32,
    DataType::U32,
    DataType::U32,
    DataType::U32,
    DataType::U32,
];
