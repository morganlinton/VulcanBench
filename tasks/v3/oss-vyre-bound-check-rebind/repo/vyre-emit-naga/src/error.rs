use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmitError {
    #[error("unsupported KernelOp kind in naga emit: {0:?}")]
    UnsupportedOp(vyre_lower::KernelOp),

    #[error("naga module construction failed: {0}")]
    NagaConstructionFailed(String),

    #[error("binding slot {slot}: {reason}")]
    InvalidBinding { slot: u32, reason: String },

    #[error("invalid descriptor: {0}")]
    InvalidDescriptor(String),
}
