//! Shared GPU macro-expansion helper builders.

use crate::parsing::c::lex::tokens::{TOK_HASHHASH, TOK_IDENTIFIER, TOK_LPAREN};
use crate::parsing::c::preprocess::synthesis::*;
use vyre::ir::{Expr, Node};
use vyre_primitives::hash::fnv1a::{fnv1a32_initial_expr, fnv1a32_update_byte_node};

use super::*;

pub(super) fn emit_macro_lookup(
    prefix: &str,
    token: Expr,
    macro_keys: &str,
    macro_vals: &str,
    output_var: &str,
) -> Vec<Node> {
    let token_name = format!("{prefix}_tok");
    let probe_slot = format!("{prefix}_probe_slot");
    let probed_key = format!("{prefix}_probed_key");
    let probe = format!("{prefix}_probe");
    let lookup_done = format!("{prefix}_lookup_done");
    let lookup_seen_empty = format!("{prefix}_lookup_seen_empty");
    vec![
        Node::let_bind(&token_name, token),
        Node::let_bind(
            &probe_slot,
            Expr::bitand(
                Expr::mul(Expr::var(&token_name), Expr::u32(2_654_435_769)),
                Expr::u32(MACRO_TABLE_MASK),
            ),
        ),
        Node::let_bind(output_var, Expr::u32(EMPTY_MACRO_SLOT)),
        Node::let_bind(&lookup_done, Expr::u32(0)),
        Node::let_bind(&lookup_seen_empty, Expr::u32(0)),
        Node::loop_for(
            probe,
            Expr::u32(0),
            Expr::u32(MACRO_TABLE_SLOTS),
            vec![Node::if_then(
                Expr::eq(Expr::var(&lookup_done), Expr::u32(0)),
                vec![
                    Node::let_bind(&probed_key, Expr::load(macro_keys, Expr::var(&probe_slot))),
                    Node::if_then(
                        Expr::eq(Expr::var(&probed_key), Expr::var(&token_name)),
                        vec![
                            Node::assign(
                                output_var,
                                Expr::load(macro_vals, Expr::var(&probe_slot)),
                            ),
                            Node::assign(&lookup_done, Expr::u32(1)),
                        ],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var(&probed_key), Expr::u32(EMPTY_MACRO_SLOT)),
                        vec![
                            Node::assign(&lookup_seen_empty, Expr::u32(1)),
                            Node::assign(&lookup_done, Expr::u32(1)),
                        ],
                    ),
                    Node::assign(
                        &probe_slot,
                        Expr::bitand(
                            Expr::add(Expr::var(&probe_slot), Expr::u32(1)),
                            Expr::u32(MACRO_TABLE_MASK),
                        ),
                    ),
                ],
            )],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var(output_var), Expr::u32(EMPTY_MACRO_SLOT)),
                Expr::eq(Expr::var(&lookup_seen_empty), Expr::u32(0)),
            ),
            vec![Node::trap(
                Expr::var(&token_name),
                "macro-lookup-table-full-without-empty-slot",
            )],
        ),
    ]
}

