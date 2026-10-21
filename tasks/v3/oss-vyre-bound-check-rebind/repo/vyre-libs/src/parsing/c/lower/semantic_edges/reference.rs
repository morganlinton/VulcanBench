use super::*;

pub(crate) fn resolved_semantic_edges(
    vast_nodes: &[u32],
    node_idx: usize,
    node_count: usize,
    kind: u32,
) -> (SemanticEdge, SemanticEdge) {
    assert_vast_rows_present(vast_nodes, node_count);
    match kind {
        C_AST_KIND_GOTO_STMT => {
            let target = resolved_goto_target_label(vast_nodes, node_idx, node_count);
            if target == u32::MAX {
                (SemanticEdge::NONE, SemanticEdge::NONE)
            } else {
                (
                    SemanticEdge::new(C_AST_PG_EDGE_GOTO_TARGET, node_idx as u32, target),
                    SemanticEdge::NONE,
                )
            }
        }
        C_AST_KIND_SWITCH_STMT => {
            let selector = switch_selector_idx(vast_nodes, node_idx, node_count);
            if selector == u32::MAX {
                (SemanticEdge::NONE, SemanticEdge::NONE)
            } else {
                (
                    SemanticEdge::new(C_AST_PG_EDGE_SWITCH_SELECTOR, node_idx as u32, selector),
                    SemanticEdge::NONE,
                )
            }
        }
        C_AST_KIND_CASE_STMT => {
            let value = case_value_idx(vast_nodes, node_idx, node_count);
            let switch_idx = enclosing_switch_idx(vast_nodes, node_idx, node_count);
            let edge3 = if value == u32::MAX {
                SemanticEdge::NONE
            } else {
                SemanticEdge::new(C_AST_PG_EDGE_CASE_VALUE, node_idx as u32, value)
            };
            let edge4 = if switch_idx == u32::MAX {
                SemanticEdge::NONE
            } else {
                SemanticEdge::new(C_AST_PG_EDGE_SWITCH_CASE, switch_idx, node_idx as u32)
            };
            (edge3, edge4)
        }
        C_AST_KIND_DEFAULT_STMT => {
            let switch_idx = enclosing_switch_idx(vast_nodes, node_idx, node_count);
            if switch_idx == u32::MAX {
                (SemanticEdge::NONE, SemanticEdge::NONE)
            } else {
                (
                    SemanticEdge::new(C_AST_PG_EDGE_SWITCH_DEFAULT, switch_idx, node_idx as u32),
                    SemanticEdge::NONE,
                )
            }
        }
        _ => (SemanticEdge::NONE, SemanticEdge::NONE),
    }
}

/// Fail fast when the VAST row buffer is too short to cover `node_count` nodes.
///
/// `field_if_valid` returns the `u32::MAX` sentinel for an out-of-buffer word,
/// so resolving edges over a truncated buffer would SILENTLY emit `NONE` edges
/// (dropped control-flow/semantic edges) with no signal. Prevalidate the buffer
/// shape once at the entry so every downstream field read is in bounds.
fn assert_vast_rows_present(vast_nodes: &[u32], node_count: usize) {
    let required = node_count
        .checked_mul(VAST_NODE_STRIDE_U32 as usize)
        .expect("truncated VAST: node_count * row stride overflowed usize");
    assert!(
        vast_nodes.len() >= required,
        "truncated VAST: {} row word(s) for {node_count} node(s); need {required} \
         (node_count * {VAST_NODE_STRIDE_U32}). Fix: pass the full VAST node-row buffer — \
         resolving semantic edges over a truncated buffer would silently drop edges.",
        vast_nodes.len()
    );
}

pub(crate) fn field_if_valid(
    vast_nodes: &[u32],
    node_idx: usize,
    field: usize,
    node_count: usize,
) -> u32 {
    if node_idx >= node_count {
        return u32::MAX;
    }
    let Some(word_idx) = node_idx
        .checked_mul(VAST_NODE_STRIDE_U32 as usize)
        .and_then(|base| base.checked_add(field))
    else {
        return u32::MAX;
    };
    *vast_nodes.get(word_idx).unwrap_or(&u32::MAX)
}

pub(crate) fn root_idx(vast_nodes: &[u32], node_idx: usize, node_count: usize) -> u32 {
    if node_idx >= node_count {
        return u32::MAX;
    }
    let mut root = node_idx as u32;
    let mut parent = field_if_valid(vast_nodes, node_idx, IDX_PARENT, node_count);
    for _ in 0..node_count {
        let Ok(parent_idx) = usize::try_from(parent) else {
            break;
        };
        if parent_idx >= node_count {
            break;
        }
        root = parent;
        parent = field_if_valid(vast_nodes, parent_idx, IDX_PARENT, node_count);
    }
    root
}

