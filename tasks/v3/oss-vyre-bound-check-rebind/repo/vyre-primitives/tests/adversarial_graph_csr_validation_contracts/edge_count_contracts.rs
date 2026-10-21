use super::*;

// ---------------------------------------------------------------------------
// edge_offsets last count mismatch
// ---------------------------------------------------------------------------

#[test]
fn validate_passes_when_offsets_last_matches_edge_count() {
    let shape = ProgramGraphShape::new(3, 3);
    let result = validate_program_graph(
        shape,
        &[0, 0, 0],
        &[0, 1, 2, 3], // last == 3 == edge_count
        &[1, 2, 0],
        &[1, 1, 1],
        &[0, 0, 0],
    );
    assert_eq!(result, Ok(()), "offsets.last() == edge_count must validate");
}

#[test]
fn validate_rejects_offsets_last_less_than_edge_count() {
    let shape = ProgramGraphShape::new(3, 3);
    let err = validate_program_graph(
        shape,
        &[0, 0, 0],
        &[0, 1, 2, 2], // last == 2, but edge_count == 3
        &[1, 2, 0],    // len == 3, matches edge_count
        &[1, 1, 1],
        &[0, 0, 0],
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            GraphValidationError::EdgeCountMismatch {
                expected: 3,
                got: 2
            }
        ),
        "offsets[last] < edge_count must be rejected"
    );
}

#[test]
fn validate_rejects_offsets_last_greater_than_edge_count() {
    let shape = ProgramGraphShape::new(3, 2);
    let err = validate_program_graph(
        shape,
        &[0, 0, 0],
        &[0, 1, 2, 5], // last == 5, but edge_count == 2
        &[1, 2],       // len == 2, matches edge_count
        &[1, 1],
        &[0, 0, 0],
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            GraphValidationError::EdgeCountMismatch {
                expected: 2,
                got: 5
            }
        ),
        "offsets[last] > edge_count must be rejected"
    );
}

#[test]
fn csr_cpu_ref_uses_offsets_last_as_authoritative_edge_count() {
    // cpu_ref derives edge_count from edge_offsets.last(), not from a shape parameter.
    // offsets = [0,1,2,2] means node 0 has 1 edge (0→1), node 1 has 1 edge (1→2).
    // Frontier = {0}, so only edge 0 is processed.
    let out = csr_cpu_ref(
        3,
        &[0, 1, 2, 2], // offsets say 2 edges total
        &[1, 2],
        &[1, 1],
        &[0b0001],
        0xFFFF_FFFF,
    );
    assert_eq!(
        out,
        vec![0b0010],
        "node 0 reaches node 1 via its single edge"
    );
}

#[test]
fn csr_cpu_ref_offsets_last_less_than_provided_targets_ignores_extras() {
    // offsets say 1 edge, but we provide 3 targets.
    // cpu_ref only iterates up to offsets.last() == 1.
    let out = csr_cpu_ref(
        3,
        &[0, 1, 1, 1], // last == 1
        &[1, 2, 0],    // 3 targets provided
        &[1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
    );
    assert_eq!(
        out,
        vec![0b0010],
        "only first edge (to node 1) is considered; extras ignored"
    );
}

#[test]
#[should_panic(expected = "complete CSR edge buffers")]
fn csr_cpu_ref_offsets_last_greater_than_provided_targets_fails_loudly() {
    // offsets say 5 edges, but we provide only 2 targets.
    let _ = csr_cpu_ref(
        3,
        &[0, 5, 5, 5], // last == 5
        &[1, 2],       // only 2 targets
        &[1, 1, 1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
    );
}

