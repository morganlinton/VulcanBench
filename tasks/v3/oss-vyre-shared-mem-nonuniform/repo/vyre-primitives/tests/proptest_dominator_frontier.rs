//! Property gates for `vyre_primitives::graph::dominator_frontier::cpu_ref`.

#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::graph::dominator_frontier::cpu_ref;

/// Diamond CFG: 0 -> {1,2}, 1 -> 3, 2 -> 3.
/// Dominator sets (by dominator): 0->{0,1,2,3}, 1->{1}, 2->{2}, 3->{3}.
fn diamond_doms() -> (Vec<u32>, Vec<u32>) {
    let offsets = vec![0, 4, 5, 6, 7];
    let targets = vec![0, 1, 2, 3, 1, 2, 3];
    (offsets, targets)
}

fn diamond_preds() -> (Vec<u32>, Vec<u32>) {
    let offsets = vec![0, 0, 1, 2, 4];
    let targets = vec![0, 0, 1, 2];
    (offsets, targets)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn empty_seed_yields_empty_frontier(_dummy in 0..1) {
        let _ = _dummy;
        let (dom_offsets, dom_targets) = diamond_doms();
        let (pred_offsets, pred_targets) = diamond_preds();
        let seed = vec![0u32];
        let out = cpu_ref(4, &dom_offsets, &dom_targets, &pred_offsets, &pred_targets, &seed);
        prop_assert_eq!(out, vec![0u32]);
    }

    #[test]
    fn seed_node_one_frontier_is_node_three(_dummy in 0..1) {
        let _ = _dummy;
        let (dom_offsets, dom_targets) = diamond_doms();
        let (pred_offsets, pred_targets) = diamond_preds();
        let seed = vec![0b0010]; // node 1
        let out = cpu_ref(4, &dom_offsets, &dom_targets, &pred_offsets, &pred_targets, &seed);
        // node 3 is in DF(1)
        prop_assert_eq!(out, vec![0b1000]);
    }

    #[test]
    fn seed_node_two_frontier_is_node_three(_dummy in 0..1) {
        let _ = _dummy;
        let (dom_offsets, dom_targets) = diamond_doms();
        let (pred_offsets, pred_targets) = diamond_preds();
        let seed = vec![0b0100]; // node 2
        let out = cpu_ref(4, &dom_offsets, &dom_targets, &pred_offsets, &pred_targets, &seed);
        // node 3 is in DF(2)
        prop_assert_eq!(out, vec![0b1000]);
    }

    #[test]
    fn seed_entry_node_frontier_is_empty(_dummy in 0..1) {
        let _ = _dummy;
        let (dom_offsets, dom_targets) = diamond_doms();
        let (pred_offsets, pred_targets) = diamond_preds();
        let seed = vec![0b0001]; // node 0 (entry)
        let out = cpu_ref(4, &dom_offsets, &dom_targets, &pred_offsets, &pred_targets, &seed);
        // entry strictly dominates everything, so no frontier
        prop_assert_eq!(out, vec![0u32]);
    }
}
