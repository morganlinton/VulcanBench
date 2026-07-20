use crate::parsing::c::lex::tokens::*;
use crate::parsing::c::sema::lookup::{
    DECL_KIND_ENUM_CONSTANT, DECL_KIND_FUNCTION, DECL_KIND_FUNCTION_DECL, DECL_KIND_LABEL,
    DECL_KIND_NONE, DECL_KIND_TYPEDEF, DECL_KIND_VARIABLE,
};

/// Compute the same mapping in the explicit CPU oracle for conformance and
/// witness generation.
#[must_use]
#[deprecated(
    note = "CPU oracle only; production C semantic scope analysis must dispatch c_sema_scope* builders"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_scope_tree(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    haystack: &[u32],
) -> Vec<u32> {
    let mut out = Vec::with_capacity(tok_types.len() * 4);
    for node_idx in 0..tok_types.len() {
        let scope_id = scope_id_for_node(tok_types, node_idx);
        let scope_parent_id = scope_parent_id_for_node(tok_types, node_idx, scope_id);
        let decl_kind = decl_kind_for_node(tok_types, tok_starts, tok_lens, haystack, node_idx);
        let identifier_intern_id =
            identifier_intern_id_for_node(tok_types, tok_starts, tok_lens, haystack, node_idx);

        out.push(scope_id);
        out.push(scope_parent_id);
        out.push(decl_kind);
        out.push(identifier_intern_id);
    }

    out
}

fn scope_id_for_node(tok_types: &[u32], node_idx: usize) -> u32 {
    if let Some((scope_id, _)) = function_parameter_scope(tok_types, node_idx) {
        return scope_id;
    }

    brace_scope_id_for_node(tok_types, node_idx)
}

