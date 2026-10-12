// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_macro_shaped_declaration_list_head_classifies() {
    let fix = fixture_macro_shaped_declaration_list_head();
    let typed = classify_fixture(&fix);

    assert_eq!(
        row_indices(&typed, node_kind::CALL),
        vec![0],
        "LIST_HEAD must classify as CALL"
    );
}

#[test]
fn gpu_parity_macro_shaped_declaration_define_per_cpu() {
    let fix = fixture_macro_shaped_declaration_define_per_cpu();
    assert_full_pipeline_parity(&fix, "macro_shaped_declaration_define_per_cpu");
}

#[test]
fn gpu_parity_macro_shaped_declaration_list_head() {
    let fix = fixture_macro_shaped_declaration_list_head();
    assert_full_pipeline_parity(&fix, "macro_shaped_declaration_list_head");
}

// ---------------------------------------------------------------------------
// 14. Nested conditional preprocessing
// ---------------------------------------------------------------------------

fn fixture_nested_conditional_preproc() -> Fixture {
    assemble_fixture(&[
        ("#if 1", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("a", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("#ifdef MISSING", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("b", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("#else", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("c", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("#endif", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#elif 0", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("d", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("#else", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("e", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("#endif", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("f", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ])
}

