use super::*;

#[test]
fn csr_forward_or_changed_expands_frontier() {
    let (frontier, changed) = csr_cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[1, 1, 1, 1],
        &[0b0001],
        1,
    );
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(changed, 1);
}

#[test]
fn csr_forward_or_changed_no_change_when_frontier_unchanged() {
    let (frontier, changed) = csr_cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[1, 1, 1, 1],
        &[0b1111],
        1,
    );
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(changed, 0, "saturated frontier must signal no change");
}

#[test]
fn csr_forward_or_changed_empty_frontier() {
    let (frontier, changed) =
        csr_cpu_ref(4, &[0, 2, 3, 4, 4], &[1, 2, 3, 3], &[1, 1, 1, 1], &[0], 1);
    assert_eq!(frontier, vec![0]);
    assert_eq!(changed, 0);
}

#[test]
fn csr_forward_or_changed_edge_mask_blocks() {
    let (frontier, changed) = csr_cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[0b10, 0b01, 0b01, 0b01],
        &[0b0001],
        0b01,
    );
    // In-place expansion: node 0 adds node 2 (allowed edge), then node 2
    // (now set in the same buffer) adds node 3, producing {0,2,3}.
    assert_eq!(
        frontier,
        vec![0b1101],
        "in-place expansion cascades within one pass"
    );
    assert_eq!(changed, 1);
}

#[test]
fn csr_forward_or_changed_zero_nodes() {
    // A 0-node graph still yields a ONE-word frontier: the layout computes
    // frontier_words = bitset_words(0).max(1) = 1 (csr_forward_or_changed/
    // validate.rs:66), a deliberate min-1-word invariant shared with
    // node_words/edge_storage_words so GPU buffer bindings are never
    // zero-sized. frontier_words drives the launch-plan buffer size, so the GPU
    // writes one cleared word; the CPU oracle MUST mirror that layout. The
    // expanded frontier is therefore a single zero word, not empty, with no
    // change because there are no source nodes to expand from.
    let (frontier, changed) = csr_cpu_ref(0, &[0], &[], &[], &[], 1);
    assert_eq!(frontier, vec![0]);
    assert_eq!(changed, 0);
}

// ---------------------------------------------------------------------------
// Dominator frontier
// ---------------------------------------------------------------------------

#[test]
fn dominator_frontier_empty_seed_empty_frontier() {
    let out = dom_cpu_ref(4, &[0, 0, 0, 0, 0], &[], &[0, 0, 0, 0, 0], &[], &[0]);
    assert_eq!(out, vec![0]);
}

#[test]
fn dominator_frontier_single_node_no_predecessors() {
    let out = dom_cpu_ref(2, &[0, 0, 0], &[], &[0, 0, 0], &[], &[0b01]);
    assert_eq!(out, vec![0]);
}

#[test]
fn dominator_frontier_join_node_appears() {
    // CFG: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
    let pred_offsets = vec![0u32, 0, 1, 2, 4];
    let pred_targets = vec![0u32, 0, 1, 2];
    // Dominator sets: 0 dominates everyone; 1 dominates {1}; 2 dominates {2}; 3 dominates {3}
    let dom_offsets = vec![0u32, 4, 5, 6, 7];
    let dom_targets = vec![0u32, 1, 2, 3, 1, 2, 3];
    let out = dom_cpu_ref(
        4,
        &dom_offsets,
        &dom_targets,
        &pred_offsets,
        &pred_targets,
        &[0b0010],
    );
    assert_eq!(out, vec![0b1000], "df(1) must include join node 3");
}

#[test]
#[should_panic(expected = "expected seed length 1 words for 2 nodes")]
fn dominator_frontier_missing_seed_fails_loudly() {
    // An empty seed for a 2-node graph is incomplete: the bitset needs
    // ceil(2/32) = 1 word. The oracle fails loud naming the EXACT shortfall
    // ("expected seed length 1 words for 2 nodes, got 0") rather than a vague
    // phrase, so the contract assertion pins the real required/actual counts.
    let _ = dom_cpu_ref(2, &[0, 0, 0], &[], &[0, 0, 0], &[], &[]);
}

// ---------------------------------------------------------------------------
// Fixpoint convergence invariants
// ---------------------------------------------------------------------------
