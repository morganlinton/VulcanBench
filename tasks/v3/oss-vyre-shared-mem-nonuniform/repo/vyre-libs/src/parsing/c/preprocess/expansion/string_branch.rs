//! Stringification branch builder for macro replacement.

use crate::parsing::c::preprocess::materialization::*;
use vyre::ir::{Expr, Node};

use super::helpers::*;
use super::*;

pub(super) fn emit_materialized_stringification_branch(
    macro_replacement_params: &str,
    macro_replacement_starts: &str,
    macro_replacement_lens: &str,
    macro_replacement_words: &str,
    macro_replacement_layout: MacroByteLayout,
    macro_replacement_source_len: Expr,
    macro_arg_starts: &str,
    macro_arg_ends: &str,
    in_tok_starts: &str,
    in_tok_lens: &str,
    source_words: &str,
    source_layout: MacroByteLayout,
    source_len: Expr,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_source_words: &str,
    max_out_tokens: u32,
    max_out_source_bytes: u32,
    num_tokens: Expr,
) -> Vec<Node> {
    let mut stringify = vec![
        Node::let_bind(
            "macro_stringify_next_offset",
            Expr::add(
                Expr::var("named_macro_idx"),
                Expr::add(Expr::var("named_repl_i"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "macro_stringify_next_param",
            Expr::load(
                macro_replacement_params,
                Expr::var("macro_stringify_next_offset"),
            ),
        ),
    ];
    stringify.push(Node::if_then_else(
        Expr::eq(
            Expr::var("macro_stringify_next_param"),
            Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
        ),
        emit_materialized_output_token(
            "function_hash_literal",
            Expr::var("named_repl_tok"),
            macro_replacement_words,
            macro_replacement_layout,
            Expr::load(macro_replacement_starts, Expr::var("named_repl_offset")),
            Expr::load(macro_replacement_lens, Expr::var("named_repl_offset")),
            macro_replacement_source_len,
            out_tok_types,
            out_tok_starts,
            out_tok_lens,
            out_source_words,
            max_out_tokens,
            max_out_source_bytes,
            "function-like-stringification-literal-hash-has-no-source-table",
        ),
        {
            let mut branch = vec![Node::if_then(
                Expr::ge(
                    Expr::var("macro_stringify_next_param"),
                    Expr::var("named_param_count"),
                ),
                vec![Node::trap(
                    Expr::var("macro_stringify_next_param"),
                    "function-like-stringification-parameter-out-of-range",
                )],
            )];
            branch.extend(emit_stringified_argument_token(
                "function_stringify",
                selected_arg_bound(macro_arg_starts, Expr::var("macro_stringify_next_param")),
                selected_arg_bound(macro_arg_ends, Expr::var("macro_stringify_next_param")),
                in_tok_starts,
                in_tok_lens,
                source_words,
                source_layout,
                source_len,
                out_tok_types,
                out_tok_starts,
                out_tok_lens,
                out_source_words,
                max_out_tokens,
                max_out_source_bytes,
                num_tokens,
            ));
            branch.push(Node::assign("named_skip_repl", Expr::u32(1)));
            branch
        },
    ));
    stringify
}
