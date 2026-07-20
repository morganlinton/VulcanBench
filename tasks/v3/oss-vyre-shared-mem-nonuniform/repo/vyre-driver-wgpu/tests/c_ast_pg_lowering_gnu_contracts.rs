//! GNU C AST-to-ProgramGraph semantic lowering contracts.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[allow(dead_code)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    build_fixture, row_indices, run_gpu_semantic_pg_lower as run_gpu_semantic_lower, word_at,
    Fixture, FixtureToken, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{
    reference_ast_to_pg_semantic_graph, C_AST_PG_CATEGORY_GNU, C_AST_PG_EDGE_PARENT,
    C_AST_PG_EDGE_ROWS_PER_NODE, C_AST_PG_EDGE_STRIDE_U32, C_AST_PG_ROLE_ASM_CLOBBER,
    C_AST_PG_ROLE_ASM_GOTO_LABEL, C_AST_PG_ROLE_ASM_INPUT, C_AST_PG_ROLE_ASM_OUTPUT,
    C_AST_PG_ROLE_ASM_TEMPLATE, C_AST_PG_ROLE_GNU_ATTRIBUTE, C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
    C_AST_PG_ROLE_INLINE_ASM, C_AST_PG_SEMANTIC_NODE_STRIDE_U32,
};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASM_CLOBBERS_LIST,
    C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND, C_AST_KIND_ASM_OUTPUT_OPERAND,
    C_AST_KIND_ASM_TEMPLATE, C_AST_KIND_ATTRIBUTE_ALIGNED, C_AST_KIND_ATTRIBUTE_SECTION,
    C_AST_KIND_ATTRIBUTE_USED, C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_INLINE_ASM,
};

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

fn assert_gnu_role(nodes: &[u8], idx: usize, kind: u32, role: u32) {
    assert_eq!(semantic_node_word(nodes, idx, 0), kind, "kind[{idx}]");
    assert_eq!(
        semantic_node_word(nodes, idx, 6),
        C_AST_PG_CATEGORY_GNU,
        "category[{idx}]"
    );
    assert_eq!(semantic_node_word(nodes, idx, 7), role, "role[{idx}]");
}

fn fixture_asm_goto() -> Fixture {
    build_fixture(&[
        FixtureToken::new("asm", TOK_IDENTIFIER),
        FixtureToken::new("goto", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"jmp %l0\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"=r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("out", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"r\"", TOK_STRING),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("in", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("\"memory\"", TOK_STRING),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"cc\"", TOK_STRING),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("fail", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("ok", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attributes() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\".text\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("16", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("used", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("probe", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn gnu_asm_operands_have_semantic_roles_and_edges() {
    let typed = classify(&fixture_asm_goto());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);

    assert_gnu_role(
        &semantic.nodes,
        row_indices(&typed, C_AST_KIND_INLINE_ASM)[0],
        C_AST_KIND_INLINE_ASM,
        C_AST_PG_ROLE_INLINE_ASM,
    );
    assert_gnu_role(
        &semantic.nodes,
        row_indices(&typed, C_AST_KIND_ASM_TEMPLATE)[0],
        C_AST_KIND_ASM_TEMPLATE,
        C_AST_PG_ROLE_ASM_TEMPLATE,
    );
    assert_gnu_role(
        &semantic.nodes,
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND)[0],
        C_AST_KIND_ASM_OUTPUT_OPERAND,
        C_AST_PG_ROLE_ASM_OUTPUT,
    );
    assert_gnu_role(
        &semantic.nodes,
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND)[0],
        C_AST_KIND_ASM_INPUT_OPERAND,
        C_AST_PG_ROLE_ASM_INPUT,
    );

    for idx in row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST) {
        assert_gnu_role(
            &semantic.nodes,
            idx,
            C_AST_KIND_ASM_CLOBBERS_LIST,
            C_AST_PG_ROLE_ASM_CLOBBER,
        );
        assert_eq!(
            semantic_edge_word(&semantic.edges, idx, 0, 0),
            C_AST_PG_EDGE_PARENT
        );
        assert_eq!(
            semantic_edge_word(&semantic.edges, idx, 0, 1),
            word_at(&typed, idx * VAST_STRIDE_U32 + 1)
        );
    }

    for idx in row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS) {
        assert_gnu_role(
            &semantic.nodes,
            idx,
            C_AST_KIND_ASM_GOTO_LABELS,
            C_AST_PG_ROLE_ASM_GOTO_LABEL,
        );
    }
}

#[test]
fn gnu_attributes_have_wrapper_and_detail_roles() {
    let typed = classify(&fixture_attributes());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);

    assert_gnu_role(
        &semantic.nodes,
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE)[0],
        C_AST_KIND_GNU_ATTRIBUTE,
        C_AST_PG_ROLE_GNU_ATTRIBUTE,
    );
    for kind in [
        C_AST_KIND_ATTRIBUTE_SECTION,
        C_AST_KIND_ATTRIBUTE_ALIGNED,
        C_AST_KIND_ATTRIBUTE_USED,
    ] {
        let idx = row_indices(&typed, kind)[0];
        assert_gnu_role(
            &semantic.nodes,
            idx,
            kind,
            C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        );
    }
}

#[test]
fn gpu_semantic_lowering_matches_cpu_oracle_for_gnu_asm() {
    let typed = classify(&fixture_asm_goto());
    let semantic = reference_ast_to_pg_semantic_graph(&typed);
    let (gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    assert_eq!(
        gpu_nodes, semantic.nodes,
        "GNU semantic node GPU/CPU parity"
    );
    assert_eq!(
        gpu_edges, semantic.edges,
        "GNU semantic edge GPU/CPU parity"
    );
}
