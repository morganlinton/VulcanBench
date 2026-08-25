#![allow(missing_docs)]
mod block_totals;

use crate::parsing::c::lex::tokens::*;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub use block_totals::c11_lexer_regular_sparse_packed_haystack_with_block_totals;

use super::helpers::{
    byte_at_or_zero, byte_eq, is_digit, is_ident_continue, is_ident_start, set_token,
};

mod bounds;
mod entrypoints;
mod sparse_impl;

pub use entrypoints::{
    c11_lexer_regular_sparse_no_directives_no_backscan,
    c11_lexer_regular_sparse_packed_haystack_with_flags,
    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives,
    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives_no_backscan,
    c11_lexer_regular_sparse_u8_haystack_with_flags,
};

use bounds::*;
use sparse_impl::c11_lexer_regular_sparse_impl;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SparseHaystackLayout {
    PackedU32,
    ExpandedU32,
    RawU8,
}
