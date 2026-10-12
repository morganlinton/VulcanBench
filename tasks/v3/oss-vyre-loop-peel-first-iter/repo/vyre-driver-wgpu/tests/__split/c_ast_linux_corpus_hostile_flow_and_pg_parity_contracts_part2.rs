use super::*;

#[test]
fn hostile_mixed_flow_full_parity_and_pg_lower() {
    let fix = super::c_ast_linux_corpus_hostile_flow_and_pg_parity_contracts_part1::fixture_hostile_mixed_flow();
    assert_full_pipeline_parity(&fix, "hostile_mixed_flow");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    // All major control-flow kinds must be present
    assert!(
        !row_indices(&typed, C_AST_KIND_SWITCH_STMT).is_empty(),
        "hostile mixed flow must contain SWITCH_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_CASE_STMT).is_empty(),
        "must contain CASE_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_DEFAULT_STMT).is_empty(),
        "must contain DEFAULT_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_IF_STMT).is_empty(),
        "must contain IF_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_WHILE_STMT).is_empty(),
        "must contain WHILE_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GOTO_STMT).is_empty(),
        "must contain GOTO_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_BREAK_STMT).is_empty(),
        "must contain BREAK_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_RETURN_STMT).is_empty(),
        "must contain RETURN_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE).is_empty(),
        "must contain GNU_ATTRIBUTE for fallthrough"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_FALLTHROUGH),
        vec![35],
        "fallthrough attribute name must produce ATTRIBUTE_FALLTHROUGH"
    );
    assert!(
        !row_indices(&typed, node_kind::BASIC_BLOCK).is_empty(),
        "must contain BASIC_BLOCK from stmt exprs"
    );

    // PG lowerer parity for the combined hostile fixture
    assert_gpu_pg_parity(&fix, &typed, "hostile_mixed_flow");
}

#[test]
fn hostile_mixed_flow_pg_preserves_all_control_kinds() {
    let fix = super::c_ast_linux_corpus_hostile_flow_and_pg_parity_contracts_part1::fixture_hostile_mixed_flow();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for kind in [
        C_AST_KIND_SWITCH_STMT,
        C_AST_KIND_CASE_STMT,
        C_AST_KIND_DEFAULT_STMT,
        C_AST_KIND_IF_STMT,
        C_AST_KIND_WHILE_STMT,
        C_AST_KIND_GOTO_STMT,
        C_AST_KIND_BREAK_STMT,
        C_AST_KIND_RETURN_STMT,
    ] {
        for idx in row_indices(&typed, kind) {
            assert_pg_preserves_row(&typed, &pg, &fix.tok_starts, &fix.tok_lens, idx, kind);
        }
    }
}
