//! Explicit CPU oracle for VAST expression-shape construction.
//!
//! Production expression-shape construction must use the dispatchable
//! `c11_build_expression_shape_nodes*` builders. This module remains for
//! oracle witnesses and hostile corpus diagnosis.

#![allow(missing_docs)] // Internal oracle helpers are documented at the owning module boundary.
#![allow(deprecated)]
use crate::parsing::c::lex::tokens::*;
use vyre::ir::Expr;

use super::expr_shape::*;
use super::ref_decode_err::*;
use super::*;

#[deprecated(
    note = "CPU oracle only; production expression-shape construction must dispatch c11_build_expression_shape_nodes* builders"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_c11_build_expression_shape_nodes(
    raw_vast_node_bytes: &[u8],
    typed_vast_node_bytes: &[u8],
) -> Result<Vec<u8>, CReferenceDecodeError> {
    let raw_vast_nodes = try_vast_words_from_bytes(raw_vast_node_bytes)?;
    let typed_vast_nodes = try_vast_words_from_bytes(typed_vast_node_bytes)?;
    let raw_rows = raw_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let typed_rows = typed_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    if raw_rows != typed_rows {
        return Err(CReferenceDecodeError::MismatchedVastRows {
            raw_rows,
            typed_rows,
        });
    }
    Ok(reference_c11_build_expression_shape_nodes_from_words(
        &raw_vast_nodes,
        &typed_vast_nodes,
    ))
}

/// CPU oracle for `c11_build_expression_shape_nodes`.
#[must_use]
#[deprecated(
    note = "CPU oracle only; production expression-shape construction must dispatch c11_build_expression_shape_nodes* builders"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c11_build_expression_shape_nodes(
    raw_vast_node_bytes: &[u8],
    typed_vast_node_bytes: &[u8],
) -> Vec<u8> {
    try_reference_c11_build_expression_shape_nodes(raw_vast_node_bytes, typed_vast_node_bytes)
        .unwrap_or_else(|error| {
            panic!("C VAST expression-shape reference oracle received malformed input: {error}")
        })
}

fn reference_c11_build_expression_shape_nodes_from_words(
    raw_vast_nodes: &[u32],
    typed_vast_nodes: &[u32],
) -> Vec<u8> {
    let node_count = raw_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    assert_eq!(
        node_count,
        typed_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize,
        "raw and typed VAST streams diverged after decode. Fix: pass matching streams from the same C translation unit."
    );
    let mut out = Vec::with_capacity(node_count * C_EXPR_SHAPE_STRIDE_U32 as usize);

    for node_idx in 0..node_count {
        let raw_kind = vast_kind(raw_vast_nodes, node_idx);
        let typed_kind = vast_kind(typed_vast_nodes, node_idx);
        let parent = vast_parent(raw_vast_nodes, node_idx);
        let shape = reference_c_expr_shape_kind(raw_kind, typed_kind);
        let precedence = reference_c_expr_operator_precedence(raw_kind, typed_kind);
        let associativity = reference_c_expr_operator_associativity(typed_kind);

        let (field5, field6, field7) = match shape {
            C_EXPR_SHAPE_BINARY => {
                let use_ternary_boundaries =
                    reference_binary_segment_uses_ternary_boundaries(raw_vast_nodes, node_idx);
                let (seg_start, seg_end) =
                    reference_expr_segment_bounds(raw_vast_nodes, node_idx, use_ternary_boundaries);
                let (left_bound, right_bound) = reference_binary_operand_bounds(
                    raw_vast_nodes,
                    typed_vast_nodes,
                    node_idx,
                    parent,
                    seg_start,
                    seg_end,
                    precedence,
                    associativity,
                );
                (
                    reference_expr_root(
                        raw_vast_nodes,
                        typed_vast_nodes,
                        left_bound,
                        node_idx,
                        parent,
                    ),
                    reference_expr_root(
                        raw_vast_nodes,
                        typed_vast_nodes,
                        node_idx.saturating_add(1),
                        right_bound,
                        parent,
                    ),
                    SENTINEL,
                )
            }
            C_EXPR_SHAPE_CONDITIONAL => {
                let (_, seg_end) = reference_expr_segment_bounds(raw_vast_nodes, node_idx, false);
                let (condition_start, _) =
                    reference_expr_segment_bounds(raw_vast_nodes, node_idx, true);
                let condition_start = reference_conditional_condition_start(
                    raw_vast_nodes,
                    typed_vast_nodes,
                    condition_start,
                    node_idx,
                    parent,
                    precedence,
                );
                let colon = reference_matching_ternary_colon(raw_vast_nodes, node_idx, seg_end);
                if let Some(colon_idx) = colon {
                    (
                        reference_expr_root(
                            raw_vast_nodes,
                            typed_vast_nodes,
                            condition_start,
                            node_idx,
                            parent,
                        ),
                        reference_expr_root(
                            raw_vast_nodes,
                            typed_vast_nodes,
                            node_idx.saturating_add(1),
                            colon_idx,
                            parent,
                        ),
                        reference_expr_root(
                            raw_vast_nodes,
                            typed_vast_nodes,
                            colon_idx.saturating_add(1),
                            seg_end,
                            parent,
                        ),
                    )
                } else {
                    (SENTINEL, SENTINEL, SENTINEL)
                }
            }
            _ => (SENTINEL, SENTINEL, SENTINEL),
        };

        out.extend_from_slice(&[
            shape,
            if shape == C_EXPR_SHAPE_NONE {
                SENTINEL
            } else {
                node_idx as u32
            },
            raw_kind,
            precedence,
            associativity,
            field5,
            field6,
            field7,
        ]);
    }

    u32_words_to_bytes(&out)
}

