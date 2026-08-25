//! Generated integrity matrix for the frozen invariant catalog.
//!
//! The catalog is consumed by backend conformance tooling. These tests pin
//! declaration order, display strings, lookup identity, category partitioning,
//! and generated test descriptor structure so index drift is caught locally.

use std::collections::BTreeSet;

use vyre_spec::{
    by_category, by_id, invariants, EngineInvariant, InvariantCategory, InvariantId, TestDescriptor,
};

const GENERATED_REPLAY_CASES: usize = 4096;

#[test]
fn generated_invariant_id_order_display_and_lookup_are_frozen() {
    let expected = [
        InvariantId::I1,
        InvariantId::I2,
        InvariantId::I3,
        InvariantId::I4,
        InvariantId::I5,
        InvariantId::I6,
        InvariantId::I7,
        InvariantId::I8,
        InvariantId::I9,
        InvariantId::I10,
        InvariantId::I11,
        InvariantId::I12,
        InvariantId::I13,
        InvariantId::I14,
        InvariantId::I15,
    ];

    let iterated = EngineInvariant::iter().collect::<Vec<_>>();
    assert_eq!(iterated, expected);
    assert_eq!(invariants().len(), expected.len());

    for (index, id) in expected.iter().copied().enumerate() {
        let ordinal = (index + 1) as u8;
        assert_eq!(id.ordinal(), ordinal);
        assert_eq!(id.to_string(), format!("I{ordinal}"));

        let catalog_entry = &invariants()[index];
        assert_eq!(catalog_entry.id, id);

        let resolved = by_id(id).expect("invariant id must resolve through by_id");
        assert!(
            core::ptr::eq(resolved, catalog_entry),
            "by_id({id}) must return the canonical catalog entry"
        );
    }
}

#[test]
fn generated_invariant_category_partitions_are_frozen() {
    let expected: &[(InvariantCategory, &[InvariantId])] = &[
        (
            InvariantCategory::Execution,
            [
                InvariantId::I1,
                InvariantId::I2,
                InvariantId::I3,
                InvariantId::I4,
                InvariantId::I5,
                InvariantId::I6,
            ]
            .as_slice(),
        ),
        (
            InvariantCategory::Algebra,
            [InvariantId::I7, InvariantId::I8, InvariantId::I9].as_slice(),
        ),
        (
            InvariantCategory::Resource,
            [InvariantId::I10, InvariantId::I11, InvariantId::I12].as_slice(),
        ),
        (
            InvariantCategory::Stability,
            [InvariantId::I13, InvariantId::I14, InvariantId::I15].as_slice(),
        ),
    ];

    let mut covered = BTreeSet::new();
    for (category, expected_ids) in expected {
        let actual = by_category(*category)
            .map(|invariant| invariant.id)
            .collect::<Vec<_>>();
        assert_eq!(actual, *expected_ids);
        covered.extend(actual);
    }

    assert_eq!(covered.len(), invariants().len());
    assert!(invariants()
        .iter()
        .all(|invariant| covered.contains(&invariant.id)));
}

#[test]
fn generated_test_descriptor_pairs_are_complete_unique_and_actionable() {
    let mut descriptor_names = BTreeSet::new();
    let mut descriptor_count = 0usize;

    for invariant in invariants() {
        let descriptors = (invariant.test_family)();
        assert_eq!(
            descriptors.len(),
            2,
            "each built-in invariant must pin one happy and one adversarial descriptor"
        );
        assert_descriptor_shape(invariant.id, &descriptors[0], "Happy path:");
        assert_descriptor_shape(invariant.id, &descriptors[1], "Adversarial path:");

        for descriptor in descriptors {
            assert!(
                descriptor_names.insert(descriptor.name),
                "duplicate generated descriptor name: {}",
                descriptor.name
            );
            descriptor_count += 1;
        }
    }

    assert_eq!(descriptor_count, invariants().len() * 2);
}

#[test]
fn generated_catalog_replay_stays_deterministic_under_repeated_queries() {
    let ids = EngineInvariant::iter().collect::<Vec<_>>();

    for case in 0..GENERATED_REPLAY_CASES {
        let id = ids[next_index(case as u64, ids.len())];
        let first = by_id(id).expect("invariant id must resolve");
        let second = by_id(id).expect("invariant id must resolve deterministically");
        assert!(core::ptr::eq(first, second));
        assert_eq!(first.id, id);
        assert_eq!(first.id.ordinal(), ((id as u8).max(1)));

        let category_entries = by_category(first.category).collect::<Vec<_>>();
        assert!(category_entries
            .iter()
            .any(|candidate| core::ptr::eq(*candidate, first)));
    }
}

fn assert_descriptor_shape(
    expected_invariant: InvariantId,
    descriptor: &TestDescriptor,
    purpose_prefix: &str,
) {
    assert_eq!(descriptor.invariant, expected_invariant);
    assert!(descriptor
        .name
        .starts_with("conform/vyre-conform-enforce/tests/invariants.rs::"));
    assert!(descriptor.name.len() > "conform/vyre-conform-enforce/tests/invariants.rs::".len());
    assert!(descriptor.purpose.starts_with(purpose_prefix));
    assert!(
        descriptor.purpose.len() > purpose_prefix.len() + 32,
        "descriptor purpose must be specific and actionable"
    );
}

fn next_index(seed: u64, len: usize) -> usize {
    let value = seed
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(0xd1b5_4a32_d192_ed03)
        .rotate_left(17);
    (value as usize) % len
}
