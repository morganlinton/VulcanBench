use super::*;

// ---------------------------------------------------------------------------
// Non-monotonic offsets
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_first_offset_nonzero() {
    let shape = ProgramGraphShape::new(2, 1);
    let err = validate_program_graph(
        shape,
        &[0, 0],
        &[1, 1, 1], // first offset must be 0
        &[0],
        &[0],
        &[0, 0],
    )
    .unwrap_err();
    assert!(
        matches!(err, GraphValidationError::NonMonotonicOffsets { index: 0 }),
        "first offset nonzero must be rejected at index 0"
    );
}

#[test]
fn validate_rejects_strictly_decreasing_offsets() {
    let shape = ProgramGraphShape::new(3, 2);
    let err = validate_program_graph(
        shape,
        &[0, 0, 0],
        &[0, 3, 1, 2], // 3 → 1 is a decrease
        &[1, 2],
        &[1, 1],
        &[0, 0, 0],
    )
    .unwrap_err();
    assert!(
        matches!(err, GraphValidationError::NonMonotonicOffsets { index: 1 }),
        "decrease at index 1 must be caught"
    );
}

#[test]
fn validate_rejects_equal_then_decrease_offsets() {
    let shape = ProgramGraphShape::new(4, 2);
    let err = validate_program_graph(
        shape,
        &[0, 0, 0, 0],
        &[0, 2, 2, 1, 2], // equal (ok), then decrease 2→1
        &[1, 2],
        &[1, 1],
        &[0, 0, 0, 0],
    )
    .unwrap_err();
    assert!(
        matches!(err, GraphValidationError::NonMonotonicOffsets { index: 2 }),
        "decrease at index 2 must be caught"
    );
}

#[test]
#[should_panic(expected = "non-monotonic CSR offsets")]
fn csr_cpu_ref_rejects_non_monotonic_edge_start_gt_end() {
    let _ = csr_cpu_ref(
        2,
        &[0, 2, 1],
        &[1, 0],
        &[1, 1],
        &[0b0001], // only node 0 is in frontier
        0xFFFF_FFFF,
    );
}

