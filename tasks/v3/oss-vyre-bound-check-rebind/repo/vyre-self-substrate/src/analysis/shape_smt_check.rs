//! Shape-predicate / liquid-type SMT substrate consumer (P-PRIM-15).
//!
//! Wires `vyre_primitives::types::shape_smt::evaluate` into the
//! dispatch path so the optimizer can prove buffer-count facts
//! (non-empty, alignment, exact size) before lowering. The same
//! primitive downstream analyzer / future SMT frontends use to lift higher-level
//! type refinements into a formula the substrate can decide.

use vyre_primitives::types::{evaluate_shape, ShapeFormula};

/// Evaluate `formula` against `count`. Bumps the dataflow-fixpoint
/// substrate counter so observability dashboards see every per-buffer
/// shape-predicate decision.
#[must_use]
pub fn evaluate_shape_formula(formula: &ShapeFormula, count: u32) -> bool {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    evaluate_shape(formula, count)
}

/// True iff the formula proves a non-zero lower bound on `count`.
/// The optimizer uses this to skip empty-buffer fast paths without
/// re-evaluating the formula at runtime.
#[must_use]
pub fn formula_proves_non_empty(formula: &ShapeFormula) -> bool {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    formula.proves_non_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_least_evaluation() {
        let f = ShapeFormula::AtLeast(8);
        assert!(!evaluate_shape_formula(&f, 0));
        assert!(evaluate_shape_formula(&f, 8));
    }

    #[test]
    fn alignment_predicate() {
        let f = ShapeFormula::MultipleOf(4);
        assert!(evaluate_shape_formula(&f, 12));
        assert!(!evaluate_shape_formula(&f, 11));
    }

    #[test]
    fn conjunction_evaluation() {
        let f = ShapeFormula::And(
            Box::new(ShapeFormula::AtLeast(8)),
            Box::new(ShapeFormula::AtMost(256)),
        );
        assert!(!evaluate_shape_formula(&f, 7));
        assert!(evaluate_shape_formula(&f, 100));
        assert!(!evaluate_shape_formula(&f, 257));
    }

    /// Closure-bar: substrate output equals primitive output exactly.
    #[test]
    fn matches_primitive_directly() {
        let f = ShapeFormula::And(
            Box::new(ShapeFormula::AtLeast(16)),
            Box::new(ShapeFormula::MultipleOf(4)),
        );
        for c in [0u32, 8, 15, 16, 17, 20, 100] {
            assert_eq!(
                evaluate_shape_formula(&f, c),
                evaluate_shape(&f, c),
                "drift on c = {c}"
            );
        }
    }

    /// Adversarial: MultipleOf(0) must never pass  -  module-by-zero
    /// guarded by the primitive.
    #[test]
    fn multiple_of_zero_never_holds() {
        let f = ShapeFormula::MultipleOf(0);
        for c in [0u32, 1, 100, u32::MAX] {
            assert!(
                !evaluate_shape_formula(&f, c),
                "MultipleOf(0) must never hold (c = {c})"
            );
        }
    }

    #[test]
    fn proves_non_empty_at_least_one() {
        assert!(formula_proves_non_empty(&ShapeFormula::AtLeast(1)));
        assert!(formula_proves_non_empty(&ShapeFormula::Exactly(64)));
    }

    #[test]
    fn proves_non_empty_no_lower_bound() {
        assert!(!formula_proves_non_empty(&ShapeFormula::AtMost(256)));
        assert!(!formula_proves_non_empty(&ShapeFormula::MultipleOf(4)));
    }

    /// Adversarial: non-empty proof must propagate through
    /// conjunction  -  if EITHER conjunct proves non-empty, the AND
    /// proves it.
    #[test]
    fn conjunction_inherits_non_empty_from_either_side() {
        let left_proves = ShapeFormula::And(
            Box::new(ShapeFormula::AtLeast(1)),
            Box::new(ShapeFormula::AtMost(100)),
        );
        let right_proves = ShapeFormula::And(
            Box::new(ShapeFormula::AtMost(100)),
            Box::new(ShapeFormula::AtLeast(1)),
        );
        assert!(formula_proves_non_empty(&left_proves));
        assert!(formula_proves_non_empty(&right_proves));
    }
}
