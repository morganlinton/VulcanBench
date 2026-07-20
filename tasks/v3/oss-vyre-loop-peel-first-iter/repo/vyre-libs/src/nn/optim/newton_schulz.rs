//! Newton-Schulz 5-step orthogonalization (F32).
//!
//! `X_{k+1} = a*X_k + b*X_k@X_k^T@X_k + c*X_k@X_k^T@X_k@X_k^T@X_k`
//! Coefficients: a=3.4445, b=-4.7750, c=2.0315.
//!
//! Used by Muon optimizer. This is a multi-pass matmul composition.

use vyre::ir::Program;
use vyre_primitives::math::preconditioner::newton_schulz_poly5_f32;

use crate::region::tag_program;

const OP_ID: &str = "vyre-libs::optim::newton_schulz_5step";

/// Newton-Schulz orthogonalization polynomial applied for five iterations.
#[must_use]
pub fn newton_schulz_5step(mat: &str, output: &str, rows: u32, cols: u32) -> Program {
    tag_program(OP_ID, newton_schulz_poly5_f32(mat, output, rows, cols))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || newton_schulz_5step("mat", "output", 2, 2),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[0.5, 0.0, 0.0, 0.5]),
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![vec![
                178, 243, 67, 63, 0, 0, 0, 0, 0, 0, 0, 0, 178, 243, 67, 63,
            ]]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::{Expr, Node};

    #[test]
    fn emitted_expression_tree_stays_linear_in_iterations() {
        let program = newton_schulz_5step("mat", "output", 2, 2);
        let expr_nodes = program.entry().iter().map(count_node_exprs).sum::<usize>();

        assert!(
            expr_nodes <= 128,
            "Fix: newton_schulz_5step must emit shared let-bound SSA expressions, not recursively clone the polynomial tree; expr_nodes={expr_nodes}"
        );
        assert!(
            program.stats().node_count <= 32,
            "Fix: newton_schulz_5step should remain a small fixed-size Cat-A composition; nodes={}",
            program.stats().node_count
        );
    }

    fn count_node_exprs(node: &Node) -> usize {
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => count_expr(value),
            Node::Store { index, value, .. } => count_expr(index) + count_expr(value),
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                count_expr(cond)
                    + then.iter().map(count_node_exprs).sum::<usize>()
                    + otherwise.iter().map(count_node_exprs).sum::<usize>()
            }
            Node::Loop { from, to, body, .. } => {
                count_expr(from) + count_expr(to) + body.iter().map(count_node_exprs).sum::<usize>()
            }
            Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
                count_expr(offset) + count_expr(size)
            }
            Node::Trap { address, .. } => count_expr(address),
            Node::Block(nodes) => nodes.iter().map(count_node_exprs).sum(),
            Node::Region { body, .. } => body.iter().map(count_node_exprs).sum(),
            Node::IndirectDispatch { .. }
            | Node::AsyncWait { .. }
            | Node::Resume { .. }
            | Node::Return
            | Node::Barrier {
                ordering: vyre::memory_model::MemoryOrdering::SeqCst,
            }
            | Node::Opaque(_)
            | Node::Barrier { .. } => 0,
            _ => 0,
        }
    }

    fn count_expr(expr: &Expr) -> usize {
        1 + match expr {
            Expr::Load { index, .. }
            | Expr::UnOp { operand: index, .. }
            | Expr::Cast { value: index, .. }
            | Expr::SubgroupBallot { cond: index }
            | Expr::SubgroupReduce { value: index, .. } => count_expr(index),
            Expr::BinOp { left, right, .. }
            | Expr::SubgroupShuffle {
                value: left,
                lane: right,
            } => count_expr(left) + count_expr(right),
            Expr::Select {
                cond,
                true_val,
                false_val,
            }
            | Expr::Fma {
                a: cond,
                b: true_val,
                c: false_val,
            } => count_expr(cond) + count_expr(true_val) + count_expr(false_val),
            Expr::Call { args, .. } => args.iter().map(count_expr).sum(),
            Expr::Atomic {
                index,
                expected,
                value,
                ..
            } => {
                count_expr(index)
                    + expected.as_deref().map(count_expr).unwrap_or(0)
                    + count_expr(value)
            }
            Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::Var(_)
            | Expr::BufLen { .. }
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
            | Expr::Opaque(_) => 0,
            _ => 0,
        }
    }
}
