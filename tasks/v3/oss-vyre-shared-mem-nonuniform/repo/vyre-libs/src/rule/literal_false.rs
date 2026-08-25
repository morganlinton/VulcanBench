use crate::rule::condition_op;
use vyre_foundation::ir::{Expr, Program};

impl LiteralFalse {
    /// Build the canonical IR program.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_libs::rule::literal_false::LiteralFalse;
    ///
    /// assert!(!LiteralFalse::program().entry().is_empty());
    /// ```
    #[must_use]
    pub fn program() -> Program {
        condition_op::condition_program(OP_ID, || Expr::u32(0))
    }
}

/// Literal false condition operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct LiteralFalse;

/// Stable operation id for constant false leaves.
pub const OP_ID: &str = "vyre-libs::rule::literal_false";
