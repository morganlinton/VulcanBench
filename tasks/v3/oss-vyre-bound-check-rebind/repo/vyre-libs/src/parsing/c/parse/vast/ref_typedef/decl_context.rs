use super::*;

pub(super) struct DeclContext {
    pub(super) has_prefix: bool,
}

pub(super) fn decl_context_before(
    vast_nodes: &[u32],
    node_idx: usize,
    cur_parent: u32,
) -> DeclContext {
    let mut has_decl_prefix = false;
    let mut last_kind = SENTINEL;
    let mut prev_kind = SENTINEL;
    for scan_idx in 0..node_idx {
        let scan_parent = parent_at(vast_nodes, scan_idx);
        if scan_parent != cur_parent {
            continue;
        }
        let scan_kind = kind_at(vast_nodes, scan_idx);
        let aggregate_body_open =
            is_aggregate_specifier_body_open_raw(scan_kind, last_kind, prev_kind);
        if is_decl_prefix_reset_raw(scan_kind) {
            has_decl_prefix = false;
        }
        let scan_typedef_flags = flags_at(vast_nodes, scan_idx);
        if is_decl_prefix_at(vast_nodes, scan_idx)
            || aggregate_body_open
            || (scan_kind == TOK_IDENTIFIER
                && (scan_typedef_flags & C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME) != 0)
        {
            has_decl_prefix = true;
        }
        prev_kind = last_kind;
        last_kind = scan_kind;
    }
    DeclContext {
        has_prefix: has_decl_prefix,
    }
}

pub(super) fn declaration_initializer_prefix_before(
    vast_nodes: &[u32],
    node_idx: usize,
    cur_parent: u32,
) -> bool {
    let mut has_decl_prefix = false;
    for scan_idx in 0..node_idx {
        if parent_at(vast_nodes, scan_idx) != cur_parent {
            continue;
        }
        let scan_kind = kind_at(vast_nodes, scan_idx);
        if matches!(scan_kind, TOK_SEMICOLON | TOK_LBRACE | TOK_RBRACE) {
            has_decl_prefix = false;
        }
        if is_decl_prefix_at(vast_nodes, scan_idx) {
            has_decl_prefix = true;
        }
    }
    has_decl_prefix
}

pub(super) fn suffix_function_boundary_kind(
    vast_nodes: &[u32],
    start_idx: u32,
    max_steps: usize,
) -> u32 {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut scan_idx = start_idx;
    for _ in 0..max_steps {
        let Ok(idx) = usize::try_from(scan_idx) else {
            return SENTINEL;
        };
        if idx >= node_count {
            return SENTINEL;
        }
        let scan_kind = kind_at(vast_nodes, idx);
        if matches!(scan_kind, TOK_LBRACE | TOK_SEMICOLON) {
            return scan_kind;
        }
        if scan_kind == TOK_RPAREN {
            let scan_parent = parent_at(vast_nodes, idx);
            if let Ok(parent_idx) = usize::try_from(scan_parent) {
                let parent_next = next_sibling_at(vast_nodes, parent_idx);
                if let Ok(parent_next_idx) = usize::try_from(parent_next) {
                    if parent_next_idx < node_count {
                        let parent_next_kind = kind_at(vast_nodes, parent_next_idx);
                        if matches!(parent_next_kind, TOK_LPAREN | TOK_LBRACKET | TOK_SEMICOLON) {
                            return parent_next_kind;
                        }
                        if parent_next_kind == TOK_RPAREN {
                            let parent_next_parent = parent_at(vast_nodes, parent_next_idx);
                            if let Ok(parent_next_parent_idx) = usize::try_from(parent_next_parent)
                            {
                                let outer_next =
                                    next_sibling_at(vast_nodes, parent_next_parent_idx);
                                if let Ok(outer_next_idx) = usize::try_from(outer_next) {
                                    if outer_next_idx < node_count {
                                        let outer_next_kind = kind_at(vast_nodes, outer_next_idx);
                                        if matches!(
                                            outer_next_kind,
                                            TOK_LPAREN | TOK_LBRACKET | TOK_SEMICOLON
                                        ) {
                                            return outer_next_kind;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        scan_idx = next_sibling_at(vast_nodes, idx);
    }
    SENTINEL
}

pub(super) fn is_decl_prefix_raw(token: u32) -> bool {
    matches!(
        token,
        TOK_TYPEDEF
            | TOK_EXTERN
            | TOK_STATIC
            | TOK_INLINE
            | TOK_CONST
            | TOK_RESTRICT
            | TOK_VOLATILE
            | TOK_STRUCT
            | TOK_UNION
            | TOK_ENUM
            | TOK_VOID
            | TOK_CHAR_KW
            | TOK_INT
            | TOK_LONG
            | TOK_SHORT
            | TOK_SIGNED
            | TOK_UNSIGNED
            | TOK_FLOAT_KW
            | TOK_DOUBLE
            | TOK_BOOL
            | TOK_COMPLEX
            | TOK_IMAGINARY
            | TOK_ALIGNAS
            | TOK_ATOMIC
            | TOK_GNU_TYPEOF
            | TOK_GNU_AUTO_TYPE
            | TOK_GNU_EXTENSION
            | TOK_NORETURN
            | TOK_STATIC_ASSERT
            | TOK_THREAD_LOCAL
            | TOK_GNU_TYPEOF_UNQUAL
            | TOK_GNU_INT128
            | TOK_GNU_BUILTIN_VA_LIST
            | TOK_GNU_ADDRESS_SPACE
    )
}

pub(super) fn is_aggregate_specifier_body_open_raw(
    open_kind: u32,
    prev_kind: u32,
    prev_prev_kind: u32,
) -> bool {
    open_kind == TOK_LBRACE
        && (matches!(prev_kind, TOK_STRUCT | TOK_UNION | TOK_ENUM)
            || (prev_kind == TOK_IDENTIFIER
                && matches!(prev_prev_kind, TOK_STRUCT | TOK_UNION | TOK_ENUM)))
}

pub(super) fn is_type_name_start_raw(token: u32) -> bool {
    matches!(
        token,
        TOK_CONST
            | TOK_VOLATILE
            | TOK_STRUCT
            | TOK_UNION
            | TOK_ENUM
            | TOK_VOID
            | TOK_CHAR_KW
            | TOK_INT
            | TOK_LONG
            | TOK_SHORT
            | TOK_SIGNED
            | TOK_UNSIGNED
            | TOK_FLOAT_KW
            | TOK_DOUBLE
            | TOK_BOOL
            | TOK_COMPLEX
            | TOK_IMAGINARY
            | TOK_ATOMIC
            | TOK_RESTRICT
            | TOK_GNU_TYPEOF
            | TOK_GNU_TYPEOF_UNQUAL
            | TOK_GNU_INT128
            | TOK_GNU_BUILTIN_VA_LIST
    )
}

pub(super) fn is_decl_prefix_reset_raw(token: u32) -> bool {
    matches!(
        token,
        TOK_SEMICOLON | TOK_LBRACE | TOK_RBRACE | TOK_ASSIGN | TOK_COLON
    )
}
