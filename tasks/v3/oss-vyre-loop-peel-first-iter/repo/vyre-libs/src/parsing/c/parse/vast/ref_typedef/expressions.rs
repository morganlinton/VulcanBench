use super::*;

pub(super) fn reference_effective_expression_prev_kind(prev_kind: u32, prev_prev_kind: u32) -> u32 {
    if prev_kind == TOK_LPAREN
        && matches!(
            prev_prev_kind,
            TOK_SIZEOF | TOK_ALIGNOF | TOK_GNU_TYPEOF | TOK_GNU_TYPEOF_UNQUAL
        )
    {
        TOK_RPAREN
    } else {
        prev_kind
    }
}

pub(super) fn reference_c_expression_operator_kind(
    token: u32,
    prev_kind: u32,
    prev_prev_kind: u32,
) -> Option<u32> {
    let effective_prev_kind = reference_effective_expression_prev_kind(prev_kind, prev_prev_kind);
    let unary_context = reference_c_unary_context(effective_prev_kind);
    match token {
        TOK_ASSIGN | TOK_PLUS_EQ | TOK_MINUS_EQ | TOK_STAR_EQ | TOK_SLASH_EQ | TOK_PERCENT_EQ
        | TOK_AMP_EQ | TOK_PIPE_EQ | TOK_CARET_EQ | TOK_LSHIFT_EQ | TOK_RSHIFT_EQ => {
            Some(C_AST_KIND_ASSIGN_EXPR)
        }
        TOK_DOT | TOK_ARROW => Some(C_AST_KIND_MEMBER_ACCESS_EXPR),
        TOK_LBRACKET if reference_c_can_end_expression(effective_prev_kind) => {
            Some(C_AST_KIND_ARRAY_SUBSCRIPT_EXPR)
        }
        TOK_SIZEOF | TOK_GNU_TYPEOF | TOK_GNU_TYPEOF_UNQUAL => Some(C_AST_KIND_SIZEOF_EXPR),
        TOK_ALIGNOF => Some(C_AST_KIND_ALIGNOF_EXPR),
        TOK_QUESTION => Some(C_AST_KIND_CONDITIONAL_EXPR),
        TOK_INC | TOK_DEC if unary_context => Some(C_AST_KIND_UNARY_EXPR),
        TOK_STAR | TOK_AMP | TOK_PLUS | TOK_MINUS | TOK_BANG | TOK_TILDE | TOK_GNU_REAL
        | TOK_GNU_IMAG
            if unary_context =>
        {
            Some(C_AST_KIND_UNARY_EXPR)
        }
        TOK_PLUS | TOK_MINUS | TOK_STAR | TOK_SLASH | TOK_PERCENT | TOK_AMP | TOK_PIPE
        | TOK_CARET | TOK_EQ | TOK_NE | TOK_LE | TOK_GE | TOK_AND | TOK_OR | TOK_LSHIFT
        | TOK_RSHIFT | TOK_LT | TOK_GT
            if !unary_context =>
        {
            Some(node_kind::BINARY)
        }
        _ => None,
    }
}

pub(super) struct SiblingContext {
    pub(super) idx: u32,
    pub(super) kind: u32,
    pub(super) prev_kind: u32,
    pub(super) flags: u32,
    pub(super) symbol_hash: u32,
}

pub(super) fn previous_sibling_context(
    vast_nodes: &[u32],
    node_idx: usize,
    cur_parent: u32,
) -> SiblingContext {
    let mut prev_idx = SENTINEL;
    let mut prev_kind = SENTINEL;
    let mut prev_prev_kind = SENTINEL;
    let mut prev_flags = 0;
    let mut prev_symbol_hash = 0;
    for scan_idx in 0..node_idx {
        let scan_parent = parent_at(vast_nodes, scan_idx);
        if scan_parent == cur_parent {
            prev_prev_kind = prev_kind;
            prev_kind = kind_at(vast_nodes, scan_idx);
            prev_flags = flags_at(vast_nodes, scan_idx);
            prev_symbol_hash = symbol_hash_at(vast_nodes, scan_idx);
            prev_idx = scan_idx as u32;
        }
    }
    SiblingContext {
        idx: prev_idx,
        kind: prev_kind,
        prev_kind: prev_prev_kind,
        flags: prev_flags,
        symbol_hash: prev_symbol_hash,
    }
}

pub(super) struct ParentContext {
    pub(super) is_record_body: bool,
    pub(super) is_enum_body: bool,
}

