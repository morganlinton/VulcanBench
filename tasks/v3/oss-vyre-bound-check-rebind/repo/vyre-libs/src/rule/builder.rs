// Rule set program builder.

use crate::rule::ast::{RuleCondition, RuleFormula};
use std::sync::LazyLock;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
/// `WORKGROUP_SIZE` constant.
pub const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

/// Stable op id for the wrapping rule-set Region. Every `build_rule_program`
/// call emits one region under this generator so the optimizer + the
/// universal region-chain discipline test treat the whole rule set as an
/// atomic compile unit.
pub const RULE_SET_OP_ID: &str = "vyre-libs::rule::rule_set";

/// Error returned when rule construction cannot lower a condition truthfully.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleBuildError {
    /// The core rule builder has no IR lowering for this frozen condition.
    #[error(
        "RuleCondition::{condition} is not lowerable by the core rule builder for rule {rule_id}. Fix: {fix}"
    )]
    UnsupportedCondition {
        /// Rule id whose formula contains the condition.
        rule_id: u32,
        /// Condition variant name.
        condition: &'static str,
        /// Actionable remediation for callers.
        fix: &'static str,
    },
    /// Extension conditions require an extension-aware builder.
    #[error(
        "RuleCondition::Opaque extension {extension_id:#010x} is not lowerable by the core rule builder for rule {rule_id}. Fix: use an extension-aware rule builder that maps this extension to concrete IR, or pre-evaluate the opaque condition before building a core rule program."
    )]
    OpaqueCondition {
        /// Rule id whose formula contains the extension condition.
        rule_id: u32,
        /// Raw extension id.
        extension_id: u32,
    },
}

/// Build one IR program for an entire rule set.
///
/// Each tuple is `(formula, rule_id)`. The generated program writes each rule's
/// boolean verdict as `0` or `1` into `verdicts[rule_id]`.
///
/// # Errors
///
/// Returns [`RuleBuildError`] when a formula contains a condition the core
/// builder cannot lower truthfully.
///
/// # Examples
///
/// ```
/// use vyre_libs::rule::{build_rule_program, RuleCondition, RuleFormula};
///
/// let formula = RuleFormula::condition(RuleCondition::LiteralTrue);
/// let program = build_rule_program(&[(formula, 3)]).expect("Fix: literal rule lowers");
/// assert!(program.has_buffer("verdicts"));
/// ```
#[must_use]
pub fn build_rule_program(rules: &[(RuleFormula, u32)]) -> Result<Program, RuleBuildError> {
    let nodes = rule_nodes(rules)?;
    Ok(Program::wrapped(
        rule_buffers(),
        WORKGROUP_SIZE,
        vec![crate::region::wrap_anonymous(RULE_SET_OP_ID, nodes)],
    ))
}

/// Try to build one IR program for an entire rule set.
///
/// Returns [`RuleBuildError`] instead of emitting constant-success calls for
/// conditions that need an external text source or extension-owned buffers.
///
/// # Errors
///
/// Returns [`RuleBuildError::UnsupportedCondition`] for frozen condition
/// variants without a core IR lowering and [`RuleBuildError::OpaqueCondition`]
/// for extension conditions that require an extension-aware builder.
pub fn try_build_rule_program(rules: &[(RuleFormula, u32)]) -> Result<Program, RuleBuildError> {
    build_rule_program(rules)
}

/// Canonical buffer declarations every rule-set program starts from:
/// six read-only inputs (`rule_ids`, `pattern_ids`, `rule_bitmaps`,
/// `rule_counts`, `file_size`) plus one output (`verdicts`).
///
/// The core builder does not append extension buffers because
/// `RuleCondition::Opaque` is not lowerable without an extension-aware
/// builder.
#[must_use]
pub fn rule_buffers() -> Vec<BufferDecl> {
    static TEMPLATE: LazyLock<Vec<BufferDecl>> = LazyLock::new(|| {
        vec![
            BufferDecl::read("rule_ids", 0, DataType::U32),
            BufferDecl::read("pattern_ids", 1, DataType::U32),
            BufferDecl::read("rule_bitmaps", 2, DataType::U32),
            BufferDecl::read("rule_counts", 3, DataType::U32),
            BufferDecl::read("file_size", 4, DataType::U32),
            BufferDecl::output("verdicts", 5, DataType::U32),
        ]
    });
    TEMPLATE.clone()
}

