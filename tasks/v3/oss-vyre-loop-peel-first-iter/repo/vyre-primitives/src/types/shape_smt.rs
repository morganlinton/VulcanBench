//! Liquid-type / shape-predicate SMT-style evaluator (P-PRIM-15).
//!
//! Evaluates refinement predicates over a concrete `u32` count. The
//! grammar mirrors `vyre_foundation::ir::ShapePredicate` but lives at
//! the primitive layer so external crates and solver frontends can stage
//! predicates without pulling in the IR.
//!
//! Supported predicates cover bounds, exact cardinality, divisibility,
//! modular equality, Boolean composition, and affine ranges over the
//! count. That gives optimizer passes a single primitive predicate
//! substrate instead of pass-local proof encodings.

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

/// Refinement predicate over a `u32` count.
///
/// Mirrors `vyre_foundation::ir::ShapePredicate` so the foundation
/// validator can lower its IR-level predicate into this primitive
/// representation for evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShapeFormula {
    /// `count >= n`.
    AtLeast(u32),
    /// `count <= n`.
    AtMost(u32),
    /// `count == n`.
    Exactly(u32),
    /// `count % n == 0`. False when `n == 0` (avoids modulo-by-zero).
    MultipleOf(u32),
    /// `count % modulus == remainder`. False when `modulus == 0`.
    ModEquals {
        /// Divisor used by the modular equality.
        modulus: u32,
        /// Required remainder. Must be less than `modulus` to match.
        remainder: u32,
    },
    /// `min <= count * scale + offset <= max`, using checked `i128`
    /// arithmetic so boundary probes cannot overflow.
    AffineRange {
        /// Multiplicative coefficient applied to `count`.
        scale: i64,
        /// Constant term added after scaling.
        offset: i64,
        /// Inclusive lower bound for the affine expression.
        min: i64,
        /// Inclusive upper bound for the affine expression.
        max: i64,
    },
    /// Conjunction of two formulas.
    And(Box<ShapeFormula>, Box<ShapeFormula>),
    /// Disjunction of two formulas.
    Or(Box<ShapeFormula>, Box<ShapeFormula>),
    /// Negation of a formula.
    Not(Box<ShapeFormula>),
}

impl ShapeFormula {
    /// Evaluate the formula against a concrete `count`. Returns `true`
    /// when the formula holds.
    #[must_use]
    pub fn evaluate(&self, count: u32) -> bool {
        match self {
            Self::AtLeast(n) => count >= *n,
            Self::AtMost(n) => count <= *n,
            Self::Exactly(n) => count == *n,
            Self::MultipleOf(n) => *n != 0 && count % *n == 0,
            Self::ModEquals { modulus, remainder } => {
                *modulus != 0 && *remainder < *modulus && count % *modulus == *remainder
            }
            Self::AffineRange {
                scale,
                offset,
                min,
                max,
            } => {
                let value = i128::from(count) * i128::from(*scale) + i128::from(*offset);
                value >= i128::from(*min) && value <= i128::from(*max)
            }
            Self::And(a, b) => a.evaluate(count) && b.evaluate(count),
            Self::Or(a, b) => a.evaluate(count) || b.evaluate(count),
            Self::Not(inner) => !inner.evaluate(count),
        }
    }

    /// Human-readable rendering for error messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::AtLeast(n) => format!("count >= {n}"),
            Self::AtMost(n) => format!("count <= {n}"),
            Self::Exactly(n) => format!("count == {n}"),
            Self::MultipleOf(n) => format!("count % {n} == 0"),
            Self::ModEquals { modulus, remainder } => format!("count % {modulus} == {remainder}"),
            Self::AffineRange {
                scale,
                offset,
                min,
                max,
            } => {
                format!("{min} <= count * {scale} + {offset} <= {max}")
            }
            Self::And(a, b) => format!("({}) && ({})", a.describe(), b.describe()),
            Self::Or(a, b) => format!("({}) || ({})", a.describe(), b.describe()),
            Self::Not(inner) => format!("!({})", inner.describe()),
        }
    }

    /// Whether this formula proves a non-zero lower bound on the count.
    /// Used by the optimizer to skip empty-buffer fast paths.
    #[must_use]
    pub fn proves_non_empty(&self) -> bool {
        match self {
            Self::AtLeast(n) => *n > 0,
            Self::Exactly(n) => *n > 0,
            Self::ModEquals { modulus, remainder } => {
                *modulus != 0 && *remainder < *modulus && *remainder > 0
            }
            Self::AffineRange {
                scale,
                offset,
                min,
                max,
            } => affine_range_excludes_zero(*scale, *offset, *min, *max),
            Self::And(a, b) => a.proves_non_empty() || b.proves_non_empty(),
            Self::Or(a, b) => a.proves_non_empty() && b.proves_non_empty(),
            _ => false,
        }
    }
}

