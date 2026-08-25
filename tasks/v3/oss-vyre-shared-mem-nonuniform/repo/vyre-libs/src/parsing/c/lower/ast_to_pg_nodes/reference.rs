//! Explicit CPU oracle paths for AST→PG lowering.
//!
//! These functions exist for parity tests, fixtures, and offline corpus
//! evidence. Production lowering must use the dispatchable GPU builders in
//! `gpu_program.rs`.

use crate::parsing::c::lower::semantic_edges::*;
use crate::parsing::c::parse::vast::c_vast_word_at;

use super::semantic::*;
use super::*;

fn try_u32_words_from_bytes(bytes: &[u8]) -> Result<Vec<u32>, PgReferenceDecodeError> {
    if bytes.len() % 4 != 0 {
        return Err(PgReferenceDecodeError::MisalignedBytes { len: bytes.len() });
    }
    Ok(vyre_primitives::wire::decode_u32_le_bytes_all(bytes))
}

pub(super) fn u32_words_to_bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

/// Compute the same mapping as `c_lower_ast_to_pg_nodes` in the explicit CPU
/// oracle for tests and fixtures.
///
/// # Errors
///
/// Returns [`PgReferenceDecodeError`] when the input is not aligned to `u32`
/// words or does not contain complete VAST rows.
#[deprecated(
    note = "CPU oracle only; production AST-to-PG lowering must dispatch c_lower_ast_to_pg_nodes"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_ast_to_pg_nodes(
    vast_node_bytes: &[u8],
) -> Result<Vec<u8>, PgReferenceDecodeError> {
    let vast_nodes = try_u32_words_from_bytes(vast_node_bytes)?;
    if vast_nodes.len() % VAST_NODE_STRIDE_U32 as usize != 0 {
        return Err(PgReferenceDecodeError::PartialVastRow {
            words: vast_nodes.len(),
            stride: VAST_NODE_STRIDE_U32 as usize,
        });
    }
    Ok(reference_ast_to_pg_nodes_from_words(&vast_nodes))
}

/// Compute semantic PG node and edge witnesses in the explicit CPU oracle.
///
/// # Errors
///
/// Returns [`PgReferenceDecodeError`] when the input is not aligned to `u32`
/// words or does not contain complete VAST rows.
#[deprecated(
    note = "CPU oracle only; production semantic AST-to-PG lowering must dispatch c_lower_ast_to_pg_semantic_graph"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_ast_to_pg_semantic_graph(
    vast_node_bytes: &[u8],
) -> Result<SemanticPgReference, PgReferenceDecodeError> {
    let vast_nodes = try_u32_words_from_bytes(vast_node_bytes)?;
    if vast_nodes.len() % VAST_NODE_STRIDE_U32 as usize != 0 {
        return Err(PgReferenceDecodeError::PartialVastRow {
            words: vast_nodes.len(),
            stride: VAST_NODE_STRIDE_U32 as usize,
        });
    }
    Ok(reference_ast_to_pg_semantic_graph_from_words(&vast_nodes))
}

/// Compute the same mapping as `c_lower_ast_to_pg_nodes` in the explicit CPU oracle.
#[must_use]
#[deprecated(
    note = "CPU oracle only; production AST-to-PG lowering must dispatch c_lower_ast_to_pg_nodes"
)]
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_ast_to_pg_nodes(vast_node_bytes: &[u8]) -> Vec<u8> {
    try_reference_ast_to_pg_nodes(vast_node_bytes).unwrap_or_else(|_| {
        unreachable!("reference_ast_to_pg_nodes requires u32-aligned complete VAST rows")
    })
}

/// Compute semantic PG node and edge witnesses in the explicit CPU oracle.
#[must_use]
#[deprecated(
    note = "CPU oracle only; production semantic AST-to-PG lowering must dispatch c_lower_ast_to_pg_semantic_graph"
)]
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_ast_to_pg_semantic_graph(vast_node_bytes: &[u8]) -> SemanticPgReference {
    try_reference_ast_to_pg_semantic_graph(vast_node_bytes).unwrap_or_else(|_| {
        unreachable!("reference_ast_to_pg_semantic_graph requires u32-aligned complete VAST rows")
    })
}

fn vast_field(vast_nodes: &[u32], node_idx: usize, field_idx: usize) -> u32 {
    c_vast_word_at(vast_nodes, node_idx, field_idx)
}

fn reference_ast_to_pg_nodes_from_words(vast_nodes: &[u32]) -> Vec<u8> {
    let mut out_nodes = Vec::with_capacity(
        vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize * PG_NODE_STRIDE_U32 as usize,
    );

    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    for node_idx in 0..node_count {
        let kind = vast_field(vast_nodes, node_idx, IDX_KIND);
        let parent_idx = vast_field(vast_nodes, node_idx, IDX_PARENT);
        let first_child_idx = vast_field(vast_nodes, node_idx, IDX_FIRST_CHILD);
        let next_sibling_idx = vast_field(vast_nodes, node_idx, IDX_NEXT_SIBLING);
        let span_start = vast_field(vast_nodes, node_idx, IDX_SRC_BYTE_OFF);
        let span_len = vast_field(vast_nodes, node_idx, IDX_SRC_BYTE_LEN);
        let span_end = span_start.wrapping_add(span_len);

        out_nodes.push(kind);
        out_nodes.push(span_start);
        out_nodes.push(span_end);
        out_nodes.push(parent_idx);
        out_nodes.push(first_child_idx);
        out_nodes.push(next_sibling_idx);
    }

    u32_words_to_bytes(&out_nodes)
}

