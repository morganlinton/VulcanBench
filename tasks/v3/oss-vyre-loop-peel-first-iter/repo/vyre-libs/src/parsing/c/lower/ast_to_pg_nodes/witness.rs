//! Witness fixtures for AST→PG lowering tests.

#![allow(deprecated)]

use crate::parsing::c::parse::vast::*;

use super::reference::*;
use super::*;

fn append_vast_node(
    out: &mut Vec<u32>,
    kind: u32,
    parent_idx: u32,
    first_child_idx: u32,
    next_sibling_idx: u32,
    span_start: u32,
    span_len: u32,
) {
    out.extend_from_slice(&[
        kind,
        parent_idx,
        first_child_idx,
        next_sibling_idx,
        u32::MAX,
        span_start,
        span_len,
        kind.rotate_left(5),
        span_len,
        IDX_RESERVED as u32,
    ]);
}

fn witness_nodes() -> Vec<u32> {
    let mut vast_nodes = Vec::new();
    append_vast_node(
        &mut vast_nodes,
        node_kind::VARIABLE,
        u32::MAX,
        u32::MAX,
        1,
        0,
        11,
    );
    append_vast_node(&mut vast_nodes, node_kind::CALL, 0, 2, 5, 16, 9);
    append_vast_node(&mut vast_nodes, node_kind::LITERAL, 1, u32::MAX, 3, 32, 7);
    append_vast_node(&mut vast_nodes, node_kind::IMPORT, 1, 4, u32::MAX, 48, 13);
    append_vast_node(
        &mut vast_nodes,
        node_kind::SSA,
        3,
        u32::MAX,
        u32::MAX,
        62,
        3,
    );
    append_vast_node(
        &mut vast_nodes,
        node_kind::BASIC_BLOCK,
        u32::MAX,
        u32::MAX,
        u32::MAX,
        96,
        17,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_LABEL_STMT,
        5,
        u32::MAX,
        7,
        128,
        5,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_GNU_STATEMENT_EXPR,
        5,
        8,
        9,
        136,
        19,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_ASM_QUALIFIER,
        8,
        u32::MAX,
        10,
        160,
        8,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_ATTRIBUTE_FALLTHROUGH,
        5,
        u32::MAX,
        11,
        176,
        11,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_BUILTIN_EXPECT_EXPR,
        7,
        u32::MAX,
        12,
        192,
        16,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_SWITCH_STMT,
        5,
        u32::MAX,
        12,
        224,
        6,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_CASE_STMT,
        11,
        u32::MAX,
        13,
        240,
        4,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_DEFAULT_STMT,
        11,
        u32::MAX,
        14,
        248,
        7,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_FOR_STMT,
        5,
        u32::MAX,
        15,
        264,
        3,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_WHILE_STMT,
        5,
        u32::MAX,
        16,
        272,
        5,
    );
    append_vast_node(&mut vast_nodes, C_AST_KIND_DO_STMT, 5, u32::MAX, 17, 280, 2);
    append_vast_node(&mut vast_nodes, C_AST_KIND_IF_STMT, 5, u32::MAX, 18, 288, 2);
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_GOTO_STMT,
        5,
        u32::MAX,
        19,
        296,
        4,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_BREAK_STMT,
        5,
        u32::MAX,
        20,
        304,
        5,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_CONTINUE_STMT,
        5,
        u32::MAX,
        21,
        312,
        8,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_RETURN_STMT,
        5,
        u32::MAX,
        22,
        328,
        6,
    );
    append_vast_node(
        &mut vast_nodes,
        C_AST_KIND_CAST_EXPR,
        5,
        u32::MAX,
        u32::MAX,
        344,
        1,
    );
    vast_nodes
}

fn witness_node_count() -> u32 {
    u32::try_from(witness_nodes().len() / VAST_NODE_STRIDE_U32 as usize).unwrap_or(u32::MAX)
}

fn witness_inputs() -> Vec<Vec<Vec<u8>>> {
    let nodes = witness_nodes();
    vec![vec![
        u32_words_to_bytes(&nodes),
        vec![0; witness_node_count() as usize * PG_NODE_STRIDE_U32 as usize * 4],
    ]]
}

fn semantic_witness_inputs() -> Vec<Vec<Vec<u8>>> {
    let nodes = witness_nodes();
    let node_count = witness_node_count() as usize;
    vec![vec![
        u32_words_to_bytes(&nodes),
        vec![0; node_count * C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize * 4],
        vec![
            0;
            node_count
                * C_AST_PG_EDGE_ROWS_PER_NODE as usize
                * C_AST_PG_EDGE_STRIDE_U32 as usize
                * 4
        ],
    ]]
}

fn witness_expected() -> Vec<Vec<Vec<u8>>> {
    witness_inputs()
        .into_iter()
        .map(|input| vec![reference_ast_to_pg_nodes(&input[0])])
        .collect()
}

fn semantic_witness_expected() -> Vec<Vec<Vec<u8>>> {
    semantic_witness_inputs()
        .into_iter()
        .map(|input| {
            let semantic = reference_ast_to_pg_semantic_graph(&input[0]);
            vec![semantic.nodes, semantic.edges]
        })
        .collect()
}

inventory::submit! {
    OpEntry::new(
        OP_ID,
        || c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(witness_node_count()), "out_pg_nodes"),
        Some(witness_inputs),
        Some(witness_expected),
    )
}

inventory::submit! {
    OpEntry::new(
        SEMANTIC_OP_ID,
        || c_lower_ast_to_pg_semantic_graph(
            "vast_nodes",
            Expr::u32(witness_node_count()),
            "out_pg_nodes",
            "out_pg_edges",
        ),
        Some(semantic_witness_inputs),
        Some(semantic_witness_expected),
    )
}
