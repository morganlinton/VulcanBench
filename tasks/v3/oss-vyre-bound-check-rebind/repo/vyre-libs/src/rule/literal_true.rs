use crate::rule::condition_op;
use vyre_foundation::ir::{Expr, Program};
use vyre_spec::OperationContract;

impl LiteralTrue {
    /// Build the canonical IR program.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_libs::rule::literal_true::LiteralTrue;
    ///
    /// assert!(!LiteralTrue::program().entry().is_empty());
    /// ```
    #[must_use]
    pub fn program() -> Program {
        condition_op::condition_program(OP_ID, || Expr::u32(1))
    }
}

/// Literal true condition operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct LiteralTrue;

/// Stable operation id for constant true leaves.
pub const OP_ID: &str = "vyre-libs::rule::literal_true";

/// Execution contract annotation for the standard catalog.
pub const CONTRACT: OperationContract = crate::contracts::RULE_PREDICATE_CHEAP;
