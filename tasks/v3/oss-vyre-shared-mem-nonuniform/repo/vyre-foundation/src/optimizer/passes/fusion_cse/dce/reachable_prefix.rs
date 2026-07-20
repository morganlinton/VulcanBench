use crate::ir::Node;

#[inline]
pub(crate) fn reachable_prefix(nodes: &[Node]) -> &[Node] {
    let end = nodes
        .iter()
        .position(|node| matches!(node, Node::Return))
        .map_or(nodes.len(), |index| index + 1);
    &nodes[..end]
}
