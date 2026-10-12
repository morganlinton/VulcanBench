//! Extracted from `vast/build.rs` (T10 file-cap split  -  original file
//! crossed the 500-LOC hygiene cap at 1255 LOC; this 428-LOC body is
//! the largest single function in the original module).
//!
//! `emit_declaration_kind_for_index_inner` constructs the IR Node
//! sequence that, at GPU dispatch time, classifies a single VAST row
//! as either a typedef declarator (1), an ordinary declarator (2),
//! or neither (0). The classification reads the row's surrounding
//! token-stream context (parent kind, previous sibling kinds,
//! prefix-scan back to the first declaration-prefix-reset token,
//! and an optional typedef-name lookup against the source haystack).
//!
//! Called from `build::emit_declaration_kind_for_index` and
//! `build::emit_builtin_declaration_kind_for_index` only  -  kept
//! `pub(super)` so both call sites continue to work without going
//! through a public API.

#![allow(missing_docs)]

use crate::parsing::c::lex::tokens::*;
use vyre::ir::{Expr, Node};

use super::build::{
    emit_declaration_kind_result_assignment, emit_identifier_source_hash_for_index,
    emit_visible_typedef_name_for_index, vast_bounded_row_kind_expr, vast_prior_row_kind_expr,
    vast_row_base_expr, vast_row_field_expr, vast_row_kind_from_base_expr,
    vast_row_parent_from_base_expr,
};
use super::helpers::*;
use super::*;

