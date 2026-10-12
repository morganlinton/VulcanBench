use super::*;
use crate::object_format::{serialize_vyrecob2, SectionTag};
use vyre_libs::parsing::c::lower::{
    C_AST_PG_CATEGORY_GNU, C_AST_PG_EDGE_PARENT, C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
};
use vyre_libs::parsing::c::parse::vast::C_AST_KIND_BUILTIN_CHOOSE_EXPR;

#[test]
fn decode_object_semantic_graph_reads_nodes_edges_and_builtin_role_count() {
    let node_words = [
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        10,
        18,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        C_AST_PG_CATEGORY_GNU,
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        0,
        0,
    ];
    let edge_words = [
        C_AST_PG_EDGE_PARENT,
        0,
        0,
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        C_AST_PG_CATEGORY_GNU,
    ];
    let node_bytes = node_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let edge_bytes = edge_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[
        (SectionTag::SemanticProgramGraphNodes, node_bytes.as_slice()),
        (SectionTag::SemanticProgramGraphEdges, edge_bytes.as_slice()),
    ])
    .expect("Fix: semantic graph fixture must serialize");

    let graph =
        decode_object_semantic_graph(&object).expect("Fix: semantic graph fixture must decode");
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.nodes[0].role, C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL);
    assert_eq!(
        graph.edges[0].owner_role,
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL
    );
    assert_eq!(graph.builtin_role_nodes, 1);
    assert_eq!(graph.builtin_nodes().count(), 1);
}

#[test]
fn decode_object_semantic_graph_rejects_out_of_range_tree_links() {
    let node_words = [
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        10,
        18,
        7,
        u32::MAX,
        u32::MAX,
        C_AST_PG_CATEGORY_GNU,
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        0,
        0,
    ];
    let edge_words = [
        C_AST_PG_EDGE_PARENT,
        0,
        0,
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        C_AST_PG_CATEGORY_GNU,
    ];
    let node_bytes = node_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let edge_bytes = edge_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[
        (SectionTag::SemanticProgramGraphNodes, node_bytes.as_slice()),
        (SectionTag::SemanticProgramGraphEdges, edge_bytes.as_slice()),
    ])
    .expect("Fix: semantic graph fixture must serialize");
    let err = decode_object_semantic_graph(&object)
        .expect_err("out-of-range semantic tree link must not decode");
    assert!(
        err.contains("parent 7 is outside"),
        "semantic tree link bounds must fail loudly"
    );
}

#[test]
fn decode_object_semantic_graph_rejects_out_of_range_edges() {
    let node_words = [
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        10,
        18,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        C_AST_PG_CATEGORY_GNU,
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        0,
        0,
    ];
    let edge_words = [
        C_AST_PG_EDGE_PARENT,
        1,
        2,
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL,
        C_AST_PG_CATEGORY_GNU,
    ];
    let node_bytes = node_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let edge_bytes = edge_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[
        (SectionTag::SemanticProgramGraphNodes, node_bytes.as_slice()),
        (SectionTag::SemanticProgramGraphEdges, edge_bytes.as_slice()),
    ])
    .expect("Fix: semantic graph fixture must serialize");
    let err = decode_object_semantic_graph(&object)
        .expect_err("semantic edge outside node table must not decode");
    assert!(
        err.contains("outside 1 decoded semantic nodes"),
        "out-of-range semantic edge must fail loudly"
    );
}

#[test]
fn decode_object_semantic_graph_names_aggregate_decl_role() {
    let node_words = [
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        10,
        18,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        C_AST_PG_CATEGORY_DECLARATION,
        C_AST_PG_ROLE_AGGREGATE_DECL,
        0,
        0,
    ];
    let node_bytes = node_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[
        (SectionTag::SemanticProgramGraphNodes, node_bytes.as_slice()),
        (SectionTag::SemanticProgramGraphEdges, &[]),
    ])
    .expect("Fix: semantic graph fixture must serialize");
    let graph =
        decode_object_semantic_graph(&object).expect("Fix: aggregate role fixture must decode");
    assert_eq!(graph.role_name(0), Some("aggregate_decl"));
}

