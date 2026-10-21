// Integration test module for the containing Vyre package.
#![allow(deprecated)]

use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

use vyre::ir::{Expr, Program};
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::parsing::c::lex::tokens::{
    TOK_ASSIGN, TOK_COLON, TOK_COMMA, TOK_IDENTIFIER, TOK_LBRACE, TOK_LBRACKET, TOK_LPAREN,
    TOK_RBRACE, TOK_RBRACKET, TOK_RPAREN, TOK_SEMICOLON, TOK_TYPEDEF,
};
use vyre_libs::parsing::c::lower::{
    c_lower_ast_to_pg_nodes, c_lower_ast_to_pg_semantic_graph, reference_ast_to_pg_nodes,
};
use vyre_libs::parsing::c::parse::vast::{
    c11_annotate_global_typedef_names_fast, c11_annotate_typedef_names,
    c11_annotate_typedef_names_precomputed_scope, c11_build_expression_shape_nodes,
    c11_build_vast_nodes, c11_classify_vast_node_kinds, c11_precompute_vast_scopes,
    c11_prehash_vast_identifiers, reference_c11_annotate_typedef_names,
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds,
};
use vyre_libs::parsing::c::sema::c_sema_scope;

pub(crate) const VAST_STRIDE_U32: usize = 10;
pub(crate) const PG_STRIDE_U32: usize = 6;
const VAST_STRIDE_BYTES: usize = VAST_STRIDE_U32 * core::mem::size_of::<u32>();
const VAST_TYPEDEF_SYMBOL_FIELD: usize = 9;

mod common;
pub(crate) use common::c_fixture::*;
mod gpu_dispatch_support;
mod gpu_pipeline_support;
mod row_buffer_support;
mod typedef_gpu_support;

pub(crate) use gpu_dispatch_support::*;
pub(crate) use gpu_pipeline_support::*;
pub(crate) use row_buffer_support::*;
pub(crate) use typedef_gpu_support::*;

fn bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

pub(crate) fn word_at(buf: &[u8], word: usize) -> u32 {
    let off = word * 4;
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

pub(crate) fn node_count_from_vast(buf: &[u8]) -> u32 {
    u32::try_from(buf.len() / VAST_STRIDE_BYTES).unwrap_or_default()
}

fn haystack_words(bytes: &[u8]) -> Vec<u8> {
    vyre_primitives::wire::pack_bytes_as_u32_slice(bytes)
}