pub(super) fn emit_macro_hash_lookup(
    prefix: &str,
    name_hash: Expr,
    source_start: Expr,
    source_len: Expr,
    source_words: &str,
    source_layout: MacroByteLayout,
    macro_name_hashes: &str,
    macro_name_starts: &str,
    macro_name_lens: &str,
    macro_name_words: &str,
    macro_name_layout: MacroByteLayout,
    output_var: &str,
) -> Vec<Node> {
    let hash_name = format!("{prefix}_name_hash");
    let probe_slot = format!("{prefix}_probe_slot");
    let probed_key = format!("{prefix}_probed_key");
    let probe = format!("{prefix}_probe");
    let lookup_done = format!("{prefix}_lookup_done");
    let lookup_seen_empty = format!("{prefix}_lookup_seen_empty");
    let candidate_name_start = format!("{prefix}_candidate_name_start");
    let candidate_name_len = format!("{prefix}_candidate_name_len");
    let candidate_name_end = format!("{prefix}_candidate_name_end");
    let candidate_name_matches = format!("{prefix}_candidate_name_matches");
    let candidate_byte_i = format!("{prefix}_candidate_byte_i");
    let source_byte = format!("{prefix}_source_byte");
    let macro_name_byte = format!("{prefix}_macro_name_byte");
    vec![
        Node::let_bind(&hash_name, name_hash),
        Node::let_bind(
            &probe_slot,
            Expr::bitand(
                Expr::mul(Expr::var(&hash_name), Expr::u32(2_654_435_769)),
                Expr::u32(MACRO_TABLE_MASK),
            ),
        ),
        Node::assign(output_var, Expr::u32(EMPTY_MACRO_SLOT)),
        Node::let_bind(&lookup_done, Expr::u32(0)),
        Node::let_bind(&lookup_seen_empty, Expr::u32(0)),
        Node::loop_for(
            probe,
            Expr::u32(0),
            Expr::u32(MACRO_TABLE_SLOTS),
            vec![Node::if_then(
                Expr::eq(Expr::var(&lookup_done), Expr::u32(0)),
                vec![
                    Node::let_bind(
                        &probed_key,
                        Expr::load(macro_name_hashes, Expr::var(&probe_slot)),
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var(&probed_key), Expr::var(&hash_name)),
                        vec![
                            Node::let_bind(
                                &candidate_name_start,
                                Expr::load(macro_name_starts, Expr::var(&probe_slot)),
                            ),
                            Node::let_bind(
                                &candidate_name_len,
                                Expr::load(macro_name_lens, Expr::var(&probe_slot)),
                            ),
                            Node::let_bind(
                                &candidate_name_end,
                                Expr::add(
                                    Expr::var(&candidate_name_start),
                                    Expr::var(&candidate_name_len),
                                ),
                            ),
                            Node::if_then(
                                Expr::or(
                                    Expr::lt(
                                        Expr::var(&candidate_name_end),
                                        Expr::var(&candidate_name_start),
                                    ),
                                    Expr::gt(
                                        Expr::var(&candidate_name_end),
                                        Expr::buf_len(macro_name_words),
                                    ),
                                ),
                                vec![Node::trap(
                                    Expr::var(&candidate_name_end),
                                    "macro-name-candidate-span-out-of-bounds",
                                )],
                            ),
                            Node::let_bind(
                                &candidate_name_matches,
                                Expr::select(
                                    Expr::eq(source_len.clone(), Expr::var(&candidate_name_len)),
                                    Expr::u32(1),
                                    Expr::u32(0),
                                ),
                            ),
                            Node::loop_for(
                                candidate_byte_i.clone(),
                                Expr::u32(0),
                                Expr::var(&candidate_name_len),
                                vec![Node::if_then(
                                    Expr::eq(Expr::var(&candidate_name_matches), Expr::u32(1)),
                                    vec![
                                        Node::let_bind(
                                            &source_byte,
                                            load_macro_byte(
                                                source_words,
                                                source_layout,
                                                Expr::add(
                                                    source_start.clone(),
                                                    Expr::var(&candidate_byte_i),
                                                ),
                                            ),
                                        ),
                                        Node::let_bind(
                                            &macro_name_byte,
                                            load_macro_byte(
                                                macro_name_words,
                                                macro_name_layout,
                                                Expr::add(
                                                    Expr::var(&candidate_name_start),
                                                    Expr::var(&candidate_byte_i),
                                                ),
                                            ),
                                        ),
                                        Node::if_then(
                                            Expr::ne(
                                                Expr::var(&source_byte),
                                                Expr::var(&macro_name_byte),
                                            ),
                                            vec![Node::assign(
                                                &candidate_name_matches,
                                                Expr::u32(0),
                                            )],
                                        ),
                                    ],
                                )],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var(&candidate_name_matches), Expr::u32(1)),
                                vec![
                                    Node::assign(output_var, Expr::var(&probe_slot)),
                                    Node::assign(&lookup_done, Expr::u32(1)),
                                ],
                            ),
                        ],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var(&probed_key), Expr::u32(EMPTY_MACRO_SLOT)),
                        vec![
                            Node::assign(&lookup_seen_empty, Expr::u32(1)),
                            Node::assign(&lookup_done, Expr::u32(1)),
                        ],
                    ),
                    Node::assign(
                        &probe_slot,
                        Expr::bitand(
                            Expr::add(Expr::var(&probe_slot), Expr::u32(1)),
                            Expr::u32(MACRO_TABLE_MASK),
                        ),
                    ),
                ],
            )],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var(output_var), Expr::u32(EMPTY_MACRO_SLOT)),
                Expr::eq(Expr::var(&lookup_seen_empty), Expr::u32(0)),
            ),
            vec![Node::trap(
                Expr::var(&hash_name),
                "macro-name-lookup-table-full-without-empty-slot",
            )],
        ),
    ]
}

