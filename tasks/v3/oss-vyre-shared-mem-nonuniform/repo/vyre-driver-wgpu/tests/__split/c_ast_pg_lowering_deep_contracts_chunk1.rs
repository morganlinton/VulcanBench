// Deep C AST-to-ProgramGraph semantic lowering contracts.

// cfg(feature = "c-parser")  -  moved to parent

#[allow(dead_code)]
#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    build_fixture, row_indices, run_gpu_semantic_pg_lower as run_gpu_semantic_lower, word_at,
    Fixture, FixtureToken, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{
    reference_ast_to_pg_semantic_graph, C_AST_PG_CATEGORY_CONTROL,
    C_AST_PG_CATEGORY_DECLARATION, C_AST_PG_CATEGORY_EXPRESSION, C_AST_PG_EDGE_CASE_VALUE,
    C_AST_PG_EDGE_FIRST_CHILD, C_AST_PG_EDGE_GOTO_TARGET, C_AST_PG_EDGE_PARENT,
    C_AST_PG_EDGE_ROWS_PER_NODE, C_AST_PG_EDGE_STRIDE_U32, C_AST_PG_EDGE_SWITCH_CASE,
    C_AST_PG_EDGE_SWITCH_DEFAULT, C_AST_PG_EDGE_SWITCH_SELECTOR, C_AST_PG_ROLE_ALIGNOF,
    C_AST_PG_ROLE_ARRAY_DESIGNATOR_OR_SUBSCRIPT, C_AST_PG_ROLE_ASSIGNMENT, C_AST_PG_ROLE_BREAK,
    C_AST_PG_ROLE_CASE, C_AST_PG_ROLE_CONTINUE, C_AST_PG_ROLE_DEFAULT,
    C_AST_PG_ROLE_FIELD_DESIGNATOR_OR_MEMBER_ACCESS, C_AST_PG_ROLE_FUNCTION_DEFINITION,
    C_AST_PG_ROLE_FUNCTION_POINTER_DECL, C_AST_PG_ROLE_GOTO, C_AST_PG_ROLE_INITIALIZER_LIST,
    C_AST_PG_ROLE_LABEL, C_AST_PG_ROLE_LOOP, C_AST_PG_ROLE_RETURN, C_AST_PG_ROLE_SELECTION,
    C_AST_PG_ROLE_STATEMENT_EXPR, C_AST_PG_ROLE_SWITCH, C_AST_PG_SEMANTIC_NODE_STRIDE_U32,
};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ALIGNOF_EXPR,
    C_AST_KIND_ARRAY_SUBSCRIPT_EXPR, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_BREAK_STMT,
    C_AST_KIND_CASE_STMT, C_AST_KIND_CONTINUE_STMT, C_AST_KIND_DEFAULT_STMT, C_AST_KIND_DO_STMT,
    C_AST_KIND_FOR_STMT, C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_FUNCTION_DEFINITION,
    C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT,
    C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_LABEL_STMT, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_POINTER_DECL, C_AST_KIND_RETURN_STMT, C_AST_KIND_SWITCH_STMT, C_AST_KIND_WHILE_STMT,
};

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

fn semantic_node_word(nodes: &[u8], idx: usize, field: usize) -> u32 {
    word_at(
        nodes,
        idx * C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize + field,
    )
}

fn semantic_edge_word(edges: &[u8], node_idx: usize, edge_slot: usize, field: usize) -> u32 {
    let edge_idx = node_idx * C_AST_PG_EDGE_ROWS_PER_NODE as usize + edge_slot;
    word_at(edges, edge_idx * C_AST_PG_EDGE_STRIDE_U32 as usize + field)
}

fn vast_word(rows: &[u8], idx: usize, field: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + field)
}

fn assert_semantic_node(nodes: &[u8], idx: usize, kind: u32, category: u32, role: u32) {
    assert_eq!(semantic_node_word(nodes, idx, 0), kind, "kind[{idx}]");
    assert_eq!(
        semantic_node_word(nodes, idx, 6),
        category,
        "category[{idx}]"
    );
    assert_eq!(semantic_node_word(nodes, idx, 7), role, "role[{idx}]");
}

fn assert_parent_edge(edges: &[u8], node_idx: usize, parent_idx: u32, role: u32, category: u32) {
    assert_eq!(
        semantic_edge_word(edges, node_idx, 0, 0),
        C_AST_PG_EDGE_PARENT,
        "parent edge kind[{node_idx}]"
    );
    assert_eq!(semantic_edge_word(edges, node_idx, 0, 1), parent_idx);
    assert_eq!(semantic_edge_word(edges, node_idx, 0, 2), node_idx as u32);
    assert_eq!(semantic_edge_word(edges, node_idx, 0, 4), role);
    assert_eq!(semantic_edge_word(edges, node_idx, 0, 5), category);
}

