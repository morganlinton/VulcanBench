//! Verified shape facts the optimizer relies on (P-1.0-V3.3).
//!
//! After `validate()` accepts a `Program`, every `BufferDecl`'s
//! `shape_predicate` has been proved consistent with its static
//! `count`. Optimizer passes can therefore *trust* the predicate and
//! query it for facts: minimum count, alignment guarantees, fixed-
//! size hints. This file is the canonical query surface.
//!
//! The contract is one-directional: passes ask "does this predicate
//! prove fact X?" and only optimize when the answer is `true`. A
//! pass that needs the inverse (predicate proves NOT-X) must phrase
//! its question as a positive claim.
//!
//! Runs in optimizer hot paths (per buffer per pass invocation). All
//! query functions are O(predicate-tree-depth)  -  typically constant
//! for the leaf variants and O(1) per And-node.

use crate::ir_inner::model::program::ShapePredicate;

/// Tightest lower bound the predicate proves on the buffer's `count`.
/// Returns 0 when the predicate gives no positive lower bound.
#[must_use]
pub fn min_count(predicate: &ShapePredicate) -> u32 {
    match predicate {
        ShapePredicate::AtLeast(n) | ShapePredicate::Exactly(n) => *n,
        ShapePredicate::AtMost(_) | ShapePredicate::MultipleOf(_) | ShapePredicate::Not(_) => 0,
        ShapePredicate::ModEquals { modulus, remainder } => {
            if *modulus != 0 && *remainder < *modulus {
                *remainder
            } else {
                0
            }
        }
        ShapePredicate::AffineRange {
            scale, offset, min, ..
        } => affine_min_count(*scale, *offset, *min).unwrap_or(0),
        ShapePredicate::And(a, b) => min_count(a).max(min_count(b)),
        ShapePredicate::Or(a, b) => min_count(a).min(min_count(b)),
    }
}

/// Tightest upper bound the predicate proves on the buffer's `count`.
/// Returns `None` when the predicate gives no upper bound.
#[must_use]
pub fn max_count(predicate: &ShapePredicate) -> Option<u32> {
    match predicate {
        ShapePredicate::AtMost(n) | ShapePredicate::Exactly(n) => Some(*n),
        ShapePredicate::AtLeast(_)
        | ShapePredicate::MultipleOf(_)
        | ShapePredicate::ModEquals { .. }
        | ShapePredicate::Not(_) => None,
        ShapePredicate::AffineRange {
            scale, offset, max, ..
        } => affine_max_count(*scale, *offset, *max),
        ShapePredicate::And(a, b) => match (max_count(a), max_count(b)) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        },
        ShapePredicate::Or(a, b) => match (max_count(a), max_count(b)) {
            (Some(x), Some(y)) => Some(x.max(y)),
            _ => None,
        },
    }
}

/// Whether the predicate proves the count is a multiple of `factor`.
/// Used by vectorization passes to decide whether a buffer can be
/// processed in `factor`-wide SIMD lanes without a tail loop.
#[must_use]
pub fn is_multiple_of(predicate: &ShapePredicate, factor: u32) -> bool {
    if factor == 0 {
        return false;
    }
    match predicate {
        ShapePredicate::MultipleOf(n) => *n != 0 && *n % factor == 0,
        ShapePredicate::ModEquals { modulus, remainder } => {
            *remainder == 0 && *modulus != 0 && *modulus % factor == 0
        }
        ShapePredicate::Exactly(n) => *n % factor == 0,
        ShapePredicate::AtLeast(_)
        | ShapePredicate::AtMost(_)
        | ShapePredicate::AffineRange { .. }
        | ShapePredicate::Not(_) => false,
        ShapePredicate::And(a, b) => is_multiple_of(a, factor) || is_multiple_of(b, factor),
        ShapePredicate::Or(a, b) => is_multiple_of(a, factor) && is_multiple_of(b, factor),
    }
}

/// Whether the predicate proves the count is exactly `n`. Used by
/// the unroll pass to decide whether a loop can be fully unrolled.
#[must_use]
pub fn is_fixed_at(predicate: &ShapePredicate, n: u32) -> bool {
    match predicate {
        ShapePredicate::Exactly(m) => *m == n,
        ShapePredicate::And(a, b) => is_fixed_at(a, n) || is_fixed_at(b, n),
        ShapePredicate::Or(a, b) => is_fixed_at(a, n) && is_fixed_at(b, n),
        _ => false,
    }
}

/// Convenience: does the predicate prove the count is non-empty?
/// (i.e. proves `count > 0`.) Used by passes that want to skip
/// dead-buffer guards when the buffer is statically known to have
/// at least one element.
#[must_use]
pub fn proves_non_empty(predicate: &ShapePredicate) -> bool {
    min_count(predicate) > 0
}

