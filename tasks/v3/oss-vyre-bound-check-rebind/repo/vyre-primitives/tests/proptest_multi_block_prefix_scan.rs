//! Generated truth and structure checks for the arbitrary-length prefix scan.

#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_foundation::ir::{Expr, Node, Program};
use vyre_foundation::MemoryOrdering;
use vyre_primitives::reduce::multi_block_prefix_scan::{
    cpu_ref, multi_block_prefix_scan_sum_u32, BLOCK_LANES,
};

fn independent_wrapping_prefix(values: &[u32]) -> Vec<u32> {
    let mut acc = 0_u32;
    values
        .iter()
        .map(|value| {
            acc = acc.wrapping_add(*value);
            acc
        })
        .collect()
}

fn contains_loop(program: &Program) -> bool {
    program.entry().iter().any(node_contains_loop)
}

fn node_contains_loop(node: &Node) -> bool {
    match node {
        Node::Loop { .. } => true,
        Node::Block(children) => children.iter().any(node_contains_loop),
        Node::If {
            then, otherwise, ..
        } => then.iter().any(node_contains_loop) || otherwise.iter().any(node_contains_loop),
        Node::Region { body, .. } => body.iter().any(node_contains_loop),
        _ => false,
    }
}

fn contains_invocation_id(program: &Program) -> bool {
    program.entry().iter().any(node_contains_invocation_id)
}

fn grid_sync_barrier_count(program: &Program) -> usize {
    program
        .entry()
        .iter()
        .map(node_grid_sync_barrier_count)
        .sum()
}

fn node_grid_sync_barrier_count(node: &Node) -> usize {
    match node {
        Node::Barrier {
            ordering: MemoryOrdering::GridSync,
        } => 1,
        Node::Block(children) => children.iter().map(node_grid_sync_barrier_count).sum(),
        Node::If {
            then, otherwise, ..
        } => {
            then.iter().map(node_grid_sync_barrier_count).sum::<usize>()
                + otherwise
                    .iter()
                    .map(node_grid_sync_barrier_count)
                    .sum::<usize>()
        }
        Node::Loop { body, .. } => body.iter().map(node_grid_sync_barrier_count).sum(),
        Node::Region { body, .. } => body.iter().map(node_grid_sync_barrier_count).sum(),
        _ => 0,
    }
}

fn node_contains_invocation_id(node: &Node) -> bool {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => expr_contains_invocation_id(value),
        Node::Store { index, value, .. } => {
            expr_contains_invocation_id(index) || expr_contains_invocation_id(value)
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_contains_invocation_id(cond)
                || then.iter().any(node_contains_invocation_id)
                || otherwise.iter().any(node_contains_invocation_id)
        }
        Node::Loop { from, to, body, .. } => {
            expr_contains_invocation_id(from)
                || expr_contains_invocation_id(to)
                || body.iter().any(node_contains_invocation_id)
        }
        Node::Block(children) => children.iter().any(node_contains_invocation_id),
        Node::Region { body, .. } => body.iter().any(node_contains_invocation_id),
        _ => false,
    }
}

fn expr_contains_invocation_id(expr: &Expr) -> bool {
    match expr {
        Expr::InvocationId { .. } => true,
        Expr::Load { index, .. } | Expr::UnOp { operand: index, .. } => {
            expr_contains_invocation_id(index)
        }
        Expr::BinOp { left, right, .. } => {
            expr_contains_invocation_id(left) || expr_contains_invocation_id(right)
        }
        Expr::Call { args, .. } => args.iter().any(expr_contains_invocation_id),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_contains_invocation_id(cond)
                || expr_contains_invocation_id(true_val)
                || expr_contains_invocation_id(false_val)
        }
        Expr::Atomic {
            index,
            value,
            expected,
            ..
        } => {
            expr_contains_invocation_id(index)
                || expr_contains_invocation_id(value)
                || expected
                    .as_ref()
                    .is_some_and(|expr| expr_contains_invocation_id(expr))
        }
        Expr::Cast { value, .. } => expr_contains_invocation_id(value),
        Expr::Fma { a, b, c } => {
            expr_contains_invocation_id(a)
                || expr_contains_invocation_id(b)
                || expr_contains_invocation_id(c)
        }
        _ => false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4_096))]

    #[test]
    fn cpu_ref_matches_independent_wrapping_prefix_for_generated_inputs(
        values in proptest::collection::vec(any::<u32>(), 0..=2048),
    ) {
        prop_assert_eq!(cpu_ref(&values), independent_wrapping_prefix(&values));
    }

    #[test]
    fn large_builder_uses_parallel_multi_block_chain_for_generated_sizes(
        n in (BLOCK_LANES + 1)..=(BLOCK_LANES * 4),
    ) {
        let program = multi_block_prefix_scan_sum_u32("input", "output", n);
        let num_blocks = n.div_ceil(BLOCK_LANES);

        prop_assert_eq!(program.workgroup_size(), [BLOCK_LANES, 1, 1]);
        prop_assert!(
            !contains_loop(&program),
            "multi-block scan must not regress to the serial prefix_scan_large loop for n={n}"
        );
        prop_assert!(
            !contains_invocation_id(&program),
            "large multi-block scan must use local/workgroup ids so fused overdispatch cannot address per-block scratch with a global lane for n={n}"
        );
        prop_assert_eq!(
            grid_sync_barrier_count(&program),
            2,
            "three-pass multi-block scan must split Pass A, Pass B, and Pass C with grid-level barriers for n={}",
            n
        );
        let has_partials = program.buffers().iter().any(|buffer| {
            buffer.name() == "__output_mbps_partials"
                && buffer.count() == num_blocks * BLOCK_LANES
                && !buffer.is_output()
                && buffer.is_pipeline_live_out()
        });
        let guarded_scratch_words = program
            .buffers()
            .iter()
            .filter(|buffer| buffer.name().contains("_guarded_scan_"))
            .map(|buffer| buffer.count())
            .collect::<Vec<_>>();
        let has_block_totals = program.buffers().iter().any(|buffer| {
            buffer.name() == "__output_mbps_block_totals"
                && buffer.count() == num_blocks
                && !buffer.is_output()
                && buffer.is_pipeline_live_out()
        });
        let has_output = program.buffers().iter().any(|buffer| {
            buffer.name() == "output" && buffer.count() == n && buffer.is_output()
        });
        let output_markers = program
            .buffers()
            .iter()
            .filter(|buffer| buffer.is_output())
            .count();

        prop_assert_eq!(output_markers, 1);
        prop_assert_eq!(
            guarded_scratch_words,
            vec![BLOCK_LANES, BLOCK_LANES],
            "guarded internal block-total scan must allocate full-block scratch for fused 1024-lane launches"
        );
        prop_assert!(has_partials);
        prop_assert!(has_block_totals);
        prop_assert!(has_output);
    }
}
