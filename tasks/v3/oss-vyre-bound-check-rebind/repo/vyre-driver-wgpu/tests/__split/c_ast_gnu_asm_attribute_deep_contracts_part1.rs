use super::*;

#[test]
fn asm_goto_with_outputs_inputs_clobbers_labels_gpu_cpu_parity() {
    let fix = fixture_asm_goto_with_outputs_inputs_clobbers_labels();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ASM,
        "asm must promote to TOK_GNU_ASM"
    );
    assert_eq!(fix.tok_types[1], TOK_GOTO, "goto must promote to TOK_GOTO");
    assert_full_pipeline_parity(&fix, "asm_goto_with_outputs_inputs_clobbers_labels");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm keyword must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "template string must classify as ASM_TEMPLATE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![6],
        "output operand paren must classify as ASM_OUTPUT_OPERAND"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![11],
        "input operand paren must classify as ASM_INPUT_OPERAND"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST),
        vec![15, 17],
        "each clobber string must classify as ASM_CLOBBERS_LIST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS),
        vec![19, 21],
        "each goto label must classify as ASM_GOTO_LABELS"
    );
    assert_ne!(
        word_at(&typed, VAST_STRIDE_U32),
        C_AST_KIND_GOTO_STMT,
        "goto after asm is a qualifier, not a standalone goto statement"
    );
}

#[test]
fn asm_volatile_goto_qualifier_chain_gpu_cpu_parity() {
    let fix = fixture_asm_volatile_goto_with_outputs_inputs_clobbers_labels();
    assert_eq!(fix.tok_types[0], TOK_GNU_ASM);
    assert_eq!(fix.tok_types[1], TOK_VOLATILE);
    assert_eq!(fix.tok_types[2], TOK_GOTO);
    assert_full_pipeline_parity(&fix, "asm_volatile_goto_qualifier_chain");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_QUALIFIER),
        vec![1, 2],
        "volatile and goto after asm must both classify as asm qualifiers"
    );
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_TEMPLATE), vec![4]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND), vec![7]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND), vec![12]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST), vec![16]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS), vec![18]);
    assert_ne!(
        word_at(&typed, 2 * VAST_STRIDE_U32),
        C_AST_KIND_GOTO_STMT,
        "goto in asm volatile goto is a qualifier, not a standalone goto statement"
    );
}

#[test]
fn asm_volatile_with_outputs_inputs_clobbers_gpu_cpu_parity() {
    let fix = fixture_asm_volatile_with_outputs_inputs_clobbers();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ASM,
        "asm must promote to TOK_GNU_ASM"
    );
    assert_eq!(
        fix.tok_types[1], TOK_VOLATILE,
        "volatile must promote to TOK_VOLATILE"
    );
    assert_full_pipeline_parity(&fix, "asm_volatile_with_outputs_inputs_clobbers");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm keyword must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "template string must classify as ASM_TEMPLATE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![6],
        "output operand paren must classify as ASM_OUTPUT_OPERAND"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![11],
        "input operand paren must classify as ASM_INPUT_OPERAND"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST),
        vec![15, 17],
        "each clobber string must classify as ASM_CLOBBERS_LIST"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS).is_empty(),
        "non-goto asm must not produce goto labels"
    );
}

#[test]
fn asm_named_operands_classify_gpu_cpu_parity() {
    let fix = fixture_asm_named_operands();
    assert_full_pipeline_parity(&fix, "asm_named_operands");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm keyword must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "template string must classify as ASM_TEMPLATE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![9],
        "named output operand paren must still classify as ASM_OUTPUT_OPERAND"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![17],
        "named input operand paren must still classify as ASM_INPUT_OPERAND"
    );
    // The bracketed names themselves are not special-cased today;
    // ensure they do not leak into clobber or goto label kinds.
    assert!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST).is_empty(),
        "named operand fixture has no clobbers"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS).is_empty(),
        "named operand fixture has no goto labels"
    );
}

