use super::{collect_expr_refs, expr_has_effect, reachable_prefix, LiveResult};
use crate::ir::{Ident, Node};
use im::HashSet;

#[inline]
#[expect(
    clippy::too_many_lines,
    reason = "reverse liveness/DCE pass keeps Node reconstruction and live-set transfer together"
)]
pub(crate) fn eliminate_dead_lets(nodes: Vec<Node>, live_after: HashSet<Ident>) -> LiveResult {
    let reachable_len = reachable_prefix(&nodes).len();
    let mut live = live_after;
    let mut kept = Vec::with_capacity(reachable_len);

    for node in nodes.into_iter().take(reachable_len).rev() {
        match node {
            Node::Let { name, value } if !live.contains(&name) && !expr_has_effect(&value) => {}
            Node::Let { name, value } => {
                live.remove(&name);
                collect_expr_refs(&value, &mut live);
                kept.push(Node::Let { name, value });
            }
            Node::Assign { name, value } => {
                live.insert(name.clone());
                collect_expr_refs(&value, &mut live);
                kept.push(Node::Assign { name, value });
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                collect_expr_refs(&index, &mut live);
                collect_expr_refs(&value, &mut live);
                kept.push(Node::Store {
                    buffer,
                    index,
                    value,
                });
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let then_result = eliminate_dead_lets(then, live.clone());
                let otherwise_result = eliminate_dead_lets(otherwise, live.clone());
                let mut branch_live = then_result.live_in;
                branch_live.extend(otherwise_result.live_in);
                collect_expr_refs(&cond, &mut branch_live);
                live = branch_live;
                kept.push(Node::If {
                    cond,
                    then: then_result.nodes,
                    otherwise: otherwise_result.nodes,
                });
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                let mut body_live_after = live.clone();
                body_live_after.insert(var.clone());
                let body_result = eliminate_dead_lets(body, body_live_after);
                live.extend(body_result.live_in);
                live.remove(&var);
                collect_expr_refs(&from, &mut live);
                collect_expr_refs(&to, &mut live);
                kept.push(Node::Loop {
                    var,
                    from,
                    to,
                    body: body_result.nodes,
                });
            }
            Node::Block(block_nodes) => {
                let block_result = eliminate_dead_lets(block_nodes, live.clone());
                live.extend(block_result.live_in);
                kept.push(Node::Block(block_result.nodes));
            }
            Node::Return => kept.push(Node::Return),
            Node::Barrier { ordering } => kept.push(Node::Barrier { ordering }),
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => kept.push(Node::IndirectDispatch {
                count_buffer,
                count_offset,
            }),
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                collect_expr_refs(&offset, &mut live);
                collect_expr_refs(&size, &mut live);
                kept.push(Node::AsyncLoad {
                    source,
                    destination,
                    offset,
                    size,
                    tag,
                });
            }
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                collect_expr_refs(&offset, &mut live);
                collect_expr_refs(&size, &mut live);
                kept.push(Node::AsyncStore {
                    source,
                    destination,
                    offset,
                    size,
                    tag,
                });
            }
            Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. } => kept.push(node),
            Node::AsyncWait { tag } => kept.push(Node::AsyncWait { tag }),
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                let body_nodes =
                    std::sync::Arc::try_unwrap(body).unwrap_or_else(|arc| (*arc).clone());
                let body_result = eliminate_dead_lets(body_nodes, live.clone());
                live.extend(body_result.live_in);
                kept.push(Node::Region {
                    generator,
                    source_region,
                    body: std::sync::Arc::new(body_result.nodes),
                });
            }
            Node::Trap { address, tag } => {
                collect_expr_refs(&address, &mut live);
                kept.push(Node::Trap { address, tag });
            }
            Node::Resume { tag } => kept.push(Node::Resume { tag }),
            Node::Opaque(extension) => kept.push(Node::Opaque(extension)),
        }
    }

    kept.reverse();
    LiveResult {
        nodes: kept,
        live_in: live,
    }
}
