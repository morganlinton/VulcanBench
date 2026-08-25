//! Property tests for `vyre_primitives::matching::region`.
//!
//! Locks three invariants that the unit tests sample only at hand-
//! picked points:
//!
//!   1. **Idempotence.** `dedup(dedup(x)) == dedup(x)` for ANY input.
//!   2. **Sortedness.** Output is sorted by `(pid, start, end)`.
//!   3. **No same-pid overlaps in output.** For every pair of
//!      adjacent outputs with the same pid, `prev.end < next.start`.
//!
//! Generated input shape: 0..=64 triples with `pid ∈ 0..=7`,
//! `start ∈ 0..=255`, `end = start + (0..=32)`. Bounded ranges keep
//! shrinking fast and exercise both clusters and isolated spans.

#![cfg(all(feature = "matching", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::matching::{dedup_regions_inplace, RegionTriple};

fn dedup_regions_cpu(input: Vec<RegionTriple>) -> Vec<RegionTriple> {
    let mut owned = input;
    dedup_regions_inplace(&mut owned);
    owned
}

fn arb_triple() -> impl Strategy<Value = RegionTriple> {
    (0u32..=7, 0u32..=255, 0u32..=32)
        .prop_map(|(pid, start, len)| RegionTriple::new(pid, start, start.saturating_add(len)))
}

fn arb_input() -> impl Strategy<Value = Vec<RegionTriple>> {
    proptest::collection::vec(arb_triple(), 0..=64)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn dedup_is_idempotent(input in arb_input()) {
        let once = dedup_regions_cpu(input.clone());
        let twice = dedup_regions_cpu(once.clone());
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn output_is_sorted(input in arb_input()) {
        let out = dedup_regions_cpu(input);
        for w in out.windows(2) {
            prop_assert!(w[0] <= w[1], "output not sorted: {:?}", w);
        }
    }

    #[test]
    fn no_overlapping_same_pid_in_output(input in arb_input()) {
        let out = dedup_regions_cpu(input);
        for w in out.windows(2) {
            if w[0].pid == w[1].pid {
                prop_assert!(
                    w[0].end < w[1].start,
                    "adjacent same-pid outputs overlap: {:?}", w
                );
            }
        }
    }

    #[test]
    fn dedup_never_invents_pids(input in arb_input()) {
        let input_pids: std::collections::BTreeSet<u32> =
            input.iter().map(|t| t.pid).collect();
        let out = dedup_regions_cpu(input);
        for t in &out {
            prop_assert!(input_pids.contains(&t.pid), "fabricated pid {} in output", t.pid);
        }
    }

    #[test]
    fn dedup_preserves_pid_set(input in arb_input()) {
        let input_pids: std::collections::BTreeSet<u32> =
            input.iter().map(|t| t.pid).collect();
        let out = dedup_regions_cpu(input);
        let out_pids: std::collections::BTreeSet<u32> =
            out.iter().map(|t| t.pid).collect();
        prop_assert_eq!(input_pids, out_pids);
    }

    #[test]
    fn dedup_output_no_larger_than_input(input in arb_input()) {
        let n_in = input.len();
        let n_out = dedup_regions_cpu(input).len();
        prop_assert!(n_out <= n_in);
    }

    #[test]
    fn inplace_matches_owned(input in arb_input()) {
        // The in-place sibling MUST produce the same output as the
        // owned-Vec variant for every input. Locks the contract that
        // performance optimization can't drift from semantics.
        let owned_result = dedup_regions_cpu(input.clone());
        let mut inplace = input;
        dedup_regions_inplace(&mut inplace);
        prop_assert_eq!(owned_result, inplace);
    }
}
