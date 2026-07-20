use super::*;

// ---------------------------------------------------------------------------
// Malformed CSR lengths
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_short_edge_offsets() {
    let shape = ProgramGraphShape::new(3, 2);
    let err = validate_program_graph(
        shape,
        &[0, 0, 0],
        &[0, 1, 2], // need 4 entries, got 3
        &[1, 2],
        &[1, 1],
        &[0, 0, 0],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::EdgeOffsetsLen { got: 3, .. }
    ));
}

#[test]
fn validate_rejects_long_edge_offsets() {
    let shape = ProgramGraphShape::new(2, 1);
    let err = validate_program_graph(
        shape,
        &[0, 0],
        &[0, 1, 1, 1], // need 3 entries, got 4
        &[0],
        &[0],
        &[0, 0],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::EdgeOffsetsLen { got: 4, .. }
    ));
}

#[test]
fn validate_rejects_short_edge_targets() {
    let shape = ProgramGraphShape::new(2, 2);
    let err = validate_program_graph(
        shape,
        &[0, 0],
        &[0, 1, 2],
        &[1], // need 2 entries, got 1
        &[1, 1],
        &[0, 0],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::EdgeTargetsLen { got: 1, .. }
    ));
}

#[test]
fn validate_rejects_long_edge_targets() {
    let shape = ProgramGraphShape::new(2, 1);
    let err = validate_program_graph(
        shape,
        &[0, 0],
        &[0, 1, 1],
        &[0, 0], // need 1 entry (max(1,1)=1), but got 2
        &[0],
        &[0, 0],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::EdgeTargetsLen { got: 2, .. }
    ));
}

#[test]
fn validate_rejects_short_edge_kind_mask() {
    let shape = ProgramGraphShape::new(2, 2);
    let err = validate_program_graph(
        shape,
        &[0, 0],
        &[0, 1, 2],
        &[1, 2],
        &[1], // need 2 entries, got 1
        &[0, 0],
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::EdgeKindMaskLen { got: 1, .. }
    ));
}

#[test]
fn validate_rejects_short_nodes() {
    let shape = ProgramGraphShape::new(3, 0);
    let err = validate_program_graph(
        shape,
        &[0, 0], // need 3 entries, got 2
        &[0, 0, 0, 0],
        &[0],
        &[0],
        &[0, 0, 0],
    )
    .unwrap_err();
    assert!(matches!(err, GraphValidationError::NodesLen { got: 2, .. }));
}

#[test]
fn validate_rejects_short_node_tags() {
    let shape = ProgramGraphShape::new(3, 0);
    let err = validate_program_graph(
        shape,
        &[0, 0, 0],
        &[0, 0, 0, 0],
        &[0],
        &[0],
        &[0, 0], // need 3 entries, got 2
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GraphValidationError::NodeTagsLen { got: 2, .. }
    ));
}

#[test]
#[should_panic(expected = "node_count + 1 CSR offsets")]
fn csr_cpu_ref_rejects_short_edge_offsets() {
    let _ = csr_cpu_ref(3, &[0, 1], &[1, 2], &[1, 1], &[0b0001], 0xFFFF_FFFF);
}

#[test]
#[should_panic(expected = "complete CSR edge buffers")]
fn csr_cpu_ref_rejects_short_edge_targets_vs_offsets() {
    // edge_offsets says 2 edges, but edge_targets only has 1
    let _ = csr_cpu_ref(2, &[0, 1, 2], &[0], &[0, 0], &[0b0001], 0xFFFF_FFFF);
}

#[test]
#[should_panic(expected = "complete CSR edge buffers")]
fn csr_cpu_ref_rejects_short_edge_kind_mask_vs_offsets() {
    // edge_offsets says 2 edges, but edge_kind_mask only has 1
    let _ = csr_cpu_ref(2, &[0, 1, 2], &[0, 0], &[0], &[0b0001], 0xFFFF_FFFF);
}

