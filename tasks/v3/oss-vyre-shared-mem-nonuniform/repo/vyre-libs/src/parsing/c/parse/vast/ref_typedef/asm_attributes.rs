use super::*;

pub(super) fn enclosing_brace_is_initializer_list(vast_nodes: &[u32], cur_parent: u32) -> bool {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let Ok(parent_idx) = usize::try_from(cur_parent) else {
        return false;
    };
    if parent_idx >= node_count || kind_at(vast_nodes, parent_idx) != TOK_LBRACE {
        return false;
    }

    let parent_parent = parent_at(vast_nodes, parent_idx);
    let parent_prev = previous_sibling_context(vast_nodes, parent_idx, parent_parent);
    if matches!(parent_prev.kind, TOK_ASSIGN | TOK_COMMA) {
        return true;
    }
    if parent_prev.kind != TOK_LBRACE {
        return false;
    }

    let Ok(grandparent_idx) = usize::try_from(parent_parent) else {
        return false;
    };
    if grandparent_idx >= node_count || kind_at(vast_nodes, grandparent_idx) != TOK_LBRACE {
        return false;
    }
    let grandparent_parent = parent_at(vast_nodes, grandparent_idx);
    let grandparent_prev =
        previous_sibling_context(vast_nodes, grandparent_idx, grandparent_parent);
    matches!(grandparent_prev.kind, TOK_ASSIGN | TOK_COMMA | TOK_LBRACE)
}

pub(super) fn reference_c_asm_context_kind(
    vast_nodes: &[u32],
    node_idx: usize,
    raw_kind: u32,
    cur_parent: u32,
) -> Option<u32> {
    let Ok(parent_idx) = usize::try_from(cur_parent) else {
        return None;
    };
    if parent_idx >= vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize
        || kind_at(vast_nodes, parent_idx) != TOK_LPAREN
    {
        return None;
    }
    let parent_parent = parent_at(vast_nodes, parent_idx);
    if !asm_prefix_before(vast_nodes, parent_idx, parent_parent) {
        return None;
    }
    let colon_count = sibling_colons_before(vast_nodes, node_idx, cur_parent);
    let prev = previous_sibling_context(vast_nodes, node_idx, cur_parent);
    match raw_kind {
        TOK_STRING if colon_count == 0 => Some(C_AST_KIND_ASM_TEMPLATE),
        TOK_STRING if colon_count >= 3 => Some(C_AST_KIND_ASM_CLOBBERS_LIST),
        TOK_LPAREN if prev.kind == TOK_STRING && colon_count == 1 => {
            Some(C_AST_KIND_ASM_OUTPUT_OPERAND)
        }
        TOK_LPAREN if prev.kind == TOK_STRING && colon_count == 2 => {
            Some(C_AST_KIND_ASM_INPUT_OPERAND)
        }
        TOK_IDENTIFIER
            if colon_count >= 4
                && asm_has_goto_qualifier_before(vast_nodes, parent_idx, parent_parent) =>
        {
            Some(C_AST_KIND_ASM_GOTO_LABELS)
        }
        _ => None,
    }
}

pub(super) fn asm_prefix_before(vast_nodes: &[u32], before_idx: usize, parent: u32) -> bool {
    for scan_idx in (0..before_idx).rev() {
        if parent_at(vast_nodes, scan_idx) != parent {
            continue;
        }
        match kind_at(vast_nodes, scan_idx) {
            TOK_GNU_ASM => return true,
            TOK_VOLATILE | TOK_GOTO => continue,
            _ => return false,
        }
    }
    false
}

pub(super) fn asm_has_goto_qualifier_before(
    vast_nodes: &[u32],
    before_idx: usize,
    parent: u32,
) -> bool {
    let mut saw_goto = false;
    for scan_idx in (0..before_idx).rev() {
        if parent_at(vast_nodes, scan_idx) != parent {
            continue;
        }
        match kind_at(vast_nodes, scan_idx) {
            TOK_GOTO => saw_goto = true,
            TOK_VOLATILE => continue,
            TOK_GNU_ASM => return saw_goto,
            _ => return false,
        }
    }
    false
}

pub(super) fn sibling_colons_before(vast_nodes: &[u32], node_idx: usize, cur_parent: u32) -> u32 {
    let mut colons = 0u32;
    for scan_idx in 0..node_idx {
        if parent_at(vast_nodes, scan_idx) == cur_parent
            && kind_at(vast_nodes, scan_idx) == TOK_COLON
        {
            colons = colons.saturating_add(1);
        }
    }
    colons
}

