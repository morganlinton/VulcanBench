use super::*;
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::optimizer::dispatcher::oracle::CpuOracleDispatcher;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use std::sync::Mutex;
use vyre_primitives::graph::exploded::build_cpu_reference;

mod support;

use support::{canonical_expected, MalformedIfdsDispatcher, RecordingIfdsOracle};

/// Two procs, 2 blocks each, 2 facts each. One intra edge per
/// proc, no inter, no flow. The CSR row count must equal the
/// total node count.
#[test]
fn csr_row_count_matches_node_count() {
    let (row_ptr, _) = reference_build_ifds_csr(2, 2, 2, &[(0, 0, 1), (1, 0, 1)], &[], &[], &[]);
    // Total = 2 * 2 * 2 = 8.
    assert_eq!(row_ptr.len(), 9);
    assert_eq!(ifds_node_count(2, 2, 2), 8);
}

/// Closure-bar: substrate output equals primitive output.
#[test]
fn matches_primitive_directly() {
    let intra = vec![(0, 0, 1), (1, 0, 1)];
    let inter = vec![(0, 1, 1, 0)];
    let gen_edges = vec![(0, 0, 1)];
    let kill = vec![(1, 0, 0)];
    let via_substrate = reference_build_ifds_csr(2, 2, 2, &intra, &inter, &gen_edges, &kill);
    let via_primitive = build_cpu_reference(2, 2, 2, &intra, &inter, &gen_edges, &kill);
    assert_eq!(via_substrate, via_primitive);
}

/// Empty IFDS domains are invalid: parity/reference graph construction
/// needs a real exploded-supergraph domain, not a fake host-side empty CSR.
#[test]
fn empty_graph_rejects_zero_domain() {
    let message = try_reference_build_ifds_csr(0, 0, 0, &[], &[], &[], &[])
        .expect_err("empty IFDS reference domain must fail");
    assert!(
        message.contains("exploded IFDS CPU reference dimensions must be nonzero"),
        "Fix: empty-domain rejection must remain explicit, got: {message}"
    );
}

/// Adversarial: KILL must suppress fact propagation along an
/// intra edge. (proc 0, block 0, fact 1) is killed → no edge
/// emitted from (0, 0, 1) to (0, 1, 1).
#[test]
fn kill_suppresses_fact_propagation() {
    let intra = vec![(0, 0, 1)];
    let kill = vec![(0, 0, 1)];
    let (row_ptr, col_idx) = reference_build_ifds_csr(1, 2, 2, &intra, &[], &[], &kill);
    // Node (0, 0, 1) is at dense index 0 * 4 + 0 * 2 + 1 = 1.
    let src = 1usize;
    let row_start = row_ptr[src] as usize;
    let row_end = row_ptr[src + 1] as usize;
    let neighbors = &col_idx[row_start..row_end];
    // Should have NO edge to (0, 1, 1) (= dense 0*4 + 1*2 + 1 = 3).
    assert!(!neighbors.contains(&3), "killed fact must not propagate");
}

/// Adversarial: GEN must inject the new fact along the intra
/// edge. (proc 0, block 0, gen fact 1) → edge from (0, 0, 0)
/// (the 0-fact) to (0, 1, 1).
#[test]
fn gen_injects_new_fact() {
    let intra = vec![(0, 0, 1)];
    let gen_edges = vec![(0, 0, 1)];
    let (row_ptr, col_idx) = reference_build_ifds_csr(1, 2, 2, &intra, &[], &gen_edges, &[]);
    // 0-fact at (0, 0, 0) → dense index 0.
    let row_start = row_ptr[0] as usize;
    let row_end = row_ptr[1] as usize;
    let neighbors = &col_idx[row_start..row_end];
    // Edge to (0, 1, 1) → dense 3.
    assert!(neighbors.contains(&3), "gen must emit edge to new fact");
}

/// Round-trip dense ↔ encoded must be identity for valid indices.
#[test]
fn round_trip_dense_is_identity() {
    let blocks_per_proc = 4;
    let facts_per_proc = 8;
    for dense in 0..32 {
        assert_eq!(
            round_trip_dense(dense, blocks_per_proc, facts_per_proc),
            Some(dense)
        );
    }
}

