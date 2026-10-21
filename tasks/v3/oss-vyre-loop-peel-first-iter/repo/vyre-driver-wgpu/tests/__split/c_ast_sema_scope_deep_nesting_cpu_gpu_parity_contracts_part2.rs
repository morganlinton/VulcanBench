use super::*;

#[test]
fn gpu_parity_full_pipeline_typedef_shadow_restore_deep() {
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
    for _ in 0..6 {
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
    for _ in 0..3 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_full_deep", &atoms);

    // VAST builder parity
    let cpu_vast = raw_vast(&fix);
    let gpu_vast = run_gpu_vast_builder(&fix);
    assert_eq!(gpu_vast, cpu_vast, "VAST builder parity");

    // Annotation parity
    let cpu_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(gpu_ann, cpu_ann, "Annotation parity");

    // Classifier parity
    let cpu_typed = reference_c11_classify_vast_node_kinds(&cpu_ann);
    let gpu_typed = run_gpu_classify(&gpu_ann, fix.tok_types.len());
    assert_eq!(gpu_typed, cpu_typed, "Classifier parity");

    // PG lowerer parity
    let cpu_pg = pg_lower_cpu(&cpu_typed);
    let gpu_pg = run_gpu_pg_lower(&gpu_typed, fix.tok_types.len());
    assert_eq!(gpu_pg, cpu_pg, "PG lowerer parity");

    // Scope tree parity
    let cpu_scope = scope_tree_for(&fix);
    let gpu_scope = run_gpu_scope_tree(&fix);
    assert_eq!(gpu_scope, cpu_scope, "Scope tree parity");
}

#[test]
fn gpu_parity_full_pipeline_kr_deep_nesting() {
    let mut atoms = vec![
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        ident("T"),
        tok(TOK_RPAREN),
    ];
    for _ in 0..4 {
        atoms.push(tok(TOK_LBRACE));
    }
    atoms.push(tok(TOK_INT));
    atoms.push(ident("T"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..4 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_LBRACE));
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_kr_deep", &atoms);

    let cpu_scope = scope_tree_for(&fix);
    let gpu_scope = run_gpu_scope_tree(&fix);
    assert_eq!(gpu_scope, cpu_scope, "Scope tree parity for K&R deep");

    let cpu_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(gpu_ann, cpu_ann, "Annotation parity for K&R deep");
}

#[test]
fn gpu_parity_full_pipeline_enum_tag_deep() {
    let mut atoms = vec![
        tok(TOK_ENUM),
        ident("E"),
        tok(TOK_LBRACE),
        ident("A"),
        tok(TOK_RBRACE),
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
    atoms.push(tok(TOK_RETURN));
    atoms.push(ident("A"));
    atoms.push(tok(TOK_SEMICOLON));
    for _ in 0..5 {
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_enum_deep", &atoms);

    let cpu_scope = scope_tree_for(&fix);
    let gpu_scope = run_gpu_scope_tree(&fix);
    assert_eq!(gpu_scope, cpu_scope, "Scope tree parity for enum deep");

    let cpu_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(gpu_ann, cpu_ann, "Annotation parity for enum deep");
}
