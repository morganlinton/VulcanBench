//! Failure-oriented adversarial tests for math primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(all(feature = "math", feature = "cpu-parity"))]

use vyre_foundation::ir::{Expr, Node};
use vyre_primitives::math::conv1d::{
    conv1d_program, pack_params, MAX_RADIUS, OP_ID as CONV1D_OP_ID,
};
use vyre_primitives::math::prefix_scan::{cpu_ref as prefix_scan_cpu_ref, ScanKind};
use vyre_primitives::math::stream_compact::cpu_ref as stream_compact_cpu_ref;
use vyre_primitives::math::tensor_scc::cpu_ref as tensor_scc_cpu_ref;

fn find_region<'a>(nodes: &'a [Node], generator: &str) -> Option<&'a [Node]> {
    nodes.iter().find_map(|node| match node {
        Node::Region {
            generator: found,
            body,
            ..
        } if found.as_str() == generator => Some(body.as_ref().as_slice()),
        _ => None,
    })
}

fn node_binds_expr_named<'a>(nodes: &'a [Node], wanted: &str) -> Option<&'a Expr> {
    nodes.iter().find_map(|node| match node {
        Node::Let { name, value } if name.as_str() == wanted => Some(value),
        Node::If {
            then, otherwise, ..
        } => {
            node_binds_expr_named(then, wanted).or_else(|| node_binds_expr_named(otherwise, wanted))
        }
        Node::Loop { body, .. } => node_binds_expr_named(body, wanted),
        Node::Region { body, .. } => node_binds_expr_named(body, wanted),
        _ => None,
    })
}

fn expr_contains_select(expr: &Expr) -> bool {
    match expr {
        Expr::Select { .. } => true,
        Expr::BinOp { left, right, .. } => {
            expr_contains_select(left) || expr_contains_select(right)
        }
        Expr::UnOp { operand, .. } => expr_contains_select(operand),
        Expr::Cast { value, .. } => expr_contains_select(value),
        Expr::Fma { a, b, c } => {
            expr_contains_select(a) || expr_contains_select(b) || expr_contains_select(c)
        }
        _ => false,
    }
}

#[test]
fn prefix_scan_cpu_ref_overflow_wraps() {
    let input = vec![u32::MAX, 1, u32::MAX, 1];
    let got = prefix_scan_cpu_ref(&input, ScanKind::InclusiveSum);
    assert_eq!(got, vec![u32::MAX, 0, u32::MAX, 0]);
}

#[test]
fn conv1d_program_uses_current_ir_entrypoints_and_select_clamp() {
    let program = conv1d_program(17, MAX_RADIUS + 9);
    let body = find_region(program.entry(), CONV1D_OP_ID).expect("conv1d region must exist");

    assert!(
        matches!(
            node_binds_expr_named(body, "idx"),
            Some(Expr::InvocationId { axis: 0 })
        ),
        "conv1d must use Expr::gid_x/InvocationId x for dispatch indexing"
    );

    let src_idx = node_binds_expr_named(body, "src_idx").expect("src_idx binding must exist");
    assert!(
        expr_contains_select(src_idx),
        "boundary clamp must lower to explicit Select nodes, not a removed Expr::clamp helper"
    );

    let weights = program
        .buffers
        .iter()
        .find(|buffer| buffer.name() == "weights")
        .expect("weights buffer must exist");
    assert_eq!(
        weights.count,
        2 * MAX_RADIUS + 1,
        "conv1d radius must clamp to MAX_RADIUS before sizing weights"
    );
}

#[test]
fn conv1d_pack_params_clamps_radius() {
    assert_eq!(
        pack_params(32, 4, MAX_RADIUS + 1),
        vec![32, 4, MAX_RADIUS, 0]
    );
}

#[test]
fn prefix_scan_cpu_ref_exclusive_overflow_wraps() {
    let input = vec![u32::MAX, 1];
    let got = prefix_scan_cpu_ref(&input, ScanKind::ExclusiveSum);
    assert_eq!(got, vec![0, u32::MAX]);
}

#[test]
fn prefix_scan_cpu_ref_empty() {
    assert_eq!(
        prefix_scan_cpu_ref(&[], ScanKind::InclusiveSum),
        Vec::<u32>::new()
    );
    assert_eq!(
        prefix_scan_cpu_ref(&[], ScanKind::ExclusiveSum),
        Vec::<u32>::new()
    );
}

#[test]
fn prefix_scan_cpu_ref_hostile_powers_of_two() {
    for n in [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
        let input = vec![1u32; n as usize];
        let got = prefix_scan_cpu_ref(&input, ScanKind::InclusiveSum);
        assert_eq!(got.len(), n as usize);
        for (i, &v) in got.iter().enumerate() {
            assert_eq!(
                v,
                (i + 1) as u32,
                "inclusive sum mismatch at index {i} for n={n}"
            );
        }
    }
}

#[test]
fn stream_compact_cpu_ref_mismatched_lengths_truncates_to_shorter_input() {
    let (compacted, live) = stream_compact_cpu_ref(&[1, 2], &[1]);
    assert_eq!(compacted, vec![1]);
    assert_eq!(live, 1);
}

#[test]
fn stream_compact_cpu_ref_empty() {
    let (compacted, live) = stream_compact_cpu_ref(&[], &[]);
    assert!(compacted.is_empty());
    assert_eq!(live, 0);
}

#[test]
fn stream_compact_cpu_ref_all_zeros() {
    let (compacted, live) = stream_compact_cpu_ref(&[10, 20, 30], &[0, 0, 0]);
    assert!(compacted.is_empty());
    assert_eq!(live, 0);
}

#[test]
fn stream_compact_cpu_ref_all_ones() {
    let (compacted, live) = stream_compact_cpu_ref(&[10, 20, 30], &[1, 1, 1]);
    assert_eq!(compacted, vec![10, 20, 30]);
    assert_eq!(live, 3);
}

#[test]
fn stream_compact_cpu_ref_alternating() {
    let (compacted, live) = stream_compact_cpu_ref(&[1, 2, 3, 4, 5], &[1, 0, 1, 0, 1]);
    assert_eq!(compacted, vec![1, 3, 5]);
    assert_eq!(live, 3);
}

#[test]
fn tensor_scc_cpu_ref_empty_matrix() {
    let got = tensor_scc_cpu_ref(&[], 0b0001, 0b1111, 8);
    assert_eq!(got, 0b0001);
}

#[test]
fn tensor_scc_cpu_ref_seed_outside_group_masked() {
    let rows = [0b1111; 32];
    let got = tensor_scc_cpu_ref(&rows, 0b0001, 0b0001, 8);
    assert_eq!(got, 0b0001);
}

#[test]
fn tensor_scc_cpu_ref_iteration_limit_zero() {
    let rows = [0b0010, 0b0000];
    let got = tensor_scc_cpu_ref(&rows, 0b0001, 0b1111, 0);
    assert_eq!(got, 0b0001);
}

#[test]
fn tensor_scc_cpu_ref_cycle_inside_group() {
    let rows = [0b0010, 0b0100, 0b0001];
    let got = tensor_scc_cpu_ref(&rows, 0b0001, 0b1111, 8);
    assert_eq!(got, 0b0111);
}

#[test]
fn tensor_scc_cpu_ref_group_mask_filters() {
    let rows = [0b1111, 0b1111, 0b1111];
    let got = tensor_scc_cpu_ref(&rows, 0b0001, 0b0010, 8);
    // seed (0b0001) is masked by group (0b0010) => 0
    assert_eq!(got, 0);
}
