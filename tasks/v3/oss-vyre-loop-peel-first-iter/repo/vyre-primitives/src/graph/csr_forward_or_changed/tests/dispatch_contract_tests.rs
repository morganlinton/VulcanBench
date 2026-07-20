use super::super::*;

#[test]
fn static_input_key_tracks_same_shape_graph_content() {
    let plan = plan_csr_forward_or_changed_launch(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        0xFFFF_FFFF,
        4,
    )
    .expect("Fix: valid CSR should produce a launch plan");
    let first = plan
        .static_input_key(&[0, 1, 2, 3, 3], &[1, 2, 3], &[1, 1, 1])
        .expect("Fix: matching CSR should produce a static input key");
    let changed_targets = plan
        .static_input_key(&[0, 1, 2, 3, 3], &[2, 3, 0], &[1, 1, 1])
        .expect("Fix: same-shape graph content should still be keyable");

    assert_eq!(first.program_key, changed_targets.program_key);
    assert_eq!(first.edge_offsets_hash, changed_targets.edge_offsets_hash);
    assert_eq!(
        first.edge_kind_mask_hash,
        changed_targets.edge_kind_mask_hash
    );
    assert_ne!(first.edge_targets_hash, changed_targets.edge_targets_hash);
    assert_ne!(first, changed_targets);
}

#[test]
fn static_input_key_normalizes_empty_offsets_to_zero_padded_upload() {
    let empty_offsets_plan = plan_csr_forward_or_changed_launch(4, &[], &[], &[], 1, 2)
        .expect("Fix: empty zero-edge CSR shorthand should plan");
    let canonical_offsets_plan =
        plan_csr_forward_or_changed_launch(4, &[0, 0, 0, 0, 0], &[], &[], 1, 2)
            .expect("Fix: canonical zero-edge CSR should plan");
    let empty_key = empty_offsets_plan
        .static_input_key(&[], &[], &[])
        .expect("Fix: empty zero-edge CSR shorthand should key");
    let canonical_key = canonical_offsets_plan
        .static_input_key(&[0, 0, 0, 0, 0], &[], &[])
        .expect("Fix: canonical zero-edge CSR should key");

    assert_eq!(
        empty_offsets_plan.program_key(),
        canonical_offsets_plan.program_key()
    );
    assert_eq!(empty_key, canonical_key);
}

#[test]
fn static_input_key_rejects_edge_count_drift() {
    let plan = plan_csr_forward_or_changed_launch(2, &[], &[], &[], 1, 1)
        .expect("Fix: zero-edge CSR should plan");

    let err = plan
        .static_input_key(&[], &[1], &[1])
        .expect_err("Fix: stale zero-edge plan must reject edge arrays");

    assert!(err.contains("expected 0 edge target"));
}

#[test]
fn seed_copy_reserves_before_mutating_reused_frontier() {
    let mut frontier = vec![0xCAFE_BABEu32];
    let err = copy_csr_forward_seed_frontier_into(
        &[0b0001],
        1,
        &mut frontier,
        |_frontier, _words, _context| Err("injected reservation failure".to_string()),
        |message| message,
    )
    .expect_err("Fix: injected reservation failure should surface");

    assert_eq!(err, "injected reservation failure");
    assert_eq!(frontier, vec![0xCAFE_BABEu32]);
}

#[test]
fn seed_copy_rejects_bad_width_without_mutating_reused_frontier() {
    let mut frontier = vec![0xCAFE_BABEu32];
    let err = copy_csr_forward_seed_frontier_into(
        &[],
        1,
        &mut frontier,
        |_frontier, _words, _context| Ok::<(), String>(()),
        |message| message,
    )
    .expect_err("Fix: bad seed width should be rejected");

    assert!(err.contains("expected seed frontier length 1"));
    assert_eq!(frontier, vec![0xCAFE_BABEu32]);
}

#[test]
fn changed_flag_validation_rejects_non_boolean_values() {
    validate_csr_forward_or_changed_flag(0).expect("Fix: changed=0 is valid");
    validate_csr_forward_or_changed_flag(1).expect("Fix: changed=1 is valid");
    let err =
        validate_csr_forward_or_changed_flag(2).expect_err("Fix: changed flag must be boolean");

    assert!(err.contains("non-boolean changed flag 2"));
}
