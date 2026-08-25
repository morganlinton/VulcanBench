//! Tests for `ShapePredicate::holds` evaluation.
//!
//! Shape predicates constrain buffer element counts. The validator
//! uses `holds()` to check each predicate against the static count.

use vyre::ir::ShapePredicate;

#[test]
fn at_least_holds_when_equal() {
    assert!(ShapePredicate::AtLeast(64).holds(64));
}

#[test]
fn at_least_holds_when_greater() {
    assert!(ShapePredicate::AtLeast(64).holds(100));
}

#[test]
fn at_least_fails_when_less() {
    assert!(!ShapePredicate::AtLeast(64).holds(63));
}

#[test]
fn at_most_holds_when_equal() {
    assert!(ShapePredicate::AtMost(64).holds(64));
}

#[test]
fn at_most_holds_when_less() {
    assert!(ShapePredicate::AtMost(64).holds(10));
}

#[test]
fn at_most_fails_when_greater() {
    assert!(!ShapePredicate::AtMost(64).holds(65));
}

#[test]
fn exactly_holds_when_equal() {
    assert!(ShapePredicate::Exactly(64).holds(64));
}

#[test]
fn exactly_fails_when_different() {
    assert!(!ShapePredicate::Exactly(64).holds(63));
    assert!(!ShapePredicate::Exactly(64).holds(65));
}

#[test]
fn multiple_of_holds_when_divisible() {
    assert!(ShapePredicate::MultipleOf(64).holds(128));
}

#[test]
fn multiple_of_fails_when_not_divisible() {
    assert!(!ShapePredicate::MultipleOf(64).holds(100));
}

#[test]
fn multiple_of_fails_when_zero_divisor() {
    // Zero divisor should return false for any count
    assert!(!ShapePredicate::MultipleOf(0).holds(64));
}

#[test]
fn mod_equals_holds_when_matches() {
    assert!(ShapePredicate::ModEquals {
        modulus: 8,
        remainder: 3
    }
    .holds(19));
}

#[test]
fn mod_equals_fails_when_different() {
    assert!(!ShapePredicate::ModEquals {
        modulus: 8,
        remainder: 3
    }
    .holds(20));
}

#[test]
fn affine_range_holds_when_inside() {
    // scale=2, offset=0, min=0, max=100 => count*2 must be in [0, 100]
    assert!(ShapePredicate::AffineRange {
        scale: 2,
        offset: 0,
        min: 0,
        max: 100
    }
    .holds(50));
}

#[test]
fn affine_range_fails_when_outside() {
    assert!(!ShapePredicate::AffineRange {
        scale: 2,
        offset: 0,
        min: 0,
        max: 100
    }
    .holds(51));
}

#[test]
fn and_holds_when_both_hold() {
    let pred = ShapePredicate::And(
        Box::new(ShapePredicate::AtLeast(10)),
        Box::new(ShapePredicate::AtMost(100)),
    );
    assert!(pred.holds(50));
}

#[test]
fn and_fails_when_one_fails() {
    let pred = ShapePredicate::And(
        Box::new(ShapePredicate::AtLeast(10)),
        Box::new(ShapePredicate::AtMost(100)),
    );
    assert!(!pred.holds(5));
    assert!(!pred.holds(200));
}

#[test]
fn or_holds_when_either_holds() {
    let pred = ShapePredicate::Or(
        Box::new(ShapePredicate::Exactly(7)),
        Box::new(ShapePredicate::Exactly(42)),
    );
    assert!(pred.holds(7));
    assert!(pred.holds(42));
}

#[test]
fn or_fails_when_neither_holds() {
    let pred = ShapePredicate::Or(
        Box::new(ShapePredicate::Exactly(7)),
        Box::new(ShapePredicate::Exactly(42)),
    );
    assert!(!pred.holds(10));
}

#[test]
fn not_inverts_result() {
    let pred = ShapePredicate::Not(Box::new(ShapePredicate::Exactly(0)));
    assert!(pred.holds(1));
    assert!(!pred.holds(0));
}

#[test]
fn describe_is_non_empty() {
    assert!(ShapePredicate::AtLeast(10).describe().contains("10"));
    assert!(ShapePredicate::Exactly(5).describe().contains('5'));
    assert!(ShapePredicate::MultipleOf(64).describe().contains("64"));
}
