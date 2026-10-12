//! GPU typedef-name annotation and symbol-linking builders.

#![allow(missing_docs)] // Internal VAST-builder helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;
use crate::parsing::c::source_bytes::{load_source_byte, source_haystack_words};
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::build::*;
use super::helpers::*;
use super::*;

mod annotate;
mod annotation_helpers;
mod common;
mod decl_contexts;
mod global_fast;
mod prehash;
mod scopes;
mod symbol_links;

pub use annotate::{
    c11_annotate_typedef_names, c11_annotate_typedef_names_packed_haystack,
    c11_annotate_typedef_names_precomputed_context,
    c11_annotate_typedef_names_precomputed_context_packed_haystack,
    c11_annotate_typedef_names_precomputed_scope,
    c11_annotate_typedef_names_precomputed_scope_packed_haystack,
};
pub use annotation_helpers::c11_precompute_vast_decl_prefix_starts;
pub use decl_contexts::c11_precompute_vast_decl_contexts;
pub use global_fast::c11_annotate_global_typedef_names_fast;
pub use prehash::{c11_prehash_vast_identifiers, c11_prehash_vast_identifiers_packed_haystack};
pub use scopes::{c11_precompute_vast_scopes, c11_precompute_vast_scopes_uses_global_stack};
pub use symbol_links::c11_link_vast_typedef_symbols;

use common::*;
