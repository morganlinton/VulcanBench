//! Induction-variable substitution over the IR tree.
//!
//! `substitute_nodes` replaces every free occurrence of a variable `var` with
//! an arbitrary `replacement` expression, recursing through every nested
//! `Expr` and `Node` that can carry a variable reference. It is the single
//! canonical implementation shared by the optimizer loop passes (strip-mine,
//! unroll) and the reverse-mode autodiff loop arm (which substitutes the
//! reversed-index expression into the adjoint body).
//!
//! Completeness is load-bearing: a missed `Expr`/`Node` variant would silently
//! leave a stale `var` reference behind — a wrong loop tiling or a wrong
//! reversed gradient. Every variant that holds a nested `Expr` is handled
//! explicitly; the catch-all only covers leaf variants that provably carry no
//! variable reference.

use crate::ir::{Expr, Ident, Node};
use std::sync::Arc;

/// Substitute every free occurrence of `var` with `replacement` across `nodes`.
pub(crate) fn substitute_nodes(nodes: &[Node], var: &Ident, replacement: &Expr) -> Vec<Node> {
    nodes
        .iter()
        .map(|node| substitute_node(node, var, replacement))
        .collect()
}

/// Substitute every free occurrence of `var` with `replacement` in one node.
pub(crate) fn substitute_node(node: &Node, var: &Ident, replacement: &Expr) -> Node {
    match node {
        Node::Let { name, value } => Node::let_bind(name, substitute_expr(value, var, replacement)),
        Node::Assign { name, value } => {
            Node::assign(name, substitute_expr(value, var, replacement))
        }
        Node::Store {
            buffer,
            index,
            value,
        } => Node::store(
            buffer,
            substitute_expr(index, var, replacement),
            substitute_expr(value, var, replacement),
        ),
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::if_then_else(
            substitute_expr(cond, var, replacement),
            substitute_nodes(then, var, replacement),
            substitute_nodes(otherwise, var, replacement),
        ),
        Node::Loop {
            var: inner,
            from,
            to,
            body,
        } => {
            let from = substitute_expr(from, var, replacement);
            let to = substitute_expr(to, var, replacement);
            let body = if inner == var {
                body.clone()
            } else {
                substitute_nodes(body, var, replacement)
            };
            Node::loop_for(inner, from, to, body)
        }
        Node::Block(body) => Node::block(substitute_nodes(body, var, replacement)),
        Node::Region {
            generator,
            source_region,
            body,
        } => Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(substitute_nodes(body, var, replacement)),
        },
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncLoad {
            source: source.clone(),
            destination: destination.clone(),
            offset: Box::new(substitute_expr(offset, var, replacement)),
            size: Box::new(substitute_expr(size, var, replacement)),
            tag: tag.clone(),
        },
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncStore {
            source: source.clone(),
            destination: destination.clone(),
            offset: Box::new(substitute_expr(offset, var, replacement)),
            size: Box::new(substitute_expr(size, var, replacement)),
            tag: tag.clone(),
        },
        Node::Trap { address, tag } => Node::Trap {
            address: Box::new(substitute_expr(address, var, replacement)),
            tag: tag.clone(),
        },
        Node::IndirectDispatch { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::Opaque(_) => node.clone(),
    }
}

fn substitute_expr(expr: &Expr, var: &Ident, replacement: &Expr) -> Expr {
    match expr {
        Expr::Var(name) if name == var => replacement.clone(),
        Expr::Load { buffer, index } => {
            Expr::load(buffer, substitute_expr(index, var, replacement))
        }
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op: *op,
            left: Box::new(substitute_expr(left, var, replacement)),
            right: Box::new(substitute_expr(right, var, replacement)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op: op.clone(),
            operand: Box::new(substitute_expr(operand, var, replacement)),
        },
        Expr::Call { op_id, args } => Expr::call(
            op_id,
            args.iter()
                .map(|arg| substitute_expr(arg, var, replacement))
                .collect(),
        ),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::select(
            substitute_expr(cond, var, replacement),
            substitute_expr(true_val, var, replacement),
            substitute_expr(false_val, var, replacement),
        ),
        Expr::Cast { target, value } => {
            Expr::cast(target.clone(), substitute_expr(value, var, replacement))
        }
        Expr::Fma { a, b, c } => Expr::fma(
            substitute_expr(a, var, replacement),
            substitute_expr(b, var, replacement),
            substitute_expr(c, var, replacement),
        ),
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => Expr::Atomic {
            op: *op,
            buffer: buffer.clone(),
            index: Box::new(substitute_expr(index, var, replacement)),
            expected: expected
                .as_ref()
                .map(|expr| Box::new(substitute_expr(expr, var, replacement))),
            value: Box::new(substitute_expr(value, var, replacement)),
            ordering: *ordering,
        },
        Expr::SubgroupBallot { cond } => {
            Expr::subgroup_ballot(substitute_expr(cond, var, replacement))
        }
        Expr::SubgroupShuffle { value, lane } => Expr::subgroup_shuffle(
            substitute_expr(value, var, replacement),
            substitute_expr(lane, var, replacement),
        ),
        Expr::SubgroupReduce { value, .. } => Expr::subgroup_add(substitute_expr(value, var, replacement)),
        _ => expr.clone(),
    }
}
