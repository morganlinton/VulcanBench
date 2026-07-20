//! GPU VAST structural-node construction builders.

#![allow(missing_docs)] // Internal VAST-builder helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;
use crate::parsing::c::source_bytes::load_source_byte;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::build_declaration_kind_inner::emit_declaration_kind_for_index_inner;
use super::helpers::*;
use super::*;

mod current_decl_annotation;
mod declaration_kind;
mod enclosing_function;
mod helpers;
mod identifier_hash;
mod scope_lookup;
mod structural_builder;
mod typedef_visibility;

pub(super) use current_decl_annotation::emit_current_declaration_annotation;
pub(super) use declaration_kind::{
    emit_builtin_declaration_kind_for_index, emit_declaration_kind_for_index,
};
pub(super) use enclosing_function::emit_enclosing_function_lparen_for_index;
pub(super) use helpers::{
    emit_declaration_kind_result_assignment, emit_identifier_source_hash_for_index,
    vast_bounded_row_kind_expr, vast_prior_row_kind_expr, vast_row_base_expr, vast_row_field_expr,
    vast_row_kind_expr, vast_row_kind_from_base_expr, vast_row_parent_from_base_expr,
};
pub(super) use identifier_hash::emit_identifier_hash_for_row;
pub(super) use scope_lookup::{emit_scope_open_for_index, emit_scope_open_scan_assign_for_index};
pub use structural_builder::{c11_build_vast_nodes, c11_build_vast_nodes_uses_global_last_child};
pub(super) use typedef_visibility::{
    emit_precomputed_declaration_kind_for_index, emit_typedef_visibility_scan,
    emit_typedef_visibility_scan_precomputed_context, emit_visible_typedef_name_for_index,
};
