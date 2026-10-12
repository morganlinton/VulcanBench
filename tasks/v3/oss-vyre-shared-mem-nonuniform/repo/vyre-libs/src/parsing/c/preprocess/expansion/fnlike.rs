//! Function-like macro expansion builder.

use crate::parsing::c::lex::tokens::*;
use crate::parsing::c::preprocess::synthesis::*;
use vyre::ir::{Expr, Node};

use super::arg_scan::emit_function_like_argument_scan;
use super::helpers::*;
use super::*;

pub(super) fn emit_function_like_replacement(
    in_tok_types: &str,
    macro_vals: &str,
    macro_replacement_params: &str,
    out_tok_types: &str,
    macro_arg_starts: &str,
    macro_arg_ends: &str,
    num_tokens: Expr,
    max_out_tokens: u32,
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
                                Expr::load(macro_replacement_params, Expr::var("named_repl_offset")),
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
                            vec![
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
                                Node::if_then_else(
                                    Expr::eq(
                                        Expr::var("macro_stringify_next_param"),
                                        Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
                                    ),
                                    emit_one_output_token(
                                        out_tok_types,
                                        Expr::var("named_repl_tok"),
                                        max_out_tokens,
                                    ),
                                    {
                                        let mut stringify = vec![Node::if_then(
                                            Expr::ge(
                                                Expr::var("macro_stringify_next_param"),
                                                Expr::var("named_param_count"),
                                            ),
                                            vec![Node::trap(
                                                Expr::var("macro_stringify_next_param"),
                                                "function-like-stringification-parameter-out-of-range",
                                            )],
                                        )];
                                        stringify.extend(emit_one_output_token(
                                            out_tok_types,
                                            Expr::u32(stringification_token_type()),
                                            max_out_tokens,
                                        ));
                                        stringify.push(Node::assign("named_skip_repl", Expr::u32(1)));
                                        stringify
                                    },
                                ),
                            ],
                            vec![Node::if_then_else(
                                Expr::eq(Expr::var("named_repl_tok"), Expr::u32(TOK_HASHHASH)),
                                {
                                    let paste = vec![
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
                                        Node::let_bind("macro_paste_arg_start", Expr::u32(0)),
                                        Node::let_bind("macro_paste_arg_end", Expr::u32(0)),
                                        Node::if_then_else(
                                            Expr::eq(
                                                Expr::var("macro_paste_next_param"),
                                                Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
                                            ),
                                            vec![Node::assign(
                                                "macro_paste_right_tok",
                                                Expr::load(
                                                    macro_vals,
                                                    Expr::var("macro_paste_next_offset"),
                                                ),
                                            )],
                                            {
                                                let arg_start = selected_arg_bound(
                                                    macro_arg_starts,
                                                    Expr::var("macro_paste_next_param"),
                                                );
                                                let arg_end = selected_arg_bound(
                                                    macro_arg_ends,
                                                    Expr::var("macro_paste_next_param"),
                                                );
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
                                                        Expr::ge(
                                                            Expr::var("macro_paste_arg_start"),
                                                            Expr::var("macro_paste_arg_end"),
                                                        ),
                                                        vec![Node::trap(
                                                            Expr::var("macro_paste_next_param"),
                                                            "function-like-token-paste-empty-argument",
                                                        )],
                                                    ),
                                                    Node::assign(
                                                        "macro_paste_right_tok",
                                                        Expr::load(
                                                            in_tok_types,
                                                            Expr::var("macro_paste_arg_start"),
                                                        ),
                                                    ),
                                                ]
                                            },
                                        ),
                                        Node::let_bind(
                                            "macro_paste_left_tok",
                                            Expr::load(
                                                out_tok_types,
                                                Expr::sub(
                                                    Expr::var("named_out_idx"),
                                                    Expr::u32(1),
                                                ),
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
                                                "function-like-token-paste-cannot-synthesize-token-type",
                                            )],
                                        ),
                                        Node::store(
                                            out_tok_types,
                                            Expr::sub(Expr::var("named_out_idx"), Expr::u32(1)),
                                            Expr::var("macro_paste_synth_tok"),
                                        ),
                                        Node::if_then(
                                            Expr::ne(
                                                Expr::var("macro_paste_next_param"),
                                                Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
                                            ),
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
                                                            "macro_paste_rhs_rest_tok",
                                                            Expr::load(
                                                                in_tok_types,
                                                                Expr::add(
                                                                    Expr::var("macro_paste_arg_start"),
                                                                    Expr::var(
                                                                        "macro_paste_rhs_rest_rel",
                                                                    ),
                                                                ),
                                                            ),
                                                        )];
                                                        copy.extend(emit_one_output_token(
                                                            out_tok_types,
                                                            Expr::var("macro_paste_rhs_rest_tok"),
                                                            max_out_tokens,
                                                        ));
                                                        copy
                                                    },
                                                )],
                                            )],
                                        ),
                                        Node::assign("named_skip_repl", Expr::u32(1)),
                                    ];
                                    paste
                                },
                                {
                                    let regular_literal = emit_one_output_token(
                                        out_tok_types,
                                        Expr::var("named_repl_tok"),
                                        max_out_tokens,
                                    );
                                    let arg_start = selected_arg_bound(
                                        macro_arg_starts,
                                        Expr::var("named_repl_param"),
                                    );
                                    let arg_end = selected_arg_bound(
                                        macro_arg_ends,
                                        Expr::var("named_repl_param"),
                                    );
                                    vec![Node::if_then_else(
                                        Expr::eq(
                                            Expr::var("named_repl_param"),
                                            Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
                                        ),
                                        regular_literal,
                                        vec![
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
                                            Node::loop_for(
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
                                                            "macro_sub_arg_tok",
                                                            Expr::load(
                                                                in_tok_types,
                                                                Expr::add(
                                                                    Expr::var("macro_sub_arg_start"),
                                                                    Expr::var("macro_sub_arg_rel"),
                                                                ),
                                                            ),
                                                        )];
                                                        copy.extend(emit_one_output_token(
                                                            out_tok_types,
                                                            Expr::var("macro_sub_arg_tok"),
                                                            max_out_tokens,
                                                        ));
                                                        copy
                                                    },
                                                )],
                                            ),
                                        ],
                                    )]
                                },
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
