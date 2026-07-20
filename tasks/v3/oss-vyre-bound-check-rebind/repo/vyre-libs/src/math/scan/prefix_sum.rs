//! Prefix-sum scan  -  inclusive scan over a u32 buffer.
//!
//! Category A composition backed by Tier-2.5 scan primitives: a compact
//! workgroup scan for one-block inputs and the multi-block scan for larger
//! buffers.

use vyre::ir::Program;
use vyre_primitives::math::prefix_scan::{prefix_scan_with_op_id, ScanKind};
use vyre_primitives::reduce::multi_block_prefix_scan::multi_block_prefix_scan_sum_u32;

const OP_ID: &str = "vyre-libs::math::scan_prefix_sum";

/// Build a Program that computes the inclusive prefix sum of `input`
/// into `output`, both sized `n`.
///
/// **Overflow semantics** (V7-CORR-018): all accumulator additions
/// use `u32::wrapping_add`. For inputs whose cumulative sum exceeds
/// `u32::MAX`, the output wraps modulo 2^32.
#[must_use]
pub fn scan_prefix_sum(input: &str, output: &str, n: u32) -> Program {
    if n == 0 {
        return crate::builder::invalid_output_program(
            OP_ID,
            output,
            vyre::ir::DataType::U32,
            "Fix: scan_prefix_sum requires n > 0.".to_string(),
        );
    }
    if (1..=1024).contains(&n) {
        prefix_scan_with_op_id(input, output, n, ScanKind::InclusiveSum, OP_ID)
    } else {
        wrap_large_scan_program(multi_block_prefix_scan_sum_u32(input, output, n))
    }
}

