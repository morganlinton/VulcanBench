//! Generated property coverage for invariant id and catalog contracts.

use proptest::prelude::*;
use vyre_spec::{by_category, by_id, EngineInvariant, InvariantCategory};

fn invariant_strategy() -> impl Strategy<Value = EngineInvariant> {
    prop_oneof![
        Just(EngineInvariant::I1),
        Just(EngineInvariant::I2),
        Just(EngineInvariant::I3),
        Just(EngineInvariant::I4),
        Just(EngineInvariant::I5),
        Just(EngineInvariant::I6),
        Just(EngineInvariant::I7),
        Just(EngineInvariant::I8),
        Just(EngineInvariant::I9),
        Just(EngineInvariant::I10),
        Just(EngineInvariant::I11),
        Just(EngineInvariant::I12),
        Just(EngineInvariant::I13),
        Just(EngineInvariant::I14),
        Just(EngineInvariant::I15),
    ]
}

fn invariant_category_strategy() -> impl Strategy<Value = InvariantCategory> {
    prop_oneof![
        Just(InvariantCategory::Execution),
        Just(InvariantCategory::Algebra),
        Just(InvariantCategory::Resource),
        Just(InvariantCategory::Stability),
    ]
}

proptest! {
    #[test]
    fn generated_invariant_ids_are_lookup_addressable(id in invariant_strategy()) {
        let entry = by_id(id).expect("Fix: every EngineInvariant id must be present in by_id");

        prop_assert_eq!(entry.id, id);
        prop_assert_eq!(id.to_string(), format!("I{}", id.ordinal()));
        prop_assert!((1..=15).contains(&id.ordinal()));
    }

    #[test]
    fn generated_invariant_categories_only_return_matching_entries(category in invariant_category_strategy()) {
        let entries = by_category(category).collect::<Vec<_>>();

        prop_assert!(!entries.is_empty());
        for entry in entries {
            prop_assert_eq!(entry.category, category);
            prop_assert!(by_id(entry.id).is_some());
        }
    }
}
