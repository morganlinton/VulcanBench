use super::*;

#[test]
fn gpu_parity_classifier_abstract_declarator_cast() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_cast();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for abstract declarator cast"
    );
}

#[test]
fn gpu_parity_classifier_abstract_declarator_sizeof() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_sizeof();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for abstract declarator sizeof"
    );
}

#[test]
fn gpu_parity_classifier_kr_function() {
    let (tok_types, tok_starts, tok_lens) = fixture_kr_function();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for K&R function declaration"
    );
}

#[test]
fn gpu_parity_classifier_deeply_parenthesised_pointer() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_parenthesised_pointer();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for deeply parenthesised pointer"
    );
}

#[test]
fn gpu_parity_classifier_qualified_pointer_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_qualified_pointer_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let expected = reference_c11_classify_vast_node_kinds(&raw);
    let gpu = run_gpu_classifier(&raw, node_count_from_vast(&raw));
    assert_eq!(
        gpu, expected,
        "GPU classifier must match CPU for qualified pointer array"
    );
}

// ---------------------------------------------------------------------------
// GPU parity tests  -  PG lowerer
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_array_of_function_pointers() {
    let (tok_types, tok_starts, tok_lens) = fixture_array_of_function_pointers();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for array of function pointers"
    );
}

#[test]
fn gpu_parity_pg_lower_function_returning_fnptr() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_returning_fnptr();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for function returning fnptr"
    );
}

#[test]
fn gpu_parity_pg_lower_nested_qualifiers() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_qualifiers();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested qualifiers"
    );
}

#[test]
fn gpu_parity_pg_lower_parameter_typedef_shadowing() {
    let fix = fixture_parameter_typedef_shadowing();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, &fix.haystack);
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for parameter typedef shadowing"
    );
}

#[test]
fn gpu_parity_pg_lower_abstract_declarator_cast() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_cast();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for abstract declarator cast"
    );
}

#[test]
fn gpu_parity_pg_lower_abstract_declarator_sizeof() {
    let (tok_types, tok_starts, tok_lens) = fixture_abstract_declarator_sizeof();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for abstract declarator sizeof"
    );
}

#[test]
fn gpu_parity_pg_lower_kr_function() {
    let (tok_types, tok_starts, tok_lens) = fixture_kr_function();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for K&R function declaration"
    );
}

#[test]
fn gpu_parity_pg_lower_deeply_parenthesised_pointer() {
    let (tok_types, tok_starts, tok_lens) = fixture_deeply_parenthesised_pointer();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for deeply parenthesised pointer"
    );
}

#[test]
fn gpu_parity_pg_lower_qualified_pointer_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_qualified_pointer_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, node_count_from_vast(&typed));
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for qualified pointer array"
    );
}
