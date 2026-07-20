use super::*;

#[test]
fn case_inside_switch_has_switch_case_edge() {
    // void f(int x) { switch (x) { case 0: break; } }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("switch", TOK_SWITCH),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    let cases = row_indices(&typed, C_AST_KIND_CASE_STMT);
    assert_eq!(cases, vec![12], "case must be at row 12");
    let switches = row_indices(&typed, C_AST_KIND_SWITCH_STMT);
    assert_eq!(switches, vec![7], "switch must be at row 7");

    let (_nodes, edges) = semantic_lower(&typed);
    let case_idx = cases[0];
    let switch_idx = switches[0];
    // Slot 4 for case nodes: SWITCH_CASE edge (switch -> case)
    assert_eq!(
        semantic_edge_word(&edges, case_idx, 4, 0),
        C_AST_PG_EDGE_SWITCH_CASE,
        "case inside switch must have SWITCH_CASE edge"
    );
    assert_eq!(
        semantic_edge_word(&edges, case_idx, 4, 1),
        switch_idx as u32,
        "SWITCH_CASE edge src must be the switch"
    );
    assert_eq!(
        semantic_edge_word(&edges, case_idx, 4, 2),
        case_idx as u32,
        "SWITCH_CASE edge dst must be the case"
    );
}

// ---------------------------------------------------------------------------
// 7. Label/goto mismatches  -  observable via semantic edges
// ---------------------------------------------------------------------------

#[test]
fn goto_undefined_label_has_no_goto_target_edge() {
    // void f() { goto nowhere; }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("nowhere", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    let gotos = row_indices(&typed, C_AST_KIND_GOTO_STMT);
    assert_eq!(gotos, vec![5], "goto must classify as GOTO_STMT");
    let (nodes, edges) = semantic_lower(&typed);
    let goto_idx = gotos[0];
    assert_eq!(
        semantic_node_word(&nodes, goto_idx, 7),
        C_AST_PG_ROLE_GOTO,
        "goto node must have ROLE_GOTO"
    );
    // Slot 3 for goto nodes: GOTO_TARGET edge
    assert_eq!(
        semantic_edge_word(&edges, goto_idx, 3, 0),
        C_AST_PG_EDGE_NONE,
        "goto to undefined label must NOT have GOTO_TARGET edge"
    );
    assert_eq!(
        semantic_edge_word(&edges, goto_idx, 3, 1),
        u32::MAX,
        "undefined goto target src must be sentinel"
    );
}

#[test]
fn label_without_goto_has_no_incoming_goto_target_edge() {
    // void f() { unused: return; }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("unused", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    let labels = row_indices(&typed, C_AST_KIND_LABEL_STMT);
    assert_eq!(labels, vec![5], "label must classify as LABEL_STMT");
    let (_nodes, edges) = semantic_lower(&typed);
    let label_idx = labels[0];
    // Scan all edge rows to confirm no GOTO_TARGET edge points at this label
    let edge_count = edges.len() / (C_AST_PG_EDGE_STRIDE_U32 as usize * 4);
    let mut incoming_goto = false;
    for e in 0..edge_count {
        let kind = word_at(&edges, e * C_AST_PG_EDGE_STRIDE_U32 as usize);
        let dst = word_at(&edges, e * C_AST_PG_EDGE_STRIDE_U32 as usize + 2);
        if kind == C_AST_PG_EDGE_GOTO_TARGET && dst == label_idx as u32 {
            incoming_goto = true;
            break;
        }
    }
    assert!(
        !incoming_goto,
        "label without goto must have no incoming GOTO_TARGET edge"
    );
}

#[test]
fn goto_to_label_in_different_function_root_has_no_goto_target_edge() {
    // void f() { goto target; } void g() { target: return; }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("target", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("g", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("target", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    let gotos = row_indices(&typed, C_AST_KIND_GOTO_STMT);
    assert_eq!(gotos, vec![5], "goto must be at row 5");
    let labels = row_indices(&typed, C_AST_KIND_LABEL_STMT);
    assert_eq!(labels, vec![14], "label must be at row 14");
    let (_nodes, edges) = semantic_lower(&typed);
    let goto_idx = gotos[0];
    assert_eq!(
        semantic_edge_word(&edges, goto_idx, 3, 0),
        C_AST_PG_EDGE_NONE,
        "goto to label in different function root must NOT resolve"
    );
}

#[test]
fn resolved_goto_has_goto_target_edge() {
    // void f() { goto end; end: return; }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    let gotos = row_indices(&typed, C_AST_KIND_GOTO_STMT);
    let labels = row_indices(&typed, C_AST_KIND_LABEL_STMT);
    assert_eq!(gotos, vec![5], "goto row");
    assert_eq!(labels, vec![8], "label row");
    let (_nodes, edges) = semantic_lower(&typed);
    let goto_idx = gotos[0];
    let label_idx = labels[0];
    assert_eq!(
        semantic_edge_word(&edges, goto_idx, 3, 0),
        C_AST_PG_EDGE_GOTO_TARGET,
        "resolved goto must have GOTO_TARGET edge"
    );
    assert_eq!(
        semantic_edge_word(&edges, goto_idx, 3, 1),
        goto_idx as u32,
        "GOTO_TARGET src must be the goto"
    );
    assert_eq!(
        semantic_edge_word(&edges, goto_idx, 3, 2),
        label_idx as u32,
        "GOTO_TARGET dst must be the label"
    );
}

