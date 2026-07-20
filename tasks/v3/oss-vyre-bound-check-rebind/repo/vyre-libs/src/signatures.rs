use vyre_foundation::ir::DataType;

/// Type signature constant: single `Bytes` input.
pub const BYTES_TO_BYTES_INPUTS: &[DataType] = &[DataType::Bytes];
/// Type signature constant: single `Bytes` output.
pub const BYTES_TO_BYTES_OUTPUTS: &[DataType] = &[DataType::Bytes];
/// Type signature constant: single `U32` output from `Bytes` input.
pub const BYTES_TO_U32_OUTPUTS: &[DataType] = &[DataType::U32];
/// Type signature constant: single `U32` input.
pub const U32_INPUTS: &[DataType] = &[DataType::U32];
/// Type signature constant: pair of `U32` inputs.
pub const U32_U32_INPUTS: &[DataType] = &[DataType::U32, DataType::U32];
/// Type signature constant: single `U32` output.
pub const U32_OUTPUTS: &[DataType] = &[DataType::U32];
/// Type signature constant: single `F32` input.
pub const F32_INPUTS: &[DataType] = &[DataType::F32];
/// Type signature constant: pair of `F32` inputs.
pub const F32_F32_INPUTS: &[DataType] = &[DataType::F32, DataType::F32];
/// Type signature constant: three `F32` inputs.
pub const F32_F32_F32_INPUTS: &[DataType] = &[DataType::F32, DataType::F32, DataType::F32];
/// Type signature constant: single `F32` output.
pub const F32_OUTPUTS: &[DataType] = &[DataType::F32];
/// Type signature constant: single `I32` output.
pub const I32_OUTPUTS: &[DataType] = &[DataType::I32];
/// Type signature constant: single `Bool` output.
pub const BOOL_OUTPUTS: &[DataType] = &[DataType::Bool];
