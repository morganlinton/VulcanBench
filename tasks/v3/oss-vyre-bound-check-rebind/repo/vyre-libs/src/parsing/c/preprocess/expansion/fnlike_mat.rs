//! Materialized function-like macro expansion builder.

use crate::parsing::c::lex::tokens::*;
use vyre::ir::{Expr, Node};

use super::arg_scan::*;
use super::paste_branch::*;
use super::regular_branch::*;
use super::string_branch::*;
use super::MacroByteLayout;

pub(super) fn emit_materialized_function_like_replacement(
    in_tok_types: &str,
    in_tok_starts: &str,
    in_tok_lens: &str,
    source_words: &str,
    source_layout: MacroByteLayout,
    macro_vals: &str,
    macro_replacement_params: &str,
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
    let mut nodes = emit_function_like_argument_scan(
        in_tok_types,
        macro_arg_starts,
        macro_arg_ends,
        num_tokens.clone(),
    );
    nodes.extend([
        Node::let_bind("named_skip_repl", Expr::u32(0)),
        Node::loop_for(
            "named_repl_i",
            Expr::u32(0),
            Expr::var("named_repl_size"),
            {
                vec![Node::if_then_else(
                    Expr::eq(Expr::var("named_skip_repl"), Expr::u32(1)),
                    vec![Node::assign("named_skip_repl", Expr::u32(0))],
                    {
                        let mut repl = vec![
                            Node::let_bind(
                                "named_repl_offset",
                                Expr::add(Expr::var("named_macro_idx"), Expr::var("named_repl_i")),
                            ),
                            Node::let_bind(
                                "named_repl_param",
                                Expr::load(
                                    macro_replacement_params,
                                    Expr::var("named_repl_offset"),
                                ),
                            ),
                            Node::let_bind(
                                "named_repl_tok",
                                Expr::load(macro_vals, Expr::var("named_repl_offset")),
                            ),
                        ];
                        repl.push(Node::if_then_else(
                            Expr::and(
                                Expr::eq(Expr::var("named_repl_tok"), Expr::u32(TOK_HASH)),
                                Expr::lt(
                                    Expr::add(Expr::var("named_repl_i"), Expr::u32(1)),
                                    Expr::var("named_repl_size"),
                                ),
                            ),
                            emit_materialized_stringification_branch(
                                macro_replacement_params,
                                macro_replacement_starts,
                                macro_replacement_lens,
                                macro_replacement_words,
                                macro_replacement_layout,
                                macro_replacement_source_len.clone(),
                                macro_arg_starts,
                                macro_arg_ends,
                                in_tok_starts,
                                in_tok_lens,
                                source_words,
                                source_layout,
                                source_len.clone(),
                                out_tok_types,
                                out_tok_starts,
                                out_tok_lens,
                                out_source_words,
                                max_out_tokens,
                                max_out_source_bytes,
                                num_tokens.clone(),
                            ),
                            vec![Node::if_then_else(
                                Expr::eq(Expr::var("named_repl_tok"), Expr::u32(TOK_HASHHASH)),
                                emit_materialized_function_paste_branch(
                                    in_tok_types,
                                    in_tok_starts,
                                    in_tok_lens,
                                    source_words,
                                    source_layout,
                                    macro_vals,
                                    macro_replacement_params,
                                    macro_replacement_starts,
                                    macro_replacement_lens,
                                    macro_replacement_words,
                                    macro_replacement_layout,
                                    out_tok_types,
                                    out_tok_starts,
                                    out_tok_lens,
                                    out_source_words,
                                    macro_arg_starts,
                                    macro_arg_ends,
                                    num_tokens.clone(),
                                    source_len.clone(),
                                    macro_replacement_source_len.clone(),
                                    max_out_tokens,
                                    max_out_source_bytes,
                                ),
                                emit_materialized_regular_replacement_branch(
                                    in_tok_types,
                                    in_tok_starts,
                                    in_tok_lens,
                                    source_words,
                                    source_layout,
                                    macro_replacement_starts,
                                    macro_replacement_lens,
                                    macro_replacement_words,
                                    macro_replacement_layout,
                                    out_tok_types,
                                    out_tok_starts,
                                    out_tok_lens,
                                    out_source_words,
                                    macro_arg_starts,
                                    macro_arg_ends,
                                    num_tokens.clone(),
                                    source_len.clone(),
                                    macro_replacement_source_len.clone(),
                                    max_out_tokens,
                                    max_out_source_bytes,
                                ),
                            )],
                        ));
                        repl
                    },
                )]
            },
        ),
        Node::assign(
            "named_i",
            Expr::add(Expr::var("macro_close_idx"), Expr::u32(1)),
        ),
    ]);
    nodes
}