fn vast_field(vast_nodes: &[u32], node_idx: usize, field_idx: usize) -> u32 {
    c_vast_word_at(vast_nodes, node_idx, field_idx)
}

fn vast_kind(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field(vast_nodes, node_idx, 0)
}

fn vast_parent(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field(vast_nodes, node_idx, 1)
}

fn reference_c_expr_shape_kind(raw_kind: u32, typed_kind: u32) -> u32 {
    if typed_kind == C_AST_KIND_CONDITIONAL_EXPR || raw_kind == TOK_QUESTION {
        C_EXPR_SHAPE_CONDITIONAL
    } else if typed_kind == node_kind::BINARY || typed_kind == C_AST_KIND_ASSIGN_EXPR {
        C_EXPR_SHAPE_BINARY
    } else {
        C_EXPR_SHAPE_NONE
    }
}

fn reference_c_expr_operator_precedence(raw_kind: u32, typed_kind: u32) -> u32 {
    if typed_kind != node_kind::BINARY
        && typed_kind != C_AST_KIND_ASSIGN_EXPR
        && typed_kind != C_AST_KIND_CONDITIONAL_EXPR
        && raw_kind != TOK_QUESTION
    {
        0
    } else if typed_kind == C_AST_KIND_ASSIGN_EXPR {
        2
    } else if typed_kind == C_AST_KIND_CONDITIONAL_EXPR {
        3
    } else {
        match raw_kind {
            TOK_OR => 4,
            TOK_AND => 5,
            TOK_PIPE => 6,
            TOK_CARET => 7,
            TOK_AMP => 8,
            TOK_EQ | TOK_NE => 9,
            TOK_LT | TOK_GT | TOK_LE | TOK_GE => 10,
            TOK_LSHIFT | TOK_RSHIFT => 11,
            TOK_PLUS | TOK_MINUS => 12,
            TOK_STAR | TOK_SLASH | TOK_PERCENT => 13,
            _ => 0,
        }
    }
}

fn reference_c_expr_operator_associativity(typed_kind: u32) -> u32 {
    if typed_kind == C_AST_KIND_ASSIGN_EXPR || typed_kind == C_AST_KIND_CONDITIONAL_EXPR {
        C_EXPR_ASSOC_RIGHT
    } else if typed_kind == node_kind::BINARY {
        C_EXPR_ASSOC_LEFT
    } else {
        C_EXPR_ASSOC_NONE
    }
}

fn reference_expr_segment_bounds(
    raw_vast_nodes: &[u32],
    node_idx: usize,
    include_ternary_parts: bool,
) -> (usize, usize) {
    let node_count = raw_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let parent = vast_parent(raw_vast_nodes, node_idx);
    let mut start = 0usize;
    let mut scan = node_idx;
    while scan > 0 {
        scan -= 1;
        let scan_parent = vast_parent(raw_vast_nodes, scan);
        let scan_raw = vast_kind(raw_vast_nodes, scan);
        if scan_parent == parent
            && reference_is_expr_shape_boundary(scan_raw, include_ternary_parts)
        {
            start = scan.saturating_add(1);
            break;
        }
    }

    let mut end = node_count;
    for scan in node_idx.saturating_add(1)..node_count {
        let scan_parent = vast_parent(raw_vast_nodes, scan);
        let scan_raw = vast_kind(raw_vast_nodes, scan);
        if scan_parent == parent
            && reference_is_expr_shape_boundary(scan_raw, include_ternary_parts)
        {
            end = scan;
            break;
        }
    }

    (start, end)
}

fn reference_is_expr_shape_boundary(raw_kind: u32, include_ternary_parts: bool) -> bool {
    matches!(raw_kind, TOK_SEMICOLON | TOK_COMMA)
        || (include_ternary_parts && matches!(raw_kind, TOK_QUESTION | TOK_COLON))
}

