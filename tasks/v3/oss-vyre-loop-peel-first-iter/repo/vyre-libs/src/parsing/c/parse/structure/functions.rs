use super::*;

/// Extracted C11 Functions using Tier 3 Subgroup Allocation Strategy
#[must_use]
pub fn c11_extract_functions(
    tok_types: &str,
    paren_pairs: &str,
    brace_pairs: &str,
    num_tokens: Expr,
    out_functions: &str,
    out_counts: &str,
) -> Program {
    let t = Expr::var("t");

    // Flattened guard: `Expr::load` has no side effects, so reading
    // `next_type`, `matching_rparen`, `after_rparen_type`, and
    // `matching_rbrace` unconditionally at every index is cheaper
    // than the original 5-level nested if_then and keeps the
    // composition under the depth-6 budget enforced by
    // vyre-conform-enforce. Non-identifier positions read values
    // that never reach the `is_match` write path because the
    // guard expression gates the whole decision.
    let mut loop_body = emit_token_context(
        tok_types,
        paren_pairs,
        &num_tokens,
        &t,
        TokenContextOptions {
            before_wrapper_type: true,
            parenthesized_wrapper_rparen: true,
            after_wrapper_type_and_rparen: true,
            ..TokenContextOptions::default()
        },
    );
    loop_body.extend([
        Node::let_bind(
            "is_parenthesized_function_name",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::and(
                        Expr::eq(Expr::var("prev_type"), Expr::u32(TOK_LPAREN)),
                        Expr::eq(Expr::var("next_type"), Expr::u32(TOK_RPAREN)),
                    ),
                ),
                Expr::and(
                    Expr::eq(
                        Expr::var("parenthesized_wrapper_rparen"),
                        Expr::add(t.clone(), Expr::u32(1)),
                    ),
                    Expr::eq(Expr::var("after_wrapper_type"), Expr::u32(TOK_LPAREN)),
                ),
            ),
        ),
        Node::let_bind(
            "is_numeric_suffix_function_name",
            Expr::and(
                Expr::and(
                    Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
                    Expr::eq(Expr::var("next_type"), Expr::u32(TOK_INTEGER)),
                ),
                Expr::eq(Expr::var("after_wrapper_type"), Expr::u32(TOK_LPAREN)),
            ),
        ),
        Node::if_then(
            Expr::var("is_numeric_suffix_function_name"),
            vec![Node::assign(
                "matching_rparen",
                Expr::var("after_wrapper_rparen"),
            )],
        ),
        Node::if_then(
            Expr::var("is_parenthesized_function_name"),
            vec![Node::assign(
                "matching_rparen",
                Expr::var("after_wrapper_rparen"),
            )],
        ),
    ]);
    loop_body.extend([
        Node::let_bind("function_body_scan_start", num_tokens.clone()),
        Node::if_then(
            Expr::ne(Expr::var("matching_rparen"), Expr::u32(u32::MAX)),
            vec![Node::assign(
                "function_body_scan_start",
                Expr::add(Expr::var("matching_rparen"), Expr::u32(1)),
            )],
        ),
    ]);
    loop_body.extend(emit_body_open_scan(
        tok_types,
        Expr::var("function_body_scan_start"),
        num_tokens.clone(),
        "body_open",
    ));
    loop_body.extend([
        Node::let_bind("matching_rbrace", Expr::u32(u32::MAX)),
        Node::if_then(
            Expr::ne(Expr::var("body_open"), Expr::u32(u32::MAX)),
            vec![Node::assign(
                "matching_rbrace",
                Expr::load(brace_pairs, Expr::var("body_open")),
            )],
        ),
        // Single flattened predicate. 5-way AND collapses the
        // previously-nested shape into one if_then.
        Node::let_bind(
            "is_attribute_suffix",
            Expr::and(
                Expr::eq(Expr::var("prev_type"), Expr::u32(TOK_RPAREN)),
                Expr::eq(Expr::var("before_wrapper_type"), Expr::u32(TOK_RPAREN)),
            ),
        ),
        Node::let_bind(
            "is_match",
            Expr::and(
                Expr::and(
                    Expr::and(
                        Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
                        Expr::or(
                            Expr::and(
                                Expr::or(
                                    Expr::eq(Expr::var("next_type"), Expr::u32(TOK_LPAREN)),
                                    Expr::var("is_numeric_suffix_function_name"),
                                ),
                                Expr::or(
                                    function_prefix_token(Expr::var("prev_type")),
                                    Expr::var("is_attribute_suffix"),
                                ),
                            ),
                            Expr::and(
                                Expr::var("is_parenthesized_function_name"),
                                function_prefix_token(Expr::var("before_wrapper_type")),
                            ),
                        ),
                    ),
                    Expr::and(
                        Expr::ne(Expr::var("matching_rparen"), Expr::u32(u32::MAX)),
                        Expr::ne(Expr::var("body_open"), Expr::u32(u32::MAX)),
                    ),
                ),
                Expr::ne(Expr::var("matching_rbrace"), Expr::u32(u32::MAX)),
            ),
        ),
        Node::if_then(
            Expr::var("is_match"),
            emit_sparse_record_write(
                out_functions,
                t.clone(),
                3,
                vec![
                    t.clone(),
                    Expr::var("body_open"),
                    Expr::var("matching_rbrace"),
                ],
            ),
        ),
    ]);

    let tok_count = literal_u32_or(&num_tokens, 1);
    let mut buffers = token_pair_input_buffers(tok_types, paren_pairs, tok_count);
    buffers.extend([
        BufferDecl::storage(brace_pairs, 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(tok_count),
    ]);
    append_record_output_buffers(
        &mut buffers,
        out_functions,
        3,
        3,
        tok_count,
        out_counts,
        4,
        false,
    );
    let sparse_zero_pre_loop =
        emit_sparse_record_output_init(out_functions, out_counts, t.clone(), num_tokens.clone(), 3);
    threaded_structure_program(
        "vyre-libs::parsing::c11_extract_functions",
        buffers,
        sparse_zero_pre_loop,
        Expr::lt(t, Expr::sub(num_tokens, Expr::u32(2))),
        loop_body,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn function_body_scan_does_not_add_one_to_match_none() {
        let source = include_str!("functions.rs");
        assert!(
            source.contains("function_body_scan_start"),
            "Fix: function extraction must route body scans through a guarded start variable."
        );
        assert!(
            !source.contains("emit_body_open_scan(\n        tok_types,\n        Expr::add(Expr::var(\"matching_rparen\"), Expr::u32(1))"),
            "Fix: function extraction must not add one to MATCH_NONE before guarding the body scan."
        );
    }
}
