use super::*;

#[test]
fn plan_owns_padding_outputs_and_grid() {
    let plan = plan_ifds_csr_dispatch(
        2,
        2,
        2,
        &[(0, 0, 1)],
        &[(0, 1, 1, 0)],
        &[(0, 0, 1)],
        &[(1, 0, 0)],
    )
    .expect("Fix: valid IFDS CSR dispatch plan should build");

    assert_eq!(plan.grid, ifds_csr_dispatch_grid(1, 8));
    assert_eq!(plan.killed_words, 8);
    assert_eq!(plan.intra_field_words, 1);
    assert_eq!(plan.inter_field_words, 1);
    assert_eq!(plan.gen_field_words, 1);
    assert_eq!(plan.kill_field_words, 1);
    assert_eq!(plan.row_ptr_words, 9);
    assert_eq!(plan.row_cursor_words, 8);
    assert_eq!(plan.col_idx_words, 5);
    assert_eq!(plan.col_len_words, 1);
    assert_eq!(plan.max_col_count, 5);
    assert_eq!(
        plan.program_cache_key(),
        IfdsCsrProgramCacheKey {
            num_procs: 2,
            blocks_per_proc: 2,
            facts_per_proc: 2,
            intra_count: 1,
            inter_count: 1,
            gen_count: 1,
            kill_count: 1,
            max_col_count: 5,
        }
    );
    assert!(!plan.layout.empty);
}

#[test]
fn empty_plan_keeps_dispatch_buffers_nonempty_without_fake_rules() {
    let plan = plan_ifds_csr_dispatch(0, 0, 0, &[], &[], &[], &[])
        .expect("Fix: empty no-rule IFDS dispatch plan should be representable");

    assert!(plan.layout.empty);
    assert_eq!(plan.intra_field_words, 1);
    assert_eq!(plan.inter_field_words, 1);
    assert_eq!(plan.gen_field_words, 1);
    assert_eq!(plan.kill_field_words, 1);
    assert_eq!(plan.row_ptr_words, 1);
    assert_eq!(plan.row_cursor_words, 1);
    assert_eq!(plan.col_idx_words, 1);
    assert_eq!(plan.col_len_words, 1);
    assert_eq!(plan.grid, IFDS_CSR_EMPTY_DISPATCH_GRID);
}

#[test]
fn dispatch_grid_stays_single_block_for_lane_zero_builder() {
    let plan = plan_ifds_csr_dispatch(2, 2, 2, &[(0, 0, 1), (0, 1, 0), (1, 0, 1)], &[], &[], &[])
        .expect("Fix: multi-edge IFDS dispatch plan should build");

    assert_eq!(plan.layout.total_nodes, 8);
    assert_eq!(plan.layout.intra_count, 3);
    assert_eq!(plan.grid, ifds_csr_dispatch_grid(3, 8));
    assert_eq!(plan.grid, [1, 1, 1]);
    assert_eq!(plan.program().workgroup_size, IFDS_CSR_WORKGROUP_SIZE);
}

#[test]
fn large_dispatch_plan_does_not_launch_idle_blocks() {
    let intra = (0..513).map(|edge| (0, edge, edge + 1)).collect::<Vec<_>>();
    let plan = plan_ifds_csr_dispatch(1, 515, 4, &intra, &[], &[], &[])
        .expect("Fix: large IFDS CSR dispatch plan should build");

    assert_eq!(plan.layout.total_nodes, 2060);
    assert_eq!(plan.layout.intra_count, 513);
    assert_eq!(plan.grid, [1, 1, 1]);
    assert_eq!(ifds_csr_dispatch_grid(513, 2060), [1, 1, 1]);
}

#[test]
fn rule_input_fingerprint_distinguishes_same_count_rule_content() {
    let base = IfdsCsrRuleInputFingerprint::from_rules(
        &[(0, 0, 1)],
        &[(0, 1, 1, 0)],
        &[(0, 0, 1)],
        &[(1, 0, 0)],
    );

    assert_eq!(
        base,
        IfdsCsrRuleInputFingerprint::from_rules(
            &[(0, 0, 1)],
            &[(0, 1, 1, 0)],
            &[(0, 0, 1)],
            &[(1, 0, 0)],
        )
    );
    assert_ne!(
        base,
        IfdsCsrRuleInputFingerprint::from_rules(
            &[(0, 1, 0)],
            &[(0, 1, 1, 0)],
            &[(0, 0, 1)],
            &[(1, 0, 0)],
        )
    );
    assert_ne!(
        base,
        IfdsCsrRuleInputFingerprint::from_rules(
            &[(0, 0, 1)],
            &[(0, 1, 1, 1)],
            &[(0, 0, 1)],
            &[(1, 0, 0)],
        )
    );
}

#[test]
fn static_input_key_combines_program_shape_and_rule_content() {
    let plan = plan_ifds_csr_dispatch(1, 2, 1, &[(0, 0, 1)], &[], &[], &[])
        .expect("Fix: valid IFDS dispatch plan should build");
    let first = IfdsCsrRuleInputFingerprint::from_rules(&[(0, 0, 1)], &[], &[], &[]);
    let changed = IfdsCsrRuleInputFingerprint::from_rules(&[(0, 1, 0)], &[], &[], &[]);

    assert_eq!(plan.static_input_key(first), plan.static_input_key(first));
    assert_ne!(plan.static_input_key(first), plan.static_input_key(changed));
    assert_eq!(
        plan.static_input_key(first).program_key,
        plan.program_cache_key()
    );
}

#[test]
fn readback_validator_rejects_malformed_csr_outputs() {
    let plan = plan_ifds_csr_dispatch(1, 2, 1, &[(0, 0, 1)], &[], &[], &[])
        .expect("Fix: valid IFDS dispatch plan should build");
    let layout = &plan.layout;

    assert_eq!(
        validate_ifds_csr_readback(layout, &[0, 1, 1], &[1], 1)
            .expect("Fix: canonical readback should validate"),
        1
    );
    assert!(validate_ifds_csr_readback(layout, &[1, 1, 1], &[1], 1)
        .expect_err("Fix: row_ptr[0] drift must be rejected")
        .contains("row_ptr[0]"));
    assert!(validate_ifds_csr_readback(layout, &[0, 1, 0], &[1], 1)
        .expect_err("Fix: nonmonotonic row_ptr must be rejected")
        .contains("not monotonic"));
    assert!(validate_ifds_csr_readback(layout, &[0, 1, 1], &[2], 1)
        .expect_err("Fix: out-of-domain column must be rejected")
        .contains("outside total_nodes"));
}