fn affine_min_count(scale: i64, offset: i64, min: i64) -> Option<u32> {
    if scale <= 0 {
        return None;
    }
    let numerator = i128::from(min) - i128::from(offset);
    if numerator <= 0 {
        return Some(0);
    }
    let value = div_ceil_positive(numerator, i128::from(scale));
    u32::try_from(value).ok()
}

fn affine_max_count(scale: i64, offset: i64, max: i64) -> Option<u32> {
    if scale <= 0 {
        return None;
    }
    let numerator = i128::from(max) - i128::from(offset);
    if numerator < 0 {
        return Some(0);
    }
    let value = numerator / i128::from(scale);
    Some(u32::try_from(value).unwrap_or(u32::MAX))
}

fn div_ceil_positive(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(numerator > 0);
    debug_assert!(denominator > 0);
    (numerator + denominator - 1) / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_least_gives_lower_bound_no_upper_bound() {
        let p = ShapePredicate::AtLeast(64);
        assert_eq!(min_count(&p), 64);
        assert_eq!(max_count(&p), None);
        assert!(proves_non_empty(&p));
    }

    #[test]
    fn at_most_gives_upper_bound_no_lower_bound() {
        let p = ShapePredicate::AtMost(64);
        assert_eq!(min_count(&p), 0);
        assert_eq!(max_count(&p), Some(64));
        assert!(!proves_non_empty(&p));
    }

    #[test]
    fn exactly_pins_both_bounds() {
        let p = ShapePredicate::Exactly(32);
        assert_eq!(min_count(&p), 32);
        assert_eq!(max_count(&p), Some(32));
        assert!(is_fixed_at(&p, 32));
        assert!(!is_fixed_at(&p, 31));
    }

    #[test]
    fn multiple_of_proves_alignment_for_divisors() {
        let p = ShapePredicate::MultipleOf(64);
        assert!(is_multiple_of(&p, 64));
        assert!(is_multiple_of(&p, 32));
        assert!(is_multiple_of(&p, 16));
        assert!(is_multiple_of(&p, 4));
        assert!(!is_multiple_of(&p, 128)); // 128 doesn't divide 64
        assert!(!is_multiple_of(&p, 0));
    }

    #[test]
    fn exactly_proves_alignment_when_n_aligned() {
        let p = ShapePredicate::Exactly(96);
        assert!(is_multiple_of(&p, 32));
        assert!(is_multiple_of(&p, 16));
        assert!(!is_multiple_of(&p, 64));
    }

    #[test]
    fn and_combines_lower_and_upper_bounds() {
        let p = ShapePredicate::And(
            Box::new(ShapePredicate::AtLeast(64)),
            Box::new(ShapePredicate::AtMost(256)),
        );
        assert_eq!(min_count(&p), 64);
        assert_eq!(max_count(&p), Some(256));
        assert!(proves_non_empty(&p));
    }

    #[test]
    fn and_combines_alignment_with_minimum() {
        // count >= 64 && count % 32 == 0 -> can vectorize at 32-wide
        let p = ShapePredicate::And(
            Box::new(ShapePredicate::AtLeast(64)),
            Box::new(ShapePredicate::MultipleOf(32)),
        );
        assert!(is_multiple_of(&p, 32));
        assert!(is_multiple_of(&p, 16));
        assert!(!is_multiple_of(&p, 64));
        assert!(proves_non_empty(&p));
    }

    #[test]
    fn or_only_proves_facts_shared_by_both_sides() {
        let p = ShapePredicate::Or(
            Box::new(ShapePredicate::Exactly(32)),
            Box::new(ShapePredicate::Exactly(64)),
        );
        assert_eq!(min_count(&p), 32);
        assert_eq!(max_count(&p), Some(64));
        assert!(is_multiple_of(&p, 16));
        assert!(!is_multiple_of(&p, 64));
        assert!(!is_fixed_at(&p, 32));
    }

    #[test]
    fn mod_equals_zero_remainder_proves_alignment() {
        let p = ShapePredicate::ModEquals {
            modulus: 64,
            remainder: 0,
        };
        assert_eq!(min_count(&p), 0);
        assert!(is_multiple_of(&p, 32));
        assert!(!is_multiple_of(&p, 128));
    }

    #[test]
    fn mod_equals_nonzero_remainder_proves_non_empty() {
        let p = ShapePredicate::ModEquals {
            modulus: 8,
            remainder: 3,
        };
        assert_eq!(min_count(&p), 3);
        assert!(proves_non_empty(&p));
        assert!(!is_multiple_of(&p, 2));
    }

    #[test]
    fn affine_range_derives_positive_scale_bounds() {
        let p = ShapePredicate::AffineRange {
            scale: 4,
            offset: -8,
            min: 24,
            max: 40,
        };
        assert_eq!(min_count(&p), 8);
        assert_eq!(max_count(&p), Some(12));
        assert!(proves_non_empty(&p));
    }

    #[test]
    fn unrelated_predicates_do_not_prove_alignment() {
        let p = ShapePredicate::AtLeast(64);
        assert!(!is_multiple_of(&p, 32));
    }
}
