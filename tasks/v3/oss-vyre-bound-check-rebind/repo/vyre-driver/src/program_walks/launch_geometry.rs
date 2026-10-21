//! Launch-geometry dependency walks.

use vyre_foundation::ir::{Expr, Node, Program};

/// True when the program reads launch geometry that makes workgroup shape
/// semantically visible to the kernel body.
#[must_use]
pub(crate) fn program_uses_launch_geometry_ids(program: &Program) -> bool {
    program.entry().iter().any(node_uses_launch_geometry_ids)
}

fn node_uses_launch_geometry_ids(node: &Node) -> bool {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            expr_uses_launch_geometry_ids(value)
        }
        Node::Store { index, value, .. } => {
            expr_uses_launch_geometry_ids(index) || expr_uses_launch_geometry_ids(value)
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_uses_launch_geometry_ids(cond)
                || then.iter().any(node_uses_launch_geometry_ids)
                || otherwise.iter().any(node_uses_launch_geometry_ids)
        }
        Node::Loop { from, to, body, .. } => {
            expr_uses_launch_geometry_ids(from)
                || expr_uses_launch_geometry_ids(to)
                || body.iter().any(node_uses_launch_geometry_ids)
        }
        Node::Block(children) => children.iter().any(node_uses_launch_geometry_ids),
        Node::Region { body, .. } => body.iter().any(node_uses_launch_geometry_ids),
        Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
            expr_uses_launch_geometry_ids(offset) || expr_uses_launch_geometry_ids(size)
        }
        Node::Trap { address, .. } => expr_uses_launch_geometry_ids(address),
        Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::Opaque(_) => false,
        _ => false,
    }
}

fn expr_uses_launch_geometry_ids(expr: &Expr) -> bool {
    match expr {
        Expr::LocalId { .. } | Expr::WorkgroupId { .. } => true,
        Expr::Load { index, .. } | Expr::UnOp { operand: index, .. } => {
            expr_uses_launch_geometry_ids(index)
        }
        Expr::BinOp { left, right, .. } => {
            expr_uses_launch_geometry_ids(left) || expr_uses_launch_geometry_ids(right)
        }
        Expr::Call { args, .. } => args.iter().any(expr_uses_launch_geometry_ids),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_uses_launch_geometry_ids(cond)
                || expr_uses_launch_geometry_ids(true_val)
                || expr_uses_launch_geometry_ids(false_val)
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            expr_uses_launch_geometry_ids(index)
                || expected
                    .as_ref()
                    .is_some_and(|expr| expr_uses_launch_geometry_ids(expr))
                || expr_uses_launch_geometry_ids(value)
        }
        Expr::Cast { value, .. } => expr_uses_launch_geometry_ids(value),
        Expr::Fma { a, b, c } => {
            expr_uses_launch_geometry_ids(a)
                || expr_uses_launch_geometry_ids(b)
                || expr_uses_launch_geometry_ids(c)
        }
        Expr::SubgroupBallot { cond } => expr_uses_launch_geometry_ids(cond),
        Expr::SubgroupShuffle { value, lane } => {
            expr_uses_launch_geometry_ids(value) || expr_uses_launch_geometry_ids(lane)
        }
        Expr::SubgroupReduce { value, .. } => expr_uses_launch_geometry_ids(value),
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => false,
        _ => false,
    }
}