fn reference_binary_segment_uses_ternary_boundaries(
    raw_vast_nodes: &[u32],
    node_idx: usize,
) -> bool {
    let parent = vast_parent(raw_vast_nodes, node_idx);
    let mut scan = node_idx;
    while scan > 0 {
        scan -= 1;
        if vast_parent(raw_vast_nodes, scan) != parent {
            continue;
        }
        match vast_kind(raw_vast_nodes, scan) {
            TOK_QUESTION | TOK_COLON => return true,
            TOK_SEMICOLON | TOK_COMMA => return false,
            _ => {}
        }
    }
    false
}

fn reference_conditional_condition_start(
    raw_vast_nodes: &[u32],
    typed_vast_nodes: &[u32],
    segment_start: usize,
    question_idx: usize,
    parent: u32,
    conditional_precedence: u32,
) -> usize {
    let mut condition_start = segment_start;
    for scan in segment_start..question_idx {
        if vast_parent(raw_vast_nodes, scan) != parent {
            continue;
        }
        let raw_kind = vast_kind(raw_vast_nodes, scan);
        let typed_kind = vast_kind(typed_vast_nodes, scan);
        if reference_c_expr_shape_kind(raw_kind, typed_kind) == C_EXPR_SHAPE_NONE {
            continue;
        }
        if reference_c_expr_operator_precedence(raw_kind, typed_kind) < conditional_precedence {
            condition_start = scan.saturating_add(1);
        }
    }
    condition_start
}

fn reference_binary_operand_bounds(
    raw_vast_nodes: &[u32],
    typed_vast_nodes: &[u32],
    node_idx: usize,
    parent: u32,
    seg_start: usize,
    seg_end: usize,
    target_precedence: u32,
    target_associativity: u32,
) -> (usize, usize) {
    let mut left_bound = seg_start;
    let mut right_bound = seg_end;
    let mut left_parent_op = SENTINEL;
    let mut right_parent_op = SENTINEL;

    for scan in seg_start..seg_end {
        if scan == node_idx {
            continue;
        }
        if vast_parent(raw_vast_nodes, scan) != parent {
            continue;
        }
        let raw_kind = vast_kind(raw_vast_nodes, scan);
        let typed_kind = vast_kind(typed_vast_nodes, scan);
        if reference_c_expr_shape_kind(raw_kind, typed_kind) == C_EXPR_SHAPE_NONE {
            continue;
        }
        let precedence = reference_c_expr_operator_precedence(raw_kind, typed_kind);
        let equal_assoc_parent = precedence == target_precedence
            && ((target_associativity == C_EXPR_ASSOC_LEFT && node_idx < scan)
                || (target_associativity == C_EXPR_ASSOC_RIGHT && scan < node_idx));
        if precedence < target_precedence || equal_assoc_parent {
            if scan < node_idx {
                left_parent_op = scan as u32;
            } else if scan > node_idx
                && (right_parent_op == SENTINEL || scan < right_parent_op as usize)
            {
                right_parent_op = scan as u32;
            }
        }
    }

    if left_parent_op != SENTINEL {
        left_bound = (left_parent_op as usize).saturating_add(1);
    }
    if right_parent_op != SENTINEL {
        right_bound = right_parent_op as usize;
    }

    (left_bound, right_bound)
}

fn reference_expr_root(
    raw_vast_nodes: &[u32],
    typed_vast_nodes: &[u32],
    lo: usize,
    hi: usize,
    parent: u32,
) -> u32 {
    let node_count = raw_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    assert_eq!(
        node_count,
        typed_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize,
        "raw and typed VAST streams diverged while resolving expression root. Fix: pass matching streams from the same C translation unit."
    );
    let end = hi.min(node_count);
    let mut root = SENTINEL;
    let mut root_prec = u32::MAX;
    let mut first_operand = SENTINEL;

    for scan in lo.min(end)..end {
        let scan_parent = vast_parent(raw_vast_nodes, scan);
        let raw_kind = vast_kind(raw_vast_nodes, scan);
        let typed_kind = vast_kind(typed_vast_nodes, scan);
        let shape = reference_c_expr_shape_kind(raw_kind, typed_kind);
        if scan_parent != parent && shape == C_EXPR_SHAPE_NONE {
            continue;
        }
        if shape == C_EXPR_SHAPE_NONE {
            if scan_parent == parent
                && first_operand == SENTINEL
                && !reference_is_expr_shape_boundary(raw_kind, true)
            {
                first_operand = scan as u32;
            }
            continue;
        }

        let prec = reference_c_expr_operator_precedence(raw_kind, typed_kind);
        let assoc = reference_c_expr_operator_associativity(typed_kind);
        if root == SENTINEL || prec < root_prec || (prec == root_prec && assoc == C_EXPR_ASSOC_LEFT)
        {
            root = scan as u32;
            root_prec = prec;
        }
    }

    if root == SENTINEL {
        first_operand
    } else {
        root
    }
}

