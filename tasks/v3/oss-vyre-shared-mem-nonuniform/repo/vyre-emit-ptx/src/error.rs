use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmitError {
    #[error("unsupported KernelOp kind in PTX emit: {0:?}")]
    UnsupportedOp(vyre_lower::KernelOp),

    #[error("PTX module construction failed: {0}")]
    PtxConstructionFailed(String),

    #[error("binding slot {slot}: {reason}")]
    InvalidBinding { slot: u32, reason: String },

    #[error("invalid descriptor: {0}")]
    InvalidDescriptor(String),

    #[error("unsupported data type for PTX scalar emit: {0}")]
    UnsupportedDataType(String),
}
