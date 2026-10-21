#[test]
fn pg_lower_preserves_asm_symbolic_names_and_earlyclobber() {
    let fix = fixture_asm_symbolic_names_and_earlyclobber();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_INLINE_ASM);
    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_ASM_OUTPUT_OPERAND);
    assert_pg_preserves_row(&typed, &pg, &fix, 12, C_AST_KIND_ASM_INPUT_OPERAND);
}

#[test]
fn pg_lower_preserves_asm_extended_output_only() {
    let fix = fixture_asm_extended_output_only();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_INLINE_ASM);
    assert_pg_preserves_row(&typed, &pg, &fix, 6, C_AST_KIND_ASM_OUTPUT_OPERAND);
}

#[test]
fn pg_lower_preserves_asm_goto_three_labels() {
    let fix = fixture_asm_goto_three_labels();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let pg = reference_ast_to_pg_nodes(&typed);

    assert_pg_preserves_row(&typed, &pg, &fix, 0, C_AST_KIND_INLINE_ASM);
    for idx in row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ASM_GOTO_LABELS);
    }
}

// ---------------------------------------------------------------------------
// GPU/CPU parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_asm_multiple_output_input_operands() {
    let fix = fixture_asm_multiple_output_input_operands();
    assert_full_pipeline_parity(&fix, "asm_multiple_output_input_operands");
}

#[test]
fn gpu_parity_asm_memory_and_cc_clobbers() {
    let fix = fixture_asm_memory_and_cc_clobbers();
    assert_full_pipeline_parity(&fix, "asm_memory_and_cc_clobbers");
}

#[test]
fn gpu_parity_asm_goto_multiple_labels() {
    let fix = fixture_asm_goto_multiple_labels();
    assert_full_pipeline_parity(&fix, "asm_goto_multiple_labels");
}

#[test]
fn gpu_parity_asm_symbolic_names_and_earlyclobber() {
    let fix = fixture_asm_symbolic_names_and_earlyclobber();
    assert_full_pipeline_parity(&fix, "asm_symbolic_names_and_earlyclobber");
}

#[test]
fn gpu_parity_asm_extended_output_only() {
    let fix = fixture_asm_extended_output_only();
    assert_full_pipeline_parity(&fix, "asm_extended_output_only");
}

#[test]
fn gpu_parity_asm_goto_three_labels() {
    let fix = fixture_asm_goto_three_labels();
    assert_full_pipeline_parity(&fix, "asm_goto_three_labels");
}

// ---------------------------------------------------------------------------
// GPU PG lowering parity contracts
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_pg_lower_asm_multiple_output_input_operands() {
    let fix = fixture_asm_multiple_output_input_operands();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for asm_multiple_output_input_operands"
    );
}

#[test]
fn gpu_parity_pg_lower_asm_memory_and_cc_clobbers() {
    let fix = fixture_asm_memory_and_cc_clobbers();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for asm_memory_and_cc_clobbers"
    );
}

#[test]
fn gpu_parity_pg_lower_asm_goto_multiple_labels() {
    let fix = fixture_asm_goto_multiple_labels();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for asm_goto_multiple_labels"
    );
}

#[test]
fn gpu_parity_pg_lower_asm_symbolic_names_and_earlyclobber() {
    let fix = fixture_asm_symbolic_names_and_earlyclobber();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for asm_symbolic_names_and_earlyclobber"
    );
}

#[test]
fn gpu_parity_pg_lower_asm_extended_output_only() {
    let fix = fixture_asm_extended_output_only();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for asm_extended_output_only"
    );
}

#[test]
fn gpu_parity_pg_lower_asm_goto_three_labels() {
    let fix = fixture_asm_goto_three_labels();
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let expected = reference_ast_to_pg_nodes(&typed);
    let gpu = run_gpu_pg_lower(&typed);
    assert_eq!(
        gpu, expected,
        "GPU PG lowerer must match CPU for asm_goto_three_labels"
    );
}
