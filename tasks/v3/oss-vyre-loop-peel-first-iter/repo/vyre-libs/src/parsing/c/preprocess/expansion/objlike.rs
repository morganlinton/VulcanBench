//! Object-like macro expansion builder.

use vyre::ir::{Expr, Node};

use super::helpers::*;

pub(super) fn emit_object_like_replacement(
    macro_vals: &str,
    macro_replacement_params: &str,
    out_tok_types: &str,
    max_out_tokens: u32,
) -> Vec<Node> {
    let mut paste_branch = emit_object_like_token_paste_prefix(
        macro_vals,
        macro_replacement_params,
        out_tok_types,
        "object-like-token-paste-cannot-synthesize-token-type",
    );
    paste_branch.push(Node::assign("named_skip_repl", Expr::u32(1)));
    let literal_branch =
        emit_one_output_token(out_tok_types, Expr::var("named_repl_tok"), max_out_tokens);
    emit_object_like_replacement_loop(
        macro_vals,
        macro_replacement_params,
        paste_branch,
        literal_branch,
    )
}