pub(super) fn emit_source_span_hash(
    prefix: &str,
    token_index: Expr,
    in_tok_starts: &str,
    in_tok_lens: &str,
    source_words: &str,
    source_layout: MacroByteLayout,
    source_len: Expr,
    output_var: &str,
) -> Vec<Node> {
    let start = format!("{prefix}_start");
    let len = format!("{prefix}_len");
    let end = format!("{prefix}_end");
    let byte_idx = format!("{prefix}_byte_idx");
    let byte = format!("{prefix}_byte");
    vec![
        Node::let_bind(&start, Expr::load(in_tok_starts, token_index.clone())),
        Node::let_bind(&len, Expr::load(in_tok_lens, token_index)),
        Node::let_bind(&end, Expr::add(Expr::var(&start), Expr::var(&len))),
        Node::if_then(
            Expr::or(
                Expr::lt(Expr::var(&end), Expr::var(&start)),
                Expr::gt(Expr::var(&end), source_len),
            ),
            vec![Node::trap(
                Expr::var(&end),
                "macro-name-source-span-out-of-bounds",
            )],
        ),
        Node::let_bind(output_var, fnv1a32_initial_expr()),
        Node::loop_for(
            byte_idx.clone(),
            Expr::u32(0),
            Expr::var(&len),
            vec![
                Node::let_bind(
                    &byte,
                    load_macro_byte(
                        source_words,
                        source_layout,
                        Expr::add(Expr::var(&start), Expr::var(&byte_idx)),
                    ),
                ),
                fnv1a32_update_byte_node(output_var, Expr::var(&byte)),
            ],
        ),
    ]
}

pub(super) struct NamedMacroScanSpec<'a> {
    pub(super) in_tok_types: &'a str,
    pub(super) in_tok_starts: &'a str,
    pub(super) in_tok_lens: &'a str,
    pub(super) source_words: &'a str,
    pub(super) source_layout: MacroByteLayout,
    pub(super) macro_name_hashes: &'a str,
    pub(super) macro_name_starts: &'a str,
    pub(super) macro_name_lens: &'a str,
    pub(super) macro_name_words: &'a str,
    pub(super) macro_name_layout: MacroByteLayout,
    pub(super) macro_vals: &'a str,
    pub(super) macro_kinds: &'a str,
    pub(super) macro_param_counts: &'a str,
    pub(super) source_len: Expr,
    pub(super) decode_variadic_param_count: bool,
}