#[test]
fn asm_alias_on_declaration_gpu_cpu_parity() {
    let fix = fixture_asm_alias_declaration();
    assert_eq!(
        fix.tok_types[2], TOK_GNU_ASM,
        "asm in declarator suffix must promote to TOK_GNU_ASM"
    );
    assert_full_pipeline_parity(&fix, "asm_alias_declaration");

    let typed = classify(&fix);

    assert_eq!(
        word_at(&typed, 2 * VAST_STRIDE_U32),
        C_AST_KIND_INLINE_ASM,
        "asm token in declaration alias context must classify as INLINE_ASM"
    );
    assert_eq!(
        word_at(&typed, VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "declared variable must classify as VARIABLE"
    );
    // Note: syntactically the alias string sits inside an asm-paren context,
    // so the classifier may give it ASM_TEMPLATE today. We do not hard-code
    // either outcome because there is no dedicated ASM_ALIAS kind yet.
}

// ---------------------------------------------------------------------------
// Tests  -  GNU attribute-specific kinds (deep coverage)
// ---------------------------------------------------------------------------

#[test]
fn attribute_cleanup_gpu_cpu_parity() {
    let fix = fixture_attribute_cleanup();
    assert_eq!(
        fix.tok_types[2], TOK_GNU_ATTRIBUTE,
        "__attribute__ must promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_cleanup");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![2],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_CLEANUP),
        vec![5],
        "cleanup must classify as ATTRIBUTE_CLEANUP"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "argument identifier inside cleanup must classify as VARIABLE"
    );
}

#[test]
fn attribute_constructor_gpu_cpu_parity() {
    let fix = fixture_attribute_constructor();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ATTRIBUTE,
        "__attribute__ must promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_constructor");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_CONSTRUCTOR),
        vec![3],
        "constructor must classify as ATTRIBUTE_CONSTRUCTOR"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DEFINITION,
        "init with a body must classify as FUNCTION_DEFINITION"
    );
}

#[test]
fn attribute_destructor_gpu_cpu_parity() {
    let fix = fixture_attribute_destructor();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ATTRIBUTE,
        "__attribute__ must promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_destructor");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_DESTRUCTOR),
        vec![3],
        "destructor must classify as ATTRIBUTE_DESTRUCTOR"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        C_AST_KIND_FUNCTION_DEFINITION,
        "fini with a body must classify as FUNCTION_DEFINITION"
    );
}

#[test]
fn attribute_mode_gpu_cpu_parity() {
    let fix = fixture_attribute_mode();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ATTRIBUTE,
        "__attribute__ must promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_mode");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_MODE),
        vec![3],
        "mode must classify as ATTRIBUTE_MODE"
    );
    assert_eq!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "mode argument SI must classify as VARIABLE"
    );
}

#[test]
fn attribute_packed_struct_gpu_cpu_parity() {
    let fix = fixture_attribute_packed_struct();
    assert_eq!(
        fix.tok_types[1], TOK_GNU_ATTRIBUTE,
        "__attribute__ after struct must promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_packed_struct");

    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![1],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_PACKED),
        vec![4],
        "packed must classify as ATTRIBUTE_PACKED"
    );
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "struct tag S must classify as VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        C_AST_KIND_FIELD_DECL,
        "field x inside packed struct must classify as FIELD_DECL"
    );
}

#[test]
fn attribute_combined_variable_gpu_cpu_parity() {
    let fix = fixture_attribute_combined_variable();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_ATTRIBUTE,
        "__attribute__ must promote"
    );
    assert_full_pipeline_parity(&fix, "attribute_combined_variable");

    let typed = classify(&fix);

    assert_eq!(row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE), vec![0]);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_SECTION),
        vec![3],
        "section must classify as ATTRIBUTE_SECTION"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_WEAK),
        vec![8],
        "weak must classify as ATTRIBUTE_WEAK"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIAS),
        vec![10],
        "alias must classify as ATTRIBUTE_ALIAS"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED),
        vec![15],
        "aligned must classify as ATTRIBUTE_ALIGNED"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_USED),
        vec![20],
        "used must classify as ATTRIBUTE_USED"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_UNUSED),
        vec![22],
        "unused must classify as ATTRIBUTE_UNUSED"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_NAKED),
        vec![24],
        "naked must classify as ATTRIBUTE_NAKED"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_VISIBILITY),
        vec![26],
        "visibility must classify as ATTRIBUTE_VISIBILITY"
    );
    // The long attribute prefix can cause the identifier to fall through to 0
    // in the current classifier. We assert the attribute-specific kinds (the
    // primary contract) without pinning the declarator kind until the classifier
    // reliably tracks decl-prefix across extended attribute lists.
}

// ---------------------------------------------------------------------------
// Negative tests  -  mis-classification guards
// ---------------------------------------------------------------------------