fn wrap_large_scan_program(program: Program) -> Program {
    Program::wrapped(
        program.buffers().to_vec(),
        program.workgroup_size(),
        vec![crate::region::wrap_anonymous(
            OP_ID,
            program.entry().to_vec(),
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || scan_prefix_sum("input", "output", 4),
        test_inputs: Some(|| vec![vec![
            vyre_primitives::wire::pack_u32_slice(&[1u32, 2, 3, 4]),
        ]]),
        expected_output: Some(|| vec![vec![
            // Only ReadWrite buffer: prefix sum [1, 3, 6, 10]
            vyre_primitives::wire::pack_u32_slice(&[1u32, 3, 6, 10]),
        ]]),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::{bytes_to_u32 as decode_u32_words, u32_bytes};
    use vyre::ir::{Expr, Node};
    use vyre_reference::value::Value;

    fn run_scan(n: u32, input: &[u32]) -> Vec<u32> {
        let program = scan_prefix_sum("input", "output", n);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(u32_bytes(input)),
                Value::from(vec![0u8; (n as usize).saturating_mul(4)]),
            ],
        )
        .expect("Fix: prefix sum must execute");
        decode_u32_words(&outputs[0].to_bytes())
    }

    #[test]
    fn prefix_sum_single_element() {
        let input = [42u32];
        let actual = run_scan(1, &input);
        assert_eq!(actual, vec![42u32]);
    }

    #[test]
    fn prefix_sum_empty_n_zero_should_trap() {
        let program = scan_prefix_sum("input", "output", 0);
        let error = vyre_reference::reference_eval(
            &program,
            &[Value::from(vec![0u8; 0]), Value::from(vec![0u8; 0])],
        )
        .expect_err("n=0 prefix_sum must trap instead of returning empty");
        let msg = error.to_string();
        assert!(
            msg.contains("trap") || msg.contains("Fix:"),
            "n=0 prefix_sum error must be actionable: {msg}"
        );
    }

    #[test]
    fn prefix_sum_boundary_small_path() {
        let input: Vec<u32> = (1..=1024).collect();
        let actual = run_scan(1024, &input);
        let expected: Vec<u32> = input
            .iter()
            .scan(0u32, |acc, &x| {
                *acc = acc.wrapping_add(x);
                Some(*acc)
            })
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn prefix_sum_boundary_large_path_is_parallel_multi_block() {
        let program = scan_prefix_sum("input", "output", 1025);
        assert_top_region_generator(&program, OP_ID);
        assert_eq!(program.workgroup_size(), [1024, 1, 1]);
        assert!(
            !contains_loop(&program),
            "large scan_prefix_sum must not route through prefix_scan_large's serial loop"
        );
        assert!(
            !contains_invocation_zero_gate(&program),
            "large scan_prefix_sum must not gate useful work behind InvocationId.x == 0"
        );
        assert!(program
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "output" && buffer.is_output()));
    }

    #[test]
    fn prefix_sum_large_path_parallel_shape_sweep() {
        for n in 1025..=4097 {
            let program = scan_prefix_sum("input", "output", n);
            assert_top_region_generator(&program, OP_ID);
            assert_eq!(program.workgroup_size(), [1024, 1, 1], "n={n}");
            assert!(
                !contains_loop(&program),
                "n={n}: large scan_prefix_sum must not emit a serial loop"
            );
            assert!(
                !contains_invocation_zero_gate(&program),
                "n={n}: large scan_prefix_sum must not gate useful work behind InvocationId.x == 0"
            );
            assert!(
                program
                    .buffers()
                    .iter()
                    .any(|buffer| buffer.name() == "output"
                        && buffer.is_output()
                        && buffer.count() == n),
                "n={n}: final output buffer must be declared with the requested element count"
            );
        }
    }

    #[test]
    fn prefix_sum_overflow_wraps() {
        let input = [u32::MAX, 1u32, 1u32];
        let actual = run_scan(3, &input);
        assert_eq!(actual[0], u32::MAX);
        assert_eq!(actual[1], 0u32, "u32::MAX + 1 must wrap to 0");
        assert_eq!(actual[2], 1u32, "0 + 1 must be 1");
    }

    fn assert_top_region_generator(program: &Program, expected: &str) {
        match program.entry() {
            [Node::Region { generator, .. }] => assert_eq!(generator.as_str(), expected),
            other => panic!("expected single top-level Region, got {other:?}"),
        }
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

    fn contains_invocation_zero_gate(program: &Program) -> bool {
        program
            .entry()
            .iter()
            .any(node_contains_invocation_zero_gate)
    }

    fn node_contains_invocation_zero_gate(node: &Node) -> bool {
        match node {
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                expr_is_invocation_zero(cond)
                    || then.iter().any(node_contains_invocation_zero_gate)
                    || otherwise.iter().any(node_contains_invocation_zero_gate)
            }
            Node::Block(children) => children.iter().any(node_contains_invocation_zero_gate),
            Node::Loop { body, .. } => body.iter().any(node_contains_invocation_zero_gate),
            Node::Region { body, .. } => body.iter().any(node_contains_invocation_zero_gate),
            _ => false,
        }
    }

    fn expr_is_invocation_zero(expr: &Expr) -> bool {
        match expr {
            Expr::BinOp { op, left, right } if *op == vyre::ir::BinOp::Eq => {
                matches!(
                    (&**left, &**right),
                    (Expr::InvocationId { axis: 0 }, Expr::LitU32(0))
                        | (Expr::LitU32(0), Expr::InvocationId { axis: 0 })
                )
            }
            Expr::BinOp { left, right, .. } => {
                expr_is_invocation_zero(left) || expr_is_invocation_zero(right)
            }
            Expr::UnOp { operand, .. } => expr_is_invocation_zero(operand),
            Expr::Load { index, .. } => expr_is_invocation_zero(index),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                expr_is_invocation_zero(cond)
                    || expr_is_invocation_zero(true_val)
                    || expr_is_invocation_zero(false_val)
            }
            Expr::Atomic {
                index,
                expected,
                value,
                ..
            } => {
                expr_is_invocation_zero(index)
                    || expected
                        .as_ref()
                        .is_some_and(|expr| expr_is_invocation_zero(expr))
                    || expr_is_invocation_zero(value)
            }
            Expr::Cast { value, .. } => expr_is_invocation_zero(value),
            Expr::Call { args, .. } => args.iter().any(expr_is_invocation_zero),
            Expr::Fma { a, b, c } => {
                expr_is_invocation_zero(a)
                    || expr_is_invocation_zero(b)
                    || expr_is_invocation_zero(c)
            }
            _ => false,
        }
    }
}
