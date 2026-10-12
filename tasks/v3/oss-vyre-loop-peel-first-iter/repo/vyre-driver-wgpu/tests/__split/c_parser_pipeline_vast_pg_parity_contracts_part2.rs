use super::*;

#[test]
fn gpu_expr_shape_parity_on_ternary() {
    let fix = build_fixture(&[
        ("a", TOK_IDENTIFIER),
        ("?", TOK_QUESTION),
        ("b", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("c", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let shape_cpu = run_cpu_expr_shape(&raw, &typed);
    let shape_gpu = run_gpu_expr_shape(&raw, &typed);
    assert_eq!(
        shape_gpu, shape_cpu,
        "expression-shape GPU parity on ternary"
    );
}

#[test]
fn gpu_expr_shape_parity_on_assignment_chain() {
    let fix = build_fixture(&[
        ("a", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("b", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("c", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let shape_cpu = run_cpu_expr_shape(&raw, &typed);
    let shape_gpu = run_gpu_expr_shape(&raw, &typed);
    assert_eq!(
        shape_gpu, shape_cpu,
        "expression-shape GPU parity on assignment chain"
    );
}

// ---------------------------------------------------------------------------
// 8. Edge-case GPU parity: minimal and adversarial inputs
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_single_token_identifier() {
    let fix = build_fixture(&[("x", TOK_IDENTIFIER)]);
    assert_full_pipeline_parity(&fix, "single_token_identifier");
}

#[test]
fn gpu_parity_single_delimiter_pair() {
    let fix = build_fixture(&[("(", TOK_LPAREN), (")", TOK_RPAREN)]);
    assert_full_pipeline_parity(&fix, "single_delimiter_pair");
}

#[test]
fn gpu_parity_keyword_only_stream() {
    let fix = build_fixture(&[
        ("if", TOK_IF),
        ("else", TOK_ELSE),
        ("while", TOK_WHILE),
        ("for", TOK_FOR),
        ("return", TOK_RETURN),
    ]);
    assert_full_pipeline_parity(&fix, "keyword_only_stream");
}

#[test]
fn gpu_parity_operator_only_stream() {
    let fix = build_fixture(&[
        ("+", TOK_PLUS),
        ("-", TOK_MINUS),
        ("*", TOK_STAR),
        ("/", TOK_SLASH),
        ("%", TOK_PERCENT),
        ("&", TOK_AMP),
        ("|", TOK_PIPE),
        ("^", TOK_CARET),
    ]);
    assert_full_pipeline_parity(&fix, "operator_only_stream");
}

#[test]
fn gpu_parity_punctuation_only_stream() {
    let fix = build_fixture(&[
        (";", TOK_SEMICOLON),
        (",", TOK_COMMA),
        (".", TOK_DOT),
        ("->", TOK_ARROW),
        ("...", TOK_ELLIPSIS),
    ]);
    assert_full_pipeline_parity(&fix, "punctuation_only_stream");
}

// ---------------------------------------------------------------------------
// 9. No silent empty outputs on GPU
// ---------------------------------------------------------------------------

#[test]
fn gpu_vast_builder_nonempty_input_produces_nonempty_output() {
    let fix = build_fixture(&[
        ("int", TOK_INT),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]);
    let gpu_raw = run_gpu_vast_builder(&fix);
    assert!(
        !gpu_raw.is_empty(),
        "GPU VAST builder must not silently return empty for non-empty input"
    );
    assert!(
        gpu_raw.iter().any(|&b| b != 0),
        "GPU VAST builder must not return all-zero buffer for non-empty input"
    );
}

#[test]
fn gpu_classifier_nonempty_input_produces_nonempty_output() {
    let fix = build_fixture(&[("int", TOK_INT), ("x", TOK_IDENTIFIER)]);
    let raw = run_cpu_vast_builder(&fix);
    let gpu_typed = run_gpu_classifier(&raw);
    assert_ne!(gpu_typed.len(), 0,
        "GPU classifier must not silently return empty for non-empty input"
    );
}

#[test]
fn gpu_pg_lower_nonempty_input_produces_nonempty_output() {
    let fix = build_fixture(&[("int", TOK_INT), ("x", TOK_IDENTIFIER)]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let gpu_pg = run_gpu_pg_lower(&typed);
    assert_ne!(gpu_pg.len(), 0,
        "GPU PG lowerer must not silently return empty for non-empty input"
    );
}

#[test]
fn gpu_expr_shape_nonempty_input_produces_nonempty_output() {
    let fix = build_fixture(&[
        ("a", TOK_IDENTIFIER),
        ("+", TOK_PLUS),
        ("b", TOK_IDENTIFIER),
    ]);
    let raw = run_cpu_vast_builder(&fix);
    let typed = run_cpu_classifier(&raw);
    let gpu_shape = run_gpu_expr_shape(&raw, &typed);
    assert_ne!(gpu_shape.len(), 0,
        "GPU expr-shape must not silently return empty for non-empty input"
    );
}