pub(crate) fn resolved_goto_target_label(
    vast_nodes: &[u32],
    node_idx: usize,
    node_count: usize,
) -> u32 {
    let target_idx = field_if_valid(vast_nodes, node_idx, IDX_NEXT_SIBLING, node_count);
    let Ok(target_idx) = usize::try_from(target_idx) else {
        return u32::MAX;
    };
    if target_idx >= node_count {
        return u32::MAX;
    }
    let target_hash = field_if_valid(vast_nodes, target_idx, IDX_SYMBOL_HASH, node_count);
    if target_hash == 0 {
        return u32::MAX;
    }
    let current_root = root_idx(vast_nodes, node_idx, node_count);
    for candidate_idx in 0..node_count {
        if field_if_valid(vast_nodes, candidate_idx, IDX_KIND, node_count) != C_AST_KIND_LABEL_STMT
        {
            continue;
        }
        if field_if_valid(vast_nodes, candidate_idx, IDX_SYMBOL_HASH, node_count) == target_hash
            && root_idx(vast_nodes, candidate_idx, node_count) == current_root
        {
            return candidate_idx as u32;
        }
    }
    u32::MAX
}

pub(crate) fn switch_selector_idx(vast_nodes: &[u32], node_idx: usize, node_count: usize) -> u32 {
    let condition_group = field_if_valid(vast_nodes, node_idx, IDX_NEXT_SIBLING, node_count);
    let Ok(condition_group) = usize::try_from(condition_group) else {
        return u32::MAX;
    };
    let selector = field_if_valid(vast_nodes, condition_group, IDX_FIRST_CHILD, node_count);
    let Ok(selector_idx) = usize::try_from(selector) else {
        return u32::MAX;
    };
    if selector_idx >= node_count {
        return u32::MAX;
    }
    if field_if_valid(vast_nodes, selector_idx, IDX_PARENT, node_count) != condition_group as u32 {
        return u32::MAX;
    }
    selector
}

pub(crate) fn switch_body_idx(vast_nodes: &[u32], switch_idx: usize, node_count: usize) -> u32 {
    let condition_group = field_if_valid(vast_nodes, switch_idx, IDX_NEXT_SIBLING, node_count);
    let Ok(condition_group) = usize::try_from(condition_group) else {
        return u32::MAX;
    };
    let body = field_if_valid(vast_nodes, condition_group, IDX_NEXT_SIBLING, node_count);
    let Ok(body_idx) = usize::try_from(body) else {
        return u32::MAX;
    };
    if body_idx >= node_count {
        return u32::MAX;
    }
    body
}

pub(crate) fn case_value_idx(vast_nodes: &[u32], node_idx: usize, node_count: usize) -> u32 {
    let value = field_if_valid(vast_nodes, node_idx, IDX_NEXT_SIBLING, node_count);
    let Ok(value_idx) = usize::try_from(value) else {
        return u32::MAX;
    };
    if value_idx >= node_count {
        return u32::MAX;
    }
    let case_parent = field_if_valid(vast_nodes, node_idx, IDX_PARENT, node_count);
    if case_parent == u32::MAX {
        return u32::MAX;
    }
    if field_if_valid(vast_nodes, value_idx, IDX_PARENT, node_count) != case_parent {
        return u32::MAX;
    }
    value
}

pub(crate) fn enclosing_switch_idx(vast_nodes: &[u32], node_idx: usize, node_count: usize) -> u32 {
    let parent = field_if_valid(vast_nodes, node_idx, IDX_PARENT, node_count);
    if let Some(switch_idx) = ancestor_switch_idx(vast_nodes, parent, node_count) {
        return switch_idx;
    }

    let mut resolved = u32::MAX;
    let mut best_distance = usize::MAX;
    for candidate_idx in 0..node_count {
        if field_if_valid(vast_nodes, candidate_idx, IDX_KIND, node_count) != C_AST_KIND_SWITCH_STMT
        {
            continue;
        }
        let distance = ancestor_distance(
            vast_nodes,
            parent,
            switch_body_idx(vast_nodes, candidate_idx, node_count),
            node_count,
        );
        if let Some(distance) = distance {
            if distance < best_distance {
                best_distance = distance;
                resolved = candidate_idx as u32;
            }
        }
    }
    resolved
}

pub(crate) fn ancestor_switch_idx(
    vast_nodes: &[u32],
    mut node: u32,
    node_count: usize,
) -> Option<u32> {
    for _ in 0..node_count {
        let Ok(node_idx) = usize::try_from(node) else {
            return None;
        };
        if node_idx >= node_count {
            return None;
        }
        if field_if_valid(vast_nodes, node_idx, IDX_KIND, node_count) == C_AST_KIND_SWITCH_STMT {
            return Some(node);
        }
        node = field_if_valid(vast_nodes, node_idx, IDX_PARENT, node_count);
    }
    None
}

pub(crate) fn ancestor_distance(
    vast_nodes: &[u32],
    mut node: u32,
    ancestor: u32,
    node_count: usize,
) -> Option<usize> {
    if node == u32::MAX || ancestor == u32::MAX {
        return None;
    }
    for distance in 0..node_count {
        if node == ancestor {
            return Some(distance);
        }
        let Ok(node_idx) = usize::try_from(node) else {
            return None;
        };
        if node_idx >= node_count {
            return None;
        }
        node = field_if_valid(vast_nodes, node_idx, IDX_PARENT, node_count);
    }
    None
}