/// Adversarial: inter-procedural edge propagates EVERY fact
/// (IFDS upper bound). For 2 facts, expect 2 edges from
/// (sp, sb, *) to (dp, db, *).
#[test]
fn inter_edge_propagates_every_fact() {
    let inter = vec![(0, 0, 1, 1)];
    let (row_ptr, col_idx) = reference_build_ifds_csr(2, 2, 2, &[], &inter, &[], &[]);
    let dense_src_f0 = 0; // (0, 0, 0)
    let dense_src_f1 = 1; // (0, 0, 1)
    let row0 = &col_idx[row_ptr[dense_src_f0] as usize..row_ptr[dense_src_f0 + 1] as usize];
    let row1 = &col_idx[row_ptr[dense_src_f1] as usize..row_ptr[dense_src_f1 + 1] as usize];
    // (1, 1, 0) = 1*4 + 1*2 + 0 = 6
    // (1, 1, 1) = 1*4 + 1*2 + 1 = 7
    assert!(row0.contains(&6), "fact 0 must propagate via inter edge");
    assert!(row1.contains(&7), "fact 1 must propagate via inter edge");
}

#[test]
fn via_decodes_exact_csr_outputs_into_reused_buffers() {
    let dispatcher = CpuOracleDispatcher::new();
    let intra = [(0, 0, 1)];
    let expected = canonical_expected(1, 2, 1, &intra, &[], &[], &[]);
    let mut row_ptr = Vec::with_capacity(4);
    let mut col_idx = Vec::with_capacity(4);
    let row_ptr_ptr = row_ptr.as_ptr();
    let col_idx_ptr = col_idx.as_ptr();
    build_ifds_csr_via_into(
        &dispatcher,
        1,
        2,
        1,
        &intra,
        &[],
        &[],
        &[],
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: CPU oracle IFDS dispatch succeeds");
    assert_eq!((row_ptr.clone(), col_idx.clone()), expected);
    assert_eq!(row_ptr.as_ptr(), row_ptr_ptr);
    assert_eq!(col_idx.as_ptr(), col_idx_ptr);
}

#[test]
fn via_refreshes_static_rule_inputs_for_same_shape_rule_content_change() {
    let dispatcher = RecordingIfdsOracle {
        inner: CpuOracleDispatcher::new(),
        intra_src_blocks: Mutex::new(Vec::new()),
    };
    let mut scratch = IfdsCsrGpuScratch::default();
    let mut row_ptr = Vec::new();
    let mut col_idx = Vec::new();

    build_ifds_csr_via_with_scratch_into(
        &dispatcher,
        1,
        2,
        1,
        &[(0, 0, 1)],
        &[],
        &[],
        &[],
        &mut scratch,
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: first IFDS same-shape dispatch should succeed");
    build_ifds_csr_via_with_scratch_into(
        &dispatcher,
        1,
        2,
        1,
        &[(0, 1, 0)],
        &[],
        &[],
        &[],
        &mut scratch,
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: second IFDS same-shape dispatch should refresh rule columns");

    let recorded = dispatcher
        .intra_src_blocks
        .lock()
        .expect("Fix: IFDS recording mutex should not be poisoned");
    assert_eq!(recorded.as_slice(), &[vec![0], vec![1]]);
    assert_eq!(
        scratch.program_builds(),
        1,
        "Fix: same-count IFDS rule changes should refresh static rule inputs without rebuilding the generated Program."
    );
}

#[test]
fn via_with_scratch_reuses_split_dispatch_decode_and_output_storage() {
    let dispatcher = CpuOracleDispatcher::new();
    let mut scratch = IfdsCsrGpuScratch::default();
    let mut row_ptr = Vec::with_capacity(3);
    let mut col_idx = Vec::with_capacity(1);
    let first_intra = [(0, 0, 1)];
    let second_intra = [(0, 1, 0)];
    let two_edge_intra = [(0, 0, 1), (0, 1, 0)];

    build_ifds_csr_via_with_scratch_into(
        &dispatcher,
        1,
        2,
        1,
        &first_intra,
        &[],
        &[],
        &[],
        &mut scratch,
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: dispatch succeeds");

    let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
    let intra_proc_capacity = scratch.rule_columns.intra_proc.capacity();
    let row_cursor_capacity = scratch.row_cursor.capacity();
    let col_len_capacity = scratch.col_len_words.capacity();
    let row_ptr_capacity = row_ptr.capacity();
    let col_idx_capacity = col_idx.capacity();
    assert_eq!(
        scratch.program_builds(),
        1,
        "first non-empty IFDS dispatch should materialize one primitive Program"
    );

    build_ifds_csr_via_with_scratch_into(
        &dispatcher,
        1,
        2,
        1,
        &first_intra,
        &[],
        &[],
        &[],
        &mut scratch,
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: dispatch succeeds");

    assert_eq!(
        scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
        input_capacities
    );
    assert_eq!(
        scratch.rule_columns.intra_proc.capacity(),
        intra_proc_capacity
    );
    assert_eq!(scratch.row_cursor.capacity(), row_cursor_capacity);
    assert_eq!(scratch.col_len_words.capacity(), col_len_capacity);
    assert_eq!(row_ptr.capacity(), row_ptr_capacity);
    assert_eq!(col_idx.capacity(), col_idx_capacity);
    assert_eq!(
        (row_ptr.clone(), col_idx.clone()),
        canonical_expected(1, 2, 1, &first_intra, &[], &[], &[])
    );
    assert_eq!(
        scratch.program_builds(),
        1,
        "same IFDS program shape should reuse the primitive generated Program"
    );

    build_ifds_csr_via_with_scratch_into(
        &dispatcher,
        1,
        2,
        1,
        &second_intra,
        &[],
        &[],
        &[],
        &mut scratch,
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: same-shape dispatch with different rule values succeeds");
    assert_eq!(
        scratch.program_builds(),
        1,
        "IFDS program cache key must depend on primitive shape, not rule values"
    );

    build_ifds_csr_via_with_scratch_into(
        &dispatcher,
        1,
        2,
        1,
        &two_edge_intra,
        &[],
        &[],
        &[],
        &mut scratch,
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: changed IFDS rule count should still dispatch");
    assert_eq!(
        scratch.program_builds(),
        2,
        "changed IFDS program shape must materialize a new primitive Program"
    );
}

#[test]
fn production_source_uses_primitive_rule_column_scratch_not_local_splitters() {
    let source = format!(
        "{}\n{}",
        include_str!("../mod.rs"),
        include_str!("../dispatch.rs")
    );

    assert!(source.contains("IfdsCsrRuleColumns"));
    assert!(source.contains(".rule_columns"));
    assert!(source.contains(".prepare(intra_edges, inter_edges, flow_gen, flow_kill)"));
    assert!(!source.contains(concat!("fn split", "3_into")));
    assert!(!source.contains(concat!("fn split", "4_into")));
    assert!(!source.contains("split_ifds_rule_triples_into as"));
    assert!(!source.contains("split_ifds_rule_quads_into as"));
}

#[test]
fn production_source_uses_primitive_static_key_and_readback_validation() {
    let source = format!(
        "{}\n{}",
        include_str!("../mod.rs"),
        include_str!("../dispatch.rs")
    );

    assert!(source.contains("IfdsCsrStaticInputKey"));
    assert!(source.contains("plan.static_input_key(rule_fingerprint)"));
    assert!(source.contains("validate_ifds_csr_readback"));
    assert!(!source.contains("struct IfdsCsrStaticInputKey"));
}

#[test]
fn empty_via_path_does_not_materialize_program_or_dispatch() {
    let dispatcher = CpuOracleDispatcher::new();
    let mut scratch = IfdsCsrGpuScratch::default();
    let mut row_ptr = vec![99];
    let mut col_idx = vec![88];

    build_ifds_csr_via_with_scratch_into(
        &dispatcher,
        0,
        0,
        0,
        &[],
        &[],
        &[],
        &[],
        &mut scratch,
        &mut row_ptr,
        &mut col_idx,
    )
    .expect("Fix: empty no-rule IFDS dispatch should complete without backend work");

    assert_eq!(row_ptr, vec![0]);
    assert!(col_idx.is_empty());
    assert_eq!(
        scratch.program_builds(),
        0,
        "empty IFDS plan should not build a generated Program"
    );
    assert!(
        scratch.inputs.is_empty(),
        "empty IFDS plan should not prepare upload buffers"
    );
}

#[test]
fn via_matches_reference_on_generated_ifds_graphs() {
    let dispatcher = CpuOracleDispatcher::new();
    for case in 0..512usize {
        let num_procs = 1 + (case % 3) as u32;
        let blocks_per_proc = 1 + ((case / 3) % 5) as u32;
        let facts_per_proc = 1 + ((case / 15) % 5) as u32;
        let mut intra_edges = Vec::new();
        let mut inter_edges = Vec::new();
        let mut flow_gen = Vec::new();
        let mut flow_kill = Vec::new();

        for p in 0..num_procs {
            for b in 0..blocks_per_proc {
                let next_b = (b + 1) % blocks_per_proc;
                let mixed = case
                    .wrapping_mul(37)
                    .wrapping_add((p as usize).wrapping_mul(11))
                    .wrapping_add((b as usize).wrapping_mul(7));
                if blocks_per_proc > 1 && mixed % 2 == 0 {
                    intra_edges.push((p, b, next_b));
                }
                let fact = (mixed as u32) % facts_per_proc;
                if mixed % 3 == 0 {
                    flow_gen.push((p, b, fact));
                }
                if mixed % 5 == 0 && fact != 0 {
                    flow_kill.push((p, b, fact));
                }
            }
        }
        if num_procs > 1 {
            for p in 0..num_procs - 1 {
                if (case + p as usize) % 2 == 0 {
                    inter_edges.push((p, 0, p + 1, 0));
                }
            }
        }

        let expected = canonical_expected(
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra_edges,
            &inter_edges,
            &flow_gen,
            &flow_kill,
        );
        let actual = build_ifds_csr_via(
            &dispatcher,
            num_procs,
            blocks_per_proc,
            facts_per_proc,
            &intra_edges,
            &inter_edges,
            &flow_gen,
            &flow_kill,
        )
        .unwrap_or_else(|error| {
            panic!("Fix: generated IFDS case {case} must dispatch through CPU oracle: {error:?}")
        });
        assert_eq!(
            actual, expected,
            "Fix: CPU oracle via path diverged from reference at generated case {case}."
        );
    }
}

#[test]
fn via_rejects_extra_outputs() {
    let dispatcher = MalformedIfdsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0, 0]),
            u32_slice_to_le_bytes(&[0]),
            u32_slice_to_le_bytes(&[0]),
            u32_slice_to_le_bytes(&[0]),
            u32_slice_to_le_bytes(&[0]),
            u32_slice_to_le_bytes(&[0]),
        ],
    };
    let err = build_ifds_csr_via(&dispatcher, 1, 1, 1, &[], &[], &[], &[])
        .expect_err("extra outputs must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn via_rejects_trailing_col_len_bytes() {
    let dispatcher = MalformedIfdsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[0, 0]),
            u32_slice_to_le_bytes(&[0]),
            u32_slice_to_le_bytes(&[0]),
            vec![0, 0, 0, 0, 1],
        ],
    };
    let err = build_ifds_csr_via(&dispatcher, 1, 1, 1, &[], &[], &[], &[])
        .expect_err("trailing col_len bytes must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}

#[test]
fn via_rejects_inconsistent_row_ptr_readback() {
    let dispatcher = MalformedIfdsDispatcher {
        outputs: vec![
            u32_slice_to_le_bytes(&[1, 1]),
            u32_slice_to_le_bytes(&[0]),
            u32_slice_to_le_bytes(&[0]),
            u32_slice_to_le_bytes(&[0]),
        ],
    };
    let err = build_ifds_csr_via(&dispatcher, 1, 1, 1, &[], &[], &[], &[])
        .expect_err("row_ptr[0] drift must be rejected");
    assert!(
        matches!(err, DispatchError::BackendError(_)),
        "unexpected error: {err:?}"
    );
}
