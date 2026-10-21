use super::*;

// ---------------------------------------------------------------------------
// High node counts / multi-word bitsets
// ---------------------------------------------------------------------------

#[test]
fn csr_cpu_ref_33_nodes_two_words() {
    // 33 nodes: frontier on node 32 (second word, bit 0)
    // Node 32 has one edge to node 0
    let mut offsets = vec![0u32; 34];
    for i in 0..34 {
        offsets[i] = if i <= 32 { 0 } else { 1 };
    }
    let mut frontier = zero_frontier(33);
    frontier[1] = 1; // node 32

    let out = csr_cpu_ref(
        33,
        &offsets,
        &[0], // 32 → 0
        &[1],
        &frontier,
        0xFFFF_FFFF,
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], 1, "node 0 set");
    assert_eq!(out[1], 0, "node 32 not in output");
}

#[test]
fn csr_cpu_ref_64_nodes_exactly_two_words() {
    // 64 nodes: frontier on node 63 (word 1, bit 31)
    // Node 63 → node 0
    let mut offsets = vec![0u32; 65];
    for i in 0..65 {
        offsets[i] = if i <= 63 { 0 } else { 1 };
    }
    let mut frontier = zero_frontier(64);
    frontier[1] = 1u32 << 31; // node 63

    let out = csr_cpu_ref(64, &offsets, &[0], &[1], &frontier, 0xFFFF_FFFF);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], 1);
    assert_eq!(out[1], 0);
}

#[test]
fn csr_cpu_ref_65_nodes_three_words() {
    // 65 nodes: frontier on node 64 (word 2, bit 0)
    // Node 64 → node 64 (self-loop)
    let mut offsets = vec![0u32; 66];
    for i in 0..66 {
        offsets[i] = if i <= 64 { 0 } else { 1 };
    }
    let mut frontier = zero_frontier(65);
    frontier[2] = 1; // node 64

    let out = csr_cpu_ref(65, &offsets, &[64], &[1], &frontier, 0xFFFF_FFFF);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0], 0);
    assert_eq!(out[1], 0);
    assert_eq!(out[2], 1, "node 64 self-loop preserved");
}

#[test]
fn csr_cpu_ref_all_nodes_self_loop_preserves_actual_nodes() {
    // 100 nodes, each node has a self-loop. Frontier = all bits set (including garbage).
    // cpu_ref only processes src in 0..node_count, so only real nodes propagate.
    let n = 100u32;
    let words = bitset_words(n);
    let mut offsets = vec![0u32; n as usize + 1];
    for i in 0..=n {
        offsets[i as usize] = i;
    }
    let targets: Vec<u32> = (0..n).collect();
    let masks = vec![1u32; n as usize];
    let frontier = vec![0xFFFF_FFFF; words];

    let out = csr_cpu_ref(n, &offsets, &targets, &masks, &frontier, 0xFFFF_FFFF);
    assert_eq!(out.len(), words);
    // Real nodes 0..99 are preserved via self-loops. Garbage bits 100..127 are NOT
    // preserved because there is no src >= 100 to iterate them.
    assert_eq!(out[0], 0xFFFF_FFFF);
    assert_eq!(out[1], 0xFFFF_FFFF);
    assert_eq!(out[2], 0xFFFF_FFFF);
    assert_eq!(
        out[3], 0x0000_000F,
        "only nodes 96..99 preserved in last word"
    );
}

#[test]
fn validate_high_node_count_zero_edges() {
    // Cannot allocate u32::MAX nodes, but we can test a large count with 0 edges
    // to ensure the validation logic doesn't overflow on large counts.
    let shape = ProgramGraphShape::new(1_000_000, 0);
    // We won't allocate 1M arrays here; instead test shape invariants directly.
    let bufs = shape.read_only_buffers();
    assert_eq!(bufs[0].count(), 1_000_000);
    assert_eq!(bufs[1].count(), 1_000_001);
    assert_eq!(bufs[2].count(), 1); // placeholder
    assert_eq!(bufs[3].count(), 1); // placeholder
    assert_eq!(bufs[4].count(), 1_000_000);
}

