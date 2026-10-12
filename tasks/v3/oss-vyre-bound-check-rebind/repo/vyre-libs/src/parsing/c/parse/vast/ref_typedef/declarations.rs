use super::*;

pub(super) fn visible_declaration_kind(
    vast_nodes: &[u32],
    node_idx: usize,
    haystack: &[u8],
    name: &[u8],
) -> u32 {
    let current_scope = scope_open_before(vast_nodes, node_idx);
    let current_function = enclosing_function_lparen(vast_nodes, node_idx);

    for scan_idx in (0..node_idx).rev() {
        if identifier_lexeme(vast_nodes, scan_idx, haystack) != Some(name) {
            continue;
        }
        let decl_kind = declaration_kind_at(vast_nodes, scan_idx, haystack);
        if decl_kind == 0 {
            continue;
        }
        if decl_kind == 2 {
            let decl_function = enclosing_function_lparen(vast_nodes, scan_idx);
            if decl_function != SENTINEL && decl_function != current_function {
                continue;
            }
            if let Some(scope_end) = for_init_scope_end(vast_nodes, scan_idx) {
                if node_idx > scope_end {
                    continue;
                }
            }
        }
        let decl_scope = scope_open_before(vast_nodes, scan_idx);
        if scope_is_visible_from(vast_nodes, decl_scope, current_scope) {
            return decl_kind;
        }
    }

    0
}

pub(super) fn scope_is_visible_from(
    vast_nodes: &[u32],
    decl_scope: u32,
    current_scope: u32,
) -> bool {
    if decl_scope == SENTINEL {
        return true;
    }
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut scope = current_scope;
    for _ in 0..node_count {
        if scope == decl_scope {
            return true;
        }
        if scope == SENTINEL {
            return false;
        }
        let Ok(scope_idx) = usize::try_from(scope) else {
            return false;
        };
        if scope_idx >= node_count {
            return false;
        }
        scope = parent_at(vast_nodes, scope_idx);
    }
    false
}

pub(super) fn declaration_kind_at(vast_nodes: &[u32], node_idx: usize, haystack: &[u8]) -> u32 {
    if kind_at(vast_nodes, node_idx) != TOK_IDENTIFIER {
        return 0;
    }
    let prev_kind = if node_idx > 0 {
        kind_at(vast_nodes, node_idx - 1)
    } else {
        SENTINEL
    };
    let next_kind = if node_idx + 1 < vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize {
        kind_at(vast_nodes, node_idx + 1)
    } else {
        SENTINEL
    };
    if matches!(
        prev_kind,
        TOK_STRUCT | TOK_UNION | TOK_ENUM | TOK_DOT | TOK_ARROW
    ) || next_kind == TOK_COLON
    {
        return 0;
    }
    if prev_kind == TOK_LPAREN
        && node_idx >= 2
        && matches!(
            kind_at(vast_nodes, node_idx.saturating_sub(2)),
            TOK_SIZEOF | TOK_GNU_TYPEOF | TOK_GNU_TYPEOF_UNQUAL | TOK_ALIGNOF
        )
    {
        return 0;
    }
    if prev_kind == TOK_STAR
        && node_idx >= 2
        && kind_at(vast_nodes, node_idx.saturating_sub(2)) == TOK_RPAREN
    {
        return 0;
    }
    if parent_context(vast_nodes, parent_at(vast_nodes, node_idx)).is_record_body {
        return 0;
    }

    let mut has_typedef = false;
    let mut has_type = false;
    let mut skipped_paren_depth = 0u32;
    let mut skipped_brace_depth = 0u32;
    for scan_idx in (0..node_idx).rev() {
        let scan_kind = kind_at(vast_nodes, scan_idx);
        if scan_kind == TOK_RBRACE {
            skipped_brace_depth = skipped_brace_depth.saturating_add(1);
            continue;
        }
        if skipped_brace_depth != 0 {
            if scan_kind == TOK_LBRACE {
                skipped_brace_depth = skipped_brace_depth.saturating_sub(1);
            }
            continue;
        }
        if scan_kind == TOK_RPAREN {
            skipped_paren_depth = skipped_paren_depth.saturating_add(1);
            continue;
        }
        if skipped_paren_depth != 0 {
            if scan_kind == TOK_LPAREN {
                skipped_paren_depth = skipped_paren_depth.saturating_sub(1);
            }
            continue;
        }
        if is_decl_prefix_reset_raw(scan_kind) {
            break;
        }
        if scan_kind == TOK_TYPEDEF {
            has_typedef = true;
        }
        if is_decl_prefix_at(vast_nodes, scan_idx) {
            has_type = true;
        }
        if scan_kind == TOK_IDENTIFIER {
            if let Some(name) = identifier_lexeme(vast_nodes, scan_idx, haystack) {
                if visible_declaration_kind(vast_nodes, scan_idx, haystack, name) == 1 {
                    has_type = true;
                }
            }
        }
    }

    let declarator_follower = matches!(
        next_kind,
        TOK_SEMICOLON | TOK_COMMA | TOK_ASSIGN | TOK_LBRACKET | TOK_LPAREN | TOK_RPAREN
    );
    if declarator_follower && prev_kind == TOK_IDENTIFIER {
        if let Some(prev_name) = identifier_lexeme(vast_nodes, node_idx - 1, haystack) {
            if visible_declaration_kind(vast_nodes, node_idx - 1, haystack, prev_name) == 1 {
                return 2;
            }
        }
    }
    if declarator_follower && (has_typedef || has_type) {
        if has_typedef {
            1
        } else {
            2
        }
    } else {
        0
    }
}