fn affine_range_excludes_zero(_scale: i64, offset: i64, min: i64, max: i64) -> bool {
    let zero_value = i128::from(offset);
    zero_value < i128::from(min) || zero_value > i128::from(max)
}

/// Evaluate a [`ShapeFormula`] against a `count`. Free function form
/// for callers that prefer `evaluate(formula, count)` over the
/// method-call style.
#[must_use]
pub fn evaluate(formula: &ShapeFormula, count: u32) -> bool {
    formula.evaluate(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_least_holds_at_or_above() {
        let f = ShapeFormula::AtLeast(8);
        assert!(!f.evaluate(7));
        assert!(f.evaluate(8));
        assert!(f.evaluate(100));
    }

    #[test]
    fn at_most_holds_at_or_below() {
        let f = ShapeFormula::AtMost(8);
        assert!(f.evaluate(0));
        assert!(f.evaluate(8));
        assert!(!f.evaluate(9));
    }

    #[test]
    fn exactly_only_at_match() {
        let f = ShapeFormula::Exactly(64);
        assert!(!f.evaluate(63));
        assert!(f.evaluate(64));
        assert!(!f.evaluate(65));
    }

    #[test]
    fn multiple_of_zero_never_holds() {
        let f = ShapeFormula::MultipleOf(0);
        for c in [0u32, 1, 7, 8, u32::MAX] {
            assert!(!f.evaluate(c), "MultipleOf(0) must never hold (c = {c})");
        }
    }

    #[test]
    fn multiple_of_alignment() {
        let f = ShapeFormula::MultipleOf(4);
        for &(c, expect) in &[(0u32, true), (3, false), (4, true), (7, false), (16, true)] {
            assert_eq!(f.evaluate(c), expect, "c = {c}");
        }
    }

    #[test]
    fn conjunction_requires_both() {
        let f = ShapeFormula::And(
            Box::new(ShapeFormula::AtLeast(8)),
            Box::new(ShapeFormula::MultipleOf(4)),
        );
        // Need >= 8 AND % 4 == 0.
        assert!(!f.evaluate(0));
        assert!(!f.evaluate(7));
        assert!(!f.evaluate(8 + 1)); // 9 not multiple of 4
        assert!(f.evaluate(8));
        assert!(f.evaluate(12));
    }

    #[test]
    fn disjunction_accepts_either_side() {
        let f = ShapeFormula::Or(
            Box::new(ShapeFormula::Exactly(4)),
            Box::new(ShapeFormula::Exactly(8)),
        );
        assert!(f.evaluate(4));
        assert!(f.evaluate(8));
        assert!(!f.evaluate(6));
    }

    #[test]
    fn negation_inverts_predicate() {
        let f = ShapeFormula::Not(Box::new(ShapeFormula::AtMost(8)));
        assert!(!f.evaluate(8));
        assert!(f.evaluate(9));
    }

    #[test]
    fn modular_equality_requires_canonical_remainder() {
        let f = ShapeFormula::ModEquals {
            modulus: 8,
            remainder: 3,
        };
        assert!(f.evaluate(11));
        assert!(!f.evaluate(12));
        assert!(!ShapeFormula::ModEquals {
            modulus: 8,
            remainder: 8,
        }
        .evaluate(8));
        assert!(!ShapeFormula::ModEquals {
            modulus: 0,
            remainder: 0,
        }
        .evaluate(0));
    }

    #[test]
    fn affine_range_uses_wide_arithmetic() {
        let f = ShapeFormula::AffineRange {
            scale: 2,
            offset: -4,
            min: 12,
            max: 20,
        };
        assert!(!f.evaluate(7));
        assert!(f.evaluate(8));
        assert!(f.evaluate(12));
        assert!(!f.evaluate(13));
        assert!(!ShapeFormula::AffineRange {
            scale: i64::MAX,
            offset: i64::MAX,
            min: i64::MIN,
            max: i64::MAX,
        }
        .evaluate(u32::MAX));
    }

    #[test]
    fn proves_non_empty_at_least() {
        assert!(ShapeFormula::AtLeast(1).proves_non_empty());
        assert!(ShapeFormula::AtLeast(64).proves_non_empty());
        assert!(!ShapeFormula::AtLeast(0).proves_non_empty());
    }

    #[test]
    fn proves_non_empty_exactly() {
        assert!(ShapeFormula::Exactly(1).proves_non_empty());
        assert!(!ShapeFormula::Exactly(0).proves_non_empty());
    }

    #[test]
    fn proves_non_empty_through_conjunction() {
        // (count >= 8) && (count % 4 == 0)  -  first disjunct proves
        // non-empty.
        let f = ShapeFormula::And(
            Box::new(ShapeFormula::AtLeast(8)),
            Box::new(ShapeFormula::MultipleOf(4)),
        );
        assert!(f.proves_non_empty());
    }

    #[test]
    fn proves_non_empty_no_lower_bound() {
        // (count <= 256) && (count % 4 == 0)  -  neither bounds count > 0.
        let f = ShapeFormula::And(
            Box::new(ShapeFormula::AtMost(256)),
            Box::new(ShapeFormula::MultipleOf(4)),
        );
        assert!(!f.proves_non_empty());
    }

    #[test]
    fn proves_non_empty_for_modular_and_boolean_forms() {
        assert!(ShapeFormula::ModEquals {
            modulus: 4,
            remainder: 1,
        }
        .proves_non_empty());
        assert!(!ShapeFormula::ModEquals {
            modulus: 4,
            remainder: 0,
        }
        .proves_non_empty());
        assert!(ShapeFormula::Or(
            Box::new(ShapeFormula::AtLeast(1)),
            Box::new(ShapeFormula::Exactly(7)),
        )
        .proves_non_empty());
        assert!(!ShapeFormula::Or(
            Box::new(ShapeFormula::AtLeast(0)),
            Box::new(ShapeFormula::Exactly(7)),
        )
        .proves_non_empty());
    }

    /// Closure-bar: free-function form must agree with method form.
    #[test]
    fn free_function_matches_method() {
        let f = ShapeFormula::And(
            Box::new(ShapeFormula::AtLeast(8)),
            Box::new(ShapeFormula::AtMost(64)),
        );
        for c in [0u32, 7, 8, 16, 64, 65, 100] {
            assert_eq!(f.evaluate(c), evaluate(&f, c), "drift on c = {c}");
        }
    }

    /// Adversarial: u32::MAX boundary on AtLeast / AtMost must not
    /// overflow.
    #[test]
    fn u32_max_boundary() {
        assert!(ShapeFormula::AtLeast(u32::MAX).evaluate(u32::MAX));
        assert!(!ShapeFormula::AtLeast(u32::MAX).evaluate(u32::MAX - 1));
        assert!(ShapeFormula::AtMost(u32::MAX).evaluate(u32::MAX));
        assert!(ShapeFormula::AtMost(u32::MAX).evaluate(0));
    }

    /// describe() round-trip: rendering must contain the operand.
    #[test]
    fn describe_renders_operand() {
        assert!(ShapeFormula::AtLeast(42).describe().contains("42"));
        assert!(ShapeFormula::AtMost(7).describe().contains("7"));
        assert!(ShapeFormula::Exactly(64).describe().contains("64"));
        assert!(ShapeFormula::MultipleOf(4).describe().contains("4"));
    }
}
