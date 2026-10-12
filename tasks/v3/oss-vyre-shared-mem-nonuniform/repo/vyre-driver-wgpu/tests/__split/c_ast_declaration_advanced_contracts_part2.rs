use super::*;

#[test]
fn pg_lower_preserves_bitfield_nested_struct_rows() {
    let fix = fixture_bitfield_nested_struct();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [4usize, 11, 17, 21] {
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
fn pg_lower_preserves_gnu_attribute_field_and_typedef_rows() {
    let fix = fixture_gnu_attribute_field_and_typedef();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [2usize, 5, 12, 16, 20, 23] {
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
fn pg_lower_preserves_function_pointer_to_pointer_rows() {
    let fix = fixture_function_pointer_to_pointer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [2usize, 3, 4, 6] {
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
fn pg_lower_preserves_array_of_function_pointers_qualified_rows() {
    let fix = fixture_array_of_function_pointers_qualified();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in [2usize, 3, 4, 7, 13] {
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
// GPU parity contracts  -  full pipeline
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_nested_struct_union_enum() {
    let fix = fixture_nested_struct_union_enum();
    assert_full_pipeline_parity(&fix, "nested_struct_union_enum");
}

#[test]
fn gpu_parity_anonymous_struct_union() {
    let fix = fixture_anonymous_struct_union();
    assert_full_pipeline_parity(&fix, "anonymous_struct_union");
}

#[test]
fn gpu_parity_typedef_multiple_declarators() {
    let fix = fixture_typedef_multiple_declarators();
    assert_full_pipeline_parity(&fix, "typedef_multiple_declarators");
}

#[test]
fn gpu_parity_deeply_nested_pointer() {
    let fix = fixture_deeply_nested_pointer();
    assert_full_pipeline_parity(&fix, "deeply_nested_pointer");
}

#[test]
fn gpu_parity_storage_class_combinations() {
    let fix = fixture_storage_class_combinations();
    assert_full_pipeline_parity(&fix, "storage_class_combinations");
}

#[test]
fn gpu_parity_bitfield_nested_struct() {
    let fix = fixture_bitfield_nested_struct();
    assert_full_pipeline_parity(&fix, "bitfield_nested_struct");
}

#[test]
fn gpu_parity_gnu_attribute_field_and_typedef() {
    let fix = fixture_gnu_attribute_field_and_typedef();
    assert_full_pipeline_parity(&fix, "gnu_attribute_field_and_typedef");
}

#[test]
fn gpu_parity_function_pointer_to_pointer() {
    let fix = fixture_function_pointer_to_pointer();
    assert_full_pipeline_parity(&fix, "function_pointer_to_pointer");
}

#[test]
fn gpu_parity_array_of_function_pointers_qualified() {
    let fix = fixture_array_of_function_pointers_qualified();
    assert_full_pipeline_parity(&fix, "array_of_function_pointers_qualified");
}

// ---------------------------------------------------------------------------
// GPU parity contracts  -  PG lowerer
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_nested_struct_union_enum() {
    let fix = fixture_nested_struct_union_enum();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested struct/union/enum"
    );
}

#[test]
fn gpu_parity_pg_lower_typedef_multiple_declarators() {
    let fix = fixture_typedef_multiple_declarators();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for typedef multiple declarators"
    );
}

#[test]
fn gpu_parity_pg_lower_deeply_nested_pointer() {
    let fix = fixture_deeply_nested_pointer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for deeply nested pointer"
    );
}

#[test]
fn gpu_parity_pg_lower_storage_class_combinations() {
    let fix = fixture_storage_class_combinations();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for storage class combinations"
    );
}

#[test]
fn gpu_parity_pg_lower_bitfield_nested_struct() {
    let fix = fixture_bitfield_nested_struct();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for bitfield nested struct"
    );
}

#[test]
fn gpu_parity_pg_lower_gnu_attribute_field_and_typedef() {
    let fix = fixture_gnu_attribute_field_and_typedef();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for GNU attribute field and typedef"
    );
}

#[test]
fn gpu_parity_pg_lower_function_pointer_to_pointer() {
    let fix = fixture_function_pointer_to_pointer();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for function pointer to pointer"
    );
}

#[test]
fn gpu_parity_pg_lower_array_of_function_pointers_qualified() {
    let fix = fixture_array_of_function_pointers_qualified();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for array of function pointers qualified"
    );
}
