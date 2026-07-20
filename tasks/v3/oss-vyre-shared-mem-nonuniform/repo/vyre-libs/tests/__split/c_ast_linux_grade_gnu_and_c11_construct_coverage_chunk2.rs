#[test]
fn cpu_reference_attribute_constructor_parses() {
    let fix = fixture_attribute_constructor();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "constructor attribute fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_CONSTRUCTOR),
        vec![3],
        "constructor must classify as a specific GNU attribute kind"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION).contains(&7),
        "init declarator with a body must classify as FUNCTION_DEFINITION"
    );
}

#[test]
fn cpu_reference_attribute_destructor_parses() {
    let fix = fixture_attribute_destructor();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "destructor attribute fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_DESTRUCTOR),
        vec![3],
        "destructor must classify as a specific GNU attribute kind"
    );
}

#[test]
fn cpu_reference_attribute_mode_parses() {
    let fix = fixture_attribute_mode();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "mode attribute fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_MODE),
        vec![3],
        "mode must classify as a specific GNU attribute kind"
    );
}

#[test]
fn cpu_reference_attribute_packed_and_aligned() {
    let fix = fixture_attribute_packed_and_aligned();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "packed/aligned attribute fixture must produce a non-empty typed VAST"
    );

    let attrs = row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE);
    assert!(
        attrs.contains(&1),
        "packed __attribute__ must classify as GNU_ATTRIBUTE"
    );
    assert!(
        attrs.contains(&14),
        "aligned __attribute__ must classify as GNU_ATTRIBUTE"
    );

    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_PACKED),
        vec![4],
        "packed must classify as a specific GNU attribute kind"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED),
        vec![17],
        "aligned must classify as a specific GNU attribute kind"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&24),
        "variable l after aligned attribute must classify as VARIABLE"
    );
}

#[test]
fn cpu_reference_computed_goto_parses() {
    let fix = fixture_computed_goto();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "computed goto fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![8],
        "goto must classify as GOTO_STMT"
    );
}

#[test]
fn cpu_reference_label_and_computed_goto_interaction() {
    let fix = fixture_label_and_computed_goto_interaction();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "label/computed-goto interaction fixture must produce a non-empty typed VAST"
    );

    // &&lbl must classify as a label-address expression
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR),
        vec![10],
        "&&lbl must classify as GNU_LABEL_ADDRESS_EXPR"
    );

    // goto *t must keep goto as a jump statement
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT),
        vec![13],
        "goto must classify as GOTO_STMT"
    );

    assert_eq!(
        kind_at(&typed, 17),
        C_AST_KIND_LABEL_STMT,
        "label definition lbl must classify as LABEL_STMT"
    );
}

#[test]
fn cpu_reference_stmt_expr_initializer() {
    let fix = fixture_stmt_expr_initializer();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "statement-expression initializer must produce a non-empty typed VAST"
    );

    // The statement-expression body should be a basic block
    assert_eq!(
        kind_at(&typed, 4),
        node_kind::BASIC_BLOCK,
        "statement-expression brace must classify as BASIC_BLOCK"
    );

    // Both the outer initializer '=' and the inner assignment classify.
    let assigns = row_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert!(
        assigns.contains(&7),
        "assignment inside statement expression must classify as ASSIGN_EXPR"
    );

    // The outer variable a must survive
    assert_eq!(
        kind_at(&typed, 1),
        node_kind::VARIABLE,
        "a must classify as VARIABLE"
    );
}

#[test]
fn cpu_reference_stmt_expr_in_declarator() {
    let fix = fixture_stmt_expr_in_declarator();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "statement-expression in array declarator must produce a non-empty typed VAST"
    );

    // The array brackets should survive as an array declarator or at least
    // the overall fixture should not panic.
    assert_eq!(
        kind_at(&typed, 4),
        node_kind::BASIC_BLOCK,
        "statement-expression inside array size must contain a BASIC_BLOCK"
    );
}

#[test]
fn cpu_reference_static_assert_parses() {
    let fix = fixture_static_assert();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "_Static_assert fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        fix.tok_types[0], TOK_STATIC_ASSERT,
        "_Static_assert must promote to TOK_STATIC_ASSERT"
    );
}

#[test]
fn cpu_reference_alignas_var_parses() {
    let fix = fixture_alignas_var();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "_Alignas fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        fix.tok_types[0], TOK_ALIGNAS,
        "_Alignas must promote to TOK_ALIGNAS"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&5),
        "buf declarator after _Alignas must classify as VARIABLE"
    );
}

#[test]
fn cpu_reference_alignof_expr_parses() {
    let fix = fixture_alignof_expr();
    let typed = classify(&fix);

    assert!(
        !typed.is_empty(),
        "_Alignof fixture must produce a non-empty typed VAST"
    );
    assert_eq!(
        fix.tok_types[3], TOK_ALIGNOF,
        "_Alignof must promote to TOK_ALIGNOF"
    );

    // The public contract here is promotion plus surrounding
    // declaration parsing.
    assert_eq!(
        kind_at(&typed, 1),
        node_kind::VARIABLE,
        "a must classify as VARIABLE"
    );
}
