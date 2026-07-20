//! Regular replacement-token branch builder for macro expansion.

use crate::parsing::c::preprocess::materialization::*;
use vyre::ir::{Expr, Node};

use super::helpers::*;
use super::*;

pub(super) fn emit_materialized_regular_replacement_branch(
    in_tok_types: &str,
    in_tok_starts: &str,
    in_tok_lens: &str,
    source_words: &str,
    source_layout: MacroByteLayout,
    macro_replacement_starts: &str,
    macro_replacement_lens: &str,
    macro_replacement_words: &str,
    macro_replacement_layout: MacroByteLayout,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_source_words: &str,
    macro_arg_starts: &str,
    macro_arg_ends: &str,
    num_tokens: Expr,
    source_len: Expr,
    macro_replacement_source_len: Expr,
    max_out_tokens: u32,
    max_out_source_bytes: u32,
) -> Vec<Node> {
    let regular_literal = emit_materialized_output_token(
        "function_literal",
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
        "function-like-replacement-source-span-out-of-bounds",
    );
    let arg_start = selected_arg_bound(macro_arg_starts, Expr::var("named_repl_param"));
    let arg_end = selected_arg_bound(macro_arg_ends, Expr::var("named_repl_param"));
    vec![Node::if_then_else(
        Expr::eq(
            Expr::var("named_repl_param"),
            Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
        ),
        regular_literal,
        {
            let mut arg = vec![
                Node::if_then(
                    Expr::ge(
                        Expr::var("named_repl_param"),
                        Expr::var("named_param_count"),
                    ),
                    vec![Node::trap(
                        Expr::var("named_repl_param"),
                        "function-like-macro-replacement-parameter-out-of-range",
                    )],
                ),
                Node::let_bind("macro_sub_arg_start", arg_start),
                Node::let_bind("macro_sub_arg_end", arg_end),
            ];
            arg.push(Node::loop_for(
                "macro_sub_arg_rel",
                Expr::u32(0),
                num_tokens.clone(),
                vec![Node::if_then(
                    Expr::lt(
                        Expr::add(
                            Expr::var("macro_sub_arg_start"),
                            Expr::var("macro_sub_arg_rel"),
                        ),
                        Expr::var("macro_sub_arg_end"),
                    ),
                    {
                        let mut copy = vec![Node::let_bind(
                            "macro_sub_arg_tok_idx",
                            Expr::add(
                                Expr::var("macro_sub_arg_start"),
                                Expr::var("macro_sub_arg_rel"),
                            ),
                        )];
                        copy.extend(emit_materialized_output_token(
                            "function_arg_token",
                            Expr::load(in_tok_types, Expr::var("macro_sub_arg_tok_idx")),
                            source_words,
                            source_layout,
                            Expr::load(in_tok_starts, Expr::var("macro_sub_arg_tok_idx")),
                            Expr::load(in_tok_lens, Expr::var("macro_sub_arg_tok_idx")),
                            source_len.clone(),
                            out_tok_types,
                            out_tok_starts,
                            out_tok_lens,
                            out_source_words,
                            max_out_tokens,
                            max_out_source_bytes,
                            "function-like-argument-source-span-out-of-bounds",
                        ));
                        copy
                    },
                )],
            ));
            arg
        },
    )]
}