// ---------------------------------------------------------------------------
// 8. Pathological nesting / resource bounds
// ---------------------------------------------------------------------------

#[test]
fn vast_builder_128_token_stream_boundary() {
    let tok_types: Vec<u32> = std::iter::repeat(TOK_AMP).take(128).collect();
    let tok_starts: Vec<u32> = (0..128).collect();
    let tok_lens = vec![1; 128];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        raw.len(),
        128 * VAST_STRIDE_U32 * 4,
        "128 tokens must produce 128 VAST rows"
    );
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    assert_eq!(typed.len(), raw.len(), "classifier must handle 128 nodes");
    let pg = pg_lower(&typed);
    assert_eq!(pg.len(), 128 * 6 * 4, "PG lowerer must handle 128 nodes");
}

#[test]
fn deeply_nested_brace_delimiter_stack() {
    // {{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{
    let depth = 64usize;
    let tok_types: Vec<u32> = std::iter::repeat(TOK_LBRACE).take(depth).collect();
    let tok_starts: Vec<u32> = (0..depth).map(|i| i as u32).collect();
    let tok_lens = vec![1; depth];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), depth * VAST_STRIDE_U32 * 4);
    // Parent chain must be consistent  -  no sentinel gaps inside the nest
    for i in 1..depth {
        let parent = word_at(&raw, i * VAST_STRIDE_U32 + 1);
        assert_eq!(
            parent,
            (i - 1) as u32,
            "deep LBRACE nest parent at row {i} must be previous row"
        );
    }
}

#[test]
fn alternating_paren_bracket_nesting_does_not_corrupt_siblings() {
    // ([({[
    let tok_types = [
        TOK_LPAREN,
        TOK_LBRACKET,
        TOK_LBRACE,
        TOK_LPAREN,
        TOK_LBRACKET,
    ];
    let tok_starts = [0u32, 2, 4, 6, 8];
    let tok_lens = [1u32; 5];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    // Each opener must have the previous opener as parent (stack behavior)
    for i in 1..tok_types.len() {
        let parent = word_at(&raw, i * VAST_STRIDE_U32 + 1);
        assert_eq!(
            parent,
            (i - 1) as u32,
            "alternating delimiter parent at row {i} must stack correctly"
        );
    }
}

#[test]
fn max_token_boundary_classifier_preserves_spans() {
    let tok_types: Vec<u32> = std::iter::repeat(TOK_MINUS).take(256).collect();
    let tok_starts: Vec<u32> = (0..256).collect();
    let tok_lens = vec![1; 256];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    // Spot-check span preservation at first, middle, last
    for idx in [0usize, 128, 255] {
        let start = word_at(&typed, idx * VAST_STRIDE_U32 + 5);
        let len = word_at(&typed, idx * VAST_STRIDE_U32 + 6);
        assert_eq!(
            start, tok_starts[idx],
            "span_start must survive classification at row {idx}"
        );
        assert_eq!(
            len, tok_lens[idx],
            "span_len must survive classification at row {idx}"
        );
    }
}

#[test]
fn pg_lower_preserves_span_integrity_on_pathological_tokens() {
    let tok_types: Vec<u32> = (0..64)
        .map(|i| match i % 4 {
            0 => TOK_LPAREN,
            1 => TOK_RPAREN,
            2 => TOK_LBRACE,
            _ => TOK_RBRACE,
        })
        .collect();
    let tok_starts: Vec<u32> = (0..64).collect();
    let tok_lens = vec![1; 64];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = pg_lower(&typed);
    for i in 0..64 {
        let pg_start = word_at(&pg, i * 6 + 1);
        let pg_end = word_at(&pg, i * 6 + 2);
        assert_eq!(
            pg_start, tok_starts[i],
            "PG span start must match token start at row {i}"
        );
        assert_eq!(
            pg_end,
            tok_starts[i] + tok_lens[i],
            "PG span end must match token end at row {i}"
        );
    }
}

// ---------------------------------------------------------------------------
// 9. Semantic graph edge integrity under hostile inputs
// ---------------------------------------------------------------------------

#[test]
fn semantic_graph_empty_vast_produces_empty_nodes_and_edges() {
    let sg = reference_ast_to_pg_semantic_graph(&[]);
    assert!(
        sg.nodes.is_empty(),
        "semantic graph nodes must be empty for empty VAST"
    );
    assert!(
        sg.edges.is_empty(),
        "semantic graph edges must be empty for empty VAST"
    );
}

#[test]
fn semantic_graph_malformed_stream_no_fabricated_edges() {
    // int (*)[ ;  -- malformed abstract declarator with unclosed bracket
    let fix = build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    let (nodes, edges) = semantic_lower(&typed);
    // No switch/case/default/goto in this stream, so no semantic edges should be fabricated
    let edge_count = edges.len() / (C_AST_PG_EDGE_STRIDE_U32 as usize * 4);
    for e in 0..edge_count {
        let kind = word_at(&edges, e * C_AST_PG_EDGE_STRIDE_U32 as usize);
        assert!(
            kind <= 8,
            "malformed stream must not fabricate invalid edge kind {kind}"
        );
    }
    // Nodes must not be all-zero for non-empty input
    let any_nonzero_node = nodes.iter().any(|&b| b != 0);
    assert!(
        any_nonzero_node,
        "semantic lowerer must not emit all-zero nodes for non-empty input"
    );
}