/// Emit one `Node::Store` per rule into the shared `verdicts` buffer.
/// The store is guarded by `rule_id < buf_len("verdicts")` so callers
/// can pack extra slots without corrupting memory.
///
/// # Errors
///
/// Returns [`RuleBuildError`] when a formula contains a condition the core
/// builder cannot lower truthfully.
#[must_use]
pub fn rule_nodes(rules: &[(RuleFormula, u32)]) -> Result<Vec<Node>, RuleBuildError> {
    rules
        .iter()
        .map(|(formula, rule_id)| {
            Ok(Node::if_then(
                Expr::lt(Expr::u32(*rule_id), Expr::buf_len("verdicts")),
                vec![Node::store(
                    "verdicts",
                    Expr::u32(*rule_id),
                    formula_expr(formula, *rule_id)?,
                )],
            ))
        })
        .collect()
}

/// Try to emit one `Node::Store` per rule into the shared `verdicts` buffer.
///
/// # Errors
///
/// Returns [`RuleBuildError`] when a formula contains a condition the core
/// builder cannot lower truthfully.
pub fn try_rule_nodes(rules: &[(RuleFormula, u32)]) -> Result<Vec<Node>, RuleBuildError> {
    rule_nodes(rules)
}

/// Lower a [`RuleFormula`] to a boolean `Expr` tree  -
/// `Condition` → single predicate, `And`/`Or`/`Not` → bool combinators.
///
/// # Errors
///
/// Returns [`RuleBuildError`] when the formula contains a condition the core
/// builder cannot lower truthfully.
#[must_use]
pub fn formula_expr(formula: &RuleFormula, rule_id: u32) -> Result<Expr, RuleBuildError> {
    match formula {
        RuleFormula::Condition(condition) => condition_expr(condition, rule_id),
        RuleFormula::And(left, right) => Ok(Expr::and(
            formula_expr(left, rule_id)?,
            formula_expr(right, rule_id)?,
        )),
        RuleFormula::Or(left, right) => Ok(Expr::or(
            formula_expr(left, rule_id)?,
            formula_expr(right, rule_id)?,
        )),
        RuleFormula::Not(formula) => Ok(Expr::not(formula_expr(formula, rule_id)?)),
    }
}

/// Try to lower a [`RuleFormula`] to a boolean `Expr` tree.
///
/// # Errors
///
/// Returns [`RuleBuildError`] when any contained condition lacks a truthful
/// core IR lowering.
pub fn try_formula_expr(formula: &RuleFormula, rule_id: u32) -> Result<Expr, RuleBuildError> {
    formula_expr(formula, rule_id)
}

/// Lower a [`RuleCondition`] to the scalar boolean `Expr` the rule-set
/// program stores into `verdicts[rule_id]`.
///
/// # Errors
///
/// Returns [`RuleBuildError`] when the condition has no truthful core IR
/// lowering.
#[must_use]
pub fn condition_expr(condition: &RuleCondition, rule_id: u32) -> Result<Expr, RuleBuildError> {
    try_condition_expr(condition, rule_id)
}

