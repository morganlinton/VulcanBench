use super::*;

#[test]
fn gpu_parity_pg_lower_enum_values() {
    let (tok_types, tok_starts, tok_lens) = fixture_enum_values();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for enum values"
    );
}

#[test]
fn gpu_parity_pg_lower_sizeof_type_vs_expr() {
    let (tok_types, tok_starts, tok_lens) = fixture_sizeof_type_vs_expr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for sizeof type-vs-expr"
    );
}

#[test]
fn gpu_parity_pg_lower_stmt_expr_nesting() {
    let (tok_types, tok_starts, tok_lens) = fixture_stmt_expr_nesting();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for statement/expression nesting"
    );
}