fn reference_ast_to_pg_semantic_graph_from_words(vast_nodes: &[u32]) -> SemanticPgReference {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut nodes = Vec::with_capacity(node_count * C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize);
    let mut edges = Vec::with_capacity(
        node_count * C_AST_PG_EDGE_ROWS_PER_NODE as usize * C_AST_PG_EDGE_STRIDE_U32 as usize,
    );

    for node_idx in 0..node_count {
        let kind = vast_field(vast_nodes, node_idx, IDX_KIND);
        let parent_idx = vast_field(vast_nodes, node_idx, IDX_PARENT);
        let first_child_idx = vast_field(vast_nodes, node_idx, IDX_FIRST_CHILD);
        let next_sibling_idx = vast_field(vast_nodes, node_idx, IDX_NEXT_SIBLING);
        let span_start = vast_field(vast_nodes, node_idx, IDX_SRC_BYTE_OFF);
        let span_len = vast_field(vast_nodes, node_idx, IDX_SRC_BYTE_LEN);
        let attr_off = vast_field(vast_nodes, node_idx, IDX_ATTR_OFF);
        let attr_len = vast_field(vast_nodes, node_idx, IDX_ATTR_LEN);
        let semantic_category = semantic_category(kind);
        let parent_kind = related_kind(vast_nodes, parent_idx, node_count);
        let first_child_kind = related_kind(vast_nodes, first_child_idx, node_count);
        let next_sibling_kind = related_kind(vast_nodes, next_sibling_idx, node_count);
        let semantic_role = semantic_role(kind, parent_kind, first_child_kind, next_sibling_kind);

        nodes.extend_from_slice(&[
            kind,
            span_start,
            span_start.wrapping_add(span_len),
            parent_idx,
            first_child_idx,
            next_sibling_idx,
            semantic_category,
            semantic_role,
            attr_off,
            attr_len,
        ]);

        let has_parent = valid_node_ref(parent_idx, node_count);
        let has_first_child = valid_node_ref(first_child_idx, node_count);
        let has_next_sibling = valid_node_ref(next_sibling_idx, node_count);
        append_edge_row(
            &mut edges,
            has_parent,
            C_AST_PG_EDGE_PARENT,
            parent_idx,
            node_idx as u32,
            kind,
            semantic_role,
            semantic_category,
        );
        append_edge_row(
            &mut edges,
            has_first_child,
            C_AST_PG_EDGE_FIRST_CHILD,
            node_idx as u32,
            first_child_idx,
            kind,
            semantic_role,
            semantic_category,
        );
        append_edge_row(
            &mut edges,
            has_next_sibling,
            C_AST_PG_EDGE_NEXT_SIBLING,
            node_idx as u32,
            next_sibling_idx,
            kind,
            semantic_role,
            semantic_category,
        );
        let (edge3, edge4) = resolved_semantic_edges(vast_nodes, node_idx, node_count, kind);
        append_edge_row(
            &mut edges,
            edge3.kind != C_AST_PG_EDGE_NONE,
            edge3.kind,
            edge3.src,
            edge3.dst,
            kind,
            semantic_role,
            semantic_category,
        );
        append_edge_row(
            &mut edges,
            edge4.kind != C_AST_PG_EDGE_NONE,
            edge4.kind,
            edge4.src,
            edge4.dst,
            kind,
            semantic_role,
            semantic_category,
        );
    }

    SemanticPgReference {
        nodes: u32_words_to_bytes(&nodes),
        edges: u32_words_to_bytes(&edges),
    }
}

fn append_edge_row(
    out: &mut Vec<u32>,
    has_edge: bool,
    edge_kind: u32,
    src_idx: u32,
    dst_idx: u32,
    ast_kind: u32,
    semantic_role: u32,
    semantic_category: u32,
) {
    out.extend_from_slice(&[
        if has_edge {
            edge_kind
        } else {
            C_AST_PG_EDGE_NONE
        },
        if has_edge { src_idx } else { u32::MAX },
        if has_edge { dst_idx } else { u32::MAX },
        ast_kind,
        semantic_role,
        semantic_category,
    ]);
}

fn valid_node_ref(idx: u32, node_count: usize) -> bool {
    if idx == u32::MAX {
        return false;
    }
    match usize::try_from(idx) {
        Ok(idx) => idx < node_count,
        Err(_) => false,
    }
}

fn semantic_category(kind: u32) -> u32 {
    if GNU_KINDS.contains(&kind) {
        C_AST_PG_CATEGORY_GNU
    } else if DECLARATION_KINDS.contains(&kind) {
        C_AST_PG_CATEGORY_DECLARATION
    } else if EXPRESSION_KINDS.contains(&kind) {
        C_AST_PG_CATEGORY_EXPRESSION
    } else if CONTROL_KINDS.contains(&kind) {
        C_AST_PG_CATEGORY_CONTROL
    } else {
        C_AST_PG_CATEGORY_NONE
    }
}
