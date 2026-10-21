//! Source-arena helpers for C macro expansion.

use crate::parsing::c::preprocess::expansion::{load_macro_byte, MacroByteLayout};
use crate::parsing::c::preprocess::synthesis::stringification_token_type;
use vyre::ir::{Expr, Node};

/// Output source arena metadata buffer slots.
pub const C_MACRO_SOURCE_COUNT_BYTES: u32 = 0;

fn append_byte(out_source_words: &str, max_out_source_bytes: u32, byte: Expr) -> Vec<Node> {
    vec![
        Node::if_then(
            Expr::gt(
                Expr::add(Expr::var("named_source_out_idx"), Expr::u32(1)),
                Expr::u32(max_out_source_bytes),
            ),
            vec![Node::trap(
                Expr::add(Expr::var("named_source_out_idx"), Expr::u32(1)),
                "named-macro-source-arena-overflow",
            )],
        ),
        Node::store(
            out_source_words,
            Expr::var("named_source_out_idx"),
            Expr::bitand(byte, Expr::u32(0xff)),
        ),
        Node::assign(
            "named_source_out_idx",
            Expr::add(Expr::var("named_source_out_idx"), Expr::u32(1)),
        ),
    ]
}

fn append_separator_if_needed(out_source_words: &str, max_out_source_bytes: u32) -> Vec<Node> {
    vec![Node::if_then(
        Expr::gt(Expr::var("named_out_idx"), Expr::u32(0)),
        append_byte(
            out_source_words,
            max_out_source_bytes,
            Expr::u32(u32::from(b' ')),
        ),
    )]
}

fn check_span(
    prefix: &str,
    start: Expr,
    len: Expr,
    source_limit: Expr,
    trap: &'static str,
) -> Vec<Node> {
    let start_name = format!("{prefix}_copy_start");
    let len_name = format!("{prefix}_copy_len");
    let end_name = format!("{prefix}_copy_end");
    vec![
        Node::let_bind(&start_name, start),
        Node::let_bind(&len_name, len),
        Node::let_bind(
            &end_name,
            Expr::add(Expr::var(&start_name), Expr::var(&len_name)),
        ),
        Node::if_then(
            Expr::or(
                Expr::lt(Expr::var(&end_name), Expr::var(&start_name)),
                Expr::gt(Expr::var(&end_name), source_limit),
            ),
            vec![Node::trap(Expr::var(&end_name), trap)],
        ),
    ]
}

fn append_span_after_checked(
    prefix: &str,
    source_words: &str,
    source_layout: MacroByteLayout,
    out_source_words: &str,
    max_out_source_bytes: u32,
) -> Vec<Node> {
    let byte_rel = format!("{prefix}_copy_byte_rel");
    let source_byte = format!("{prefix}_copy_byte");
    let mut body = vec![Node::let_bind(
        &source_byte,
        load_macro_byte(
            source_words,
            source_layout,
            Expr::add(
                Expr::var(format!("{prefix}_copy_start")),
                Expr::var(&byte_rel),
            ),
        ),
    )];
    body.extend(append_byte(
        out_source_words,
        max_out_source_bytes,
        Expr::var(&source_byte),
    ));
    vec![Node::loop_for(
        byte_rel.clone(),
        Expr::u32(0),
        Expr::var(format!("{prefix}_copy_len")),
        body,
    )]
}

/// Copy one source span into the materialized output arena.
pub(crate) fn append_source_span(
    prefix: &str,
    source_words: &str,
    source_layout: MacroByteLayout,
    source_start: Expr,
    source_len: Expr,
    source_limit: Expr,
    out_source_words: &str,
    max_out_source_bytes: u32,
    trap: &'static str,
) -> Vec<Node> {
    let mut nodes = check_span(prefix, source_start, source_len, source_limit, trap);
    nodes.extend(append_span_after_checked(
        prefix,
        source_words,
        source_layout,
        out_source_words,
        max_out_source_bytes,
    ));
    nodes
}

