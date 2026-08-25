use super::*;

#[test]
fn scope_tree_depth_8_block_chain_parent_links_correct() {
    let mut atoms = vec![
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
        atoms.push(ident("x"));
        atoms.push(tok(TOK_SEMICOLON));
    }
    for _ in 0..8 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("depth8", &atoms);
    let st = scope_tree_for(&fix);

    // Verify each nested x has a distinct scope id and correct parent chain
    // Token layout after 8 nested opening braces (6-13): int at 14, x at 15, ; at 16, } at 17,
    // then int at 18, x at 19, ; at 20, } at 21, etc.
    let first_x_idx = 8;
    let mut prev_scope = scope_tree_word_at(&st, first_x_idx, 0);
    for level in 1..8 {
        let x_idx = first_x_idx + level * 4;
        let scope = scope_tree_word_at(&st, x_idx, 0);
        let parent = scope_tree_word_at(&st, x_idx, 1);
        assert_ne!(
            scope, prev_scope,
            "scope at level {level} must differ from previous"
        );
        assert_eq!(scope_tree_word_at(&st, x_idx, 2), { DECL_KIND_VARIABLE });
        // In a nested block chain, parent scope should be the previous block's scope
        assert_eq!(
            parent, prev_scope,
            "parent of level {level} should be the immediately enclosing block scope"
        );
        prev_scope = scope;
    }
}

#[test]
fn scope_tree_depth_12_typedef_visible_at_bottom() {
    let mut atoms = vec![
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
    ];
    for _ in 0..12 {
        atoms.push(tok(TOK_LBRACE));
    }
    atoms.push(ident("T"));
    atoms.push(tok(TOK_STAR));
    atoms.push(ident("p"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..12 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("depth12_td", &atoms);
    let st = scope_tree_for(&fix);

    let use_idx = 10 + 12 + 1; // T at bottom
    let use_scope = scope_tree_word_at(&st, use_idx, 0);
    assert_ne!(use_scope, 0, "deep use must be in a nested scope");
    assert_eq!(
        scope_tree_word_at(&st, use_idx, 2),
        { DECL_KIND_NONE },
        "deep use of T must not be a declaration"
    );
}

#[test]
fn scope_tree_depth_16_all_braces_balanced() {
    let mut atoms = vec![
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
    ];
    for _ in 0..16 {
        atoms.push(tok(TOK_LBRACE));
    }
    for _ in 0..16 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("depth16", &atoms);
    let st = scope_tree_for(&fix);

    // Every token should have a valid scope_id (no panics)
    for i in 0..fix.tok_types.len() {
        let _ = scope_tree_word_at(&st, i, 0);
        let _ = scope_tree_word_at(&st, i, 1);
        let _ = scope_tree_word_at(&st, i, 2);
        let _ = scope_tree_word_at(&st, i, 3);
    }
}

// ---------------------------------------------------------------------------
// Deep nesting: typedef annotation
// ---------------------------------------------------------------------------

#[test]
fn annotation_typedef_survives_10_levels_of_nesting() {
    let mut atoms = vec![
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
    ];
    for _ in 0..10 {
        atoms.push(tok(TOK_LBRACE));
    }
    atoms.push(ident("T"));
    atoms.push(tok(TOK_STAR));
    atoms.push(ident("p"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..10 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("td_10level", &atoms);
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    let t_idx = 10 + 10; // index of T use
    assert_ne!(
        flags_at(&ann, t_idx) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef T must survive 10 nesting levels"
    );
    assert_eq!(
        kind_at(&typed, t_idx + 1),
        C_AST_KIND_POINTER_DECL,
        "T * p must be pointer decl deep inside"
    );
}

#[test]
fn annotation_typedef_shadowed_at_level_5_restored_at_level_8() {
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
    for _ in 0..5 {
        atoms.push(tok(TOK_LBRACE));
    }
    atoms.push(tok(TOK_INT));
    atoms.push(ident("T"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..3 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(ident("T"));
    atoms.push(tok(TOK_STAR));
    atoms.push(ident("p"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..2 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("td_shadow5_restore8", &atoms);
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // Just verify the final T is visible
    let final_t_idx = fix.tok_types.len() - 7;
    assert_ne!(
        flags_at(&ann, final_t_idx) & TYPEDEF_FLAG_VISIBLE,
        0,
        "typedef must be restored after shadow block exits"
    );
    assert_eq!(
        kind_at(&typed, final_t_idx + 1),
        C_AST_KIND_POINTER_DECL,
        "restored T * p must be pointer decl"
    );
}

// ---------------------------------------------------------------------------
// Full pipeline CPU/GPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_scope_tree_depth_8() {
    let mut atoms = vec![
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
        atoms.push(ident("x"));
        atoms.push(tok(TOK_SEMICOLON));
    }
    for _ in 0..8 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_depth8", &atoms);
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(gpu, expected, "GPU scope tree must match CPU at depth 8");
}

#[test]
fn gpu_parity_scope_tree_depth_12() {
    let mut atoms = vec![
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
    ];
    for _ in 0..12 {
        atoms.push(tok(TOK_LBRACE));
        atoms.push(tok(TOK_INT));
        atoms.push(ident("x"));
        atoms.push(tok(TOK_SEMICOLON));
    }
    for _ in 0..12 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_depth12", &atoms);
    let expected = scope_tree_for(&fix);
    let gpu = run_gpu_scope_tree(&fix);
    assert_eq!(gpu, expected, "GPU scope tree must match CPU at depth 12");
}

#[test]
fn gpu_parity_annotation_depth_8() {
    let mut atoms = vec![
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
    ];
    for _ in 0..8 {
        atoms.push(tok(TOK_LBRACE));
    }
    atoms.push(ident("T"));
    atoms.push(tok(TOK_STAR));
    atoms.push(ident("p"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..8 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_ann_depth8", &atoms);
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU at depth 8"
    );
}

#[test]
fn gpu_parity_classifier_depth_8() {
    let mut atoms = vec![
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
    ];
    for _ in 0..8 {
        atoms.push(tok(TOK_LBRACE));
    }
    atoms.push(ident("T"));
    atoms.push(tok(TOK_STAR));
    atoms.push(ident("p"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..8 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_cls_depth8", &atoms);
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(gpu_ann, expected_ann);

    let expected_typed = reference_c11_classify_vast_node_kinds(&expected_ann);
    let gpu_typed = run_gpu_classify(&gpu_ann, fix.tok_types.len());
    assert_eq!(
        gpu_typed, expected_typed,
        "GPU classifier must match CPU at depth 8"
    );
}

#[test]
fn gpu_parity_vast_builder_depth_8() {
    let mut atoms = vec![
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
        atoms.push(ident("x"));
        atoms.push(tok(TOK_SEMICOLON));
    }
    for _ in 0..8 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_vast_depth8", &atoms);
    let expected = raw_vast(&fix);
    let gpu = run_gpu_vast_builder(&fix);
    assert_eq!(gpu, expected, "GPU VAST builder must match CPU at depth 8");
}

#[test]
fn gpu_parity_pg_lower_depth_8() {
    let mut atoms = vec![
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
        atoms.push(ident("x"));
        atoms.push(tok(TOK_SEMICOLON));
    }
    for _ in 0..8 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_pg_depth8", &atoms);
    let raw = raw_vast(&fix);
    let expected_pg = pg_lower_cpu(&raw);
    let gpu_pg = run_gpu_pg_lower(&raw, fix.tok_types.len());
    assert_eq!(
        gpu_pg, expected_pg,
        "GPU PG lowerer must match CPU at depth 8"
    );
}

