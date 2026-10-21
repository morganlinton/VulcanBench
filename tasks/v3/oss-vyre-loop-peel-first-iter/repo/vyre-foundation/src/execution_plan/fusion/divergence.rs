//! Divergence + launch-geometry analysis used by the fusion safety check.

use rustc_hash::FxHashSet;

use crate::ir::{Expr, Ident, Node};

pub(super) fn has_divergent_invocation_gated_store(
    node: &Node,
    inside_invocation_gate: bool,
) -> bool {
    match node {
        Node::Store { .. } => inside_invocation_gate,
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            inside_invocation_gate && expr_writes_atomic(value)
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            let new_gate = inside_invocation_gate || cond_depends_on_invocation_id(cond);
            then.iter()
                .chain(otherwise.iter())
                .any(|n| has_divergent_invocation_gated_store(n, new_gate))
        }
        Node::Loop { body, .. } => body
            .iter()
            .any(|n| has_divergent_invocation_gated_store(n, inside_invocation_gate)),
        Node::Block(body) => body
            .iter()
            .any(|n| has_divergent_invocation_gated_store(n, inside_invocation_gate)),
        Node::Region { body, .. } => body
            .iter()
            .any(|n| has_divergent_invocation_gated_store(n, inside_invocation_gate)),
        Node::IndirectDispatch { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => false,
    }
}

pub(super) fn has_launch_geometry_dependent_write(nodes: &[Node]) -> bool {
    let mut launch_vars = FxHashSet::default();
    nodes_have_launch_geometry_dependent_write(nodes, &mut launch_vars, false)
}

fn nodes_have_launch_geometry_dependent_write(
    nodes: &[Node],
    launch_vars: &mut FxHashSet<Ident>,
    inside_launch_gate: bool,
) -> bool {
    nodes
        .iter()
        .any(|node| node_has_launch_geometry_dependent_write(node, launch_vars, inside_launch_gate))
}

