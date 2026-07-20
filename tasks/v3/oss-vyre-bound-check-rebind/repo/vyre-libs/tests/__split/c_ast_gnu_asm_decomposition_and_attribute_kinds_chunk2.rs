#[test]
fn cpu_reference_classifies_attribute_naked() {
    let fix = fixture_attribute_naked();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_NAKED),
        vec![3],
        "naked attribute name must classify as ATTRIBUTE_NAKED"
    );
}

#[test]
fn cpu_reference_classifies_attribute_visibility() {
    let fix = fixture_attribute_visibility();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_VISIBILITY),
        vec![3],
        "visibility attribute name must classify as ATTRIBUTE_VISIBILITY"
    );
}

// ---------------------------------------------------------------------------
// Tests  -  extended GNU asm decomposition
// ---------------------------------------------------------------------------

#[test]
fn cpu_reference_classifies_asm_multiple_output_operands() {
    let fix = fixture_asm_multiple_outputs();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm keyword must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "asm template string must classify as ASM_TEMPLATE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![6, 11],
        "each output operand paren must classify as ASM_OUTPUT_OPERAND"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![16],
        "input operand paren must classify as ASM_INPUT_OPERAND"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST),
        vec![20, 22],
        "each clobber string must classify as ASM_CLOBBERS_LIST"
    );
}

#[test]
fn cpu_reference_classifies_asm_multiple_input_operands() {
    let fix = fixture_asm_multiple_inputs();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![2],
        "asm template must classify as ASM_TEMPLATE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![5],
        "single output operand paren must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![10, 15],
        "each input operand paren must classify as ASM_INPUT_OPERAND"
    );
}

#[test]
fn cpu_reference_classifies_asm_goto_multiple_labels() {
    let fix = fixture_asm_goto_multiple_labels();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![3],
        "asm goto template must classify as ASM_TEMPLATE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS),
        vec![8, 10],
        "each goto label identifier must classify as ASM_GOTO_LABELS"
    );
    assert_ne!(
        word_at(&typed, VAST_STRIDE_U32),
        C_AST_KIND_GOTO_STMT,
        "goto after asm is a qualifier, not a standalone goto statement"
    );
}

#[test]
fn cpu_reference_classifies_basic_asm() {
    let fix = fixture_basic_asm();
    let typed = classify(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "asm keyword must classify as INLINE_ASM"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE),
        vec![2],
        "basic asm template string must classify as ASM_TEMPLATE"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND).is_empty(),
        "basic asm has no output operands"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST).is_empty(),
        "basic asm has no clobbers"
    );
}

// ---------------------------------------------------------------------------
// Negative tests  -  mis-classification guards
// ---------------------------------------------------------------------------

#[test]
fn cpu_reference_negative_non_attribute_identifier() {
    let fix = fixture_non_attribute_identifier();
    let typed = classify(&fix);

    let attr_kinds = [
        C_AST_KIND_ATTRIBUTE_SECTION,
        C_AST_KIND_ATTRIBUTE_WEAK,
        C_AST_KIND_ATTRIBUTE_ALIAS,
        C_AST_KIND_ATTRIBUTE_ALIGNED,
        C_AST_KIND_ATTRIBUTE_USED,
        C_AST_KIND_ATTRIBUTE_UNUSED,
        C_AST_KIND_ATTRIBUTE_NAKED,
        C_AST_KIND_ATTRIBUTE_VISIBILITY,
    ];

    for kind in attr_kinds {
        assert!(
            row_indices(&typed, kind).is_empty(),
            "identifier 'section' in non-attribute call context must not classify as {kind:#010x}"
        );
    }

    // The identifier should remain an ordinary variable / identifier node.
    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "function argument identifier must classify as VARIABLE"
    );
}