/// Emit one output token and copy its bytes into the materialized source arena.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_materialized_output_token(
    prefix: &str,
    tok_type: Expr,
    source_words: &str,
    source_layout: MacroByteLayout,
    source_start: Expr,
    source_len: Expr,
    source_limit: Expr,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_source_words: &str,
    max_out_tokens: u32,
    max_out_source_bytes: u32,
    trap: &'static str,
) -> Vec<Node> {
    let out_start = format!("{prefix}_out_start");
    let mut nodes = vec![Node::if_then(
        Expr::gt(
            Expr::add(Expr::var("named_out_idx"), Expr::u32(1)),
            Expr::u32(max_out_tokens),
        ),
        vec![Node::trap(
            Expr::add(Expr::var("named_out_idx"), Expr::u32(1)),
            "named-macro-expansion-output-overflow",
        )],
    )];
    nodes.extend(append_separator_if_needed(
        out_source_words,
        max_out_source_bytes,
    ));
    nodes.extend([
        Node::let_bind(&out_start, Expr::var("named_source_out_idx")),
        Node::store(out_tok_types, Expr::var("named_out_idx"), tok_type),
        Node::store(
            out_tok_starts,
            Expr::var("named_out_idx"),
            Expr::var(&out_start),
        ),
    ]);
    nodes.extend(append_source_span(
        prefix,
        source_words,
        source_layout,
        source_start,
        source_len,
        source_limit,
        out_source_words,
        max_out_source_bytes,
        trap,
    ));
    nodes.extend([
        Node::store(
            out_tok_lens,
            Expr::var("named_out_idx"),
            Expr::sub(Expr::var("named_source_out_idx"), Expr::var(&out_start)),
        ),
        Node::assign(
            "named_out_idx",
            Expr::add(Expr::var("named_out_idx"), Expr::u32(1)),
        ),
    ]);
    nodes
}

/// Append bytes to the previous output token without inserting whitespace.
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_to_previous_output_token(
    prefix: &str,
    source_words: &str,
    source_layout: MacroByteLayout,
    source_start: Expr,
    source_len: Expr,
    source_limit: Expr,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_source_words: &str,
    max_out_source_bytes: u32,
    trap: &'static str,
) -> Vec<Node> {
    let prev = format!("{prefix}_prev_idx");
    let prev_start = format!("{prefix}_prev_start");
    let prev_len = format!("{prefix}_prev_len");
    let expected_cursor = format!("{prefix}_expected_cursor");
    let mut nodes = vec![
        Node::if_then(
            Expr::eq(Expr::var("named_out_idx"), Expr::u32(0)),
            vec![Node::trap(
                Expr::var("named_out_idx"),
                "token-paste-missing-materialized-left-token",
            )],
        ),
        Node::let_bind(&prev, Expr::sub(Expr::var("named_out_idx"), Expr::u32(1))),
        Node::let_bind(&prev_start, Expr::load(out_tok_starts, Expr::var(&prev))),
        Node::let_bind(&prev_len, Expr::load(out_tok_lens, Expr::var(&prev))),
        Node::let_bind(
            &expected_cursor,
            Expr::add(Expr::var(&prev_start), Expr::var(&prev_len)),
        ),
        Node::if_then(
            Expr::ne(
                Expr::var(&expected_cursor),
                Expr::var("named_source_out_idx"),
            ),
            vec![Node::trap(
                Expr::var("named_source_out_idx"),
                "token-paste-left-token-is-not-arena-tail",
            )],
        ),
    ];
    nodes.extend(append_source_span(
        prefix,
        source_words,
        source_layout,
        source_start,
        source_len,
        source_limit,
        out_source_words,
        max_out_source_bytes,
        trap,
    ));
    nodes.push(Node::store(
        out_tok_lens,
        Expr::var(&prev),
        Expr::sub(Expr::var("named_source_out_idx"), Expr::var(&prev_start)),
    ));
    nodes
}

