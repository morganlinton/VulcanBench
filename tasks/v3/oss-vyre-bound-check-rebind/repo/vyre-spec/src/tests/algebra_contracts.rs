//! Test: algebra contracts.
use crate::{all_algebraic_laws, law_catalog, AlgebraicLaw, MonotonicDirection};

#[test]
fn law_catalog_matches_name_fn() {
    for law in all_algebraic_laws() {
        assert!(
            law_catalog().contains(&law.name()),
            "LAW_CATALOG is missing the {} fingerprint",
            law.name()
        );
    }
    assert_eq!(
        law_catalog().len(),
        all_algebraic_laws().len(),
        "LAW_CATALOG must list every algebraic-law variant"
    );
    assert!(law_catalog().contains(&"custom"));
}

#[test]
fn law_arity_is_exclusive_except_custom_and_dual() {
    for law in [
        AlgebraicLaw::Commutative,
        AlgebraicLaw::Associative,
        AlgebraicLaw::Identity { element: 0 },
        AlgebraicLaw::Idempotent,
    ] {
        assert!(law.is_binary(), "{} must be binary", law.name());
        assert!(!law.is_unary(), "{} must not be unary", law.name());
    }
    for law in [
        AlgebraicLaw::Involution,
        AlgebraicLaw::Monotone,
        AlgebraicLaw::Monotonic {
            direction: MonotonicDirection::NonIncreasing,
        },
    ] {
        assert!(law.is_unary(), "{} must be unary", law.name());
        assert!(!law.is_binary(), "{} must not be binary", law.name());
    }
}
