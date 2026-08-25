//! Explicit CPU oracle for VAST kind classification.
//!
//! Production classification must use the dispatchable `c11_classify_*`
//! builders. This module exists for parity witnesses, corpus diagnosis, and
//! conformance fixtures.

#![allow(missing_docs)] // Internal oracle helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;

use super::expr_shape::*;
use super::ref_decode_err::*;
use super::ref_typedef::*;
use super::*;

#[deprecated(
    note = "CPU oracle only; production VAST classification must dispatch c11_classify_vast_node_kinds* builders"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_c11_classify_vast_node_kinds(
    vast_node_bytes: &[u8],
) -> Result<Vec<u8>, CReferenceDecodeError> {
    let raw_vast_nodes = try_vast_words_from_bytes(vast_node_bytes)?;
    Ok(reference_c11_classify_vast_node_kinds_from_words(
        &raw_vast_nodes,
    ))
}

/// CPU oracle for `c11_classify_vast_node_kinds`.
#[must_use]
#[deprecated(
    note = "CPU oracle only; production VAST classification must dispatch c11_classify_vast_node_kinds* builders"
)]
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c11_classify_vast_node_kinds(vast_node_bytes: &[u8]) -> Vec<u8> {
    try_reference_c11_classify_vast_node_kinds(vast_node_bytes).unwrap_or_else(|error| {
        panic!("C VAST classify reference oracle received malformed input: {error}")
    })
}

fn reference_c11_classify_vast_node_kinds_from_words(raw_vast_nodes: &[u32]) -> Vec<u8> {
    let mut typed_vast_nodes = raw_vast_nodes.to_vec();
    let node_count = raw_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;

    for node_idx in 0..node_count {
        let base = node_idx * VAST_NODE_STRIDE_U32 as usize;
        let typed_kind = reference_typed_kind(raw_vast_nodes, node_idx);
        typed_vast_nodes[base] = typed_kind;
        if let Some(parent) =
            reference_declarator_parent_override(raw_vast_nodes, node_idx, typed_kind)
        {
            typed_vast_nodes[base + 1] = parent;
        }
    }

    u32_words_to_bytes(&typed_vast_nodes)
}

fn reference_declarator_parent_override(
    vast_nodes: &[u32],
    node_idx: usize,
    typed_kind: u32,
) -> Option<u32> {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    match typed_kind {
        C_AST_KIND_POINTER_DECL => None,
        C_AST_KIND_ARRAY_DECL => {
            let prev_idx = previous_sibling_idx(vast_nodes, node_idx)?;
            if kind_at(vast_nodes, prev_idx) != TOK_LPAREN {
                return None;
            }
            let first_child = vast_nodes
                .get(prev_idx * VAST_NODE_STRIDE_U32 as usize + 2)
                .copied()
                .and_then(|idx| usize::try_from(idx).ok())
                .filter(|idx| *idx < node_count)?;
            (kind_at(vast_nodes, first_child) == TOK_STAR).then_some(first_child as u32)
        }
        _ => None,
    }
}

fn previous_sibling_idx(vast_nodes: &[u32], node_idx: usize) -> Option<usize> {
    let direct = vast_nodes
        .get(node_idx * VAST_NODE_STRIDE_U32 as usize + VAST_PREVIOUS_SIBLING_FIELD as usize)
        .copied()
        .unwrap_or(SENTINEL);
    if let Ok(idx) = usize::try_from(direct) {
        if idx < node_idx {
            return Some(idx);
        }
    }
    let parent = vast_nodes
        .get(node_idx * VAST_NODE_STRIDE_U32 as usize + 1)
        .copied()
        .unwrap_or(SENTINEL);
    (0..node_idx).rev().find(|scan_idx| {
        vast_nodes
            .get(scan_idx * VAST_NODE_STRIDE_U32 as usize + 1)
            .copied()
            .unwrap_or(SENTINEL)
            == parent
    })
}
