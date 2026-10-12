//! Semantic PG edge contracts for no-host parser completion.
//!
//! Tests assert concrete edge kinds and semantic roles directly on GPU output
//! without relying on the CPU semantic-lowering oracle (`reference_ast_to_pg_semantic_graph`).
//! VAST build / annotation / classify stages are used only as fixture setup.
//!
//! Constructs under test:
//!   - scope (structural parent-edge nesting)
//!   - type (pointer-declarator roles)
//!   - label / goto (`GOTO_TARGET` edge)
//!   - switch / case / default (`SWITCH_SELECTOR`, `SWITCH_CASE`, `SWITCH_DEFAULT`, `CASE_VALUE` edges)
//!   - function-pointer (`FUNCTION_POINTER_DECL` role)
//!   - typedef (`TYPEDEF_DECL` role)
//!   - tag / enum (`AGGREGATE_DECL` and `ENUMERATOR_DECL` roles)

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[allow(dead_code)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    build_fixture, row_indices, run_gpu_semantic_pg_lower as run_gpu_semantic_lower, word_at,
    Fixture, FixtureToken, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::ast_to_pg_nodes::C_AST_PG_ROLE_AGGREGATE_DECL;
use vyre_libs::parsing::c::lower::{
    C_AST_PG_CATEGORY_CONTROL, C_AST_PG_CATEGORY_DECLARATION, C_AST_PG_EDGE_CASE_VALUE,
    C_AST_PG_EDGE_GOTO_TARGET, C_AST_PG_EDGE_PARENT, C_AST_PG_EDGE_ROWS_PER_NODE,
    C_AST_PG_EDGE_STRIDE_U32, C_AST_PG_EDGE_SWITCH_CASE, C_AST_PG_EDGE_SWITCH_DEFAULT,
    C_AST_PG_EDGE_SWITCH_SELECTOR, C_AST_PG_ROLE_CASE, C_AST_PG_ROLE_DEFAULT,
    C_AST_PG_ROLE_ENUMERATOR_DECL, C_AST_PG_ROLE_FUNCTION_DEFINITION,
    C_AST_PG_ROLE_FUNCTION_POINTER_DECL, C_AST_PG_ROLE_GOTO, C_AST_PG_ROLE_LABEL,
    C_AST_PG_ROLE_POINTER_DECL, C_AST_PG_ROLE_SWITCH, C_AST_PG_ROLE_TYPEDEF_DECL,
    C_AST_PG_SEMANTIC_NODE_STRIDE_U32,
};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_CASE_STMT, C_AST_KIND_DEFAULT_STMT,
    C_AST_KIND_ENUMERATOR_DECL, C_AST_KIND_ENUM_DECL, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GOTO_STMT, C_AST_KIND_LABEL_STMT,
    C_AST_KIND_POINTER_DECL, C_AST_KIND_STRUCT_DECL, C_AST_KIND_SWITCH_STMT,
    C_AST_KIND_TYPEDEF_DECL,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    (vast.len() / (VAST_STRIDE_U32 * 4)) as u32
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
    assert_eq!(
        semantic_edge_word(edges, node_idx, 0, 1),
        parent_idx,
        "parent edge src[{node_idx}]"
    );
    assert_eq!(
        semantic_edge_word(edges, node_idx, 0, 2),
        node_idx as u32,
        "parent edge dst[{node_idx}]"
    );
    assert_eq!(
        semantic_edge_word(edges, node_idx, 0, 4),
        role,
        "parent edge role[{node_idx}]"
    );
    assert_eq!(
        semantic_edge_word(edges, node_idx, 0, 5),
        category,
        "parent edge category[{node_idx}]"
    );
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

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// typedef int T;
/// struct S { int x; };
/// enum E { A, B };
/// void (*fp)(struct S *);
/// ```
fn fixture_typedef_struct_enum_fnptr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typedef", TOK_TYPEDEF),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("enum", TOK_ENUM),
        FixtureToken::new("E", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("A", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("B", TOK_IDENTIFIER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("fp", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// ```c
/// void f(int x) {
///   switch (x) {
///     case 1: break;
///     default: goto end;
///   }
///   end: return;
/// }
/// ```
fn fixture_switch_case_default_goto_label() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
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
        FixtureToken::new("break", TOK_BREAK),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("default", TOK_DEFAULT),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("goto", TOK_GOTO),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("end", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

/// ```c
/// void f() {
///   { int a; }
///   { int b; }
/// }
/// ```
fn fixture_scope_nesting() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

// ---------------------------------------------------------------------------
// Typedef / tag / enum / function-pointer role contracts
// ---------------------------------------------------------------------------

mod c_ast_semantic_pg_no_host_edge_contracts_part1 {

    include!("__split/c_ast_semantic_pg_no_host_edge_contracts_part1.rs");
}
mod c_ast_semantic_pg_no_host_edge_contracts_part2 {
    include!("__split/c_ast_semantic_pg_no_host_edge_contracts_part2.rs");
}