pub(super) fn brace_scope_id_for_node(tok_types: &[u32], node_idx: usize) -> u32 {
    if node_idx == 0 {
        return 0;
    }

    let mut depth = 0u32;
    for scan_idx in (0..node_idx).rev() {
        match tok_types[scan_idx] {
            TOK_RBRACE => depth = depth.saturating_add(1),
            TOK_LBRACE => {
                if depth == 0 {
                    return u32::try_from(scan_idx + 1).unwrap_or(u32::MAX);
                }
                if depth > 0 {
                    depth = depth.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    0
}

fn scope_parent_id_for_node(tok_types: &[u32], node_idx: usize, scope_id: u32) -> u32 {
    if let Some((_, parent_id)) = function_parameter_scope(tok_types, node_idx) {
        return parent_id;
    }

    brace_scope_parent_id_for_node(tok_types, node_idx, scope_id)
}

pub(super) fn brace_scope_parent_id_for_node(
    tok_types: &[u32],
    node_idx: usize,
    scope_id: u32,
) -> u32 {
    if scope_id == 0 {
        return 0;
    }

    let scope_open = scope_id.saturating_sub(1) as usize;
    if scope_open == 0 {
        return 0;
    }

    let mut depth = 0u32;
    for scan_idx in (0..scope_open).rev() {
        match tok_types[scan_idx] {
            TOK_RBRACE => depth = depth.saturating_add(1),
            TOK_LBRACE => {
                if depth == 0 {
                    return if scan_idx < node_idx {
                        u32::try_from(scan_idx + 1).unwrap_or(u32::MAX)
                    } else {
                        0
                    };
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    0
}

fn decl_kind_for_node(
    tok_types: &[u32],
    _tok_starts: &[u32],
    _tok_lens: &[u32],
    _haystack: &[u32],
    node_idx: usize,
) -> u32 {
    let current = tok_types[node_idx];
    if current != TOK_IDENTIFIER {
        return DECL_KIND_NONE;
    }

    let prev_tok = if node_idx > 0 {
        tok_types[node_idx - 1]
    } else {
        0
    };
    let next_tok = if node_idx + 1 < tok_types.len() {
        tok_types[node_idx + 1]
    } else {
        0
    };

    if next_tok == TOK_COLON {
        if prev_tok == TOK_CASE || prev_tok == TOK_GOTO {
            return DECL_KIND_NONE;
        }
        return DECL_KIND_LABEL;
    }

    if is_tag_name(tok_types, node_idx) {
        return DECL_KIND_NONE;
    }

    if let Some(aggregate) = aggregate_body_kind(tok_types, node_idx) {
        return if aggregate == TOK_ENUM && is_enum_constant(tok_types, node_idx) {
            DECL_KIND_ENUM_CONSTANT
        } else {
            DECL_KIND_NONE
        };
    }

    if next_tok == TOK_LPAREN {
        let mut paren_depth = 0u32;
        let mut matching_rparen: Option<usize> = None;
        for scan_idx in (node_idx + 1)..tok_types.len() {
            match tok_types[scan_idx] {
                TOK_LPAREN => paren_depth = paren_depth.saturating_add(1),
                TOK_RPAREN => {
                    if paren_depth <= 1 {
                        matching_rparen = Some(scan_idx);
                        break;
                    }
                    paren_depth = paren_depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        if let Some(rparen_idx) = matching_rparen {
            if declaration_boundary_after_paren(tok_types, rparen_idx) == Some(TOK_LBRACE) {
                return DECL_KIND_FUNCTION;
            }
            if is_declaration_context_token(prev_tok) {
                return if prev_tok == TOK_TYPEDEF {
                    DECL_KIND_TYPEDEF
                } else {
                    DECL_KIND_FUNCTION_DECL
                };
            }
            if prev_tok == TOK_TYPEDEF {
                return DECL_KIND_TYPEDEF;
            }
        }
    }

    if prev_tok == TOK_TYPEDEF {
        return DECL_KIND_TYPEDEF;
    }

    if seen_typedef_in_declaration(tok_types, node_idx) {
        return DECL_KIND_TYPEDEF;
    }

    if is_declaration_context_token(prev_tok) {
        return DECL_KIND_VARIABLE;
    }

    DECL_KIND_NONE
}

fn is_declaration_context_token(token: u32) -> bool {
    matches!(
        token,
        TOK_INT
            | TOK_CHAR_KW
            | TOK_VOID
            | TOK_STRUCT
            | TOK_TYPEDEF
            | TOK_COMMA
            | TOK_SEMICOLON
            | TOK_LPAREN
            | TOK_RPAREN
            | TOK_STAR
            | TOK_AUTO
            | TOK_CONST
            | TOK_DOUBLE
            | TOK_ENUM
            | TOK_EXTERN
            | TOK_FLOAT_KW
            | TOK_INLINE
            | TOK_LONG
            | TOK_REGISTER
            | TOK_RESTRICT
            | TOK_SHORT
            | TOK_SIGNED
            | TOK_STATIC
            | TOK_THREAD_LOCAL
            | TOK_UNION
            | TOK_UNSIGNED
            | TOK_VOLATILE
    )
}

fn is_tag_keyword(token: u32) -> bool {
    matches!(token, TOK_STRUCT | TOK_UNION | TOK_ENUM)
}

fn is_tag_name(tok_types: &[u32], node_idx: usize) -> bool {
    node_idx > 0 && is_tag_keyword(tok_types[node_idx - 1])
}

fn aggregate_body_kind(tok_types: &[u32], node_idx: usize) -> Option<u32> {
    let mut depth = 0u32;
    for scan_idx in (0..node_idx).rev() {
        match tok_types[scan_idx] {
            TOK_RBRACE => depth = depth.saturating_add(1),
            TOK_LBRACE => {
                if depth == 0 {
                    let prev = scan_idx.checked_sub(1).map(|idx| tok_types[idx]);
                    let prev_prev = scan_idx.checked_sub(2).map(|idx| tok_types[idx]);
                    return prev
                        .filter(|token| is_tag_keyword(*token))
                        .or_else(|| prev_prev.filter(|token| is_tag_keyword(*token)));
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

fn is_enum_constant(tok_types: &[u32], node_idx: usize) -> bool {
    let prev = node_idx
        .checked_sub(1)
        .map(|idx| tok_types[idx])
        .unwrap_or(0);
    let next = tok_types.get(node_idx + 1).copied().unwrap_or(0);
    matches!(prev, TOK_LBRACE | TOK_COMMA) || matches!(next, TOK_ASSIGN | TOK_COMMA | TOK_RBRACE)
}

fn seen_typedef_in_declaration(tok_types: &[u32], node_idx: usize) -> bool {
    for scan_idx in (0..node_idx).rev() {
        match tok_types[scan_idx] {
            TOK_TYPEDEF => return true,
            TOK_SEMICOLON | TOK_LBRACE => return false,
            _ => {}
        }
    }
    false
}

fn declaration_boundary_after_paren(tok_types: &[u32], rparen_idx: usize) -> Option<u32> {
    match tok_types.get(rparen_idx + 1).copied() {
        Some(TOK_LBRACE | TOK_SEMICOLON) => tok_types.get(rparen_idx + 1).copied(),
        Some(_) => tok_types
            .iter()
            .skip(rparen_idx + 1)
            .copied()
            .find(|token| *token == TOK_LBRACE),
        None => None,
    }
}

pub(super) fn function_parameter_scope(tok_types: &[u32], node_idx: usize) -> Option<(u32, u32)> {
    let lparen_idx = enclosing_lparen(tok_types, node_idx)?;
    if lparen_idx == 0 || tok_types.get(lparen_idx - 1).copied() != Some(TOK_IDENTIFIER) {
        return None;
    }
    let prefix = lparen_idx
        .checked_sub(2)
        .and_then(|idx| tok_types.get(idx))
        .copied()
        .unwrap_or(0);
    if !is_function_name_prefix(prefix) {
        return None;
    }
    let rparen_idx = matching_rparen(tok_types, lparen_idx)?;
    if node_idx >= rparen_idx {
        return None;
    }
    let scope_open = match declaration_boundary_after_paren(tok_types, rparen_idx)? {
        TOK_LBRACE => tok_types
            .iter()
            .enumerate()
            .skip(rparen_idx + 1)
            .find_map(|(idx, token)| (*token == TOK_LBRACE).then_some(idx + 1))
            .and_then(|idx| u32::try_from(idx).ok())?,
        TOK_SEMICOLON => u32::try_from(lparen_idx + 1).ok()?,
        _ => return None,
    };

    let brace_scope = brace_scope_id_for_node(tok_types, node_idx);
    let brace_parent = brace_scope_parent_id_for_node(tok_types, node_idx, brace_scope);
    let scope_open_idx = usize::try_from(scope_open.saturating_sub(1)).ok()?;
    let mut parent = brace_scope;
    let has_pending_delimiter = tok_types.get(node_idx).copied() == Some(TOK_LBRACE)
        || tok_types
            .iter()
            .copied()
            .take(scope_open_idx)
            .skip(node_idx.saturating_add(1))
            .any(|token| matches!(token, TOK_LBRACE | TOK_RBRACE));
    if has_pending_delimiter {
        parent = brace_parent;
    }

    Some((scope_open, parent))
}

fn enclosing_lparen(tok_types: &[u32], node_idx: usize) -> Option<usize> {
    let mut depth = 0u32;
    for scan_idx in (0..node_idx).rev() {
        match tok_types[scan_idx] {
            TOK_RPAREN => depth = depth.saturating_add(1),
            TOK_LPAREN => {
                if depth == 0 {
                    return Some(scan_idx);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

fn matching_rparen(tok_types: &[u32], lparen_idx: usize) -> Option<usize> {
    let mut depth = 1u32;
    for (scan_idx, token) in tok_types.iter().copied().enumerate().skip(lparen_idx + 1) {
        match token {
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

fn is_function_name_prefix(token: u32) -> bool {
    matches!(
        token,
        TOK_AUTO
            | TOK_CHAR_KW
            | TOK_CONST
            | TOK_DOUBLE
            | TOK_ENUM
            | TOK_EXTERN
            | TOK_FLOAT_KW
            | TOK_IDENTIFIER
            | TOK_INLINE
            | TOK_INT
            | TOK_LONG
            | TOK_REGISTER
            | TOK_RESTRICT
            | TOK_SHORT
            | TOK_SIGNED
            | TOK_STATIC
            | TOK_STRUCT
            | TOK_THREAD_LOCAL
            | TOK_TYPEDEF
            | TOK_UNION
            | TOK_UNSIGNED
            | TOK_VOID
            | TOK_VOLATILE
    )
}

fn identifier_intern_id_for_node(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    haystack: &[u32],
    node_idx: usize,
) -> u32 {
    if node_idx >= tok_types.len() || tok_types[node_idx] != TOK_IDENTIFIER {
        return 0;
    }
    assert_eq!(
        tok_starts.len(),
        tok_types.len(),
        "C semantic registry reference received {} token starts for {} token types. Fix: pass aligned token streams.",
        tok_starts.len(),
        tok_types.len()
    );
    assert_eq!(
        tok_lens.len(),
        tok_types.len(),
        "C semantic registry reference received {} token lengths for {} token types. Fix: pass aligned token streams.",
        tok_lens.len(),
        tok_types.len()
    );
    let start = tok_starts[node_idx];
    let len = tok_lens[node_idx];
    if len == 0 {
        return 0;
    }
    let Some(end_u32) = start.checked_add(len) else {
        return 0;
    };
    let Ok(start_usize) = usize::try_from(start) else {
        return 0;
    };
    let Ok(end) = usize::try_from(end_u32) else {
        return 0;
    };
    assert!(
        end <= haystack.len(),
        "C semantic identifier span {start_usize}..{end} exceeds haystack length {} at token {node_idx}. Fix: pass the same haystack used for lexing.",
        haystack.len()
    );
    assert!(
        start_usize < end,
        "C semantic identifier span is empty at token {node_idx}. Fix: repair lexer identifier lengths."
    );

    vyre_primitives::hash::fnv1a::fnv1a32_packed_u32_low8(&haystack[start_usize..end])
}
