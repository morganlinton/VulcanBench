use super::*;

mod cpu_reference_tests;
mod dispatch_plan_tests;
mod rule_column_tests;

#[test]
fn encode_decode_roundtrips_at_max_values() {
    let n = encode_node(MAX_PROC_ID, MAX_BLOCK_ID, MAX_FACT_ID).unwrap();
    assert_eq!(n, u32::MAX);
    assert_eq!(decode_node(n), (MAX_PROC_ID, MAX_BLOCK_ID, MAX_FACT_ID));
}

#[test]
fn encode_decode_roundtrips_at_zero() {
    let n = encode_node(0, 0, 0).unwrap();
    assert_eq!(n, 0);
    assert_eq!(decode_node(n), (0, 0, 0));
}

#[test]
fn encode_decode_roundtrips_at_component_boundaries() {
    for (p, b, f) in [
        (0, 0, 1),
        (0, 1, 0),
        (1, 0, 0),
        (0, 0, MAX_FACT_ID),
        (0, MAX_BLOCK_ID, 0),
        (MAX_PROC_ID, 0, 0),
        (1, 2, 3),
        (42, 17, 99),
        (MAX_PROC_ID / 2, MAX_BLOCK_ID / 2, MAX_FACT_ID / 2),
    ] {
        let n = encode_node(p, b, f).unwrap();
        assert_eq!(
            decode_node(n),
            (p, b, f),
            "roundtrip failed for {p}/{b}/{f}"
        );
    }
}

#[test]
fn fits_catches_over_range_components() {
    assert!(fits(MAX_PROC_ID, MAX_BLOCK_ID, MAX_FACT_ID));
    assert!(!fits(MAX_PROC_ID + 1, 0, 0));
    assert!(!fits(0, MAX_BLOCK_ID + 1, 0));
    assert!(!fits(0, 0, MAX_FACT_ID + 1));
    assert_eq!(encode_node(MAX_PROC_ID + 1, 0, 0), None);
}

#[test]
fn csr_of_empty_graph_has_only_sentinel_row_ptr() {
    let (row_ptr, col_idx) = build_cpu_reference(1, 1, 1, &[], &[], &[], &[]);
    assert_eq!(row_ptr, vec![0, 0]);
    assert!(col_idx.is_empty());
}

// Dense-index helper mirrors the one inside build_cpu_reference.
fn di(p: u32, b: u32, f: u32, blocks: u32, facts: u32) -> u32 {
    p * blocks * facts + b * facts + f
}

#[test]
fn csr_single_intra_edge_produces_per_fact_duplicate_edges() {
    // 1 proc, 2 blocks (B0→B1), 4 facts; no kills → each fact
    // flows forward once.
    let (row_ptr, col_idx) = build_cpu_reference(1, 2, 4, &[(0, 0, 1)], &[], &[], &[]);
    assert_eq!(row_ptr.len(), 9);
    assert_eq!(col_idx.len(), 4);
    for f in 0..4 {
        let src = di(0, 0, f, 2, 4) as usize;
        let edge_start = row_ptr[src] as usize;
        assert_eq!(col_idx[edge_start], di(0, 1, f, 2, 4));
    }
}

#[test]
fn csr_kill_suppresses_edge_for_that_fact() {
    let (row_ptr, col_idx) = build_cpu_reference(
        1,
        2,
        4,
        &[(0, 0, 1)],
        &[],
        &[],
        &[(0, 0, 2)], // KILL fact 2 at (0, 0)
    );
    let n_edges: u32 = row_ptr.windows(2).map(|w| w[1] - w[0]).sum();
    assert_eq!(n_edges, 3);
    assert_eq!(col_idx.len(), n_edges as usize);
    let killed_src = di(0, 0, 2, 2, 4) as usize;
    assert_eq!(row_ptr[killed_src + 1] - row_ptr[killed_src], 0);
}

