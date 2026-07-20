// ---------------------------------------------------------------------------
// Tests – nested declarators
// ---------------------------------------------------------------------------

#[test]
fn nested_declarator_parity_and_shape() {
    let fix = fixture_nested_declarator();
    assert_full_pipeline_parity(&fix, "nested_declarator");

    let typed = classify(&fix);
    assert!(
        !typed.is_empty(),
        "fixture must produce non-empty typed VAST"
    );

    // Two POINTER_DECL rows for the two '*' operators
    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert_eq!(
        ptrs.len(),
        2,
        "int (*(*p)[3])(int) must contain exactly 2 POINTER_DECL rows"
    );

    // Function declarator row for the parameter list
    let funcs = row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR);
    assert!(
        !funcs.is_empty(),
        "nested declarator must contain FUNCTION_DECLARATOR"
    );
}

// ---------------------------------------------------------------------------
// Tests – asm / attribute interactions
// ---------------------------------------------------------------------------

#[test]
fn asm_attribute_interaction_parity_and_shape() {
    let fix = fixture_asm_attribute_interaction();
    assert_full_pipeline_parity(&fix, "asm_attribute_interaction");

    let typed = classify(&fix);
    assert_ne!(typed.len(), 0,
        "fixture must produce non-empty typed VAST"
    );

    // __attribute__ node
    let attrs = row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE);
    assert_ne!(attrs.len(), 0,
        "attribute on declaration must produce GNU_ATTRIBUTE"
    );

    // used attribute kind
    let used = row_indices(&typed, C_AST_KIND_ATTRIBUTE_USED);
    assert_ne!(used.len(), 0,
        "__attribute__((used)) must produce ATTRIBUTE_USED"
    );

    // Inline asm statement inside function body
    let asms = row_indices(&typed, C_AST_KIND_INLINE_ASM);
    assert_ne!(asms.len(), 0, "asm statement must produce INLINE_ASM");
}

// ---------------------------------------------------------------------------
// Tests – control flow (labels, goto, switch, for, while, do)
// ---------------------------------------------------------------------------

#[test]
fn control_flow_all_parity_and_shape() {
    let fix = fixture_control_flow_all();
    assert_full_pipeline_parity(&fix, "control_flow_all");

    let typed = classify(&fix);
    assert_ne!(typed.len(), 0,
        "fixture must produce non-empty typed VAST"
    );

    assert!(
        !row_indices(&typed, C_AST_KIND_FOR_STMT).is_empty(),
        "for loop must produce FOR_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_WHILE_STMT).is_empty(),
        "while loop must produce WHILE_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_DO_STMT).is_empty(),
        "do-while loop must produce DO_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_SWITCH_STMT).is_empty(),
        "switch must produce SWITCH_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_CASE_STMT).is_empty(),
        "case must produce CASE_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_DEFAULT_STMT).is_empty(),
        "default must produce DEFAULT_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_GOTO_STMT).is_empty(),
        "goto must produce GOTO_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_BREAK_STMT).is_empty(),
        "break must produce BREAK_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_CONTINUE_STMT).is_empty(),
        "continue must produce CONTINUE_STMT"
    );
    assert!(
        !row_indices(&typed, C_AST_KIND_RETURN_STMT).is_empty(),
        "return must produce RETURN_STMT"
    );
}

// ---------------------------------------------------------------------------
// Tests – typedef shadowing
// ---------------------------------------------------------------------------

#[test]
fn typedef_shadowing_parity_and_shape() {
    let fix = fixture_typedef_shadowing();
    assert_full_pipeline_parity(&fix, "typedef_shadowing");

    let typed = classify(&fix);
    assert!(
        !typed.is_empty(),
        "fixture must produce non-empty typed VAST"
    );

    // After typedef annotation, T inside f() block should be treated as an
    // ordinary identifier (variable), while T inside g() should remain a typedef.
    // We verify the fixture parses and both function bodies survive.
    let funcs = row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION);
    assert!(
        funcs.len() >= 2,
        "typedef shadowing fixture must contain at least 2 FUNCTION_DEFINITION rows, got {:?}",
        funcs
    );
}

// ---------------------------------------------------------------------------
// Tests – statement expressions
// ---------------------------------------------------------------------------

#[test]
fn statement_expression_parity_and_shape() {
    let fix = fixture_statement_expression();
    assert_full_pipeline_parity(&fix, "statement_expression");

    let typed = classify(&fix);
    assert_ne!(typed.len(), 0,
        "fixture must produce non-empty typed VAST"
    );

    // The brace-enclosed body of the statement expression should appear as a
    // BASIC_BLOCK (or similar container) in the typed VAST.
    let bbs = row_indices(&typed, node_kind::BASIC_BLOCK);
    assert_ne!(bbs.len(), 0,
        "statement expression must contain BASIC_BLOCK"
    );

    // Assignment inside the statement expression
    let assigns = row_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert_ne!(assigns.len(), 0,
        "statement expression must contain at least one ASSIGN_EXPR"
    );
}
