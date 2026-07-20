//! Explicit CPU oracle for C typedef-name annotation.
//!
//! Production typedef annotation must use the dispatchable
//! `c11_annotate_typedef_names*` builders. This module is retained for
//! conformance witnesses and parity fixtures only.

#![allow(missing_docs)] // Internal oracle helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;

use super::expr_shape::*;
use super::ref_decode_err::*;
use super::*;

mod annotator;
mod asm_attributes;
mod decl_context;
mod declarations;
mod expressions;
mod identifiers;
mod scopes;
mod typed_kind;

use annotator::*;
use asm_attributes::*;
use decl_context::*;
use declarations::*;
use expressions::*;
use identifiers::*;
use scopes::*;

pub(super) fn vast_field_at(vast_nodes: &[u32], node_idx: usize, field_idx: usize) -> u32 {
    c_vast_word_at(vast_nodes, node_idx, field_idx)
}

pub(super) fn parent_at(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field_at(vast_nodes, node_idx, 1)
}

pub(super) fn first_child_at(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field_at(vast_nodes, node_idx, 2)
}

pub(super) fn next_sibling_at(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field_at(vast_nodes, node_idx, 3)
}

pub(super) fn flags_at(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field_at(vast_nodes, node_idx, VAST_TYPEDEF_FLAGS_FIELD as usize)
}

pub(super) fn kind_at(vast_nodes: &[u32], node_idx: usize) -> u32 {
    expressions::kind_at_impl(vast_nodes, node_idx)
}

pub(super) fn reference_typed_kind(vast_nodes: &[u32], node_idx: usize) -> u32 {
    typed_kind::reference_typed_kind(vast_nodes, node_idx)
}

#[deprecated(
    note = "CPU oracle only; production typedef annotation must dispatch c11_annotate_typedef_names* builders"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_reference_c11_annotate_typedef_names(
    vast_node_bytes: &[u8],
    haystack: &[u8],
) -> Result<Vec<u8>, CReferenceDecodeError> {
    let raw_vast_nodes = try_vast_words_from_bytes(vast_node_bytes)?;
    Ok(reference_c11_annotate_typedef_names_from_words(
        raw_vast_nodes,
        haystack,
    ))
}

/// CPU oracle for `c11_annotate_typedef_names`.
#[must_use]
#[deprecated(
    note = "CPU oracle only; production typedef annotation must dispatch c11_annotate_typedef_names* builders"
)]
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c11_annotate_typedef_names(vast_node_bytes: &[u8], haystack: &[u8]) -> Vec<u8> {
    try_reference_c11_annotate_typedef_names(vast_node_bytes, haystack).unwrap_or_else(|error| {
        panic!("C VAST typedef reference oracle received malformed input: {error}")
    })
}
