use super::*;

#[test]
fn annotation_multiple_typedefs_same_name_in_disjoint_blocks() {
    let fix = fixture(
        "ann_disjoint",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_VOID),
            ident("g"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_TYPEDEF),
            tok(TOK_CHAR_KW),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);

    assert_ne!(
        flags_at(&ann, 8) & TYPEDEF_FLAG_DECL,
        0,
        "first block typedef T must be decl"
    );
    assert_ne!(
        flags_at(&ann, 19) & TYPEDEF_FLAG_DECL,
        0,
        "second block typedef T must be decl"
    );
}

#[test]
fn annotation_parameter_typedef_shadow_restored_after_function() {
    let fix = fixture(
        "ann_param_restore",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_VOID),
            ident("g"),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // Inside f: parameter T shadows typedef, so T * p is multiplication
    assert_ne!(
        flags_at(&ann, 8) & ORDINARY_FLAG_DECL,
        0,
        "parameter T must be ordinary decl"
    );
    assert_eq!(
        flags_at(&ann, 11) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be shadowed in f body"
    );
    assert_eq!(
        kind_at(&typed, 12),
        node_kind::BINARY,
        "T * p in f body must be multiply"
    );

    // Inside g parameter list: typedef T restored
    assert_ne!(
        flags_at(&ann, 19) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be visible in g params"
    );
    assert_eq!(
        kind_at(&typed, 20),
        C_AST_KIND_POINTER_DECL,
        "T * p in g must be pointer decl"
    );
}

#[test]
fn annotation_kr_parameter_shadows_typedef() {
    let fix = fixture(
        "ann_kr",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_LBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // K&R parameter declaration at token 10
    assert_ne!(
        flags_at(&ann, 10) & ORDINARY_FLAG_DECL,
        0,
        "K&R parameter T must be ordinary decl"
    );
    // Use in body at token 13
    assert_eq!(
        flags_at(&ann, 13) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef must be shadowed by K&R param"
    );
    assert_eq!(
        kind_at(&typed, 14),
        node_kind::BINARY,
        "K&R shadowed T * p must be multiply"
    );
}

#[test]
fn annotation_typedef_shadowed_by_for_loop_variable() {
    let fix = fixture(
        "ann_for",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_FOR),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_SEMICOLON),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // for-loop variable T at token 13
    assert_ne!(
        flags_at(&ann, 13) & ORDINARY_FLAG_DECL,
        0,
        "for-loop var T must be ordinary decl"
    );
    // After for loop, typedef restored
    assert_ne!(
        flags_at(&ann, 19) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be restored after for"
    );
    assert_eq!(
        kind_at(&typed, 20),
        C_AST_KIND_POINTER_DECL,
        "restored T * p must be pointer decl"
    );
}

#[test]
fn annotation_typedef_shadow_chain_three_levels() {
    let fix = fixture(
        "ann_chain3",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // Each inner block ordinary decl
    for idx in [12, 17, 22] {
        assert_ne!(
            flags_at(&ann, idx) & ORDINARY_FLAG_DECL,
            0,
            "block var T at {idx} must be ordinary decl"
        );
    }
    // After all blocks, typedef restored
    assert_ne!(
        flags_at(&ann, 25) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must be restored after chain"
    );
    assert_eq!(
        kind_at(&typed, 26),
        C_AST_KIND_POINTER_DECL,
        "restored T * p must be pointer decl"
    );
}

// ---------------------------------------------------------------------------
// GPU parity tests
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_scope_tree_typedef_shadow_restore() {
    let fix = fixture(
        "gpu_scope_shadow",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for typedef shadow/restore"
    );
}

#[test]
fn gpu_parity_annotation_typedef_shadow_restore() {
    let fix = fixture(
        "gpu_ann_shadow",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU for typedef shadow/restore"
    );

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_ann);
    let gpu_typed = run_gpu_classify(&gpu_ann, fix.tok_types.len());
    assert_eq!(gpu_typed, expected_typed, "GPU classifier must match CPU");
}

#[test]
fn gpu_parity_annotation_parameter_shadows_typedef() {
    let fix = fixture(
        "gpu_param_shadow",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
            tok(TOK_VOID),
            ident("g"),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU for param shadow"
    );
}

#[test]
fn gpu_parity_scope_tree_deep_shadow_chain() {
    let mut atoms = vec![
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
    ];
    for _ in 0..8 {
        atoms.push(tok(TOK_LBRACE));
        atoms.push(tok(TOK_INT));
        atoms.push(ident("T"));
        atoms.push(tok(TOK_SEMICOLON));
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(ident("T"));
    atoms.push(tok(TOK_STAR));
    atoms.push(ident("p"));
    atoms.push(tok(TOK_SEMICOLON));
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_deep_shadow", &atoms);
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(
        gpu, expected,
        "GPU scope tree must match CPU for deep shadow chain"
    );
}