#[test]
fn decode_object_semantic_graph_rejects_unknown_node_role() {
    let node_words = [
        C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        10,
        18,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        C_AST_PG_CATEGORY_GNU,
        u32::MAX - 1,
        0,
        0,
    ];
    let node_bytes = node_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[
        (SectionTag::SemanticProgramGraphNodes, node_bytes.as_slice()),
        (SectionTag::SemanticProgramGraphEdges, &[]),
    ])
    .expect("Fix: semantic graph fixture must serialize");
    let err = decode_object_semantic_graph(&object)
        .expect_err("unknown semantic node role must not decode");
    assert!(
        err.contains("unknown role"),
        "unknown semantic role must fail loudly"
    );
}

#[test]
fn decode_object_semantic_graph_rejects_empty_node_section() {
    let object = serialize_vyrecob2(&[
        (SectionTag::SemanticProgramGraphNodes, &[]),
        (SectionTag::SemanticProgramGraphEdges, &[]),
    ])
    .expect("Fix: semantic graph fixture must serialize");
    let err = decode_object_semantic_graph(&object)
        .expect_err("empty semantic node section must not decode");
    assert!(
        err.contains("semantic node section is empty"),
        "empty semantic nodes must fail loudly"
    );
}

#[test]
fn decode_object_ast_reads_framed_windows() {
    let ast_words = [11u32, u32::MAX, u32::MAX, 4];
    let root_words = [0u32, u32::MAX];
    let mut ast_section = Vec::new();
    ast_section.extend_from_slice(b"VYRAST1\0");
    ast_section.extend_from_slice(&32u32.to_le_bytes());
    ast_section.extend_from_slice(&64u32.to_le_bytes());
    ast_section.extend_from_slice(&(ast_words.len() as u32).to_le_bytes());
    ast_section.extend_from_slice(&(root_words.len() as u32).to_le_bytes());
    ast_section.extend(ast_words.iter().flat_map(|word| word.to_le_bytes()));
    ast_section.extend(root_words.iter().flat_map(|word| word.to_le_bytes()));
    let object = serialize_vyrecob2(&[(SectionTag::Ast, ast_section.as_slice())])
        .expect("Fix: AST fixture must serialize");

    let ast = decode_object_ast(&object).expect("Fix: AST fixture must decode");
    assert_eq!(ast.windows.len(), 1);
    assert_eq!(ast.ast_node_count, 1);
    assert_eq!(ast.windows[0].token_start, 32);
    assert_eq!(ast.windows[0].token_count, 64);
    assert_eq!(ast.windows[0].ast_words, ast_words);
    assert_eq!(ast.windows[0].root_words, root_words);
}

#[test]
fn decode_object_ast_rejects_header_only_section() {
    let object = serialize_vyrecob2(&[(SectionTag::Ast, b"VYRAST1\0".as_slice())])
        .expect("Fix: AST fixture must serialize");
    let err = decode_object_ast(&object).expect_err("header-only AST must not decode");
    assert!(
        err.contains("contains no VYRAST1 windows"),
        "header-only AST evidence must fail loudly"
    );
}

#[test]
fn decode_object_ast_rejects_rootless_window() {
    let ast_words = [11u32, u32::MAX, u32::MAX, 4];
    let mut ast_section = Vec::new();
    ast_section.extend_from_slice(b"VYRAST1\0");
    ast_section.extend_from_slice(&32u32.to_le_bytes());
    ast_section.extend_from_slice(&64u32.to_le_bytes());
    ast_section.extend_from_slice(&(ast_words.len() as u32).to_le_bytes());
    ast_section.extend_from_slice(&0u32.to_le_bytes());
    ast_section.extend(ast_words.iter().flat_map(|word| word.to_le_bytes()));
    let object = serialize_vyrecob2(&[(SectionTag::Ast, ast_section.as_slice())])
        .expect("Fix: AST fixture must serialize");
    let err = decode_object_ast(&object).expect_err("rootless AST window must not decode");
    assert!(
        err.contains("has no root entries"),
        "rootless AST window must fail loudly"
    );
}

