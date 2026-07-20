use super::*;

pub(super) fn scope_open_before(vast_nodes: &[u32], node_idx: usize) -> u32 {
    let mut depth = 0u32;
    for scan_idx in (0..node_idx).rev() {
        match kind_at(vast_nodes, scan_idx) {
            TOK_RBRACE => depth = depth.saturating_add(1),
            TOK_LBRACE => {
                if depth == 0 {
                    return scan_idx as u32;
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    SENTINEL
}

pub(super) fn enclosing_function_lparen(vast_nodes: &[u32], node_idx: usize) -> u32 {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut parent = parent_at(vast_nodes, node_idx);
    for _ in 0..node_count {
        let Ok(parent_idx) = usize::try_from(parent) else {
            break;
        };
        if parent_idx >= node_count {
            break;
        }
        if kind_at(vast_nodes, parent_idx) == TOK_LPAREN
            && lparen_starts_function_declarator(vast_nodes, parent_idx)
        {
            return parent;
        }
        parent = parent_at(vast_nodes, parent_idx);
    }

    let mut scope = scope_open_before(vast_nodes, node_idx);
    for _ in 0..node_count {
        let Ok(scope_idx) = usize::try_from(scope) else {
            break;
        };
        if scope_idx >= node_count || kind_at(vast_nodes, scope_idx) != TOK_LBRACE {
            break;
        }
        let candidate = function_lparen_before(vast_nodes, scope_idx);
        if candidate != SENTINEL {
            return candidate;
        }
        scope = parent_at(vast_nodes, scope_idx);
    }

    SENTINEL
}

pub(super) fn function_lparen_before(vast_nodes: &[u32], before_idx: usize) -> u32 {
    let mut depth = 0u32;
    for scan_idx in (0..before_idx).rev() {
        match kind_at(vast_nodes, scan_idx) {
            TOK_RPAREN => depth = depth.saturating_add(1),
            TOK_LPAREN => {
                if depth == 0 {
                    continue;
                }
                depth = depth.saturating_sub(1);
                if depth == 0 && lparen_starts_function_declarator(vast_nodes, scan_idx) {
                    return scan_idx as u32;
                }
            }
            _ => {}
        }
    }
    SENTINEL
}

pub(super) fn lparen_starts_function_declarator(vast_nodes: &[u32], lparen_idx: usize) -> bool {
    lparen_idx > 0 && kind_at(vast_nodes, lparen_idx - 1) == TOK_IDENTIFIER
}

pub(super) fn for_init_scope_end(vast_nodes: &[u32], decl_idx: usize) -> Option<usize> {
    let control_lparen = enclosing_for_control_lparen(vast_nodes, decl_idx)?;
    let control_rparen = matching_raw_rparen(vast_nodes, control_lparen)?;
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let after_control = control_rparen.saturating_add(1);
    if after_control >= node_count {
        return Some(control_rparen);
    }
    match kind_at(vast_nodes, after_control) {
        TOK_LBRACE => matching_raw_rbrace(vast_nodes, control_rparen + 1),
        TOK_SEMICOLON => Some(control_rparen + 1),
        _ => Some(control_rparen),
    }
}

pub(super) fn enclosing_for_control_lparen(vast_nodes: &[u32], node_idx: usize) -> Option<usize> {
    let mut depth = 0u32;
    for scan_idx in (0..node_idx).rev() {
        match kind_at(vast_nodes, scan_idx) {
            TOK_RPAREN => depth = depth.saturating_add(1),
            TOK_LPAREN => {
                if depth == 0 {
                    return (scan_idx > 0 && kind_at(vast_nodes, scan_idx - 1) == TOK_FOR)
                        .then_some(scan_idx);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

pub(super) fn matching_raw_rparen(vast_nodes: &[u32], lparen_idx: usize) -> Option<usize> {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut depth = 1u32;
    for scan_idx in (lparen_idx + 1)..node_count {
        match kind_at(vast_nodes, scan_idx) {
            TOK_LPAREN => depth = depth.saturating_add(1),
            TOK_RPAREN => {
                if depth == 1 {
                    return Some(scan_idx);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

pub(super) fn matching_raw_rbrace(vast_nodes: &[u32], lbrace_idx: usize) -> Option<usize> {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut depth = 1u32;
    for scan_idx in (lbrace_idx + 1)..node_count {
        match kind_at(vast_nodes, scan_idx) {
            TOK_LBRACE => depth = depth.saturating_add(1),
            TOK_RBRACE => {
                if depth == 1 {
                    return Some(scan_idx);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

pub(super) fn c99_for_init_statement_assign(
    vast_nodes: &[u32],
    raw_kind: u32,
    cur_parent: u32,
    effective_has_decl_prefix: bool,
) -> bool {
    if raw_kind != TOK_ASSIGN || !effective_has_decl_prefix {
        return false;
    }
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let Ok(control_lparen_idx) = usize::try_from(cur_parent) else {
        return false;
    };
    if control_lparen_idx >= node_count || kind_at(vast_nodes, control_lparen_idx) != TOK_LPAREN {
        return false;
    }
    let control_parent = parent_at(vast_nodes, control_lparen_idx);
    let Ok(control_parent_idx) = usize::try_from(control_parent) else {
        return false;
    };
    if control_parent_idx >= node_count {
        return false;
    }
    let control_parent_kind = kind_at(vast_nodes, control_parent_idx);
    if control_parent_kind == TOK_FOR {
        return true;
    }
    if control_parent_kind != TOK_LBRACE {
        return false;
    }
    previous_sibling_context(vast_nodes, control_lparen_idx, control_parent).kind == TOK_FOR
}
