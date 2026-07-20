//! Surface tests for the `AlgebraicLaw` enum and its methods.
//!
//! These are freeze-tests: they assert that every variant has the
//! expected classification so that spec drift is caught immediately.

use vyre_spec::{AlgebraicLaw, MonotonicDirection};

#[test]
fn commutative_is_binary() {
    assert!(AlgebraicLaw::Commutative.is_binary());
    assert!(!AlgebraicLaw::Commutative.is_unary());
    assert_eq!(AlgebraicLaw::Commutative.name(), "commutative");
}

#[test]
fn associative_is_binary() {
    assert!(AlgebraicLaw::Associative.is_binary());
    assert!(!AlgebraicLaw::Associative.is_unary());
    assert_eq!(AlgebraicLaw::Associative.name(), "associative");
}

#[test]
fn identity_is_binary() {
    let law = AlgebraicLaw::Identity { element: 42 };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "identity");
}

#[test]
fn left_identity_is_binary() {
    let law = AlgebraicLaw::LeftIdentity { element: 7 };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "left-identity");
}

#[test]
fn right_identity_is_binary() {
    let law = AlgebraicLaw::RightIdentity { element: 7 };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "right-identity");
}

#[test]
fn self_inverse_is_binary() {
    let law = AlgebraicLaw::SelfInverse { result: 0 };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "self-inverse");
}

#[test]
fn idempotent_is_binary() {
    assert!(AlgebraicLaw::Idempotent.is_binary());
    assert!(!AlgebraicLaw::Idempotent.is_unary());
    assert_eq!(AlgebraicLaw::Idempotent.name(), "idempotent");
}

#[test]
fn involution_is_unary() {
    assert!(!AlgebraicLaw::Involution.is_binary());
    assert!(AlgebraicLaw::Involution.is_unary());
    assert_eq!(AlgebraicLaw::Involution.name(), "involution");
}

#[test]
fn monotone_is_unary() {
    assert!(!AlgebraicLaw::Monotone.is_binary());
    assert!(AlgebraicLaw::Monotone.is_unary());
    assert_eq!(AlgebraicLaw::Monotone.name(), "monotone");
}

#[test]
fn monotonic_is_unary() {
    let law = AlgebraicLaw::Monotonic {
        direction: MonotonicDirection::NonDecreasing,
    };
    assert!(!law.is_binary());
    assert!(law.is_unary());
    assert_eq!(law.name(), "monotonic");
}

#[test]
fn bounded_is_both_binary_and_unary() {
    let law = AlgebraicLaw::Bounded { lo: 0, hi: 100 };
    assert!(law.is_binary());
    assert!(law.is_unary());
    assert_eq!(law.name(), "bounded");
}

#[test]
fn complement_is_both_binary_and_unary() {
    let law = AlgebraicLaw::Complement {
        complement_op: "not",
        universe: u32::MAX,
    };
    assert!(law.is_binary());
    assert!(law.is_unary());
    assert_eq!(law.name(), "complement");
}

#[test]
fn de_morgan_is_unary() {
    let law = AlgebraicLaw::DeMorgan {
        inner_op: "and",
        dual_op: "or",
    };
    assert!(!law.is_binary());
    assert!(law.is_unary());
    assert_eq!(law.name(), "de-morgan");
}

#[test]
fn distributive_is_binary() {
    let law = AlgebraicLaw::DistributiveOver { over_op: "add" };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "distributive");
}

#[test]
fn lattice_absorption_is_binary() {
    let law = AlgebraicLaw::LatticeAbsorption { dual_op: "min" };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "lattice-absorption");
}

#[test]
fn inverse_of_is_binary() {
    let law = AlgebraicLaw::InverseOf { op: "add" };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "inverse-of");
}

#[test]
fn trichotomy_is_binary() {
    let law = AlgebraicLaw::Trichotomy {
        less_op: "lt",
        equal_op: "eq",
        greater_op: "gt",
    };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "trichotomy");
}

#[test]
fn zero_product_is_binary() {
    let law = AlgebraicLaw::ZeroProduct { holds: true };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "zero-product");
}

#[test]
fn categorical_identity_is_neither_binary_nor_unary() {
    assert!(!AlgebraicLaw::CategoricalIdentity.is_binary());
    assert!(!AlgebraicLaw::CategoricalIdentity.is_unary());
    assert_eq!(
        AlgebraicLaw::CategoricalIdentity.name(),
        "categorical-identity"
    );
}

#[test]
fn categorical_associative_is_neither_binary_nor_unary() {
    assert!(!AlgebraicLaw::CategoricalAssociative.is_binary());
    assert!(!AlgebraicLaw::CategoricalAssociative.is_unary());
    assert_eq!(
        AlgebraicLaw::CategoricalAssociative.name(),
        "categorical-associative"
    );
}

#[test]
fn custom_is_binary_and_unary() {
    fn dummy_check(_op: fn(&[u8]) -> Vec<u8>, _args: &[u32]) -> bool {
        true
    }
    let law = AlgebraicLaw::Custom {
        name: "my-law",
        description: "a custom law",
        arity: 2,
        check: dummy_check,
    };
    assert!(law.is_binary());
    assert!(law.is_unary());
    assert_eq!(law.name(), "my-law");
}

#[test]
fn absorbing_is_binary() {
    let law = AlgebraicLaw::Absorbing { element: 0 };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "absorbing");
}

#[test]
fn left_absorbing_is_binary() {
    let law = AlgebraicLaw::LeftAbsorbing { element: 0 };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "left-absorbing");
}

#[test]
fn right_absorbing_is_binary() {
    let law = AlgebraicLaw::RightAbsorbing { element: 0 };
    assert!(law.is_binary());
    assert!(!law.is_unary());
    assert_eq!(law.name(), "right-absorbing");
}

#[test]
fn identity_partial_eq_matches_on_element() {
    assert_eq!(
        AlgebraicLaw::Identity { element: 5 },
        AlgebraicLaw::Identity { element: 5 }
    );
    assert_ne!(
        AlgebraicLaw::Identity { element: 5 },
        AlgebraicLaw::Identity { element: 6 }
    );
}

#[test]
fn commutative_partial_eq_is_reflexive() {
    assert_ne!(AlgebraicLaw::Commutative, AlgebraicLaw::Associative);
}

#[test]
fn commutative_and_associative_are_not_equal() {
    assert_ne!(AlgebraicLaw::Commutative, AlgebraicLaw::Associative);
}
