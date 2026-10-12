use super::parallel_common::{
    regular_parallel_buffers, regular_parallel_classifier, regular_parallel_token_start_expr,
    RegularParallelMode, REGULAR_PARALLEL_WORKGROUP_SIZE,
};
use super::*;

pub fn c11_lexer_regular_sparse(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let mut classify_at_pos = regular_parallel_classifier(
        haystack,
        haystack_len,
        t.clone(),
        RegularParallelMode::Sparse,
    );
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("emit"), Expr::u32(1)),
        vec![
            Node::store(out_tok_types, t.clone(), Expr::var("tok_type")),
            Node::store(out_tok_starts, t.clone(), Expr::var("pos")),
            Node::store(out_tok_lens, t.clone(), Expr::var("tok_len")),
        ],
    ));

    Program::wrapped(
        regular_parallel_buffers(
            haystack,
            out_tok_types,
            out_tok_starts,
            out_tok_lens,
            out_counts,
            haystack_len,
        ),
        [REGULAR_PARALLEL_WORKGROUP_SIZE, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c_lexer_regular_sparse",
            vec![Node::if_then(
                Expr::and(
                    Expr::lt(t.clone(), Expr::buf_len(haystack)),
                    regular_parallel_token_start_expr(haystack, haystack_len, t),
                ),
                classify_at_pos,
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c_lexer_regular_sparse")
    .with_non_composable_with_self(true)
}
