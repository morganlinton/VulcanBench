//! Regression tests for optimizer IR shape that can explode lowering time.

use vyre::ir::{Expr, Node};

#[test]
fn newton_schulz_ir_shape_stays_linear() {
    let program = vyre_libs::nn::optim::newton_schulz_5step("mat", "output", 2, 2);
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
        | Node::Opaque(_) => 0,
        _ => panic!("Fix: update newton_schulz IR-shape test for new Node variant."),
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
            count_expr(index) + expected.as_deref().map(count_expr).unwrap_or(0) + count_expr(value)
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
        _ => panic!("Fix: update newton_schulz IR-shape test for new Expr variant."),
    }
}
