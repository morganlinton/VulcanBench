use super::*;

#[test]
fn gpu_backend_acquisition_does_not_return_err() {
    // Per project rules, GPU absence is a configuration bug and must be
    // reported loudly (panic), not silently skipped. This test validates
    // that acquisition succeeds on a properly configured machine.
    let backend = WgpuBackend::acquire().expect(
        "WgpuBackend::acquire failed on a machine that must have a GPU. \
         This is a configuration bug, not a graceful skip.",
    );
    // Verify the backend is actually usable by reading adapter info.
    let info = backend.adapter_info();
    assert_ne!(info.name.len(), 0,
        "acquired backend must report a non-empty adapter name"
    );
}

#[test]
fn gpu_backend_new_does_not_return_err() {
    let backend = WgpuBackend::new().expect(
        "WgpuBackend::new failed on a machine that must have a GPU. \
         This is a configuration bug, not a graceful skip.",
    );
    let info = backend.adapter_info();
    assert_ne!(info.name.len(), 0);
}

// ---------------------------------------------------------------------------
// 2. Full pipeline parity on standard constructs
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_full_pipeline_simple_function() {
    let fix = build_fixture(&[
        ("int", TOK_INT),
        ("main", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_VOID),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("return", TOK_RETURN),
        ("42", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]);
    assert_full_pipeline_parity(&fix, "simple_function");
}

#[test]
fn gpu_parity_full_pipeline_function_with_arguments() {
    let fix = build_fixture(&[
        ("int", TOK_INT),
        ("add", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_INT),
        ("a", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("int", TOK_INT),
        ("b", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("return", TOK_RETURN),
        ("a", TOK_IDENTIFIER),
        ("+", TOK_PLUS),
        ("b", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]);
    assert_full_pipeline_parity(&fix, "function_with_arguments");
}

#[test]
fn gpu_parity_full_pipeline_typedef_and_declaration() {
    let fix = build_fixture(&[
        ("typedef", TOK_TYPEDEF),
        ("int", TOK_INT),
        ("myint", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("myint", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
    ]);
    assert_full_pipeline_parity(&fix, "typedef_and_declaration");
}

#[test]
fn gpu_parity_full_pipeline_if_else() {
    let fix = build_fixture(&[
        ("if", TOK_IF),
        ("(", TOK_LPAREN),
        ("x", TOK_IDENTIFIER),
        (">", TOK_GT),
        ("0", TOK_INTEGER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("return", TOK_RETURN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("else", TOK_ELSE),
        ("{", TOK_LBRACE),
        ("return", TOK_RETURN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]);
    assert_full_pipeline_parity(&fix, "if_else");
}

#[test]
fn gpu_parity_full_pipeline_for_loop() {
    let fix = build_fixture(&[
        ("for", TOK_FOR),
        ("(", TOK_LPAREN),
        ("int", TOK_INT),
        ("i", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("i", TOK_IDENTIFIER),
        ("<", TOK_LT),
        ("10", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("i", TOK_IDENTIFIER),
        ("++", TOK_INC),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]);
    assert_full_pipeline_parity(&fix, "for_loop");
}

#[test]
fn gpu_parity_full_pipeline_switch_case() {
    let fix = build_fixture(&[
        ("switch", TOK_SWITCH),
        ("(", TOK_LPAREN),
        ("x", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("case", TOK_CASE),
        ("1", TOK_INTEGER),
        (":", TOK_COLON),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("default", TOK_DEFAULT),
        (":", TOK_COLON),
        ("return", TOK_RETURN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]);
    assert_full_pipeline_parity(&fix, "switch_case");
}

// ---------------------------------------------------------------------------
// 3. Full pipeline parity with directives / preproc rows
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_full_pipeline_with_directives() {
    let fix = build_fixture(&[
        ("#ifndef FOO", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#define FOO 1", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("#endif", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    assert_full_pipeline_parity(&fix, "with_directives");
}

#[test]
fn gpu_parity_full_pipeline_with_pragma() {
    let fix = build_fixture(&[
        ("#pragma once", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("void", TOK_VOID),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]);
    assert_full_pipeline_parity(&fix, "with_pragma");
}

// ---------------------------------------------------------------------------
// 4. Full pipeline parity with strings and comments
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_full_pipeline_with_string_literal() {
    let fix = build_fixture(&[
        ("const", TOK_CONST),
        ("char", TOK_CHAR_KW),
        ("*", TOK_STAR),
        ("s", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("\"hello\"", TOK_STRING),
        (";", TOK_SEMICOLON),
    ]);
    assert_full_pipeline_parity(&fix, "with_string_literal");
}

#[test]
fn gpu_parity_full_pipeline_with_char_literal() {
    let fix = build_fixture(&[
        ("char", TOK_CHAR_KW),
        ("c", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("\'x\'", TOK_CHAR),
        (";", TOK_SEMICOLON),
    ]);
    assert_full_pipeline_parity(&fix, "with_char_literal");
}

#[test]
fn gpu_parity_full_pipeline_with_comments() {
    let fix = build_fixture(&[
        ("/* header */", TOK_COMMENT),
        ("int", TOK_INT),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("// trailer", TOK_COMMENT),
    ]);
    assert_full_pipeline_parity(&fix, "with_comments");
}

// ---------------------------------------------------------------------------
// 5. Structural / delimiter-heavy parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_full_pipeline_nested_blocks() {
    let fix = build_fixture(&[
        ("{", TOK_LBRACE),
        ("{", TOK_LBRACE),
        ("{", TOK_LBRACE),
        ("{", TOK_LBRACE),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
        ("}", TOK_RBRACE),
    ]);
    assert_full_pipeline_parity(&fix, "nested_blocks");
}

#[test]
fn gpu_parity_full_pipeline_deeply_nested_parens() {
    let fix = build_fixture(&[
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("x", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
    ]);
    assert_full_pipeline_parity(&fix, "deeply_nested_parens");
}

#[test]
fn gpu_parity_full_pipeline_all_delimiter_types() {
    let fix = build_fixture(&[
        ("(", TOK_LPAREN),
        ("[", TOK_LBRACKET),
        ("{", TOK_LBRACE),
        ("x", TOK_IDENTIFIER),
        ("}", TOK_RBRACE),
        ("]", TOK_RBRACKET),
        (")", TOK_RPAREN),
    ]);
    assert_full_pipeline_parity(&fix, "all_delimiter_types");
}

// ---------------------------------------------------------------------------
// 6. Span preservation contracts (PG output)
// ---------------------------------------------------------------------------

#[test]
fn pg_spans_preserved_across_full_pipeline_simple() {
    let fix = build_fixture(&[
        ("int", TOK_INT),
        ("x", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (";", TOK_SEMICOLON),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let pg = run_cpu_pg_lower(&typed);

    for i in 0..fix.tok_types.len() {
        let pg_start = word_at(&pg, i * PG_STRIDE_U32 + 1);
        let pg_end = word_at(&pg, i * PG_STRIDE_U32 + 2);
        let expected_start = fix.tok_starts[i];
        let expected_end = fix.tok_starts[i] + fix.tok_lens[i];
        assert_eq!(
            pg_start, expected_start,
            "PG span_start mismatch at token {i}"
        );
        assert_eq!(pg_end, expected_end, "PG span_end mismatch at token {i}");
    }
}

#[test]
fn pg_spans_preserved_with_preproc_rows() {
    let fix = build_fixture(&[
        ("#define X 1", TOK_PREPROC),
        ("\n", TOK_WHITESPACE),
        ("int", TOK_INT),
        ("y", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let pg = run_cpu_pg_lower(&typed);

    // Find the preproc row and assert its span is correct.
    for i in 0..fix.tok_types.len() {
        let pg_start = word_at(&pg, i * PG_STRIDE_U32 + 1);
        let pg_end = word_at(&pg, i * PG_STRIDE_U32 + 2);
        let expected_start = fix.tok_starts[i];
        let expected_end = fix.tok_starts[i] + fix.tok_lens[i];
        assert_eq!(
            pg_start, expected_start,
            "PG span_start mismatch at row {i}"
        );
        assert_eq!(pg_end, expected_end, "PG span_end mismatch at row {i}");
    }
}

#[test]
fn pg_parent_links_are_sentinel_or_in_range() {
    let fix = build_fixture(&[
        ("int", TOK_INT),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("return", TOK_RETURN),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let pg = run_cpu_pg_lower(&typed);
    let num_nodes = node_count_from_vast(&typed) as usize;

    for i in 0..num_nodes {
        let parent = word_at(&pg, i * PG_STRIDE_U32 + 3);
        assert!(
            parent == SENTINEL || (parent as usize) < num_nodes,
            "PG parent at row {i} must be SENTINEL or in range: got {parent}"
        );
    }
}

#[test]
fn pg_span_end_is_gte_span_start_for_all_rows() {
    let fix = build_fixture(&[
        ("if", TOK_IF),
        ("(", TOK_LPAREN),
        ("a", TOK_IDENTIFIER),
        ("&&", TOK_AND),
        ("b", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("{", TOK_LBRACE),
        ("}", TOK_RBRACE),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let pg = run_cpu_pg_lower(&typed);
    let num_nodes = node_count_from_vast(&typed) as usize;

    for i in 0..num_nodes {
        let start = word_at(&pg, i * PG_STRIDE_U32 + 1);
        let end = word_at(&pg, i * PG_STRIDE_U32 + 2);
        assert!(
            end >= start,
            "PG row {i}: span_end ({end}) must be >= span_start ({start})"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Expression-shape GPU parity on complex expressions
// ---------------------------------------------------------------------------

#[test]
fn gpu_expr_shape_parity_on_binary_chain() {
    let fix = build_fixture(&[
        ("a", TOK_IDENTIFIER),
        ("+", TOK_PLUS),
        ("b", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("c", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let shape_cpu = run_cpu_expr_shape(&raw, &typed);
    let shape_gpu = run_gpu_expr_shape(&raw, &typed);
    assert_eq!(
        shape_gpu, shape_cpu,
        "expression-shape GPU parity on binary chain"
    );
}

