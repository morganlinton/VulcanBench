//! Hostile C parser tests for malformed-but-lexically-valid token streams.
//!
//! Covers constructs that survive the lexer but violate C grammar, exposing
//! parser stage behavior through concrete VAST/PG/semantic-graph contracts.
//!
//! Targets:
//!   * unmatched delimiters
//!   * malformed declarations
//!   * unterminated attribute argument lists after lexing
//!   * bad asm operands
//!   * invalid designator nesting
//!   * case/default outside switch (observable via semantic edges)
//!   * label/goto mismatches (observable via semantic edges)
//!   * pathological nesting / resource bounds

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[allow(dead_code)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    build_fixture, row_indices, word_at, Fixture, FixtureToken, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{
    reference_ast_to_pg_nodes, reference_ast_to_pg_semantic_graph, C_AST_PG_EDGE_GOTO_TARGET,
    C_AST_PG_EDGE_NONE, C_AST_PG_EDGE_ROWS_PER_NODE, C_AST_PG_EDGE_STRIDE_U32,
    C_AST_PG_EDGE_SWITCH_CASE, C_AST_PG_EDGE_SWITCH_DEFAULT, C_AST_PG_ROLE_CASE,
    C_AST_PG_ROLE_DEFAULT, C_AST_PG_ROLE_GOTO, C_AST_PG_SEMANTIC_NODE_STRIDE_U32,
};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    C_AST_KIND_ASM_CLOBBERS_LIST, C_AST_KIND_ASM_OUTPUT_OPERAND, C_AST_KIND_CASE_STMT,
    C_AST_KIND_DEFAULT_STMT, C_AST_KIND_FIELD_DECL, C_AST_KIND_FUNCTION_DEFINITION,
    C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_GOTO_STMT, C_AST_KIND_INITIALIZER_LIST,
    C_AST_KIND_INLINE_ASM, C_AST_KIND_LABEL_STMT, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_POINTER_DECL, C_AST_KIND_SWITCH_STMT,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

fn semantic_edge_word(edges: &[u8], node_idx: usize, edge_slot: usize, field: usize) -> u32 {
    let edge_idx = node_idx * C_AST_PG_EDGE_ROWS_PER_NODE as usize + edge_slot;
    word_at(edges, edge_idx * C_AST_PG_EDGE_STRIDE_U32 as usize + field)
}

fn semantic_node_word(nodes: &[u8], idx: usize, field: usize) -> u32 {
    word_at(
        nodes,
        idx * C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize + field,
    )
}

fn pg_lower(typed_vast: &[u8]) -> Vec<u8> {
    reference_ast_to_pg_nodes(typed_vast)
}

fn semantic_lower(typed_vast: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let sg = reference_ast_to_pg_semantic_graph(typed_vast);
    (sg.nodes, sg.edges)
}

// ---------------------------------------------------------------------------
// 1. Unmatched delimiters  -  must not crash, must emit structural rows
// ---------------------------------------------------------------------------

mod c_parser_hostile_malformed_stream_contracts_part1 {

    include!("__split/c_parser_hostile_malformed_stream_contracts_part1.rs");
}
mod c_parser_hostile_malformed_stream_contracts_part2 {
    include!("__split/c_parser_hostile_malformed_stream_contracts_part2.rs");
}
mod c_parser_hostile_malformed_stream_contracts_part3 {
    include!("__split/c_parser_hostile_malformed_stream_contracts_part3.rs");
}