/// Try to lower a [`RuleCondition`] to the scalar boolean `Expr` the rule-set
/// program stores into `verdicts[rule_id]`.
///
/// # Errors
///
/// Returns [`RuleBuildError`] for condition variants that need a runtime text
/// source or an extension-aware lowering that the core rule builder does not
/// own.
pub fn try_condition_expr(condition: &RuleCondition, rule_id: u32) -> Result<Expr, RuleBuildError> {
    match condition {
        RuleCondition::PatternExists { pattern_id } => {
            Ok(Expr::ne(pattern_state(*pattern_id), Expr::u32(0)))
        }
        RuleCondition::PatternCountGt {
            pattern_id,
            threshold,
        } => Ok(Expr::gt(pattern_count(*pattern_id), Expr::u32(*threshold))),
        RuleCondition::PatternCountGte {
            pattern_id,
            threshold,
        } => Ok(Expr::ge(pattern_count(*pattern_id), Expr::u32(*threshold))),
        RuleCondition::FileSizeLt(threshold) => Ok(file_size_cmp(Expr::lt, *threshold, true)),
        RuleCondition::FileSizeLte(threshold) => Ok(file_size_cmp(Expr::le, *threshold, true)),
        RuleCondition::FileSizeGt(threshold) => Ok(file_size_cmp(Expr::gt, *threshold, false)),
        RuleCondition::FileSizeGte(threshold) => Ok(file_size_cmp(Expr::ge, *threshold, false)),
        RuleCondition::FileSizeEq(threshold) => Ok(file_size_cmp(Expr::eq, *threshold, false)),
        RuleCondition::FileSizeNe(threshold) => Ok(file_size_cmp(Expr::ne, *threshold, true)),
        RuleCondition::LiteralTrue => Ok(Expr::u32(1)),
        RuleCondition::LiteralFalse => Ok(Expr::u32(0)),
        RuleCondition::RegexMatch { .. } => Err(unsupported_rule_condition(
            rule_id,
            "RegexMatch",
            "lower the regex against a concrete buffer in an extension-aware builder, or pre-evaluate the regex condition before calling the core builder.",
        )),
        RuleCondition::SubstringMatch { .. } => Err(unsupported_rule_condition(
            rule_id,
            "SubstringMatch",
            "lower the substring predicate against a concrete buffer in an extension-aware builder, or pre-evaluate the text condition before calling the core builder.",
        )),
        RuleCondition::PrefixMatch { .. } => Err(unsupported_rule_condition(
            rule_id,
            "PrefixMatch",
            "lower the prefix predicate against a concrete buffer in an extension-aware builder, or pre-evaluate the text condition before calling the core builder.",
        )),
        RuleCondition::SuffixMatch { .. } => Err(unsupported_rule_condition(
            rule_id,
            "SuffixMatch",
            "lower the suffix predicate against a concrete buffer in an extension-aware builder, or pre-evaluate the text condition before calling the core builder.",
        )),
        RuleCondition::RangeMatch { value, min, max } => {
            Ok(bool_expr(min <= value && value <= max))
        }
        RuleCondition::SetMembership { value, set } => Ok(bool_expr(
            set.iter()
                .any(|candidate| candidate.as_ref() == value.as_ref()),
        )),
        RuleCondition::FieldInSet { .. } => Err(unsupported_rule_condition(
            rule_id,
            "FieldInSet",
            "FieldInSet requires per-record field lookup; it is supported only by the reference evaluator (`vyre_libs::rule::reference_eval`). Lower against a concrete buffer in an extension-aware builder before calling the core lowering.",
        )),
        RuleCondition::Opaque(ext) => Err(RuleBuildError::OpaqueCondition {
            rule_id,
            extension_id: ext.extension_id().as_u32(),
        }),
    }
}

fn unsupported_rule_condition(
    rule_id: u32,
    condition: &'static str,
    fix: &'static str,
) -> RuleBuildError {
    RuleBuildError::UnsupportedCondition {
        rule_id,
        condition,
        fix,
    }
}

fn bool_expr(value: bool) -> Expr {
    Expr::u32(u32::from(value))
}

/// Emit a `file_size` comparison that guards against `threshold` values
/// wider than u32. On overflow the result collapses to the constant
/// `overflow_is_true` so semantics stay well-defined.
#[must_use]
pub fn file_size_cmp<F>(cmp_fn: F, threshold: u64, overflow_is_true: bool) -> Expr
where
    F: FnOnce(Expr, Expr) -> Expr,
{
    match u32::try_from(threshold) {
        Ok(t) => cmp_fn(Expr::load("file_size", Expr::u32(0)), Expr::u32(t)),
        Err(_) => {
            if overflow_is_true {
                Expr::u32(1)
            } else {
                Expr::u32(0)
            }
        }
    }
}

/// Safe load from the `rule_bitmaps` buffer  -  returns 0 when
/// `pattern_id` is out of range so the rule predicate stays defined.
#[must_use]
pub fn pattern_state(pattern_id: u32) -> Expr {
    Expr::select(
        Expr::lt(Expr::u32(pattern_id), Expr::buf_len("rule_bitmaps")),
        Expr::load("rule_bitmaps", Expr::u32(pattern_id)),
        Expr::u32(0),
    )
}

