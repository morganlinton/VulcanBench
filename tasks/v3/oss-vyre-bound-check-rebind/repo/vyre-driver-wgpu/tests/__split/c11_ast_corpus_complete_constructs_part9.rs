use super::*;

#[test]
fn gpu_parity_pg_lower_function_pointer_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_pointer_array();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for function pointer array"
    );
}

#[test]
fn gpu_parity_pg_lower_nested_designated_init() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_designated_init();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for nested designated init"
    );
}

#[test]
fn gpu_parity_pg_lower_attribute_and_asm() {
    let (tok_types, tok_starts, tok_lens) = fixture_attribute_and_asm();
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed, tok_types.len() as u32);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for attribute and asm"
    );
}