pub(super) fn emit_named_macro_scan_prefix(spec: NamedMacroScanSpec<'_>) -> Vec<Node> {
    let mut process_current = vec![
        Node::let_bind(
            "named_tok",
            Expr::load(spec.in_tok_types, Expr::var("named_i")),
        ),
        Node::let_bind("named_macro_slot", Expr::u32(EMPTY_MACRO_SLOT)),
        Node::let_bind("named_macro_idx", Expr::u32(EMPTY_MACRO_SLOT)),
        Node::let_bind("named_macro_kind", Expr::u32(C_MACRO_KIND_OBJECT_LIKE)),
        Node::let_bind("named_param_count", Expr::u32(0)),
        Node::let_bind("named_is_variadic", Expr::u32(0)),
        Node::let_bind("named_required_param_count", Expr::u32(0)),
    ];

    process_current.push(Node::if_then(
        Expr::eq(Expr::var("named_tok"), Expr::u32(TOK_IDENTIFIER)),
        {
            let mut ident = emit_source_span_hash(
                "named",
                Expr::var("named_i"),
                spec.in_tok_starts,
                spec.in_tok_lens,
                spec.source_words,
                spec.source_layout,
                spec.source_len,
                "named_name_hash",
            );
            ident.extend(emit_macro_hash_lookup(
                "named_lookup",
                Expr::var("named_name_hash"),
                Expr::var("named_start"),
                Expr::var("named_len"),
                spec.source_words,
                spec.source_layout,
                spec.macro_name_hashes,
                spec.macro_name_starts,
                spec.macro_name_lens,
                spec.macro_name_words,
                spec.macro_name_layout,
                "named_macro_slot",
            ));
            ident
        },
    ));

    let mut found_macro = vec![
        Node::assign(
            "named_macro_idx",
            Expr::load(spec.macro_vals, Expr::var("named_macro_slot")),
        ),
        Node::assign(
            "named_macro_kind",
            Expr::load(spec.macro_kinds, Expr::var("named_macro_slot")),
        ),
    ];
    if spec.decode_variadic_param_count {
        found_macro.extend([
            Node::let_bind(
                "named_param_count_raw",
                Expr::load(spec.macro_param_counts, Expr::var("named_macro_slot")),
            ),
            Node::assign(
                "named_param_count",
                Expr::bitand(Expr::var("named_param_count_raw"), Expr::u32(0x7fff_ffff)),
            ),
            Node::assign(
                "named_is_variadic",
                Expr::shr(Expr::var("named_param_count_raw"), Expr::u32(31)),
            ),
            Node::assign(
                "named_required_param_count",
                Expr::saturating_sub(
                    Expr::var("named_param_count"),
                    Expr::var("named_is_variadic"),
                ),
            ),
        ]);
    } else {
        found_macro.extend([
            Node::assign(
                "named_param_count",
                Expr::load(spec.macro_param_counts, Expr::var("named_macro_slot")),
            ),
            Node::assign("named_required_param_count", Expr::var("named_param_count")),
        ]);
    }
    found_macro.push(Node::if_then(
        Expr::and(
            Expr::ne(
                Expr::var("named_macro_kind"),
                Expr::u32(C_MACRO_KIND_OBJECT_LIKE),
            ),
            Expr::ne(
                Expr::var("named_macro_kind"),
                Expr::u32(C_MACRO_KIND_FUNCTION_LIKE),
            ),
        ),
        vec![Node::trap(
            Expr::var("named_macro_kind"),
            "named-macro-kind-invalid",
        )],
    ));
    process_current.push(Node::if_then(
        Expr::ne(Expr::var("named_macro_slot"), Expr::u32(EMPTY_MACRO_SLOT)),
        found_macro,
    ));
    process_current
}

pub(super) fn emit_named_replacement_prelude(
    macro_sizes: &str,
    in_tok_types: &str,
    num_tokens: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind(
            "named_repl_size",
            Expr::load(macro_sizes, Expr::var("named_macro_idx")),
        ),
        Node::if_then(
            Expr::gt(
                Expr::add(Expr::var("named_macro_idx"), Expr::var("named_repl_size")),
                Expr::u32(MACRO_TABLE_SLOTS),
            ),
            vec![Node::trap(
                Expr::add(Expr::var("named_macro_idx"), Expr::var("named_repl_size")),
                "named-macro-replacement-range-out-of-bounds",
            )],
        ),
        Node::let_bind("named_has_open_paren", Expr::u32(0)),
        Node::if_then(
            Expr::lt(
                Expr::add(Expr::var("named_i"), Expr::u32(1)),
                num_tokens.clone(),
            ),
            vec![Node::if_then(
                Expr::eq(
                    Expr::load(in_tok_types, Expr::add(Expr::var("named_i"), Expr::u32(1))),
                    Expr::u32(TOK_LPAREN),
                ),
                vec![Node::assign("named_has_open_paren", Expr::u32(1))],
            )],
        ),
    ]
}

pub(super) fn selected_arg_bound(arg_bounds: &str, param: Expr) -> Expr {
    Expr::load(arg_bounds, param)
}

pub(super) fn assign_arg_bound(
    arg_bounds: &str,
    arg_index: Expr,
    value: Expr,
    num_tokens: Expr,
    overflow_trap: &'static str,
) -> Vec<Node> {
    vec![Node::if_then_else(
        Expr::lt(arg_index.clone(), num_tokens.clone()),
        vec![Node::store(arg_bounds, arg_index.clone(), value)],
        vec![Node::trap(arg_index, overflow_trap)],
    )]
}

pub(super) fn emit_one_output_token(
    out_tok_types: &str,
    token: Expr,
    max_out_tokens: u32,
) -> Vec<Node> {
    vec![
        Node::if_then(
            Expr::gt(
                Expr::add(Expr::var("named_out_idx"), Expr::u32(1)),
                Expr::u32(max_out_tokens),
            ),
            vec![Node::trap(
                Expr::add(Expr::var("named_out_idx"), Expr::u32(1)),
                "named-macro-expansion-output-overflow",
            )],
        ),
        Node::store(out_tok_types, Expr::var("named_out_idx"), token),
        Node::assign(
            "named_out_idx",
            Expr::add(Expr::var("named_out_idx"), Expr::u32(1)),
        ),
    ]
}

