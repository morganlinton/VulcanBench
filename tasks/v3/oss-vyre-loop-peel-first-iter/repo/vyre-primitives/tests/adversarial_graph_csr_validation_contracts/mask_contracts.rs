use super::*;

// ---------------------------------------------------------------------------
// Edge mask filtering
// ---------------------------------------------------------------------------

#[test]
fn csr_cpu_ref_edge_mask_zero_allow_mask_blocks_all() {
    let out = csr_cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[1, 1, 1, 1],
        &[0b0001],
        0, // allow_mask == 0
    );
    assert_eq!(out, vec![0], "zero allow_mask must block every edge");
}

#[test]
fn csr_cpu_ref_edge_mask_zero_kind_mask_blocks_all() {
    let out = csr_cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[0, 0, 0, 0], // every edge has mask 0
        &[0b0001],
        0xFFFF_FFFF,
    );
    assert_eq!(out, vec![0], "zero edge_kind_mask must block every edge");
}

#[test]
fn csr_cpu_ref_edge_mask_partial_filter() {
    // Graph: 0→1 (mask 0b01), 0→2 (mask 0b10), 1→3 (mask 0b01), 2→3 (mask 0b01)
    let out = csr_cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[0b01, 0b10, 0b01, 0b01],
        &[0b0001], // frontier = {0}
        0b01,      // only allow 0b01 edges
    );
    assert_eq!(out, vec![0b0010], "only node 1 reached via allowed edge");
}

#[test]
fn csr_cpu_ref_edge_mask_no_overlap() {
    // Every edge has mask 0b1000, allow_mask is 0b0100 → no overlap
    let out = csr_cpu_ref(
        3,
        &[0, 1, 2, 2],
        &[1, 2],
        &[0b1000, 0b1000],
        &[0b0001],
        0b0100,
    );
    assert_eq!(out, vec![0], "no overlapping bits → empty frontier");
}

#[test]
fn csr_cpu_ref_edge_mask_multi_source_mixed() {
    // Graph: 0→1 (mask 0b01), 1→2 (mask 0b10)
    // Frontier {0,1}, allow 0b01 → only 0→1 contributes
    let out = csr_cpu_ref(
        3,
        &[0, 1, 2, 2],
        &[1, 2],
        &[0b01, 0b10],
        &[0b0011], // nodes 0 and 1
        0b01,
    );
    assert_eq!(out, vec![0b0010], "only node 1 reached");
}

#[test]
fn validate_rejects_wrong_edge_kind_mask_len_for_zero_edges() {
    // shape.edge_count == 0 → expected len == 1 (placeholder)
    let shape = ProgramGraphShape::new(1, 0);
    let err = validate_program_graph(
        shape,
        &[0],
        &[0, 0],
        &[0],
        &[], // empty instead of placeholder 1
        &[0],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::EdgeKindMaskLen { got: 0, .. }
    ));
}

