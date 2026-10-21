//! External contract tests for the frozen invariant catalog.
//!
//! The invariant catalog is consumed by conformance runners and backend
//! vendors. These tests assert catalog-level properties rather than local
//! implementation details: ids are dense and stable, lookup helpers are
//! coherent, categories partition the catalog, and every built-in invariant has
//! both happy-path and adversarial executable descriptors.

use std::collections::BTreeSet;

use vyre_spec::{by_category, by_id, invariants, EngineInvariant, InvariantCategory};

const BUILTIN_INVARIANT_COUNT: usize = 15;
const CATEGORIES: [InvariantCategory; 4] = [
    InvariantCategory::Execution,
    InvariantCategory::Algebra,
    InvariantCategory::Resource,
    InvariantCategory::Stability,
];

#[test]
fn invariant_ids_are_dense_stable_and_lookup_addressable() {
    let catalog = invariants();
    assert_eq!(
        catalog.len(),
        BUILTIN_INVARIANT_COUNT,
        "Fix: adding/removing a frozen engine invariant must update the external catalog contract."
    );

    let ids: Vec<_> = EngineInvariant::iter().collect();
    assert_eq!(
        ids.len(),
        BUILTIN_INVARIANT_COUNT,
        "Fix: EngineInvariant::iter must expose every frozen invariant id."
    );

    for (index, id) in ids.into_iter().enumerate() {
        let ordinal = index + 1;
        assert_eq!(
            usize::from(id.ordinal()),
            ordinal,
            "Fix: invariant ids must remain dense and one-based."
        );
        assert_eq!(
            id.to_string(),
            format!("I{ordinal}"),
            "Fix: Display for invariant ids is part of the public certificate surface."
        );

        let from_catalog = catalog
            .iter()
            .find(|invariant| invariant.id == id)
            .unwrap_or_else(|| panic!("Fix: invariant {id} is missing from invariants()."));
        let from_lookup =
            by_id(id).unwrap_or_else(|| panic!("Fix: by_id({id}) must find the frozen invariant."));
        assert_eq!(from_lookup.id, from_catalog.id);
        assert_eq!(from_lookup.name, from_catalog.name);
        assert_eq!(from_lookup.description, from_catalog.description);
        assert_eq!(from_lookup.category, from_catalog.category);
    }
}

#[test]
fn invariant_categories_partition_the_catalog_exactly() {
    let catalog_ids: BTreeSet<_> = invariants().iter().map(|invariant| invariant.id).collect();
    let mut category_ids = BTreeSet::new();

    for category in CATEGORIES {
        let ids: Vec<_> = by_category(category)
            .map(|invariant| invariant.id)
            .collect();
        assert!(
            !ids.is_empty(),
            "Fix: every public invariant category must own at least one built-in invariant."
        );
        for id in ids {
            assert!(
                category_ids.insert(id),
                "Fix: invariant {id} appeared in more than one category partition."
            );
        }
    }

    assert_eq!(
        category_ids, catalog_ids,
        "Fix: by_category must partition invariants() without omissions or extras."
    );
}

#[test]
fn every_builtin_invariant_has_happy_and_adversarial_descriptors() {
    for invariant in invariants() {
        assert!(
            !invariant.name.trim().is_empty(),
            "Fix: invariant {} must have a human-readable name.",
            invariant.id
        );
        assert!(
            !invariant.description.trim().is_empty(),
            "Fix: invariant {} must describe the backend contract.",
            invariant.id
        );

        let tests = (invariant.test_family)();
        assert!(
            tests.len() >= 2,
            "Fix: invariant {} must have at least happy-path and adversarial descriptors.",
            invariant.id
        );

        let has_happy = tests
            .iter()
            .any(|descriptor| descriptor.purpose.starts_with("Happy path:"));
        let has_adversarial = tests
            .iter()
            .any(|descriptor| descriptor.purpose.starts_with("Adversarial path:"));
        assert!(
            has_happy && has_adversarial,
            "Fix: invariant {} must carry both happy-path and adversarial descriptor purposes.",
            invariant.id
        );

        let mut names = BTreeSet::new();
        for descriptor in tests {
            assert_eq!(
                descriptor.invariant, invariant.id,
                "Fix: descriptor `{}` is attached to the wrong invariant.",
                descriptor.name
            );
            assert!(
                descriptor
                    .name
                    .starts_with("conform/vyre-conform-enforce/tests/invariants.rs::"),
                "Fix: descriptor `{}` must point at the executable conformance invariant suite.",
                descriptor.name
            );
            assert_eq!(
                descriptor.name.matches("::").count(),
                1,
                "Fix: descriptor `{}` must name exactly one test function.",
                descriptor.name
            );
            assert!(
                names.insert(descriptor.name),
                "Fix: invariant {} has duplicate descriptor `{}`.",
                invariant.id,
                descriptor.name
            );
        }
    }
}