/// Safe load from the `rule_counts` buffer  -  returns 0 when
/// `pattern_id` is out of range so the rule predicate stays defined.
#[must_use]
pub fn pattern_count(pattern_id: u32) -> Expr {
    Expr::select(
        Expr::lt(Expr::u32(pattern_id), Expr::buf_len("rule_counts")),
        Expr::load("rule_counts", Expr::u32(pattern_id)),
        Expr::u32(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;
    use std::any::Any;
    use std::sync::Arc;
    use vyre_foundation::extension::RuleConditionExt;
    use vyre_spec::extension::ExtensionRuleConditionId;

    #[derive(Debug)]
    struct TestOpaqueCondition;

    impl RuleConditionExt for TestOpaqueCondition {
        fn extension_id(&self) -> ExtensionRuleConditionId {
            ExtensionRuleConditionId::from_name("vyre.test.rule.opaque")
        }

        fn evaluate_opaque(&self, _ctx: &dyn Any) -> bool {
            true
        }

        fn stable_fingerprint(&self) -> [u8; 32] {
            [7; 32]
        }
    }

    #[test]
    fn try_build_rule_program_preserves_supported_conditions() {
        let formula = RuleFormula::and(
            RuleFormula::condition(RuleCondition::PatternExists { pattern_id: 3 }),
            RuleFormula::not(RuleFormula::condition(RuleCondition::FileSizeLt(4096))),
        );

        let program = try_build_rule_program(&[(formula, 5)]).expect("Fix: supported rule lowers");

        assert!(program.has_buffer("rule_bitmaps"));
        assert!(program.has_buffer("rule_counts"));
        assert!(program.has_buffer("file_size"));
        assert!(program.has_buffer("verdicts"));
    }

    #[test]
    fn unsupported_conditions_return_actionable_errors() {
        let unsupported = vec![
            RuleCondition::RegexMatch {
                field: Arc::from("path"),
                pattern: Arc::from(".*\\.rs"),
            },
            RuleCondition::SubstringMatch {
                haystack: Arc::from("path"),
                needle: Arc::from("src/"),
            },
            RuleCondition::PrefixMatch {
                value: Arc::from("path"),
                prefix: Arc::from("src/"),
            },
            RuleCondition::SuffixMatch {
                value: Arc::from("path"),
                suffix: Arc::from(".rs"),
            },
        ];

        for condition in unsupported {
            let error = try_condition_expr(&condition, 42).expect_err("condition must reject");
            let message = error.to_string();

            assert!(
                matches!(
                    error,
                    RuleBuildError::UnsupportedCondition { rule_id: 42, .. }
                ),
                "wrong error: {message}"
            );
            assert!(message.contains("Fix:"), "missing fix: {message}");
            assert!(
                !message.contains("rule.unsupported"),
                "error must not expose constant-success calls: {message}"
            );
        }
    }

    #[test]
    fn static_range_and_set_conditions_lower_to_constants() {
        assert_eq!(
            try_condition_expr(
                &RuleCondition::RangeMatch {
                    value: 12,
                    min: 10,
                    max: 20,
                },
                7,
            )
            .expect("Fix: range condition lowers"),
            Expr::u32(1)
        );
        assert_eq!(
            try_condition_expr(
                &RuleCondition::SetMembership {
                    value: Arc::from("critical"),
                    set: smallvec![Arc::from("critical"), Arc::from("high")],
                },
                7,
            )
            .expect("Fix: set membership condition lowers"),
            Expr::u32(1)
        );
    }

    #[test]
    fn opaque_condition_returns_construction_error() {
        let condition = RuleCondition::Opaque(Arc::new(TestOpaqueCondition));
        let error = try_condition_expr(&condition, 9).expect_err("opaque must reject");
        let message = error.to_string();

        assert!(
            matches!(
                error,
                RuleBuildError::OpaqueCondition {
                    rule_id: 9,
                    extension_id
                } if extension_id == ExtensionRuleConditionId::from_name("vyre.test.rule.opaque").as_u32()
            ),
            "wrong error: {message}"
        );
        assert!(message.contains("extension-aware rule builder"));
    }

    #[test]
    fn condition_expr_returns_error_instead_of_panicking_or_constant_success() {
        let condition = RuleCondition::RegexMatch {
            field: Arc::from("path"),
            pattern: Arc::from(".*"),
        };

        let error = condition_expr(&condition, 1).expect_err("regex condition must reject");

        assert!(
            matches!(
                error,
                RuleBuildError::UnsupportedCondition { rule_id: 1, .. }
            ),
            "wrong error: {error}"
        );
    }
}