#[test]
fn csr_inter_edges_connect_procs() {
    let (row_ptr, col_idx) = build_cpu_reference(
        2,
        2,
        2,
        &[],
        &[(0, 1, 1, 0)], // call: P0/B1 → P1/B0
        &[],
        &[],
    );
    assert_eq!(row_ptr.len(), 9);
    assert_eq!(col_idx.len(), 2);
    let src0 = di(0, 1, 0, 2, 2) as usize;
    let src1 = di(0, 1, 1, 2, 2) as usize;
    assert_eq!(
        &col_idx[row_ptr[src0] as usize..row_ptr[src0 + 1] as usize],
        &[di(1, 0, 0, 2, 2)]
    );
    assert_eq!(
        &col_idx[row_ptr[src1] as usize..row_ptr[src1 + 1] as usize],
        &[di(1, 0, 1, 2, 2)]
    );
}

#[test]
fn dense_encoded_roundtrips() {
    for &(p, b, f, blocks, facts) in &[
        (0_u32, 0_u32, 0_u32, 2_u32, 2_u32),
        (1, 1, 1, 2, 2),
        (42, 17, 99, 64, 128),
        (MAX_PROC_ID, 3, 7, 16, 16),
    ] {
        let d = di(p, b, f, blocks, facts);
        let enc = dense_to_encoded(d, blocks, facts).unwrap();
        assert_eq!(decode_node(enc), (p, b, f));
        let back = encoded_to_dense(enc, blocks, facts).unwrap();
        assert_eq!(back, d, "roundtrip mismatch {p}/{b}/{f}");
    }
}

#[test]
fn csr_gen_introduces_new_fact_flow_from_zero_fact() {
    // B0 → B1, GEN fact 2 at B0. Per IFDS 0-fact convention,
    // GEN emits edge (B0, 0) → (B1, 2). The intra loop still
    // propagates every non-killed fact (0..3), so total edges
    // are 4 (intra) + 1 (GEN from 0-fact) = 5. The GEN edge
    // specifically targets fact 2 at B1 even though fact 2 did
    // not flow in through any predecessor  -  that is the point.
    let (row_ptr, col_idx) = build_cpu_reference(1, 2, 4, &[(0, 0, 1)], &[], &[(0, 0, 2)], &[]);
    assert_eq!(col_idx.len(), 5);
    // Verify the GEN edge is attached to the 0-fact source, not
    // to fact-2 (which would be redundant with the intra edge).
    let zero_src = di(0, 0, 0, 2, 4) as usize;
    let fact2_dst = di(0, 1, 2, 2, 4);
    let zero_neighbours = &col_idx[row_ptr[zero_src] as usize..row_ptr[zero_src + 1] as usize];
    assert!(zero_neighbours.contains(&fact2_dst));
}

#[test]
fn csr_rejects_dimensions_overflowing_encoding() {
    // (MAX_PROC_ID + 2) × anything overflows PROC_BITS.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let err = std::panic::catch_unwind(|| {
        let _ = build_cpu_reference(MAX_PROC_ID + 2, 1, 1, &[], &[], &[], &[]);
    });
    std::panic::set_hook(previous_hook);

    let payload = err.expect_err("over-encoded IFDS dimensions must fail");
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("<non-string panic>");
    assert!(
        message.contains("exceed packed IFDS limits"),
        "Fix: overflow panic should route through shared IFDS layout validation, got: {message}"
    );
}

#[test]
fn gpu_builder_rejects_row_ptr_count_overflow_without_panic() {
    let program = build_ifds_csr_program(u32::MAX, 1, 1, 0, 0, 0, 0, 0);

    assert!(program.stats().trap());
}

#[test]
fn gpu_builder_source_has_checked_row_ptr_count_without_panics() {
    let source = include_str!("../program_ir.rs");
    let builder_source = source
        .split("pub fn build_ifds_csr_program(")
        .nth(1)
        .expect("Fix: exploded IFDS GPU builder source must be present")
        .split("/// Pack a `(proc_id, block_id, fact_id)` triple")
        .next()
        .expect("Fix: exploded IFDS GPU builder source must precede node packing");

    assert!(
        builder_source.contains("let Some(row_ptr_count)")
            && !builder_source.contains(concat!("panic", "!("))
            && !builder_source.contains(".unwrap_or_else("),
        "Fix: exploded IFDS GPU builder must check row_ptr count and avoid production panics."
    );
}

#[test]
fn row_ptr_length_is_nodes_plus_one() {
    let procs = 3;
    let blocks = 4;
    let facts = 8;
    let (row_ptr, _) = build_cpu_reference(procs, blocks, facts, &[], &[], &[], &[]);
    assert_eq!(
        row_ptr.len(),
        (procs as usize * blocks as usize * facts as usize) + 1
    );
}

