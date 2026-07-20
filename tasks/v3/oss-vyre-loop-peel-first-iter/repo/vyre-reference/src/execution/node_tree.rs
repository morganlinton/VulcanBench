//! Shared recursive inspection helpers for statement IR nodes.

use vyre::ir::Node;

/// Whether any statement in `nodes` may reach a `Barrier`, scanning child
/// statement lists recursively with an exhaustive `Node` match.
pub(crate) fn contains_barrier(nodes: &[Node]) -> bool {
    nodes.iter().any(node_contains_barrier)
}

fn node_contains_barrier(node: &Node) -> bool {
    match node {
        Node::Barrier { .. } => true,
        Node::Let { .. }
        | Node::Assign { .. }
        | Node::Store { .. }
        | Node::Return
        | Node::IndirectDispatch { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => false,
        Node::If {
            then, otherwise, ..
        } => contains_barrier(then) || contains_barrier(otherwise),
        Node::Loop { body, .. } => contains_barrier(body),
        Node::Block(body) => contains_barrier(body),
        _ => false,
    }
}

/// Stable per-process identifier for a borrowed `Node`.
pub(crate) fn node_id(node: &Node) -> usize {
    std::ptr::from_ref(node).addr()
}
