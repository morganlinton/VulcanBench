//! Property model for megakernel scheduler priority partitioning.
#![allow(clippy::needless_range_loop)]

use proptest::prelude::*;

use vyre_runtime::megakernel::scheduler::{default_priority_offsets, PRIORITY_LEVELS};

// A differential model of the GPU megakernel ring buffer priority scheduling logic.
// This proves the mathematical properties of our priority implementation
// over edge cases, randomized queue capacities, and slot layouts BEFORE
// we optimize the underlying GPU dispatcher via subgroup logic.

fn model_default_priority_offsets(total_slots: u32) -> Vec<u32> {
    let mut offsets = vec![0; PRIORITY_LEVELS as usize + 1];
    let base = total_slots / PRIORITY_LEVELS;
    let rem = total_slots % PRIORITY_LEVELS;
    let mut cursor = 0;
    for i in 0..PRIORITY_LEVELS as usize {
        offsets[i] = cursor;
        let p_size = base + if i == 2 { rem } else { 0 }; // 2 is NORMAL priority
        cursor += p_size;
    }
    offsets[PRIORITY_LEVELS as usize] = total_slots;
    offsets
}

proptest! {
    #[test]
    fn test_differential_priority_offsets_equivalence(
        total_slots in 0u32..10_000_000
    ) {
        // Assert mathematical equivalence between the monolithic code and our strict model
        let actual = default_priority_offsets(total_slots);
        let expected = model_default_priority_offsets(total_slots);
        prop_assert_eq!(&actual, &expected);

        // Assert invariants:
        // 1. Array is length PRIORITY_LEVELS + 1
        prop_assert_eq!(actual.len(), PRIORITY_LEVELS as usize + 1);

        // 2. Head is 0, Tail is total_slots
        prop_assert_eq!(actual[0], 0);
        prop_assert_eq!(*actual.last().unwrap(), total_slots);

        // 3. Monotonically increasing
        for w in actual.windows(2) {
            prop_assert!(w[0] <= w[1], "Partitions must not go backwards");
        }
    }
}