#[test]
fn facts_per_workgroup_matches_max_fact_id_plus_one() {
    // G3 docstring claim: lane sizing matches NFA's.
    assert_eq!(FACTS_PER_WORKGROUP as u32, MAX_FACT_ID + 1);
}

#[test]
fn reusable_layout_contract_sizes_dispatch_buffers() {
    let layout = validate_ifds_csr_layout(2, 3, 4, 5, 7, 11).unwrap();

    assert!(!layout.empty);
    assert_eq!(layout.num_procs, 2);
    assert_eq!(layout.blocks_per_proc, 3);
    assert_eq!(layout.facts_per_proc, 4);
    assert_eq!(layout.intra_count, 5);
    assert_eq!(layout.inter_count, 7);
    assert_eq!(layout.gen_count, 11);
    assert_eq!(layout.slots_per_proc, 12);
    assert_eq!(layout.total_nodes, 24);
    assert_eq!(layout.row_words, 25);
    assert_eq!(layout.row_cursor_words, 24);
    assert_eq!(layout.max_col_count, 5 * 4 + 5 * 11 + 7 * 4);
    assert_eq!(layout.col_buffer_words, layout.max_col_count as usize);
}

#[test]
fn reusable_layout_contract_rejects_invalid_domains() {
    let err = validate_ifds_csr_layout(0, 1, 1, 0, 0, 0).unwrap_err();
    assert!(err.starts_with("Fix:") && err.contains("nonzero"));
    let err = validate_ifds_csr_layout(MAX_PROC_ID + 2, 1, 1, 0, 0, 0).unwrap_err();
    assert!(err.starts_with("Fix:") && err.contains("packed IFDS limits"));
    let err = validate_ifds_csr_layout(MAX_PROC_ID + 1, MAX_BLOCK_ID + 1, MAX_FACT_ID + 1, 0, 0, 0)
        .unwrap_err();
    assert!(
        err.starts_with("Fix:"),
        "oversized IFDS domain must fail validation: {err}"
    );
    let err = validate_ifds_csr_layout(u32::MAX, u32::MAX, 2, 0, 0, 0).unwrap_err();
    assert!(err.starts_with("Fix:"), "huge dimensions must fail: {err}");
    let err = validate_ifds_csr_layout(1, 1, 2, u32::MAX, 0, u32::MAX).unwrap_err();
    assert!(err.starts_with("Fix:"), "huge rule counts must fail: {err}");
}

#[test]
fn reusable_input_layout_contract_narrows_rule_counts_and_padding() {
    let layout = validate_ifds_csr_inputs(
        1,
        2,
        3,
        &[(0, 0, 1), (0, 1, 0)],
        &[(0, 0, 0, 1)],
        &[],
        &[(0, 0, 1)],
    )
    .unwrap();

    assert_eq!(layout.intra_count, 2);
    assert_eq!(layout.inter_count, 1);
    assert_eq!(layout.gen_count, 0);
    assert_eq!(layout.kill_count, 1);
    assert_eq!(layout.intra_storage_words, 2);
    assert_eq!(layout.inter_storage_words, 1);
    assert_eq!(layout.gen_storage_words, 1);
    assert_eq!(layout.kill_storage_words, 1);

    let empty = validate_ifds_csr_inputs(0, 0, 0, &[], &[], &[], &[]).unwrap();
    assert!(empty.empty);
    assert_eq!(empty.row_words, 1);

    let err = validate_ifds_csr_inputs(0, 0, 0, &[(0, 0, 0)], &[], &[], &[]).unwrap_err();
    assert!(err.contains("empty dimensions cannot carry rules"));
}

#[test]
fn reusable_canonicalizer_sorts_rows_and_rejects_bad_ranges() {
    let row_ptr = vec![0, 3, 5];
    let mut col_idx = vec![9, 1, 4, 8, 2];
    canonicalize_csr_within_rows_in_place(&row_ptr, &mut col_idx).unwrap();
    assert_eq!(col_idx, vec![1, 4, 9, 2, 8]);

    let mut bad_col = vec![1, 2];
    let err = canonicalize_csr_within_rows_in_place(&[0, 3], &mut bad_col).unwrap_err();
    assert!(err.contains("exceeds col_idx.len()"));
}
