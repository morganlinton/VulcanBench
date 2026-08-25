//! Materialized object-like macro expansion builder.

use crate::parsing::c::preprocess::materialization::*;
use vyre::ir::{Expr, Node};

use super::helpers::*;
use super::MacroByteLayout;

pub(super) fn emit_materialized_object_like_replacement(
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
    macro_replacement_source_len: Expr,
    max_out_tokens: u32,
    max_out_source_bytes: u32,
) -> Vec<Node> {
    let mut paste_branch = emit_object_like_token_paste_prefix(
        macro_vals,
        macro_replacement_params,
        out_tok_types,
        "object-like-token-paste-cannot-synthesize-token-type-from-materialized-bytes",
    );
    paste_branch.extend([
        Node::let_bind(
            "macro_paste_right_start",
            Expr::load(
                macro_replacement_starts,
                Expr::var("macro_paste_next_offset"),
            ),
        ),
        Node::let_bind(
            "macro_paste_right_len",
            Expr::load(macro_replacement_lens, Expr::var("macro_paste_next_offset")),
        ),
        Node::if_then(
            Expr::eq(Expr::var("macro_paste_right_len"), Expr::u32(0)),
            vec![Node::trap(
                Expr::var("macro_paste_next_offset"),
                "object-like-token-paste-right-token-has-no-source-bytes",
            )],
        ),
    ]);
    paste_branch.extend(append_to_previous_output_token(
        "object_paste_rhs",
        macro_replacement_words,
        macro_replacement_layout,
        Expr::var("macro_paste_right_start"),
        Expr::var("macro_paste_right_len"),
        macro_replacement_source_len.clone(),
        out_tok_starts,
        out_tok_lens,
        out_source_words,
        max_out_source_bytes,
        "object-like-token-paste-right-source-span-out-of-bounds",
    ));
    paste_branch.push(Node::assign("named_skip_repl", Expr::u32(1)));

    let literal_branch = emit_materialized_output_token(
        "object_literal",
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
        "object-like-replacement-source-span-out-of-bounds",
    );
    emit_object_like_replacement_loop(
        macro_vals,
        macro_replacement_params,
        paste_branch,
        literal_branch,
    )
}
