//! Single-source guard for self-substrate graph frontier helpers.

#[test]
fn graph_frontier_production_helpers_delegate_to_primitive_bitset_frontier() {
    let source = include_str!("../src/graph/frontier.rs");
    let production = source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("graph frontier source must have a production section");

    for required in [
        "vyre_primitives::bitset::frontier as primitive_frontier",
        "primitive_frontier::checked_frontier_popcount(frontier)",
        "primitive_frontier::absorb_new_frontier_bits(node_count, visited, neighbors, next_wave)",
    ] {
        assert!(
            production.contains(required),
            "Fix: self-substrate graph frontier helpers must delegate through primitive frontier authority `{required}`."
        );
    }

    for forbidden in [
        "for &word in frontier",
        "word.count_ones()",
        "1u32 << tail_bits",
        "visited_word |= new_bits",
        "neighbor_word & tail_mask",
    ] {
        assert!(
            !production.contains(forbidden),
            "Fix: self-substrate graph frontier production code must not fork primitive frontier logic through `{forbidden}`."
        );
    }
}