pub(super) fn emit_declaration_kind_for_index_inner(
    vast_nodes: &str,
    decl_contexts: Option<&str>,
    idx: Expr,
    out_name: &str,
    prefix: &str,
    prefix_typedef_lookup: Option<(&str, &Expr, bool)>,
) -> Vec<Node> {
    let base = format!("{prefix}_base");
    let kind = format!("{prefix}_kind");
    let prev_idx = format!("{prefix}_prev_idx");
    let prev_prev_idx = format!("{prefix}_prev_prev_idx");
    let next_idx = format!("{prefix}_next_idx");
    let prev_kind = format!("{prefix}_prev_kind");
    let prev_prev_kind = format!("{prefix}_prev_prev_kind");
    let next_kind = format!("{prefix}_next_kind");
    let parent_idx = format!("{prefix}_parent_idx");
    let parent_kind = format!("{prefix}_parent_kind");
    let parent_parent_idx = format!("{prefix}_parent_parent_idx");
    let parent_prev_kind = format!("{prefix}_parent_prev_kind");
    let parent_prev_prev_kind = format!("{prefix}_parent_prev_prev_kind");
    let parent_aggregate_prefix = format!("{prefix}_parent_aggregate_prefix");
    let parent_aggregate_scan = format!("{prefix}_parent_aggregate_scan");
    let parent_aggregate_base = format!("{prefix}_parent_aggregate_base");
    let parent_aggregate_kind = format!("{prefix}_parent_aggregate_kind");
    let parent_aggregate_parent = format!("{prefix}_parent_aggregate_parent");
    let in_aggregate_body = format!("{prefix}_in_aggregate_body");
    let prefix_has_typedef = format!("{prefix}_has_typedef");
    let prefix_has_type = format!("{prefix}_has_type");
    let prefix_done = format!("{prefix}_prefix_done");
    let prefix_start = format!("{prefix}_prefix_start");
    let prefix_skipped_paren_depth = format!("{prefix}_prefix_skipped_paren_depth");
    let prefix_skipped_brace_depth = format!("{prefix}_prefix_skipped_brace_depth");
    let prefix_scan = format!("{prefix}_prefix_scan");
    let prefix_idx = format!("{prefix}_prefix_idx");
    let prefix_base = format!("{prefix}_prefix_base");
    let prefix_kind = format!("{prefix}_prefix_kind");
    let prefix_symbol_hash = format!("{prefix}_prefix_symbol_hash");
    let prefix_in_skipped_paren = format!("{prefix}_prefix_in_skipped_paren");
    let prefix_in_skipped_brace = format!("{prefix}_prefix_in_skipped_brace");
    let prefix_visible_typedef = format!("{prefix}_prefix_visible_typedef");
    let is_identifier = format!("{prefix}_is_identifier");
    let declarator_follower = format!("{prefix}_declarator_follower");
    let sizeof_type_operand = format!("{prefix}_sizeof_type_operand");
    let cast_pointer_expr_operand = format!("{prefix}_cast_pointer_expr_operand");
    let prefix_typedef_lookup_node =
        if let Some((haystack, haystack_len, packed_haystack)) = prefix_typedef_lookup {
            Node::if_then(
                Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_IDENTIFIER)),
                {
                    let mut body = emit_identifier_source_hash_for_index(
                        vast_nodes,
                        haystack,
                        haystack_len,
                        Expr::var(&prefix_idx),
                        &prefix_symbol_hash,
                        &format!("{prefix}_prefix_hash"),
                        packed_haystack,
                    );
                    body.push(Node::if_then(
                        is_gnu_typeof_symbol_hash(Expr::var(&prefix_symbol_hash)),
                        vec![Node::assign(&prefix_has_type, Expr::u32(1))],
                    ));
                    body.extend(emit_visible_typedef_name_for_index(
                        vast_nodes,
                        haystack,
                        decl_contexts,
                        haystack_len,
                        Expr::var(&prefix_idx),
                        &prefix_visible_typedef,
                        &format!("{prefix}_prefix_type_name"),
                        packed_haystack,
                    ));
                    body.push(Node::if_then(
                        Expr::eq(Expr::var(&prefix_visible_typedef), Expr::u32(1)),
                        vec![Node::assign(&prefix_has_type, Expr::u32(1))],
                    ));
                    body
                },
            )
        } else {
            Node::if_then(Expr::u32(0), Vec::new())
        };

    vec![
        Node::let_bind(out_name, Expr::u32(0)),
        Node::let_bind(&base, vast_row_base_expr(idx.clone())),
        Node::let_bind(
            &kind,
            vast_row_kind_from_base_expr(vast_nodes, Expr::var(&base)),
        ),
        Node::let_bind(
            &parent_idx,
            vast_row_parent_from_base_expr(vast_nodes, Expr::var(&base)),
        ),
        Node::let_bind(
            &parent_kind,
            vast_bounded_row_kind_expr(vast_nodes, Expr::var(&parent_idx), Expr::u32(SENTINEL)),
        ),
        Node::let_bind(
            &parent_parent_idx,
            Expr::select(
                Expr::lt(Expr::var(&parent_idx), Expr::var("annot_num_nodes")),
                vast_row_field_expr(vast_nodes, Expr::var(&parent_idx), 1),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            &parent_prev_kind,
            Expr::select(
                Expr::and(
                    Expr::lt(Expr::var(&parent_idx), Expr::var("annot_num_nodes")),
                    Expr::ge(Expr::var(&parent_idx), Expr::u32(1)),
                ),
                vast_prior_row_kind_expr(vast_nodes, Expr::var(&parent_idx), 1),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            &parent_prev_prev_kind,
            Expr::select(
                Expr::and(
                    Expr::lt(Expr::var(&parent_idx), Expr::var("annot_num_nodes")),
                    Expr::ge(Expr::var(&parent_idx), Expr::u32(2)),
                ),
                vast_prior_row_kind_expr(vast_nodes, Expr::var(&parent_idx), 2),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(&parent_aggregate_prefix, Expr::u32(0)),
        Node::if_then(
            Expr::eq(Expr::var(&parent_kind), Expr::u32(TOK_LBRACE)),
            vec![Node::loop_for(
                &parent_aggregate_scan,
                Expr::u32(0),
                Expr::var(&parent_idx),
                vec![
                    Node::let_bind(
                        &parent_aggregate_base,
                        vast_row_base_expr(Expr::var(&parent_aggregate_scan)),
                    ),
                    Node::let_bind(
                        &parent_aggregate_kind,
                        vast_row_kind_from_base_expr(vast_nodes, Expr::var(&parent_aggregate_base)),
                    ),
                    Node::let_bind(
                        &parent_aggregate_parent,
                        vast_row_parent_from_base_expr(
                            vast_nodes,
                            Expr::var(&parent_aggregate_base),
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(
                            Expr::var(&parent_aggregate_parent),
                            Expr::var(&parent_parent_idx),
                        ),
                        vec![
                            Node::if_then(
                                any_token_eq(
                                    Expr::var(&parent_aggregate_kind),
                                    &[TOK_SEMICOLON, TOK_ASSIGN, TOK_COMMA],
                                ),
                                vec![Node::assign(&parent_aggregate_prefix, Expr::u32(0))],
                            ),
                            Node::if_then(
                                any_token_eq(
                                    Expr::var(&parent_aggregate_kind),
                                    &[TOK_STRUCT, TOK_UNION, TOK_ENUM],
                                ),
                                vec![Node::assign(&parent_aggregate_prefix, Expr::u32(1))],
                            ),
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind(
            &in_aggregate_body,
            Expr::and(
                Expr::eq(Expr::var(&parent_kind), Expr::u32(TOK_LBRACE)),
                Expr::eq(Expr::var(&parent_aggregate_prefix), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            &prev_idx,
            Expr::select(
                Expr::gt(idx.clone(), Expr::u32(0)),
                Expr::sub(idx.clone(), Expr::u32(1)),
                idx.clone(),
            ),
        ),
        Node::let_bind(
            &prev_prev_idx,
            Expr::select(
                Expr::gt(idx.clone(), Expr::u32(1)),
                Expr::sub(idx.clone(), Expr::u32(2)),
                idx.clone(),
            ),
        ),
        Node::let_bind(&next_idx, Expr::add(idx.clone(), Expr::u32(1))),
        Node::let_bind(
            &prev_kind,
            Expr::select(
                Expr::gt(idx.clone(), Expr::u32(0)),
                vast_row_kind_from_base_expr(vast_nodes, vast_row_base_expr(Expr::var(&prev_idx))),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            &prev_prev_kind,
            Expr::select(
                Expr::gt(idx.clone(), Expr::u32(1)),
                vast_row_kind_from_base_expr(
                    vast_nodes,
                    vast_row_base_expr(Expr::var(&prev_prev_idx)),
                ),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            &next_kind,
            vast_bounded_row_kind_expr(vast_nodes, Expr::var(&next_idx), Expr::u32(SENTINEL)),
        ),
        Node::let_bind(&prefix_has_typedef, Expr::u32(0)),
        Node::let_bind(&prefix_has_type, Expr::u32(0)),
        Node::let_bind(&prefix_done, Expr::u32(0)),
        Node::let_bind(
            &prefix_start,
            if let Some(decl_contexts) = decl_contexts {
                Expr::load(
                    decl_contexts,
                    Expr::add(
                        Expr::mul(idx.clone(), Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32)),
                        Expr::u32(VAST_DECL_CONTEXT_PREFIX_START_FIELD),
                    ),
                )
            } else {
                Expr::u32(0)
            },
        ),
        Node::let_bind(&prefix_skipped_paren_depth, Expr::u32(0)),
        Node::let_bind(&prefix_skipped_brace_depth, Expr::u32(0)),
        Node::loop_for(
            &prefix_scan,
            Expr::var(&prefix_start),
            idx.clone(),
            vec![Node::if_then(
                Expr::eq(Expr::var(&prefix_done), Expr::u32(0)),
                vec![
                    Node::let_bind(
                        &prefix_idx,
                        Expr::sub(
                            Expr::sub(idx.clone(), Expr::u32(1)),
                            Expr::sub(Expr::var(&prefix_scan), Expr::var(&prefix_start)),
                        ),
                    ),
                    Node::let_bind(&prefix_base, vast_row_base_expr(Expr::var(&prefix_idx))),
                    Node::let_bind(
                        &prefix_kind,
                        vast_row_kind_from_base_expr(vast_nodes, Expr::var(&prefix_base)),
                    ),
                    Node::let_bind(
                        &prefix_in_skipped_paren,
                        Expr::or(
                            Expr::gt(Expr::var(&prefix_skipped_paren_depth), Expr::u32(0)),
                            Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_RPAREN)),
                        ),
                    ),
                    Node::let_bind(
                        &prefix_in_skipped_brace,
                        Expr::or(
                            Expr::gt(Expr::var(&prefix_skipped_brace_depth), Expr::u32(0)),
                            Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_RBRACE)),
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_RBRACE)),
                        vec![Node::assign(
                            &prefix_skipped_brace_depth,
                            Expr::add(Expr::var(&prefix_skipped_brace_depth), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::gt(Expr::var(&prefix_skipped_brace_depth), Expr::u32(0)),
                            Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_LBRACE)),
                        ),
                        vec![Node::assign(
                            &prefix_skipped_brace_depth,
                            Expr::sub(Expr::var(&prefix_skipped_brace_depth), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_RPAREN)),
                        vec![Node::assign(
                            &prefix_skipped_paren_depth,
                            Expr::add(Expr::var(&prefix_skipped_paren_depth), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::gt(Expr::var(&prefix_skipped_paren_depth), Expr::u32(0)),
                            Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_LPAREN)),
                        ),
                        vec![Node::assign(
                            &prefix_skipped_paren_depth,
                            Expr::sub(Expr::var(&prefix_skipped_paren_depth), Expr::u32(1)),
                        )],
                    ),
                    Node::if_then(
                        Expr::not(Expr::or(
                            Expr::var(&prefix_in_skipped_brace),
                            Expr::var(&prefix_in_skipped_paren),
                        )),
                        vec![
                            Node::if_then(
                                is_decl_prefix_reset_token(Expr::var(&prefix_kind)),
                                vec![Node::assign(&prefix_done, Expr::u32(1))],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_TYPEDEF)),
                                vec![Node::assign(&prefix_has_typedef, Expr::u32(1))],
                            ),
                            Node::if_then(
                                is_decl_prefix_token(Expr::var(&prefix_kind)),
                                vec![Node::assign(&prefix_has_type, Expr::u32(1))],
                            ),
                            prefix_typedef_lookup_node,
                        ],
                    ),
                ],
            )],
        ),
        Node::let_bind(
            &is_identifier,
            Expr::eq(Expr::var(&kind), Expr::u32(TOK_IDENTIFIER)),
        ),
        Node::let_bind(
            &declarator_follower,
            is_declarator_follower_token(Expr::var(&next_kind)),
        ),
        Node::let_bind(
            &sizeof_type_operand,
            Expr::and(
                Expr::eq(Expr::var(&prev_kind), Expr::u32(TOK_LPAREN)),
                any_token_eq(
                    Expr::var(&parent_prev_kind),
                    &[TOK_SIZEOF, TOK_GNU_TYPEOF, TOK_ALIGNOF],
                ),
            ),
        ),
        Node::let_bind(
            &cast_pointer_expr_operand,
            Expr::and(
                Expr::eq(Expr::var(&prev_kind), Expr::u32(TOK_STAR)),
                Expr::eq(Expr::var(&prev_prev_kind), Expr::u32(TOK_RPAREN)),
            ),
        ),
        emit_declaration_kind_result_assignment(
            out_name,
            Expr::var(&is_identifier),
            Expr::var(&declarator_follower),
            Expr::not(is_declaration_previous_disqualifier_token(Expr::var(
                &prev_kind,
            ))),
            Expr::ne(Expr::var(&next_kind), Expr::u32(TOK_COLON)),
            Expr::and(
                Expr::not(Expr::var(&in_aggregate_body)),
                Expr::and(
                    Expr::not(Expr::var(&sizeof_type_operand)),
                    Expr::not(Expr::var(&cast_pointer_expr_operand)),
                ),
            ),
            Expr::eq(Expr::var(&prefix_has_typedef), Expr::u32(1)),
            Expr::eq(Expr::var(&prefix_has_type), Expr::u32(1)),
        ),
    ]
}