/// Emit the `# parameter` string literal token into the materialized source arena.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_stringified_argument_token(
    prefix: &str,
    arg_start: Expr,
    arg_end: Expr,
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
    let out_start = format!("{prefix}_string_out_start");
    let arg_start_name = format!("{prefix}_string_arg_start");
    let arg_end_name = format!("{prefix}_string_arg_end");
    let rel = format!("{prefix}_string_tok_rel");
    let tok_idx = format!("{prefix}_string_tok_idx");
    let tok_start = format!("{prefix}_string_tok_start");
    let tok_len = format!("{prefix}_string_tok_len");
    let tok_end = format!("{prefix}_string_tok_end");
    let byte_rel = format!("{prefix}_string_byte_rel");
    let byte = format!("{prefix}_string_byte");
    let needs_escape = format!("{prefix}_string_needs_escape");
    let seen_token = format!("{prefix}_string_seen_token");
    let quote_byte = format!("{prefix}_string_quote_byte");

    let mut nodes = vec![
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
        Node::let_bind(&arg_start_name, arg_start),
        Node::let_bind(&arg_end_name, arg_end),
        Node::if_then(
            Expr::or(
                Expr::gt(Expr::var(&arg_start_name), Expr::var(&arg_end_name)),
                Expr::gt(Expr::var(&arg_end_name), num_tokens.clone()),
            ),
            vec![Node::trap(
                Expr::var(&arg_end_name),
                "function-like-stringification-argument-span-out-of-range",
            )],
        ),
    ];
    nodes.extend(append_separator_if_needed(
        out_source_words,
        max_out_source_bytes,
    ));
    nodes.extend([
        Node::let_bind(&out_start, Expr::var("named_source_out_idx")),
        Node::let_bind(&quote_byte, Expr::u32(u32::from(b'"'))),
        Node::if_then(
            Expr::gt(
                Expr::add(Expr::var(&out_start), Expr::u32(1)),
                Expr::u32(max_out_source_bytes),
            ),
            vec![Node::trap(
                Expr::add(Expr::var(&out_start), Expr::u32(1)),
                "named-macro-source-arena-overflow",
            )],
        ),
        Node::store(
            out_source_words,
            Expr::var(&out_start),
            Expr::var(&quote_byte),
        ),
        Node::assign(
            "named_source_out_idx",
            Expr::add(Expr::var(&out_start), Expr::u32(1)),
        ),
        Node::store(
            out_tok_types,
            Expr::var("named_out_idx"),
            Expr::u32(stringification_token_type()),
        ),
        Node::store(
            out_tok_starts,
            Expr::var("named_out_idx"),
            Expr::var(&out_start),
        ),
    ]);
    nodes.push(Node::let_bind(&seen_token, Expr::u32(0)));
    nodes.push(Node::loop_for(
        rel.clone(),
        Expr::u32(0),
        num_tokens.clone(),
        vec![Node::if_then(
            Expr::lt(
                Expr::add(Expr::var(&arg_start_name), Expr::var(&rel)),
                Expr::var(&arg_end_name),
            ),
            {
                let mut body = vec![
                    Node::let_bind(
                        &tok_idx,
                        Expr::add(Expr::var(&arg_start_name), Expr::var(&rel)),
                    ),
                    Node::if_then(
                        Expr::gt(Expr::var(&seen_token), Expr::u32(0)),
                        append_byte(
                            out_source_words,
                            max_out_source_bytes,
                            Expr::u32(u32::from(b' ')),
                        ),
                    ),
                    Node::assign(&seen_token, Expr::u32(1)),
                    Node::let_bind(&tok_start, Expr::load(in_tok_starts, Expr::var(&tok_idx))),
                    Node::let_bind(&tok_len, Expr::load(in_tok_lens, Expr::var(&tok_idx))),
                    Node::let_bind(
                        &tok_end,
                        Expr::add(Expr::var(&tok_start), Expr::var(&tok_len)),
                    ),
                    Node::if_then(
                        Expr::or(
                            Expr::lt(Expr::var(&tok_end), Expr::var(&tok_start)),
                            Expr::gt(Expr::var(&tok_end), source_len.clone()),
                        ),
                        vec![Node::trap(
                            Expr::var(&tok_end),
                            "function-like-stringification-source-span-out-of-bounds",
                        )],
                    ),
                ];
                let mut byte_body = vec![
                    Node::let_bind(
                        &byte,
                        load_macro_byte(
                            source_words,
                            source_layout,
                            Expr::add(Expr::var(&tok_start), Expr::var(&byte_rel)),
                        ),
                    ),
                    Node::let_bind(
                        &needs_escape,
                        Expr::or(
                            Expr::eq(Expr::var(&byte), Expr::u32(u32::from(b'\\'))),
                            Expr::eq(Expr::var(&byte), Expr::u32(u32::from(b'"'))),
                        ),
                    ),
                    Node::if_then(
                        Expr::var(&needs_escape),
                        append_byte(
                            out_source_words,
                            max_out_source_bytes,
                            Expr::u32(u32::from(b'\\')),
                        ),
                    ),
                ];
                byte_body.extend(append_byte(
                    out_source_words,
                    max_out_source_bytes,
                    Expr::var(&byte),
                ));
                body.push(Node::loop_for(
                    byte_rel.clone(),
                    Expr::u32(0),
                    Expr::var(&tok_len),
                    byte_body,
                ));
                body
            },
        )],
    ));
    nodes.extend(append_byte(
        out_source_words,
        max_out_source_bytes,
        Expr::var(&quote_byte),
    ));
    nodes.extend([
        Node::store(
            out_tok_lens,
            Expr::var("named_out_idx"),
            Expr::sub(Expr::var("named_source_out_idx"), Expr::var(&out_start)),
        ),
        Node::assign(
            "named_out_idx",
            Expr::add(Expr::var("named_out_idx"), Expr::u32(1)),
        ),
    ]);
    nodes
}
