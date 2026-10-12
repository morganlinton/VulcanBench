//! Token-paste branch builder for macro replacement.

use crate::parsing::c::lex::tokens::TOK_COMMA;
use crate::parsing::c::preprocess::materialization::*;
use vyre::ir::{Expr, Node};

use super::helpers::*;
use super::*;

pub(super) fn emit_materialized_function_paste_branch(
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
    let mut paste = vec![
        Node::if_then(
            Expr::eq(Expr::var("named_out_idx"), Expr::u32(0)),
            vec![Node::trap(
                Expr::var("named_repl_i"),
                "function-like-token-paste-missing-left-token",
            )],
        ),
        Node::if_then(
            Expr::ge(
                Expr::add(Expr::var("named_repl_i"), Expr::u32(1)),
                Expr::var("named_repl_size"),
            ),
            vec![Node::trap(
                Expr::var("named_repl_i"),
                "function-like-token-paste-missing-right-token",
            )],
        ),
        Node::let_bind(
            "macro_paste_next_offset",
            Expr::add(
                Expr::var("named_macro_idx"),
                Expr::add(Expr::var("named_repl_i"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "macro_paste_next_param",
            Expr::load(
                macro_replacement_params,
                Expr::var("macro_paste_next_offset"),
            ),
        ),
        Node::let_bind("macro_paste_right_tok", Expr::u32(0)),
        Node::let_bind("macro_paste_right_start", Expr::u32(0)),
        Node::let_bind("macro_paste_right_len", Expr::u32(0)),
        Node::let_bind("macro_paste_right_source_limit", Expr::u32(0)),
        Node::let_bind("macro_paste_right_from_argument", Expr::u32(0)),
        Node::let_bind("macro_paste_arg_start", Expr::u32(0)),
        Node::let_bind("macro_paste_arg_end", Expr::u32(0)),
    ];
    paste.push(Node::if_then_else(
        Expr::eq(
            Expr::var("macro_paste_next_param"),
            Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
        ),
        vec![
            Node::assign(
                "macro_paste_right_tok",
                Expr::load(macro_vals, Expr::var("macro_paste_next_offset")),
            ),
            Node::assign(
                "macro_paste_right_start",
                Expr::load(
                    macro_replacement_starts,
                    Expr::var("macro_paste_next_offset"),
                ),
            ),
            Node::assign(
                "macro_paste_right_len",
                Expr::load(macro_replacement_lens, Expr::var("macro_paste_next_offset")),
            ),
            Node::assign(
                "macro_paste_right_source_limit",
                macro_replacement_source_len.clone(),
            ),
        ],
        {
            let arg_start =
                selected_arg_bound(macro_arg_starts, Expr::var("macro_paste_next_param"));
            let arg_end = selected_arg_bound(macro_arg_ends, Expr::var("macro_paste_next_param"));
            vec![
                Node::if_then(
                    Expr::ge(
                        Expr::var("macro_paste_next_param"),
                        Expr::var("named_param_count"),
                    ),
                    vec![Node::trap(
                        Expr::var("macro_paste_next_param"),
                        "function-like-token-paste-parameter-out-of-range",
                    )],
                ),
                Node::assign("macro_paste_arg_start", arg_start),
                Node::assign("macro_paste_arg_end", arg_end),
                Node::if_then(
                    Expr::lt(
                        Expr::var("macro_paste_arg_start"),
                        Expr::var("macro_paste_arg_end"),
                    ),
                    vec![
                        Node::assign(
                            "macro_paste_right_tok",
                            Expr::load(in_tok_types, Expr::var("macro_paste_arg_start")),
                        ),
                        Node::assign(
                            "macro_paste_right_start",
                            Expr::load(in_tok_starts, Expr::var("macro_paste_arg_start")),
                        ),
                        Node::assign(
                            "macro_paste_right_len",
                            Expr::load(in_tok_lens, Expr::var("macro_paste_arg_start")),
                        ),
                        Node::assign("macro_paste_right_source_limit", source_len.clone()),
                        Node::assign("macro_paste_right_from_argument", Expr::u32(1)),
                    ],
                ),
            ]
        },
    ));
    let mut nonempty_rhs = vec![
        Node::let_bind(
            "macro_paste_left_tok",
            Expr::load(
                out_tok_types,
                Expr::sub(Expr::var("named_out_idx"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "macro_paste_synth_tok",
            synthesized_paste_token(
                Expr::var("macro_paste_left_tok"),
                Expr::var("macro_paste_right_tok"),
            ),
        ),
        Node::if_then(
            Expr::eq(
                Expr::var("macro_paste_synth_tok"),
                Expr::u32(EMPTY_MACRO_SLOT),
            ),
            vec![Node::trap(
                Expr::var("macro_paste_right_tok"),
                "function-like-token-paste-cannot-synthesize-token-type-from-materialized-bytes",
            )],
        ),
        Node::store(
            out_tok_types,
            Expr::sub(Expr::var("named_out_idx"), Expr::u32(1)),
            Expr::var("macro_paste_synth_tok"),
        ),
    ];
    nonempty_rhs.push(Node::if_then_else(
        Expr::eq(Expr::var("macro_paste_right_from_argument"), Expr::u32(1)),
        append_to_previous_output_token(
            "function_paste_arg_rhs",
            source_words,
            source_layout,
            Expr::var("macro_paste_right_start"),
            Expr::var("macro_paste_right_len"),
            source_len.clone(),
            out_tok_starts,
            out_tok_lens,
            out_source_words,
            max_out_source_bytes,
            "function-like-token-paste-argument-source-span-out-of-bounds",
        ),
        append_to_previous_output_token(
            "function_paste_literal_rhs",
            macro_replacement_words,
            macro_replacement_layout,
            Expr::var("macro_paste_right_start"),
            Expr::var("macro_paste_right_len"),
            macro_replacement_source_len.clone(),
            out_tok_starts,
            out_tok_lens,
            out_source_words,
            max_out_source_bytes,
            "function-like-token-paste-literal-source-span-out-of-bounds",
        ),
    ));
    nonempty_rhs.push(Node::if_then(
        Expr::eq(Expr::var("macro_paste_right_from_argument"), Expr::u32(1)),
        vec![Node::loop_for(
            "macro_paste_rhs_rest_rel",
            Expr::u32(1),
            num_tokens.clone(),
            vec![Node::if_then(
                Expr::lt(
                    Expr::add(
                        Expr::var("macro_paste_arg_start"),
                        Expr::var("macro_paste_rhs_rest_rel"),
                    ),
                    Expr::var("macro_paste_arg_end"),
                ),
                {
                    let mut copy = vec![Node::let_bind(
                        "macro_paste_rhs_rest_idx",
                        Expr::add(
                            Expr::var("macro_paste_arg_start"),
                            Expr::var("macro_paste_rhs_rest_rel"),
                        ),
                    )];
                    copy.extend(emit_materialized_output_token(
                        "function_paste_rhs_rest",
                        Expr::load(in_tok_types, Expr::var("macro_paste_rhs_rest_idx")),
                        source_words,
                        source_layout,
                        Expr::load(in_tok_starts, Expr::var("macro_paste_rhs_rest_idx")),
                        Expr::load(in_tok_lens, Expr::var("macro_paste_rhs_rest_idx")),
                        source_len.clone(),
                        out_tok_types,
                        out_tok_starts,
                        out_tok_lens,
                        out_source_words,
                        max_out_tokens,
                        max_out_source_bytes,
                        "function-like-token-paste-rest-source-span-out-of-bounds",
                    ));
                    copy
                },
            )],
        )],
    ));
    nonempty_rhs.push(Node::assign("named_skip_repl", Expr::u32(1)));
    paste.push(Node::if_then_else(
        Expr::eq(Expr::var("macro_paste_right_len"), Expr::u32(0)),
        vec![
            Node::let_bind(
                "macro_paste_empty_prev_idx",
                Expr::sub(Expr::var("named_out_idx"), Expr::u32(1)),
            ),
            Node::let_bind(
                "macro_paste_empty_prev_tok",
                Expr::load(out_tok_types, Expr::var("macro_paste_empty_prev_idx")),
            ),
            Node::if_then(
                Expr::eq(
                    Expr::var("macro_paste_empty_prev_tok"),
                    Expr::u32(TOK_COMMA),
                ),
                vec![
                    Node::assign(
                        "named_source_out_idx",
                        Expr::load(out_tok_starts, Expr::var("macro_paste_empty_prev_idx")),
                    ),
                    Node::assign("named_out_idx", Expr::var("macro_paste_empty_prev_idx")),
                ],
            ),
            Node::assign("named_skip_repl", Expr::u32(1)),
        ],
        nonempty_rhs,
    ));
    paste
}
