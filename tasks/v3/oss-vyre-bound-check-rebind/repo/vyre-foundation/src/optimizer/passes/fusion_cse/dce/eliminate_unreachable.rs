use super::{const_loop_empty, const_truth};
use crate::ir::Node;

#[inline]
pub(crate) fn eliminate_unreachable(nodes: Vec<Node>) -> Vec<Node> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            Node::If {
                cond,
                then,
                otherwise,
            } => match const_truth(&cond) {
                Some(true) => out.extend(eliminate_unreachable(then)),
                Some(false) => out.extend(eliminate_unreachable(otherwise)),
                None => out.push(Node::if_then_else(
                    cond,
                    eliminate_unreachable(then),
                    eliminate_unreachable(otherwise),
                )),
            },
            Node::Loop {
                var: _,
                from,
                to,
                body: _,
            } if const_loop_empty(&from, &to) => {}
            Node::Loop {
                var,
                from,
                to,
                body,
            } => out.push(Node::loop_for(&var, from, to, eliminate_unreachable(body))),
            Node::Block(block_nodes) => {
                let block_nodes = eliminate_unreachable(block_nodes);
                if !block_nodes.is_empty() {
                    out.push(Node::block(block_nodes));
                }
            }
            Node::Return => {
                out.push(Node::Return);
                break;
            }
            Node::Let { name, value } => out.push(Node::let_bind(&name, value)),
            Node::Assign { name, value } => out.push(Node::assign(&name, value)),
            Node::Store {
                buffer,
                index,
                value,
            } => out.push(Node::store(&buffer, index, value)),
            Node::Barrier { ordering } => out.push(Node::barrier_with_ordering(ordering)),
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => out.push(Node::IndirectDispatch {
                count_buffer,
                count_offset,
            }),
            Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. } => out.push(node),
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => out.push(Node::async_load_ext(
                source,
                destination,
                *offset,
                *size,
                tag,
            )),
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => out.push(Node::async_store(source, destination, *offset, *size, tag)),
            Node::AsyncWait { tag } => out.push(Node::async_wait(&tag)),
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                let body_nodes =
                    std::sync::Arc::try_unwrap(body).unwrap_or_else(|arc| (*arc).clone());
                out.push(Node::Region {
                    generator,
                    source_region,
                    body: std::sync::Arc::new(eliminate_unreachable(body_nodes)),
                });
            }
            Node::Trap { .. } | Node::Resume { .. } => out.push(node.clone()),
            Node::Opaque(extension) => out.push(Node::Opaque(extension.clone())),
        }
    }
    out
}
