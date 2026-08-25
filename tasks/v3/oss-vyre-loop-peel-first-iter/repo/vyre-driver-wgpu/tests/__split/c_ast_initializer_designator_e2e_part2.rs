use super::*;

#[test]
fn gpu_parity_pg_lower_nested_designator_mixed() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designator_mixed();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = run_reference_pg_lower(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested mixed designators"
    );
}

#[test]
fn gpu_parity_pg_lower_compound_literal_expr() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_expr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = run_reference_pg_lower(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for compound literal expr"
    );
}

#[test]
fn gpu_parity_pg_lower_compound_literal_in_call() {
    let (tok_types, tok_starts, tok_lens) = fixture_compound_literal_in_call();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = run_reference_pg_lower(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for compound literal in call"
    );
}
