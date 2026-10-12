use crate::rule::condition_op;
use vyre_foundation::ir::{Expr, Program};
use vyre_spec::OperationContract;

impl PatternExists {
    /// Build the canonical IR program.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_libs::rule::pattern_exists::PatternExists;
    ///
    /// assert!(!PatternExists::program().entry().is_empty());
    /// ```
    #[must_use]
    pub fn program() -> Program {
        condition_op::condition_program(OP_ID, || {
            Expr::ne(condition_op::pattern_state(), Expr::u32(0))
        })
    }
}

/// Stable operation id for pattern existence checks.
pub const OP_ID: &str = "vyre-libs::rule::pattern_exists";

/// Execution contract annotation for the standard catalog.
pub const CONTRACT: OperationContract = crate::contracts::RULE_PREDICATE_CHEAP;

/// Pattern existence condition operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct PatternExists;
