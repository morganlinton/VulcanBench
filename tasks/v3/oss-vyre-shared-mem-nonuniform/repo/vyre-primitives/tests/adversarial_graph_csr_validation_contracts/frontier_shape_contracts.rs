use super::*;

// ---------------------------------------------------------------------------
// Property-style invariants
// ---------------------------------------------------------------------------

#[test]
fn csr_cpu_ref_empty_frontier_invariant() {
    // For any graph, empty frontier → empty output.
    for n in [1, 2, 5, 32, 33, 64, 65] {
        let offsets = vec![0u32; n + 1];
        let frontier = zero_frontier(n as u32);
        let out = csr_cpu_ref(
            n as u32,
            &offsets,
            &[0], // placeholder
            &[0], // placeholder
            &frontier,
            0xFFFF_FFFF,
        );
        assert_eq!(
            out, frontier,
            "empty frontier must produce empty output for n={n}"
        );
    }
}

#[test]
fn csr_cpu_ref_garbage_frontier_bits_not_propagated_beyond_node_count() {
    // 35 nodes → 2 words. Input frontier has garbage bits 35..63 set in word 1.
    // cpu_ref starts output at zero and only ORs from edges, so garbage bits
    // do not appear in output.
    let n = 35u32;
    let mut offsets = vec![0u32; n as usize + 1];
    for i in 0..=n {
        offsets[i as usize] = i;
    }
    let targets: Vec<u32> = (0..n).collect();
    let masks = vec![1u32; n as usize];
    let frontier = vec![0xFFFF_FFFF; 2];

    let out = csr_cpu_ref(n, &offsets, &targets, &masks, &frontier, 0xFFFF_FFFF);
    // Self-loops preserve real nodes 0..34. Bits 35..63 are not set.
    assert_eq!(out[0], 0xFFFF_FFFF);
    assert_eq!(
        out[1], 0x0000_0007,
        "only nodes 32,33,34 preserved; bits 35..63 zero"
    );
}

#[test]
fn csr_cpu_ref_frontier_word_oob_is_safely_skipped() {
    // frontier_in has fewer words than needed, but cpu_ref checks word_idx < len.
    let out = csr_cpu_ref(
        40,
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        &[0],
        &[0],
        &[0], // only 1 word for 40 nodes (need 2)
        0xFFFF_FFFF,
    );
    assert_eq!(out, vec![0, 0], "short frontier must be safely handled");
}

#[test]
fn csr_cpu_ref_monotonic_in_allow_mask() {
    // Larger allow_mask cannot block more edges than a smaller one.
    let offsets = &[0, 2, 2, 2];
    let targets = &[1, 2];
    let masks = &[0b01, 0b10];
    let frontier = &[0b0001];

    let out_narrow = csr_cpu_ref(3, offsets, targets, masks, frontier, 0b01);
    let out_wide = csr_cpu_ref(3, offsets, targets, masks, frontier, 0b11);

    // out_wide must be a superset of out_narrow (bitwise)
    assert_eq!(out_narrow, vec![0b0010]);
    assert_eq!(out_wide, vec![0b0110]);
    assert!(
        (out_wide[0] & out_narrow[0]) == out_narrow[0],
        "wider allow_mask must be superset of narrower"
    );
}

#[test]
fn program_graph_shape_new_roundtrip() {
    let s = ProgramGraphShape::new(42, 99);
    assert_eq!(s.node_count, 42);
    assert_eq!(s.edge_count, 99);
}

#[test]
fn program_graph_shape_read_only_buffers_nonzero_edge() {
    let s = ProgramGraphShape::new(5, 3);
    let bufs = s.read_only_buffers();
    assert_eq!(bufs[2].count(), 3);
    assert_eq!(bufs[3].count(), 3);
}

#[test]
fn program_graph_shape_read_only_buffers_zero_edge_placeholder() {
    let s = ProgramGraphShape::new(5, 0);
    let bufs = s.read_only_buffers();
    assert_eq!(bufs[2].count(), 1);
    assert_eq!(bufs[3].count(), 1);
}

