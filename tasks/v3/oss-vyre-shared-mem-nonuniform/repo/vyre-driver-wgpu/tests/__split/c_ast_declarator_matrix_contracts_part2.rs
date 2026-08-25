use super::*;

#[test]
fn pg_lower_preserves_nested_typedef_complex_declarator_rows() {
    let fix = fixture_nested_typedef_complex_declarator();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [0usize, 4, 10, 11] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

#[test]
fn pg_lower_preserves_parameter_array_static_restrict_rows() {
    let fix = fixture_parameter_array_static_restrict();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [1usize, 2, 4, 5] {
        assert_pg_preserves_row(
            &typed,
            &pg,
            &fix.tok_starts,
            &fix.tok_lens,
            idx,
            kind_at(&typed, idx),
        );
    }
}

// ---------------------------------------------------------------------------
// GPU parity contracts  -  full pipeline (fixtures with typedef names)
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pointer_to_array() {
    let fix = fixture_pointer_to_array();
    assert_full_pipeline_parity(&fix, "pointer_to_array");
}

#[test]
fn gpu_parity_storage_class_multi_declarator() {
    let fix = fixture_storage_class_multi_declarator();
    assert_full_pipeline_parity(&fix, "storage_class_multi_declarator");
}

#[test]
fn gpu_parity_parameter_array_static_restrict() {
    let fix = fixture_parameter_array_static_restrict();
    assert_full_pipeline_parity(&fix, "parameter_array_static_restrict");
}

#[test]
fn gpu_parity_nested_typedef_complex_declarator() {
    let fix = fixture_nested_typedef_complex_declarator();
    assert_full_pipeline_parity(&fix, "nested_typedef_complex_declarator");
}

#[test]
fn gpu_parity_struct_tag_with_mixed_declarators() {
    let fix = fixture_struct_tag_with_mixed_declarators();
    assert_full_pipeline_parity(&fix, "struct_tag_with_mixed_declarators");
}

#[test]
fn gpu_parity_union_tag_with_mixed_declarators() {
    let fix = fixture_union_tag_with_mixed_declarators();
    assert_full_pipeline_parity(&fix, "union_tag_with_mixed_declarators");
}

#[test]
fn gpu_parity_enum_tag_with_mixed_declarators() {
    let fix = fixture_enum_tag_with_mixed_declarators();
    assert_full_pipeline_parity(&fix, "enum_tag_with_mixed_declarators");
}

#[test]
fn gpu_parity_heavy_qualifiers_and_storage_multi_decl() {
    let fix = fixture_heavy_qualifiers_and_storage_multi_decl();
    assert_full_pipeline_parity(&fix, "heavy_qualifiers_and_storage_multi_decl");
}

// ---------------------------------------------------------------------------
// GPU parity contracts  -  abstract declarator (no typedef names, per-stage)
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_abstract_declarator_vast_builder() {
    let fix = fixture_abstract_declarator_with_qualifiers();
    let expected = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let gpu = run_gpu_vast_builder_from_parts(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    assert_words_eq(
        &gpu,
        &expected,
        "abstract_declarator_with_qualifiers: VAST builder GPU/CPU parity",
    );
}

#[test]
fn gpu_parity_abstract_declarator_classifier() {
    let fix = fixture_abstract_declarator_with_qualifiers();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw);
    assert_words_eq(
        &gpu,
        &expected,
        "abstract_declarator_with_qualifiers: classifier GPU/CPU parity",
    );
}

// ---------------------------------------------------------------------------
// GPU parity contracts  -  PG lowerer
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_struct_tag_mixed_declarators() {
    let fix = fixture_struct_tag_with_mixed_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_words_eq(
        &gpu,
        &expected,
        "struct_tag_mixed_declarators: GPU PG lowerer must match CPU",
    );
}

#[test]
fn gpu_parity_pg_lower_nested_typedef_complex_declarator() {
    let fix = fixture_nested_typedef_complex_declarator();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_words_eq(
        &gpu,
        &expected,
        "nested_typedef_complex_declarator: GPU PG lowerer must match CPU",
    );
}

#[test]
fn gpu_parity_pg_lower_parameter_array_static_restrict() {
    let fix = fixture_parameter_array_static_restrict();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_words_eq(
        &gpu,
        &expected,
        "parameter_array_static_restrict: GPU PG lowerer must match CPU",
    );
}

#[test]
fn gpu_parity_pg_lower_abstract_declarator_with_qualifiers() {
    let fix = fixture_abstract_declarator_with_qualifiers();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_words_eq(
        &gpu,
        &expected,
        "abstract_declarator_with_qualifiers: GPU PG lowerer must match CPU",
    );
}