pub(super) fn parent_context(vast_nodes: &[u32], cur_parent: u32) -> ParentContext {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let Ok(parent_idx) = usize::try_from(cur_parent) else {
        return ParentContext {
            is_record_body: false,
            is_enum_body: false,
        };
    };
    if parent_idx >= node_count || kind_at(vast_nodes, parent_idx) != TOK_LBRACE {
        return ParentContext {
            is_record_body: false,
            is_enum_body: false,
        };
    }

    let parent_parent = parent_at(vast_nodes, parent_idx);
    let aggregate_prefix = aggregate_prefix_before_open(vast_nodes, parent_idx, parent_parent);
    let is_record_body = aggregate_prefix == AggregatePrefix::Record;
    let is_enum_body = aggregate_prefix == AggregatePrefix::Enum;

    ParentContext {
        is_record_body,
        is_enum_body,
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum AggregatePrefix {
    None,
    Record,
    Enum,
}

pub(super) fn aggregate_prefix_before_open(
    vast_nodes: &[u32],
    open_idx: usize,
    open_parent: u32,
) -> AggregatePrefix {
    let mut prefix = AggregatePrefix::None;
    for scan_idx in 0..open_idx {
        if parent_at(vast_nodes, scan_idx) != open_parent {
            continue;
        }
        match kind_at(vast_nodes, scan_idx) {
            TOK_STRUCT | TOK_UNION => prefix = AggregatePrefix::Record,
            TOK_ENUM => prefix = AggregatePrefix::Enum,
            TOK_SEMICOLON | TOK_ASSIGN | TOK_COMMA => prefix = AggregatePrefix::None,
            _ => {}
        }
    }
    prefix
}

pub(super) fn parenthesized_declarator_context(vast_nodes: &[u32], cur_parent: u32) -> bool {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut parent = cur_parent;
    for _ in 0..8 {
        let Ok(parent_idx) = usize::try_from(parent) else {
            return false;
        };
        if parent_idx >= node_count || kind_at(vast_nodes, parent_idx) != TOK_LPAREN {
            return false;
        }

        let parent_parent = parent_at(vast_nodes, parent_idx);
        if decl_context_before(vast_nodes, parent_idx, parent_parent).has_prefix {
            return true;
        }

        let Ok(parent_parent_idx) = usize::try_from(parent_parent) else {
            return false;
        };
        if parent_parent_idx < node_count
            && is_typeof_operator_raw(
                kind_at(vast_nodes, parent_parent_idx),
                symbol_hash_at(vast_nodes, parent_parent_idx),
            )
        {
            return true;
        }
        if parent_parent_idx >= node_count || kind_at(vast_nodes, parent_parent_idx) != TOK_LPAREN {
            return false;
        }
        parent = parent_parent;
    }
    false
}

pub(super) fn kind_at_impl(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field_at(vast_nodes, node_idx, 0)
}

pub(super) fn child_kind(vast_nodes: &[u32], node_idx: usize) -> u32 {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let child_idx = first_child_at(vast_nodes, node_idx);
    let Ok(child_idx) = usize::try_from(child_idx) else {
        return 0;
    };
    if child_idx >= node_count {
        return 0;
    }
    kind_at(vast_nodes, child_idx)
}

pub(super) fn child_flags(vast_nodes: &[u32], node_idx: usize) -> u32 {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let child_idx = first_child_at(vast_nodes, node_idx);
    let Ok(child_idx) = usize::try_from(child_idx) else {
        return 0;
    };
    if child_idx >= node_count {
        return 0;
    }
    flags_at(vast_nodes, child_idx)
}

pub(super) fn child_symbol_hash(vast_nodes: &[u32], node_idx: usize) -> u32 {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let child_idx = first_child_at(vast_nodes, node_idx);
    let Ok(child_idx) = usize::try_from(child_idx) else {
        return 0;
    };
    if child_idx >= node_count {
        return 0;
    }
    symbol_hash_at(vast_nodes, child_idx)
}

pub(super) fn has_any_typedef_annotations(vast_nodes: &[u32]) -> bool {
    vast_nodes
        .chunks_exact(VAST_NODE_STRIDE_U32 as usize)
        .any(|row| row[VAST_TYPEDEF_FLAGS_FIELD as usize] != 0)
}

pub(super) fn is_reference_typedef_name(flags: u32, fallback_has_prior_typedef: bool) -> bool {
    (flags & C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME) != 0 || fallback_has_prior_typedef
}

pub(super) fn prior_typedef_seen(vast_nodes: &[u32], node_idx: usize) -> bool {
    (0..node_idx).any(|scan_idx| kind_at(vast_nodes, scan_idx) == TOK_TYPEDEF)
}

pub(super) fn prior_ordinary_decl_seen(vast_nodes: &[u32], node_idx: usize) -> bool {
    (0..node_idx)
        .any(|scan_idx| (flags_at(vast_nodes, scan_idx) & C_TYPEDEF_FLAG_ORDINARY_DECLARATOR) != 0)
}

pub(super) fn prior_raw_ordinary_decl_seen(vast_nodes: &[u32], node_idx: usize) -> bool {
    (0..node_idx).any(|scan_idx| {
        if kind_at(vast_nodes, scan_idx) != TOK_IDENTIFIER {
            return false;
        }
        let parent = parent_context(vast_nodes, parent_at(vast_nodes, scan_idx));
        if parent.is_record_body || parent.is_enum_body {
            return false;
        }
        let prev_kind = scan_idx
            .checked_sub(1)
            .map(|prev_idx| kind_at(vast_nodes, prev_idx))
            .unwrap_or(SENTINEL);
        let next_kind = if scan_idx + 1 < vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize {
            kind_at(vast_nodes, scan_idx + 1)
        } else {
            SENTINEL
        };
        is_decl_prefix_raw(prev_kind)
            && prev_kind != TOK_TYPEDEF
            && !declaration_prefix_contains_typedef(vast_nodes, scan_idx)
            && matches!(
                next_kind,
                TOK_SEMICOLON | TOK_COMMA | TOK_ASSIGN | TOK_LBRACKET
            )
    })
}

pub(super) fn prior_parenthesized_identifier_statement_seen(
    vast_nodes: &[u32],
    node_idx: usize,
) -> bool {
    (0..node_idx).any(|scan_idx| {
        scan_idx + 5 < node_idx
            && kind_at(vast_nodes, scan_idx) == TOK_LPAREN
            && kind_at(vast_nodes, scan_idx + 1) == TOK_IDENTIFIER
            && kind_at(vast_nodes, scan_idx + 2) == TOK_RPAREN
            && kind_at(vast_nodes, scan_idx + 5) == TOK_SEMICOLON
    })
}

pub(super) fn declaration_prefix_contains_typedef(vast_nodes: &[u32], node_idx: usize) -> bool {
    (0..node_idx).rev().find_map(|scan_idx| {
        let kind = kind_at(vast_nodes, scan_idx);
        if kind == TOK_TYPEDEF {
            Some(true)
        } else if is_decl_prefix_reset_raw(kind) {
            Some(false)
        } else {
            None
        }
    }) == Some(true)
}

pub(super) fn reference_c_unary_context(prev_kind: u32) -> bool {
    matches!(
        prev_kind,
        SENTINEL
            | TOK_LPAREN
            | TOK_LBRACKET
            | TOK_LBRACE
            | TOK_COMMA
            | TOK_ASSIGN
            | TOK_PLUS_EQ
            | TOK_MINUS_EQ
            | TOK_STAR_EQ
            | TOK_SLASH_EQ
            | TOK_PERCENT_EQ
            | TOK_AMP_EQ
            | TOK_PIPE_EQ
            | TOK_CARET_EQ
            | TOK_LSHIFT_EQ
            | TOK_RSHIFT_EQ
            | TOK_QUESTION
            | TOK_COLON
            | TOK_SEMICOLON
            | TOK_RETURN
            | TOK_CASE
            | TOK_SIZEOF
            | TOK_GNU_TYPEOF
            | TOK_GNU_TYPEOF_UNQUAL
            | TOK_ALIGNOF
            | TOK_PLUS
            | TOK_MINUS
            | TOK_STAR
            | TOK_SLASH
            | TOK_PERCENT
            | TOK_AMP
            | TOK_PIPE
            | TOK_CARET
            | TOK_BANG
            | TOK_TILDE
            | TOK_EQ
            | TOK_NE
            | TOK_LE
            | TOK_GE
            | TOK_AND
            | TOK_OR
            | TOK_LSHIFT
            | TOK_RSHIFT
            | TOK_LT
            | TOK_GT
    )
}

pub(super) fn reference_c_can_end_expression(prev_kind: u32) -> bool {
    matches!(
        prev_kind,
        TOK_IDENTIFIER
            | TOK_INTEGER
            | TOK_FLOAT
            | TOK_STRING
            | TOK_CHAR
            | TOK_RPAREN
            | TOK_RBRACKET
            | TOK_INC
            | TOK_DEC
    )
}

pub(super) fn reference_c_statement_kind(token: u32) -> Option<u32> {
    match token {
        TOK_IF => Some(C_AST_KIND_IF_STMT),
        TOK_ELSE => Some(C_AST_KIND_ELSE_STMT),
        TOK_SWITCH => Some(C_AST_KIND_SWITCH_STMT),
        TOK_CASE => Some(C_AST_KIND_CASE_STMT),
        TOK_DEFAULT => Some(C_AST_KIND_DEFAULT_STMT),
        TOK_FOR => Some(C_AST_KIND_FOR_STMT),
        TOK_WHILE => Some(C_AST_KIND_WHILE_STMT),
        TOK_DO => Some(C_AST_KIND_DO_STMT),
        TOK_RETURN => Some(C_AST_KIND_RETURN_STMT),
        TOK_BREAK => Some(C_AST_KIND_BREAK_STMT),
        TOK_CONTINUE => Some(C_AST_KIND_CONTINUE_STMT),
        TOK_GOTO => Some(C_AST_KIND_GOTO_STMT),
        _ => None,
    }
}
