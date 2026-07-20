use super::*;

#[test]
fn vast_unmatched_lbrace_rbrace_lparen_mixed_produces_rows() {
    let tok_types = [
        TOK_LBRACE, TOK_RPAREN, TOK_LBRACE, TOK_RBRACE, TOK_LPAREN, TOK_RBRACE,
    ];
    let tok_starts: Vec<u32> = (0..tok_types.len()).map(|i| i as u32 * 2).collect();
    let tok_lens = vec![1u32; tok_types.len()];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        raw.len(),
        tok_types.len() * VAST_STRIDE_U32 * 4,
        "unmatched mixed delimiters must still emit one row per token"
    );
}

#[test]
fn vast_unmatched_deep_paren_nesting_produces_rows() {
    // ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    let depth = 64usize;
    let tok_types: Vec<u32> = std::iter::repeat(TOK_LPAREN).take(depth).collect();
    let tok_starts: Vec<u32> = (0..depth).map(|i| i as u32).collect();
    let tok_lens = vec![1u32; depth];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        raw.len(),
        depth * VAST_STRIDE_U32 * 4,
        "64 unmatched LPAREN must produce 64 rows"
    );
    // Every LPAREN except the first should have the previous LPAREN as parent
    let parent_of_last = word_at(&raw, (depth - 1) * VAST_STRIDE_U32 + 1);
    assert_eq!(
        parent_of_last,
        (depth - 2) as u32,
        "deep nesting must chain parent links"
    );
}

#[test]
fn vast_unmatched_bracket_inside_declarator_context_produces_rows() {
    // int x[;
    let tok_types = [TOK_INT, TOK_IDENTIFIER, TOK_LBRACKET, TOK_SEMICOLON];
    let tok_starts = [0u32, 2, 4, 6];
    let tok_lens = [1u32; 4];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 4 * VAST_STRIDE_U32 * 4);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    // The classifier may or may not recognise ARRAY_DECL without a matching RBRACKET.
    // The contract is that it does not crash and span fields are preserved.
    assert_ne!(
        word_at(&typed, 2 * VAST_STRIDE_U32 + 5),
        0,
        "unmatched LBRACKET span_start must survive classification"
    );
}

// ---------------------------------------------------------------------------
// 2. Malformed declarations
// ---------------------------------------------------------------------------

#[test]
fn malformed_decl_missing_semicolon_after_declarator() {
    // int x
    let tok_types = [TOK_INT, TOK_IDENTIFIER];
    let tok_starts = [0u32, 2];
    let tok_lens = [1u32; 2];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 2 * VAST_STRIDE_U32 * 4);
    // The identifier must still be a row with valid span
    assert_eq!(
        word_at(&raw, 1 * VAST_STRIDE_U32 + 5),
        2,
        "span_start preserved"
    );
}

#[test]
fn malformed_decl_abstract_declarator_in_init_context() {
    // int (*) = 1;
    let tok_types = [
        TOK_INT,
        TOK_LPAREN,
        TOK_STAR,
        TOK_RPAREN,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_SEMICOLON,
    ];
    let tok_starts = [0u32, 2, 4, 6, 8, 10, 12];
    let tok_lens = [1u32; 7];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let ptrs = row_indices(&typed, C_AST_KIND_POINTER_DECL);
    assert!(
        ptrs.contains(&2),
        "abstract pointer declarator must still produce POINTER_DECL; got {ptrs:?}"
    );
}

#[test]
fn malformed_decl_empty_parameter_list_parens() {
    // void f(int,) { }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION),
        vec![1],
        "function identifier with a body must type as FUNCTION_DEFINITION despite trailing comma"
    );
}

#[test]
fn malformed_decl_specifier_without_declarator() {
    // static inline;
    let tok_types = [TOK_STATIC, TOK_INLINE, TOK_SEMICOLON];
    let tok_starts = [0u32, 7, 13];
    let tok_lens = [6u32, 5, 1];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 3 * VAST_STRIDE_U32 * 4);
    // No crash contract
    let any_nonzero = raw.iter().any(|&b| b != 0);
    assert!(
        any_nonzero,
        "must not emit all-zero output for non-empty input"
    );
}

// ---------------------------------------------------------------------------
// 3. Unterminated attribute argument lists after lexing
// ---------------------------------------------------------------------------

#[test]
fn unterminated_attribute_parens_do_not_crash_classifier() {
    // __attribute__((aligned(16
    let fix = build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("16", TOK_INTEGER),
    ]);
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "unterminated attribute must still classify its introducer"
    );
    // Must not crash; output must be non-empty and row count preserved
    assert_eq!(typed.len(), raw.len(), "classifier must preserve row count");
}

