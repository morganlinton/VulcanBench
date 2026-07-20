//! Rule-engine dialect: typed conditions, formulas, and program builder.

/// Typed condition / formula AST consumed by every rule-set builder.
pub mod ast;
/// Rule-set IR program builder  -  walks a `[(RuleFormula, rule_id)]` table
/// and emits one `Node::Store` per rule into the shared `verdicts` buffer.
pub mod builder;
/// Shared Cat-A helpers for scalar rule-condition ops (Tier-3 plumbing).
pub mod condition_op;

macro_rules! define_file_size_condition {
    ($module:ident, $type_name:ident, $op_id:literal, $predicate:ident) => {
        /// File-size rule predicate operation.
        pub mod $module {
            use vyre_foundation::ir::{Expr, Program};

            /// File-size condition operation marker.
            #[derive(Debug, Clone, Copy, Default)]
            pub struct $type_name;

            impl $type_name {
                /// Build the canonical IR program.
                #[must_use]
                pub fn program() -> Program {
                    crate::rule::condition_op::condition_program(OP_ID, || {
                        Expr::$predicate(
                            crate::rule::condition_op::file_size(),
                            crate::rule::condition_op::threshold(),
                        )
                    })
                }
            }

            /// Stable operation id for this file-size predicate.
            pub const OP_ID: &str = $op_id;

            /// Execution contract annotation for the standard catalog.
            pub const CONTRACT: vyre_spec::OperationContract =
                crate::contracts::RULE_PREDICATE_CHEAP;
        }
    };
}

define_file_size_condition!(
    file_size_eq,
    FileSizeEq,
    "vyre-libs::rule::file_size_eq",
    eq
);
define_file_size_condition!(
    file_size_gt,
    FileSizeGt,
    "vyre-libs::rule::file_size_gt",
    gt
);
define_file_size_condition!(
    file_size_gte,
    FileSizeGte,
    "vyre-libs::rule::file_size_gte",
    ge
);
define_file_size_condition!(
    file_size_lt,
    FileSizeLt,
    "vyre-libs::rule::file_size_lt",
    lt
);
define_file_size_condition!(
    file_size_lte,
    FileSizeLte,
    "vyre-libs::rule::file_size_lte",
    le
);
define_file_size_condition!(
    file_size_ne,
    FileSizeNe,
    "vyre-libs::rule::file_size_ne",
    ne
);

macro_rules! define_pattern_count_condition {
    ($module:ident, $type_name:ident, $op_id:literal, $predicate:ident) => {
        /// Pattern-count rule predicate operation.
        pub mod $module {
            use vyre_foundation::ir::{Expr, Program};

            /// Pattern-count condition operation marker.
            #[derive(Debug, Clone, Copy, Default)]
            pub struct $type_name;

            impl $type_name {
                /// Build the canonical IR program.
                #[must_use]
                pub fn program() -> Program {
                    crate::rule::condition_op::condition_program(OP_ID, || {
                        Expr::$predicate(
                            crate::rule::condition_op::pattern_count(),
                            crate::rule::condition_op::threshold(),
                        )
                    })
                }
            }

            /// Stable operation id for this pattern-count predicate.
            pub const OP_ID: &str = $op_id;

            /// Execution contract annotation for the standard catalog.
            pub const CONTRACT: vyre_spec::OperationContract =
                crate::contracts::RULE_PREDICATE_CHEAP;
        }
    };
}

define_pattern_count_condition!(
    pattern_count_gt,
    PatternCountGt,
    "vyre-libs::rule::pattern_count_gt",
    gt
);
define_pattern_count_condition!(
    pattern_count_gte,
    PatternCountGte,
    "vyre-libs::rule::pattern_count_gte",
    ge
);

/// Cat-A op: constant-false rule leaf.
pub mod literal_false;
/// Cat-A op: constant-true rule leaf.
pub mod literal_true;
/// Cat-A op: pattern-existence rule predicate.
pub mod pattern_exists;
/// Reference evaluator for `RuleCondition` / `RuleFormula` trees.
/// Mirror of the GPU lowering for parity checks, CI gates, and unit
/// tests that need deterministic rule outcomes without backend dispatch.
pub mod reference_eval;

pub use ast::{RuleCondition, RuleFormula};
pub use builder::build_rule_program;
pub use reference_eval::{evaluate_condition, evaluate_formula, RuleEvaluationContext};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_size_condition_family_builds_unique_programs() {
        let programs = [
            (file_size_eq::OP_ID, file_size_eq::FileSizeEq::program()),
            (file_size_gt::OP_ID, file_size_gt::FileSizeGt::program()),
            (file_size_gte::OP_ID, file_size_gte::FileSizeGte::program()),
            (file_size_lt::OP_ID, file_size_lt::FileSizeLt::program()),
            (file_size_lte::OP_ID, file_size_lte::FileSizeLte::program()),
            (file_size_ne::OP_ID, file_size_ne::FileSizeNe::program()),
        ];
        for (op_id, program) in programs {
            assert!(
                !program.entry().is_empty(),
                "Fix: generated file-size condition `{op_id}` must emit a non-empty rule program"
            );
            assert!(
                program
                    .entry()
                    .iter()
                    .any(|node| format!("{node:?}").contains(op_id)),
                "Fix: generated file-size condition `{op_id}` must preserve its op id in the IR"
            );
        }
    }

    #[test]
    fn file_size_condition_family_uses_rule_predicate_contract() {
        assert_eq!(
            file_size_eq::CONTRACT.cost_hint,
            crate::contracts::RULE_PREDICATE_CHEAP.cost_hint
        );
        assert_eq!(
            file_size_ne::CONTRACT.determinism,
            crate::contracts::RULE_PREDICATE_CHEAP.determinism
        );
    }

    #[test]
    fn pattern_count_condition_family_builds_unique_programs() {
        let programs = [
            (
                pattern_count_gt::OP_ID,
                pattern_count_gt::PatternCountGt::program(),
            ),
            (
                pattern_count_gte::OP_ID,
                pattern_count_gte::PatternCountGte::program(),
            ),
        ];
        for (op_id, program) in programs {
            assert!(
                !program.entry().is_empty(),
                "Fix: generated pattern-count condition `{op_id}` must emit a non-empty rule program"
            );
            assert!(
                program
                    .entry()
                    .iter()
                    .any(|node| format!("{node:?}").contains(op_id)),
                "Fix: generated pattern-count condition `{op_id}` must preserve its op id in the IR"
            );
        }
    }

    #[test]
    fn pattern_count_condition_family_uses_rule_predicate_contract() {
        assert_eq!(
            pattern_count_gt::CONTRACT.cost_hint,
            crate::contracts::RULE_PREDICATE_CHEAP.cost_hint
        );
        assert_eq!(
            pattern_count_gte::CONTRACT.determinism,
            crate::contracts::RULE_PREDICATE_CHEAP.determinism
        );
    }
}