fn assert_semantic_edge(
    edges: &[u8],
    node_idx: usize,
    edge_slot: usize,
    edge_kind: u32,
    src_idx: u32,
    dst_idx: u32,
) {
    assert_eq!(
        semantic_edge_word(edges, node_idx, edge_slot, 0),
        edge_kind,
        "semantic edge kind node={node_idx} slot={edge_slot}"
    );
    assert_eq!(
        semantic_edge_word(edges, node_idx, edge_slot, 1),
        src_idx,
        "semantic edge src node={node_idx} slot={edge_slot}"
    );
    assert_eq!(
        semantic_edge_word(edges, node_idx, edge_slot, 2),
        dst_idx,
        "semantic edge dst node={node_idx} slot={edge_slot}"
    );
}

fn fixture_label_case_default() -> Fixture {
    build_fixture(&[
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
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("target", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("default", TOK_DEFAULT),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_statement_expr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_initializer_designator() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_function_definition() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typedef", TOK_TYPEDEF),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_control_flow_roles() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("if", TOK_IF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("cond", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("for", TOK_FOR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("while", TOK_WHILE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("cond", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("do", TOK_DO),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("continue", TOK_CONTINUE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("while", TOK_WHILE),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("switch", TOK_SWITCH),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("case", TOK_CASE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("default", TOK_DEFAULT),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_alignof_expression() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("_Alignof", TOK_ALIGNOF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_function_pointer_declarator() -> Fixture {
    build_fixture(&[
        FixtureToken::new("static", TOK_STATIC),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("const", TOK_IDENTIFIER),
        FixtureToken::new("ops", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("device", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("probe", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("remove", TOK_IDENTIFIER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn labels_case_and_default_have_semantic_node_and_edge_witnesses() {
    let typed = classify(&fixture_label_case_default());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);

    let case_idx = row_indices(&typed, C_AST_KIND_CASE_STMT)[0];
    let label_idx = row_indices(&typed, C_AST_KIND_LABEL_STMT)[0];
    let default_idx = row_indices(&typed, C_AST_KIND_DEFAULT_STMT)[0];

    assert_semantic_node(
        &semantic.nodes,
        case_idx,
        C_AST_KIND_CASE_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_CASE,
    );
    assert_semantic_node(
        &semantic.nodes,
        label_idx,
        C_AST_KIND_LABEL_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_LABEL,
    );
    assert_semantic_node(
        &semantic.nodes,
        default_idx,
        C_AST_KIND_DEFAULT_STMT,
        C_AST_PG_CATEGORY_CONTROL,
        C_AST_PG_ROLE_DEFAULT,
    );

    assert_parent_edge(
        &semantic.edges,
        case_idx,
        word_at(&typed, case_idx * VAST_STRIDE_U32 + 1),
        C_AST_PG_ROLE_CASE,
        C_AST_PG_CATEGORY_CONTROL,
    );
    assert_parent_edge(
        &semantic.edges,
        label_idx,
        word_at(&typed, label_idx * VAST_STRIDE_U32 + 1),
        C_AST_PG_ROLE_LABEL,
        C_AST_PG_CATEGORY_CONTROL,
    );
}

#[test]
fn statement_expression_has_expression_category_and_first_child_edge() {
    let typed = classify(&fixture_statement_expr());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);
    let stmt_idx = row_indices(&typed, C_AST_KIND_GNU_STATEMENT_EXPR)[0];

    assert_semantic_node(
        &semantic.nodes,
        stmt_idx,
        C_AST_KIND_GNU_STATEMENT_EXPR,
        C_AST_PG_CATEGORY_EXPRESSION,
        C_AST_PG_ROLE_STATEMENT_EXPR,
    );
    assert_eq!(
        semantic_edge_word(&semantic.edges, stmt_idx, 1, 0),
        C_AST_PG_EDGE_FIRST_CHILD,
        "statement expression must retain first-child graph edge"
    );
    assert_eq!(
        semantic_edge_word(&semantic.edges, stmt_idx, 1, 2),
        word_at(&typed, stmt_idx * VAST_STRIDE_U32 + 2),
        "statement expression child edge must point at the VAST child"
    );
}

#[test]
fn initializer_designators_have_stable_roles() {
    let typed = classify(&fixture_initializer_designator());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);

    for idx in row_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_semantic_node(
            &semantic.nodes,
            idx,
            C_AST_KIND_INITIALIZER_LIST,
            C_AST_PG_CATEGORY_EXPRESSION,
            C_AST_PG_ROLE_INITIALIZER_LIST,
        );
    }
    for idx in row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR) {
        assert_semantic_node(
            &semantic.nodes,
            idx,
            C_AST_KIND_MEMBER_ACCESS_EXPR,
            C_AST_PG_CATEGORY_EXPRESSION,
            C_AST_PG_ROLE_FIELD_DESIGNATOR_OR_MEMBER_ACCESS,
        );
    }
    for idx in row_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR) {
        assert_semantic_node(
            &semantic.nodes,
            idx,
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
            C_AST_PG_CATEGORY_EXPRESSION,
            C_AST_PG_ROLE_ARRAY_DESIGNATOR_OR_SUBSCRIPT,
        );
    }
    for idx in row_indices(&typed, C_AST_KIND_ASSIGN_EXPR) {
        assert_semantic_node(
            &semantic.nodes,
            idx,
            C_AST_KIND_ASSIGN_EXPR,
            C_AST_PG_CATEGORY_EXPRESSION,
            C_AST_PG_ROLE_ASSIGNMENT,
        );
    }
}
