//! Errors surfaced by the lowering pass.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LowerError {
    #[error("unsupported IR construct in lowering: {0}")]
    UnsupportedConstruct(String),

    #[error("invalid program: {0}")]
    InvalidProgram(String),

    #[error("operand id space exhausted (over u32::MAX values in one kernel)")]
    OperandIdOverflow,

    #[error("nested body depth exceeded reasonable limit ({0})")]
    NestingTooDeep(usize),

    #[error("buffer not declared but referenced: {0}")]
    UndeclaredBuffer(String),
}