pub(super) fn reference_c_attribute_kind(
    vast_nodes: &[u32],
    node_idx: usize,
    raw_kind: u32,
    cur_parent: u32,
) -> Option<u32> {
    if !matches!(raw_kind, TOK_IDENTIFIER | TOK_CONST) {
        return None;
    }
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let Ok(parent_idx) = usize::try_from(cur_parent) else {
        return None;
    };
    if parent_idx >= node_count || kind_at(vast_nodes, parent_idx) != TOK_LPAREN {
        return None;
    }
    let parent_parent = parent_at(vast_nodes, parent_idx);
    let Ok(parent_parent_idx) = usize::try_from(parent_parent) else {
        return None;
    };
    if parent_parent_idx >= node_count || kind_at(vast_nodes, parent_parent_idx) != TOK_LPAREN {
        return None;
    }
    let grand_parent = parent_at(vast_nodes, parent_parent_idx);
    let attr_prefix = previous_sibling_context(vast_nodes, parent_parent_idx, grand_parent);
    let adjacent_attr_prefix = parent_parent_idx > 0
        && kind_at(vast_nodes, parent_parent_idx.saturating_sub(1)) == TOK_GNU_ATTRIBUTE;
    if attr_prefix.kind != TOK_GNU_ATTRIBUTE && !adjacent_attr_prefix {
        return None;
    }
    if raw_kind == TOK_CONST {
        return Some(C_AST_KIND_ATTRIBUTE_CONST);
    }

    let symbol = symbol_hash_at(vast_nodes, node_idx);
    C_ATTRIBUTE_KIND_HASHES
        .iter()
        .find_map(|(hash, kind)| (*hash == symbol).then_some(*kind))
}

pub(super) fn reference_c_direct_attribute_kind(
    vast_nodes: &[u32],
    raw_kind: u32,
    cur_parent: u32,
    symbol: u32,
) -> Option<u32> {
    if !matches!(raw_kind, TOK_IDENTIFIER | TOK_CONST) {
        return None;
    }
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let parent_idx = usize::try_from(cur_parent).ok()?;
    if parent_idx >= node_count || kind_at(vast_nodes, parent_idx) != TOK_LPAREN {
        return None;
    }
    let parent_parent = parent_at(vast_nodes, parent_idx);
    let parent_parent_idx = usize::try_from(parent_parent).ok()?;
    if parent_parent_idx == 0
        || parent_parent_idx >= node_count
        || kind_at(vast_nodes, parent_parent_idx) != TOK_LPAREN
        || kind_at(vast_nodes, parent_parent_idx - 1) != TOK_GNU_ATTRIBUTE
    {
        return None;
    }
    if raw_kind == TOK_CONST {
        return Some(C_AST_KIND_ATTRIBUTE_CONST);
    }
    C_ATTRIBUTE_KIND_HASHES
        .iter()
        .find_map(|(hash, kind)| (*hash == symbol).then_some(*kind))
}

pub(super) fn reference_c_builtin_expression_kind(token: u32) -> Option<u32> {
    match token {
        TOK_BUILTIN_CONSTANT_P => Some(C_AST_KIND_BUILTIN_CONSTANT_P_EXPR),
        TOK_BUILTIN_CHOOSE_EXPR => Some(C_AST_KIND_BUILTIN_CHOOSE_EXPR),
        TOK_BUILTIN_TYPES_COMPATIBLE_P => Some(C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR),
        TOK_GENERIC => Some(C_AST_KIND_GENERIC_SELECTION_EXPR),
        TOK_ELLIPSIS => Some(C_AST_KIND_RANGE_DESIGNATOR_EXPR),
        _ => None,
    }
}

pub(super) fn reference_c_builtin_identifier_expression_kind(
    raw_kind: u32,
    symbol: u32,
    next_kind: u32,
) -> Option<u32> {
    if raw_kind != TOK_IDENTIFIER || next_kind != TOK_LPAREN {
        return None;
    }
    if is_gnu_typeof_hash_raw(symbol) {
        return Some(C_AST_KIND_SIZEOF_EXPR);
    }
    if let Some(kind) =
        crate::parsing::c::parse::gnu_builtin_catalog::classify_gnu_builtin_hash(symbol)
    {
        return Some(kind);
    }
    match symbol {
        0x749d_f71e => Some(C_AST_KIND_BUILTIN_EXPECT_EXPR),
        0xdcec_13f5 => Some(C_AST_KIND_BUILTIN_OFFSETOF_EXPR),
        0x7900_03c8 => Some(C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR),
        0x21a7_53f0 => Some(C_AST_KIND_BUILTIN_PREFETCH_EXPR),
        0x4a9a_c967 => Some(C_AST_KIND_BUILTIN_UNREACHABLE_STMT),
        0x7f55_6bd5 | 0xb0bc_f282 | 0x8cc7_b276 => Some(C_AST_KIND_BUILTIN_OVERFLOW_EXPR),
        0x3909_1622 => Some(C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR),
        _ => None,
    }
}
