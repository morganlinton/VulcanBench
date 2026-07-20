use super::parallel_common::{
    regular_parallel_buffers, regular_parallel_classifier, regular_parallel_token_start_expr,
    RegularParallelMode, REGULAR_PARALLEL_WORKGROUP_SIZE,
};
use super::*;

pub fn c11_lexer_regular_ranked(
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
        RegularParallelMode::Ranked,
    );
    classify_at_pos.push(Node::if_then(
        Expr::eq(Expr::var("emit"), Expr::u32(1)),
        vec![
            Node::store(out_tok_types, Expr::var("rank"), Expr::var("tok_type")),
            Node::store(out_tok_starts, Expr::var("rank"), Expr::var("pos")),
            Node::store(out_tok_lens, Expr::var("rank"), Expr::var("tok_len")),
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
            "vyre-libs::parsing::c_lexer_regular_ranked",
            vec![
                Node::if_then(
                    Expr::lt(t.clone(), Expr::buf_len(haystack)),
                    vec![Node::if_then(
                        regular_parallel_token_start_expr(haystack, haystack_len, t.clone()),
                        classify_at_pos,
                    )],
                ),
                Node::if_then(
                    Expr::eq(t, Expr::u32(0)),
                    vec![
                        Node::let_bind("token_count", Expr::u32(0)),
                        Node::loop_for(
                            "count_scan",
                            Expr::u32(0),
                            Expr::buf_len(haystack),
                            vec![Node::if_then(
                                regular_parallel_token_start_expr(
                                    haystack,
                                    haystack_len,
                                    Expr::var("count_scan"),
                                ),
                                vec![Node::assign(
                                    "token_count",
                                    Expr::add(Expr::var("token_count"), Expr::u32(1)),
                                )],
                            )],
                        ),
                        Node::store(out_counts, Expr::u32(0), Expr::var("token_count")),
                    ],
                ),
            ],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c_lexer_regular_ranked")
    .with_non_composable_with_self(true)
}