#[test]
fn unterminated_attribute_missing_inner_rparen() {
    // __attribute__((section(".text"
    let fix = build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\".text\"", TOK_STRING),
    ]);
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    assert_ne!(typed.len(), 0,
        "unterminated attribute string arg must not crash"
    );
}

// ---------------------------------------------------------------------------
// 4. Bad asm operands
// ---------------------------------------------------------------------------

#[test]
fn asm_missing_output_colon_treated_as_template_only() {
    // asm volatile ("foo" : :
    let fix = build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("volatile", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"foo\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_INLINE_ASM),
        vec![0],
        "bad-asm introducer must classify as INLINE_ASM"
    );
    // Missing output operand between first and second colon should not fabricate one
    let outputs = row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND);
    assert!(
        outputs.is_empty(),
        "missing output operand must not fabricate ASM_OUTPUT_OPERAND"
    );
}

#[test]
fn asm_unclosed_operand_paren() {
    // asm("mov" : "=r" (out
    let fix = build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"mov\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"=r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("out", TOK_IDENTIFIER),
    ]);
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    // The classifier may or may not see the output operand without a closing paren.
    // The contract is no crash and no fabricated nodes beyond token count.
    assert_eq!(typed.len(), raw.len(), "classifier must preserve row count");
}

#[test]
fn asm_extra_trailing_colon_does_not_leak_clobber_kind() {
    // asm("nop" :::)
    let fix = build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"nop\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let typed = classify(&fix);
    assert_eq!(row_indices(&typed, C_AST_KIND_INLINE_ASM), vec![0]);
    // No clobber strings present; trailing colon alone must not invent clobber rows
    let clobbers = row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST);
    assert!(
        clobbers.is_empty(),
        "trailing colon without clobber string must not fabricate ASM_CLOBBERS_LIST"
    );
}

// ---------------------------------------------------------------------------
// 5. Invalid designator nesting
// ---------------------------------------------------------------------------

#[test]
fn dotted_expression_outside_initializer_is_member_access() {
    // x .a = 1;
    let fix = build_fixture(&[
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let dots = row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_eq!(
        dots,
        vec![1],
        "dot outside initializer is still a member-access expression"
    );
}

#[test]
fn designator_chain_without_brace_not_initializer_list() {
    // int a = .b.c = 1;
    let fix = build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("c", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let inits = row_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        inits.is_empty(),
        "designator chain without braces must not invent INITIALIZER_LIST"
    );
}

#[test]
fn bracket_expression_outside_initializer_is_array_subscript_expr() {
    // x[0] = 1;
    let fix = build_fixture(&[
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let subs = row_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert_eq!(
        subs,
        vec![1],
        "brackets outside initializer are still array-subscript expressions"
    );
}

// ---------------------------------------------------------------------------
// 6. Case/default outside switch  -  observable via semantic edges
// ---------------------------------------------------------------------------

#[test]
fn case_outside_switch_has_no_switch_case_semantic_edge() {
    // void f() { case 1: return; }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_CASE_STMT),
        vec![5],
        "case must classify as CASE_STMT even outside switch"
    );
    let (nodes, edges) = semantic_lower(&typed);
    // Find the case node index
    let case_idx = 5usize;
    assert_eq!(
        semantic_node_word(&nodes, case_idx, 7),
        C_AST_PG_ROLE_CASE,
        "case node must have ROLE_CASE"
    );
    // Edge slot 4 is the SWITCH_CASE edge for a case node
    assert_eq!(
        semantic_edge_word(&edges, case_idx, 4, 0),
        C_AST_PG_EDGE_NONE,
        "case outside switch must NOT have a SWITCH_CASE semantic edge"
    );
    assert_eq!(
        semantic_edge_word(&edges, case_idx, 4, 1),
        u32::MAX,
        "case outside switch must have sentinel src"
    );
}

#[test]
fn default_outside_switch_has_no_switch_default_semantic_edge() {
    // void f() { default: break; }
    let fix = build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("default", TOK_DEFAULT),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_DEFAULT_STMT),
        vec![5],
        "default must classify as DEFAULT_STMT even outside switch"
    );
    let (nodes, edges) = semantic_lower(&typed);
    let default_idx = 5usize;
    assert_eq!(
        semantic_node_word(&nodes, default_idx, 7),
        C_AST_PG_ROLE_DEFAULT,
        "default node must have ROLE_DEFAULT"
    );
    // Edge slot 3 is the SWITCH_DEFAULT edge for a default node
    assert_eq!(
        semantic_edge_word(&edges, default_idx, 3, 0),
        C_AST_PG_EDGE_NONE,
        "default outside switch must NOT have a SWITCH_DEFAULT semantic edge"
    );
}

