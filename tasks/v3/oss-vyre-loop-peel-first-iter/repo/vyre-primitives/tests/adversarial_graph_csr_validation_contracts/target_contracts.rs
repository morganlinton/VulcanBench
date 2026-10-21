use super::*;

// ---------------------------------------------------------------------------
// OOB targets
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_target_equal_to_node_count() {
    let shape = ProgramGraphShape::new(2, 1);
    let err = validate_program_graph(
        shape,
        &[0, 0],
        &[0, 1, 1],
        &[2], // == node_count
        &[0],
        &[0, 0],
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            GraphValidationError::EdgeOutOfRange {
                target: 2,
                node_count: 2,
                ..
            }
        ),
        "target == node_count must be OOB"
    );
}

#[test]
fn validate_rejects_target_u32_max() {
    let shape = ProgramGraphShape::new(2, 1);
    let err =
        validate_program_graph(shape, &[0, 0], &[0, 1, 1], &[u32::MAX], &[0], &[0, 0]).unwrap_err();
    assert!(
        matches!(
            err,
            GraphValidationError::EdgeOutOfRange {
                target: u32::MAX,
                ..
            }
        ),
        "u32::MAX target must be OOB"
    );
}

#[test]
fn csr_cpu_ref_oob_target_equal_to_node_count_when_fits_in_word() {
    let out = csr_cpu_ref(
        2,
        &[0, 2, 2],
        &[1, 2], // target 2 == node_count, but fits in word 0
        &[1, 1],
        &[0b0001],
        0xFFFF_FFFF,
    );
    assert_eq!(out, vec![0b0010], "cpu_ref must drop dst == node_count");
}

#[test]
fn csr_cpu_ref_oob_target_dropped_when_dst_word_exceeds_out_len() {
    // For node_count=2, dst=32 maps to word 1, which is >= out.len()=1, so dropped.
    let out = csr_cpu_ref(2, &[0, 2, 2], &[1, 32], &[1, 1], &[0b0001], 0xFFFF_FFFF);
    assert_eq!(out, vec![0b0010], "dst=32 dropped because word 1 is OOB");
}

#[test]
fn csr_cpu_ref_oob_target_u32_max_silently_dropped() {
    let out = csr_cpu_ref(
        2,
        &[0, 2, 2],
        &[1, u32::MAX],
        &[1, 1],
        &[0b0001],
        0xFFFF_FFFF,
    );
    assert_eq!(
        out,
        vec![0b0010],
        "u32::MAX destination must be silently ignored"
    );
}

#[test]
fn csr_cpu_ref_oob_target_with_multiword_bitset() {
    // 40 nodes across 2 words; frontier on node 0; edge to node 39 (valid) and 40 (OOB)
    let mut offsets = vec![2u32; 41];
    offsets[0] = 0;
    offsets[1] = 2;
    let mut frontier = zero_frontier(40);
    frontier[0] = 1; // node 0 set
    let out = csr_cpu_ref(
        40,
        &offsets,
        &[39, 40], // 39 valid, 40 == node_count must be dropped
        &[1, 1],
        &frontier,
        0xFFFF_FFFF,
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], 0);
    assert_eq!(
        out[1],
        1u32 << 7,
        "node 39 is in second word, bit 7; node 40 must be dropped"
    );
}

