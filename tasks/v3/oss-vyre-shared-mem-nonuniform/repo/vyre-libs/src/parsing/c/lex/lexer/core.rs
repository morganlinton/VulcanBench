//! `c11_lexer` builder. ~780 LOC of `Vec<Node>` accumulation  -  over the
//! 500-LOC source cap because the lexer body is one logical block whose
//! sub-sections share closure state (next_byte, byte references, etc).
//! Extracting those into helpers would change parser semantics, so this
//! file is allowed to exceed the cap (CONVENTIONS.md `KNOWN_OVER_CAP`).

#![allow(missing_docs)] // Internal lexer-builder helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::helpers::{
    ascii, byte_at_or_zero, byte_eq, byte_load, is_digit, is_ident_continue, is_ident_start,
    is_valid_escape_byte, set_token,
};

mod dense;
mod helpers;
mod parallel_common;
mod ranked;
mod scan_bounds;
mod sparse;

pub use dense::c11_lexer;
pub use helpers::c11_lexer_regular;
pub use ranked::c11_lexer_regular_ranked;
pub use sparse::c11_lexer_regular_sparse;

use scan_bounds::*;