pub(super) fn emit_object_like_token_paste_prefix(
    macro_vals: &str,
    macro_replacement_params: &str,
    out_tok_types: &str,
    synth_failure_trap: &'static str,
) -> Vec<Node> {
    vec![
        Node::if_then(
            Expr::eq(Expr::var("named_out_idx"), Expr::u32(0)),
            vec![Node::trap(
                Expr::var("named_repl_i"),
                "object-like-token-paste-missing-left-token",
            )],
        ),
        Node::if_then(
            Expr::ge(
                Expr::add(Expr::var("named_repl_i"), Expr::u32(1)),
                Expr::var("named_repl_size"),
            ),
            vec![Node::trap(
                Expr::var("named_repl_i"),
                "object-like-token-paste-missing-right-token",
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
        Node::if_then(
            Expr::ne(
                Expr::var("macro_paste_next_param"),
                Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
            ),
            vec![Node::trap(
                Expr::var("macro_paste_next_param"),
                "object-like-token-paste-cannot-reference-parameters",
            )],
        ),
        Node::let_bind(
            "macro_paste_left_tok",
            Expr::load(
                out_tok_types,
                Expr::sub(Expr::var("named_out_idx"), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "macro_paste_right_tok",
            Expr::load(macro_vals, Expr::var("macro_paste_next_offset")),
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
                synth_failure_trap,
            )],
        ),
        Node::store(
            out_tok_types,
            Expr::sub(Expr::var("named_out_idx"), Expr::u32(1)),
            Expr::var("macro_paste_synth_tok"),
        ),
    ]
}

pub(super) fn emit_object_like_replacement_loop(
    macro_vals: &str,
    macro_replacement_params: &str,
    paste_branch: Vec<Node>,
    literal_branch: Vec<Node>,
) -> Vec<Node> {
    vec![
        Node::let_bind("named_skip_repl", Expr::u32(0)),
        Node::loop_for(
            "named_repl_i",
            Expr::u32(0),
            Expr::var("named_repl_size"),
            vec![Node::if_then_else(
                Expr::eq(Expr::var("named_skip_repl"), Expr::u32(1)),
                vec![Node::assign("named_skip_repl", Expr::u32(0))],
                {
                    let mut body = vec![
                        Node::let_bind(
                            "named_repl_offset",
                            Expr::add(Expr::var("named_macro_idx"), Expr::var("named_repl_i")),
                        ),
                        Node::let_bind(
                            "named_repl_param",
                            Expr::load(macro_replacement_params, Expr::var("named_repl_offset")),
                        ),
                        Node::if_then(
                            Expr::ne(
                                Expr::var("named_repl_param"),
                                Expr::u32(C_MACRO_REPLACEMENT_LITERAL),
                            ),
                            vec![Node::trap(
                                Expr::var("named_repl_param"),
                                "object-like-macro-replacement-cannot-reference-parameters",
                            )],
                        ),
                        Node::let_bind(
                            "named_repl_tok",
                            Expr::load(macro_vals, Expr::var("named_repl_offset")),
                        ),
                    ];
                    body.push(Node::if_then_else(
                        Expr::eq(Expr::var("named_repl_tok"), Expr::u32(TOK_HASHHASH)),
                        paste_branch,
                        literal_branch,
                    ));
                    body
                },
            )],
        ),
        Node::assign("named_i", Expr::add(Expr::var("named_i"), Expr::u32(1))),
    ]
}

pub(super) fn synthesized_paste_token(left: Expr, right: Expr) -> Expr {
    C_TOKEN_PASTE_RULES.iter().rev().fold(
        Expr::u32(EMPTY_MACRO_SLOT),
        |fallback, (left_tok, right_tok, out_tok)| {
            Expr::select(
                Expr::and(
                    Expr::eq(left.clone(), Expr::u32(*left_tok)),
                    Expr::eq(right.clone(), Expr::u32(*right_tok)),
                ),
                Expr::u32(*out_tok),
                fallback,
            )
        },
    )
}