fn reference_matching_ternary_colon(
    raw_vast_nodes: &[u32],
    question_idx: usize,
    seg_end: usize,
) -> Option<usize> {
    let node_count = raw_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let parent = vast_parent(raw_vast_nodes, question_idx);
    let mut depth = 0u32;

    for scan in question_idx.saturating_add(1)..seg_end.min(node_count) {
        if vast_parent(raw_vast_nodes, scan) != parent {
            continue;
        }
        match vast_kind(raw_vast_nodes, scan) {
            TOK_QUESTION => depth = depth.saturating_add(1),
            TOK_COLON if depth == 0 => return Some(scan),
            TOK_COLON => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    None
}

pub(super) fn pop_matching(stack: &mut Vec<u32>, tok_types: &[u32], opener: u32) {
    if stack
        .last()
        .and_then(|idx| tok_types.get(*idx as usize))
        .copied()
        == Some(opener)
    {
        stack.pop();
    }
}

fn witness_inputs() -> Vec<Vec<Vec<u8>>> {
    let tok_types = [107u32, 1, 10, 11, 12, 104, 2, 16, 13];
    let tok_starts = [0u32, 4, 8, 9, 10, 11, 18, 19, 20];
    let tok_lens = [3u32, 4, 1, 1, 1, 6, 1, 1, 1];
    vec![vec![
        u32_words_to_bytes(&tok_types),
        u32_words_to_bytes(&tok_starts),
        u32_words_to_bytes(&tok_lens),
        vec![0u8; tok_types.len() * VAST_NODE_STRIDE_U32 as usize * 4],
        vec![0u8; 4],
    ]]
}

fn witness_expected() -> Vec<Vec<Vec<u8>>> {
    let tok_types = [107u32, 1, 10, 11, 12, 104, 2, 16, 13];
    let tok_starts = [0u32, 4, 8, 9, 10, 11, 18, 19, 20];
    let tok_lens = [3u32, 4, 1, 1, 1, 6, 1, 1, 1];
    vec![vec![
        reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens),
        u32_words_to_bytes(&[tok_types.len() as u32]),
    ]]
}

inventory::submit! {
    OpEntry::new(
        BUILD_VAST_OP_ID,
        || c11_build_vast_nodes("tok_types", "tok_starts", "tok_lens", Expr::u32(9), "out_vast_nodes", "out_count"),
        Some(witness_inputs),
        Some(witness_expected),
    )
}

fn classify_witness_vast() -> Vec<u8> {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_starts = [0u32, 4, 8, 9, 10, 11, 18, 19, 20];
    let tok_lens = [3u32, 4, 1, 1, 1, 6, 1, 1, 1];
    reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens)
}

fn classify_witness_inputs() -> Vec<Vec<Vec<u8>>> {
    let vast = classify_witness_vast();
    vec![vec![vast, vec![0u8; 9 * VAST_NODE_STRIDE_U32 as usize * 4]]]
}

fn classify_witness_expected() -> Vec<Vec<Vec<u8>>> {
    vec![vec![reference_c11_classify_vast_node_kinds(
        &classify_witness_vast(),
    )]]
}

inventory::submit! {
    OpEntry::new(
        CLASSIFY_VAST_OP_ID,
        || c11_classify_vast_node_kinds("vast_nodes", Expr::u32(9), "out_typed_vast_nodes"),
        Some(classify_witness_inputs),
        Some(classify_witness_expected),
    )
}

fn expression_shape_witness_raw_vast() -> Vec<u8> {
    let tok_types = [
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = [1u32; 14];
    let tok_starts = (0..14u32).collect::<Vec<_>>();
    reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens)
}

fn expression_shape_witness_inputs() -> Vec<Vec<Vec<u8>>> {
    let raw = expression_shape_witness_raw_vast();
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    vec![vec![
        raw,
        typed,
        vec![0; 14 * C_EXPR_SHAPE_STRIDE_U32 as usize * 4],
    ]]
}

fn expression_shape_witness_expected() -> Vec<Vec<Vec<u8>>> {
    expression_shape_witness_inputs()
        .into_iter()
        .map(|input| {
            vec![reference_c11_build_expression_shape_nodes(
                &input[0], &input[1],
            )]
        })
        .collect()
}

inventory::submit! {
    OpEntry::new(
        EXPR_SHAPE_OP_ID,
        || c11_build_expression_shape_nodes(
            "raw_vast_nodes",
            "typed_vast_nodes",
            Expr::u32(14),
            "out_expr_shape_nodes",
        ),
        Some(expression_shape_witness_inputs),
        Some(expression_shape_witness_expected),
    )
}
