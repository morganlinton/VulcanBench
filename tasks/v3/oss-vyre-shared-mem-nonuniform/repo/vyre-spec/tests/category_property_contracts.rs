//! Property gates for `vyre_spec::Category` and backend availability predicates.

use proptest::prelude::*;
use vyre_spec::{BackendAvailability, BackendAvailabilityPredicate, Category};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn backend_predicate_matches_closure_semantics(
        op in "[a-z]{1,16}",
    ) {
        let pred = BackendAvailabilityPredicate::new(|name| name.len() % 2 == 0);
        prop_assert_eq!(pred.available(&op), op.len() % 2 == 0);
    }

    #[test]
    fn fn_impl_backend_availability_matches_predicate(
        op in "[a-z]{1,12}",
        flag in proptest::bool::ANY,
    ) {
        let pred = |name: &str| -> bool { name.contains('a') == flag };
        prop_assert_eq!(pred.available(&op), op.contains('a') == flag);
    }
}

#[test]
fn unclassified_marker_is_detected() {
    assert!(Category::unclassified().is_unclassified());
}

#[test]
fn category_a_with_ops_is_not_unclassified() {
    let cat = Category::A {
        composition_of: vec!["add", "mul"],
    };
    assert!(!cat.is_unclassified());
}

#[test]
fn category_c_equality_compares_hardware_only() {
    let always = BackendAvailabilityPredicate::new(|_| true);
    let never = BackendAvailabilityPredicate::new(|_| false);
    let left = Category::C {
        hardware: "cuda",
        backend_availability: always,
    };
    let right = Category::C {
        hardware: "cuda",
        backend_availability: never,
    };
    assert_eq!(left, right);
}