fn node_has_launch_geometry_dependent_write(
    node: &Node,
    launch_vars: &mut FxHashSet<Ident>,
    inside_launch_gate: bool,
) -> bool {
    match node {
        Node::Let { name, value } | Node::Assign { name, value } => {
            let writes_atomic = expr_writes_atomic(value);
            let depends_on_launch = expr_depends_on_launch_geometry(value, launch_vars);
            if depends_on_launch {
                launch_vars.insert(name.clone());
            } else {
                launch_vars.remove(name);
            }
            writes_atomic && (inside_launch_gate || depends_on_launch)
        }
        Node::Store { index, value, .. } => {
            inside_launch_gate
                || expr_depends_on_launch_geometry(index, launch_vars)
                || expr_depends_on_launch_geometry(value, launch_vars)
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            let new_gate = inside_launch_gate || expr_depends_on_launch_geometry(cond, launch_vars);
            let mut then_vars = launch_vars.clone();
            let mut otherwise_vars = launch_vars.clone();
            let then_writes =
                nodes_have_launch_geometry_dependent_write(then, &mut then_vars, new_gate);
            let otherwise_writes = nodes_have_launch_geometry_dependent_write(
                otherwise,
                &mut otherwise_vars,
                new_gate,
            );
            launch_vars.extend(then_vars);
            launch_vars.extend(otherwise_vars);
            then_writes || otherwise_writes
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            let new_gate = inside_launch_gate
                || expr_depends_on_launch_geometry(from, launch_vars)
                || expr_depends_on_launch_geometry(to, launch_vars);
            let mut body_vars = launch_vars.clone();
            if new_gate {
                body_vars.insert(var.clone());
            }
            nodes_have_launch_geometry_dependent_write(body, &mut body_vars, new_gate)
        }
        Node::Block(body) => {
            let mut body_vars = launch_vars.clone();
            nodes_have_launch_geometry_dependent_write(body, &mut body_vars, inside_launch_gate)
        }
        Node::Region { body, .. } => {
            let mut body_vars = launch_vars.clone();
            nodes_have_launch_geometry_dependent_write(body, &mut body_vars, inside_launch_gate)
        }
        Node::AsyncStore { offset, size, .. } => {
            inside_launch_gate
                || expr_depends_on_launch_geometry(offset, launch_vars)
                || expr_depends_on_launch_geometry(size, launch_vars)
        }
        Node::IndirectDispatch { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => false,
    }
}

/// `true` when `expr` references `Expr::InvocationId` (any axis),
/// `Expr::WorkgroupId`, or `Expr::LocalId` somewhere in its tree.
/// These are the canonical "this thread is special" predicates.
fn cond_depends_on_invocation_id(expr: &Expr) -> bool {
    match expr {
        Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => true,
        Expr::BinOp { left, right, .. } => {
            cond_depends_on_invocation_id(left) || cond_depends_on_invocation_id(right)
        }
        Expr::UnOp { operand, .. } => cond_depends_on_invocation_id(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            cond_depends_on_invocation_id(cond)
                || cond_depends_on_invocation_id(true_val)
                || cond_depends_on_invocation_id(false_val)
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
            cond_depends_on_invocation_id(value)
        }
        Expr::Fma { a, b, c } => {
            cond_depends_on_invocation_id(a)
                || cond_depends_on_invocation_id(b)
                || cond_depends_on_invocation_id(c)
        }
        Expr::Load { index, .. } => cond_depends_on_invocation_id(index),
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            cond_depends_on_invocation_id(index)
                || expected
                    .as_deref()
                    .is_some_and(cond_depends_on_invocation_id)
                || cond_depends_on_invocation_id(value)
        }
        Expr::Call { args, .. } => args.iter().any(cond_depends_on_invocation_id),
        Expr::SubgroupBallot { cond } => cond_depends_on_invocation_id(cond),
        Expr::SubgroupShuffle { value, lane } => {
            cond_depends_on_invocation_id(value) || cond_depends_on_invocation_id(lane)
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::Opaque(_) => false,
    }
}

fn expr_depends_on_launch_geometry(expr: &Expr, launch_vars: &FxHashSet<Ident>) -> bool {
    match expr {
        Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => true,
        Expr::Var(name) => launch_vars.contains(name),
        Expr::BinOp { left, right, .. } => {
            expr_depends_on_launch_geometry(left, launch_vars)
                || expr_depends_on_launch_geometry(right, launch_vars)
        }
        Expr::UnOp { operand, .. } => expr_depends_on_launch_geometry(operand, launch_vars),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_depends_on_launch_geometry(cond, launch_vars)
                || expr_depends_on_launch_geometry(true_val, launch_vars)
                || expr_depends_on_launch_geometry(false_val, launch_vars)
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
            expr_depends_on_launch_geometry(value, launch_vars)
        }
        Expr::Fma { a, b, c } => {
            expr_depends_on_launch_geometry(a, launch_vars)
                || expr_depends_on_launch_geometry(b, launch_vars)
                || expr_depends_on_launch_geometry(c, launch_vars)
        }
        Expr::Load { index, .. } => expr_depends_on_launch_geometry(index, launch_vars),
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            expr_depends_on_launch_geometry(index, launch_vars)
                || expected
                    .as_deref()
                    .is_some_and(|expr| expr_depends_on_launch_geometry(expr, launch_vars))
                || expr_depends_on_launch_geometry(value, launch_vars)
        }
        Expr::Call { args, .. } => args
            .iter()
            .any(|expr| expr_depends_on_launch_geometry(expr, launch_vars)),
        Expr::SubgroupBallot { cond } => expr_depends_on_launch_geometry(cond, launch_vars),
        Expr::SubgroupShuffle { value, lane } => {
            expr_depends_on_launch_geometry(value, launch_vars)
                || expr_depends_on_launch_geometry(lane, launch_vars)
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::Opaque(_) => false,
    }
}

/// `true` when `expr` is an atomic operation (which writes memory).
/// Used by the divergent-gate detector to pick up `Let { value:
/// Atomic { ... } }` / `Assign { value: Atomic { ... } }` patterns  -
/// the canonical `lower_call_to` `atomic_or` shape.
pub(super) fn expr_writes_atomic(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic { .. } => true,
        Expr::BinOp { left, right, .. } => expr_writes_atomic(left) || expr_writes_atomic(right),
        Expr::UnOp { operand, .. } => expr_writes_atomic(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_writes_atomic(cond)
                || expr_writes_atomic(true_val)
                || expr_writes_atomic(false_val)
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => expr_writes_atomic(value),
        Expr::Fma { a, b, c } => {
            expr_writes_atomic(a) || expr_writes_atomic(b) || expr_writes_atomic(c)
        }
        Expr::Load { index, .. } => expr_writes_atomic(index),
        Expr::Call { args, .. } => args.iter().any(expr_writes_atomic),
        Expr::SubgroupBallot { cond } => expr_writes_atomic(cond),
        Expr::SubgroupShuffle { value, lane } => {
            expr_writes_atomic(value) || expr_writes_atomic(lane)
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::BufLen { .. }
        | Expr::Opaque(_) => false,
    }
}
