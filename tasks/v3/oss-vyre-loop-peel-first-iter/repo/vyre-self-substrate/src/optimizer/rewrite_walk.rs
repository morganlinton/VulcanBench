use std::sync::Arc;

use vyre_foundation::ir::{Expr, Node, Program};

pub(super) fn rewrite_program_with_expr_rewriter<F>(
    program: Program,
    mut rewrite_expr: F,
) -> Program
where
    F: FnMut(&Expr, &mut u32) -> Expr,
{
    let body: Vec<Node> = match program.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };

    let mut counter = 0u32;
    let rebuilt = rewrite_scope(&body, &mut rewrite_expr, &mut counter);

    let new_entry = match program.entry() {
        [Node::Region {
            generator,
            source_region,
            ..
        }] => vec![Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(rebuilt),
        }],
        _ => rebuilt,
    };
    program.with_rewritten_entry(new_entry)
}

fn rewrite_scope<F>(body: &[Node], rewrite_expr: &mut F, counter: &mut u32) -> Vec<Node>
where
    F: FnMut(&Expr, &mut u32) -> Expr,
{
    let prefix_len = super::encode::reachable_prefix_len(body);
    let mut out = Vec::with_capacity(prefix_len);
    for node in &body[..prefix_len] {
        out.push(rewrite_node(node, rewrite_expr, counter));
    }
    out
}

fn rewrite_node<F>(node: &Node, rewrite_expr: &mut F, counter: &mut u32) -> Node
where
    F: FnMut(&Expr, &mut u32) -> Expr,
{
    match node {
        Node::Let { name, value } => Node::let_bind(name.clone(), rewrite_expr(value, counter)),
        Node::Assign { name, value } => Node::assign(name.clone(), rewrite_expr(value, counter)),
        Node::Store {
            buffer,
            index,
            value,
        } => Node::store(
            buffer.clone(),
            rewrite_expr(index, counter),
            rewrite_expr(value, counter),
        ),
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::if_then_else(
            rewrite_expr(cond, counter),
            rewrite_scope(then, rewrite_expr, counter),
            rewrite_scope(otherwise, rewrite_expr, counter),
        ),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::loop_for(
            var.clone(),
            rewrite_expr(from, counter),
            rewrite_expr(to, counter),
            rewrite_scope(body, rewrite_expr, counter),
        ),
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncLoad {
            source: source.clone(),
            destination: destination.clone(),
            offset: Box::new(rewrite_expr(offset, counter)),
            size: Box::new(rewrite_expr(size, counter)),
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
            offset: Box::new(rewrite_expr(offset, counter)),
            size: Box::new(rewrite_expr(size, counter)),
            tag: tag.clone(),
        },
        Node::Trap { address, tag } => Node::Trap {
            address: Box::new(rewrite_expr(address, counter)),
            tag: tag.clone(),
        },
        Node::Block(body) => Node::Block(rewrite_scope(body, rewrite_expr, counter)),
        Node::Region {
            generator,
            source_region,
            body,
        } => Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(rewrite_scope(body.as_slice(), rewrite_expr, counter)),
        },
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => node.clone(),
        _ => node.clone(),
    }
}
