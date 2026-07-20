use super::*;

/// Call site extraction using Subgroup Allocation
#[must_use]
pub fn c11_extract_calls(
    tok_types: &str,
    paren_pairs: &str,
    functions: &str,
    num_tokens: Expr,
    num_functions: Expr,
    out_calls: &str,
    out_counts: &str,
) -> Program {
    let t = Expr::var("t");

    let mut loop_body = emit_token_context(
        tok_types,
        paren_pairs,
        &num_tokens,
        &t,
        TokenContextOptions {
            prev_prev_type: true,
            next2_type_and_rparen: true,
            ..TokenContextOptions::default()
        },
    );
    loop_body.extend([
        Node::let_bind(
            "is_numeric_suffix_call_name",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::eq(Expr::var("next_type"), Expr::u32(TOK_INTEGER)),
                ),
                Expr::eq(Expr::var("next2_type"), Expr::u32(TOK_LPAREN)),
            ),
        ),
        Node::if_then(
            Expr::var("is_numeric_suffix_call_name"),
            vec![Node::assign(
                "matching_rparen",
                Expr::var("numeric_suffix_rparen"),
            )],
        ),
        Node::let_bind("after_direct_call", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::ne(Expr::var("matching_rparen"), Expr::u32(u32::MAX)),
                Expr::lt(
                    Expr::add(Expr::var("matching_rparen"), Expr::u32(1)),
                    num_tokens.clone(),
                ),
            ),
            vec![Node::assign(
                "after_direct_call",
                Expr::load(
                    tok_types,
                    Expr::add(Expr::var("matching_rparen"), Expr::u32(1)),
                ),
            )],
        ),
        Node::let_bind("is_function_name_record", Expr::u32(0)),
        Node::loop_for(
            "call_fn_record_scan",
            Expr::u32(0),
            num_functions.clone(),
            vec![
                Node::let_bind(
                    "call_fn_record_name",
                    Expr::load(
                        functions,
                        Expr::mul(Expr::var("call_fn_record_scan"), Expr::u32(3)),
                    ),
                ),
                Node::if_then(
                    Expr::eq(Expr::var("call_fn_record_name"), t.clone()),
                    vec![Node::assign("is_function_name_record", Expr::u32(1))],
                ),
            ],
        ),
        Node::let_bind(
            "is_direct_call",
            Expr::and(
                Expr::and(
                    Expr::and(
                        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
                        Expr::or(
                            Expr::eq(Expr::var("next_type"), Expr::u32(TOK_LPAREN)),
                            Expr::var("is_numeric_suffix_call_name"),
                        ),
                    ),
                    Expr::and(
                        Expr::ne(Expr::var("matching_rparen"), Expr::u32(u32::MAX)),
                        Expr::eq(Expr::var("is_function_name_record"), Expr::u32(0)),
                    ),
                ),
                Expr::or(
                    Expr::not(function_prefix_token(Expr::var("prev_type"))),
                    Expr::and(
                        Expr::ne(Expr::var("after_direct_call"), Expr::u32(TOK_SEMICOLON)),
                        Expr::ne(Expr::var("after_direct_call"), Expr::u32(TOK_LBRACE)),
                    ),
                ),
            ),
        ),
        Node::let_bind("args_end", Expr::var("matching_rparen")),
        Node::let_bind("ptr_wrapper_rparen", Expr::u32(u32::MAX)),
        Node::let_bind("before_ptr_wrapper_type", Expr::u32(TOK_EOF)),
        Node::if_then(
            Expr::gt(t.clone(), Expr::u32(1)),
            vec![
                Node::assign(
                    "ptr_wrapper_rparen",
                    Expr::load(paren_pairs, Expr::sub(t.clone(), Expr::u32(2))),
                ),
                Node::if_then(
                    Expr::gt(t.clone(), Expr::u32(2)),
                    vec![Node::assign(
                        "before_ptr_wrapper_type",
                        Expr::load(tok_types, Expr::sub(t.clone(), Expr::u32(3))),
                    )],
                ),
            ],
        ),
        Node::let_bind(
            "ptr_call_lparen",
            Expr::add(Expr::var("ptr_wrapper_rparen"), Expr::u32(1)),
        ),
        Node::let_bind("ptr_call_lparen_type", Expr::u32(0)),
        Node::let_bind("ptr_call_rparen", Expr::u32(u32::MAX)),
        Node::if_then(
            Expr::lt(Expr::var("ptr_call_lparen"), num_tokens.clone()),
            vec![
                Node::assign(
                    "ptr_call_lparen_type",
                    Expr::load(tok_types, Expr::var("ptr_call_lparen")),
                ),
                Node::assign(
                    "ptr_call_rparen",
                    Expr::load(paren_pairs, Expr::var("ptr_call_lparen")),
                ),
            ],
        ),
        Node::let_bind(
            "is_ptr_call",
            Expr::and(
                Expr::and(
                    Expr::and(
                        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
                        Expr::not(function_prefix_token(Expr::var("before_ptr_wrapper_type"))),
                    ),
                    Expr::and(
                        Expr::eq(Expr::var("prev_type"), Expr::u32(TOK_STAR)),
                        Expr::eq(Expr::var("prev_prev_type"), Expr::u32(TOK_LPAREN)),
                    ),
                ),
                Expr::and(
                    Expr::eq(Expr::var("next_type"), Expr::u32(TOK_RPAREN)),
                    Expr::and(
                        Expr::eq(Expr::var("ptr_call_lparen_type"), Expr::u32(TOK_LPAREN)),
                        Expr::ne(Expr::var("ptr_call_rparen"), Expr::u32(u32::MAX)),
                    ),
                ),
            ),
        ),
    ]);
    loop_body.extend(emit_enclosing_function_lookup(
        functions,
        num_functions.clone(),
        t.clone(),
    ));
    loop_body.extend([
        // Per-lane global allocation: each matching lane claims a
        // 4-slot record via one atomic_add. The previous design used
        // subgroup_add + subgroup_shuffle to batch claims per warp,
        // but this library must stay backend-neutral. Concrete drivers
        // can recognize this atomic allocation pattern and lower it to
        // target-native subgroup allocation without changing library IR.
        Node::if_then(
            Expr::var("is_direct_call"),
            emit_sparse_record_write(
                out_calls,
                t.clone(),
                4,
                vec![
                    Expr::var("caller_id"),
                    t.clone(),
                    Expr::add(t.clone(), Expr::u32(1)),
                    Expr::var("args_end"),
                ],
            ),
        ),
        Node::if_then(
            Expr::var("is_ptr_call"),
            emit_sparse_record_write(
                out_calls,
                t.clone(),
                4,
                vec![
                    Expr::var("caller_id"),
                    t.clone(),
                    Expr::var("ptr_call_lparen"),
                    Expr::var("ptr_call_rparen"),
                ],
            ),
        ),
    ]);

    let tok_count = literal_u32_or(&num_tokens, 1);
    let fn_count = literal_u32_or(&num_functions, tok_count);
    let fn_u32_words = fn_count.saturating_mul(3).max(3);
    let mut buffers = token_pair_input_buffers(tok_types, paren_pairs, tok_count);
    buffers.extend([
        BufferDecl::storage(functions, 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(fn_u32_words),
    ]);
    append_record_output_buffers(
        &mut buffers,
        out_calls,
        3,
        4,
        tok_count,
        out_counts,
        4,
        false,
    );
    let pre_loop_nodes =
        emit_sparse_record_output_init(out_calls, out_counts, t.clone(), num_tokens.clone(), 4);

    threaded_structure_program(
        "vyre-libs::parsing::c11_extract_calls",
        buffers,
        pre_loop_nodes,
        Expr::lt(t, Expr::sub(num_tokens, Expr::u32(1))),
        loop_body,
    )
}