#[test]
fn decode_object_sema_scope_reads_scope_declaration_identifier_rows() {
    let scope_words = [7u32, u32::MAX, 2, 99, 11, 4, 8, 7, 0, 0, 15, 1];
    let scope_bytes = scope_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[(SectionTag::SemaScope, scope_bytes.as_slice())])
        .expect("Fix: sema scope fixture must serialize");

    let scope = decode_object_sema_scope(&object).expect("Fix: sema scope fixture must decode");
    assert_eq!(scope.records.len(), 2);
    assert_eq!(scope.records[0].scope_id, 7);
    assert_eq!(scope.records[0].parent_scope_id, u32::MAX);
    assert_eq!(scope.records[0].decl_kind, 2);
    assert_eq!(scope.records[0].identifier_id, 99);
    assert_eq!(scope.records[0].token_start, 11);
    assert_eq!(scope.records[0].token_len, 4);
    assert_eq!(scope.declaration_rows, 1);
    assert_eq!(scope.identifier_rows, 1);
}

#[test]
fn decode_object_sema_scope_rejects_missing_parent_scope() {
    let scope_words = [7u32, 3, 2, 99, 11, 4];
    let scope_bytes = scope_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[(SectionTag::SemaScope, scope_bytes.as_slice())])
        .expect("Fix: sema scope fixture must serialize");
    let err = decode_object_sema_scope(&object).expect_err("missing parent scope must not decode");
    assert!(
        err.contains("references missing parent scope 3"),
        "missing semantic parent scope must fail loudly"
    );
}

#[test]
fn decode_object_sema_scope_accepts_multiple_token_rows_per_scope() {
    let scope_words = [7u32, u32::MAX, 0, 0, 0, 0, 7, u32::MAX, 0, 0, 0, 0];
    let scope_bytes = scope_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[(SectionTag::SemaScope, scope_bytes.as_slice())])
        .expect("Fix: sema scope fixture must serialize");
    let scope = decode_object_sema_scope(&object)
        .expect("Fix: multiple token rows in one scope must decode");
    assert_eq!(scope.records.len(), 2);
    assert!(scope.records.iter().all(|record| record.scope_id == 7));
}

#[test]
fn decode_object_sema_scope_rejects_multiple_roots() {
    let scope_words = [7u32, u32::MAX, 0, 0, 0, 0, 8, u32::MAX, 0, 0, 0, 0];
    let scope_bytes = scope_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[(SectionTag::SemaScope, scope_bytes.as_slice())])
        .expect("Fix: sema scope fixture must serialize");
    let err = decode_object_sema_scope(&object).expect_err("multiple root scopes must not decode");
    assert!(
        err.contains("root scope rows"),
        "multiple semantic scope roots must fail loudly"
    );
}

#[test]
fn decode_object_sema_scope_rejects_unknown_decl_kind() {
    let scope_words = [7u32, u32::MAX, u32::MAX - 1, 99, 11, 4];
    let scope_bytes = scope_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>();
    let object = serialize_vyrecob2(&[(SectionTag::SemaScope, scope_bytes.as_slice())])
        .expect("Fix: sema scope fixture must serialize");
    let err =
        decode_object_sema_scope(&object).expect_err("unknown declaration kind must not decode");
    assert!(
        err.contains("unknown declaration kind"),
        "unknown declaration kind must fail loudly"
    );
}

#[test]
fn decode_object_sema_scope_rejects_empty_section() {
    let object = serialize_vyrecob2(&[(SectionTag::SemaScope, &[])])
        .expect("Fix: sema scope fixture must serialize");
    let err =
        decode_object_sema_scope(&object).expect_err("empty SemaScope section must not decode");
    assert!(
        err.contains("SemaScope section is empty"),
        "empty semantic scope evidence must fail loudly"
    );
}
