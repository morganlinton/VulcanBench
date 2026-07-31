//! GPU macro-expansion program builders.
//!
//! Each expansion pass has one file under `expansion/`, while this module
//! owns the shared ABI constants and the public pass exports. Keep helper code
//! in the pass that uses it unless it is shared by multiple active builders.

use vyre::ir::{DataType, Expr};

pub(crate) const EMPTY_MACRO_SLOT: u32 = u32::MAX;
pub(crate) const MACRO_TABLE_SLOTS: u32 = 4_096;
pub(crate) const MACRO_TABLE_MASK: u32 = MACRO_TABLE_SLOTS - 1;

/// Object-like C macro table kind for `opt_named_macro_expansion`.
pub const C_MACRO_KIND_OBJECT_LIKE: u32 = 0;
/// Function-like C macro table kind for `opt_named_macro_expansion`.
pub const C_MACRO_KIND_FUNCTION_LIKE: u32 = 1;
/// Replacement parameter marker meaning this replacement token is literal.
pub const C_MACRO_REPLACEMENT_LITERAL: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MacroByteLayout {
    ExpandedU32,
    RawU8,
}

pub(crate) fn load_macro_byte(buffer: &str, layout: MacroByteLayout, addr: Expr) -> Expr {
    let loaded = Expr::load(buffer, addr);
    let as_u32 = match layout {
        MacroByteLayout::ExpandedU32 => loaded,
        MacroByteLayout::RawU8 => Expr::cast(DataType::U32, loaded),
    };
    Expr::bitand(as_u32, Expr::u32(0xff))
}

mod arg_scan;
mod conditional;
mod dynamic_pass;
mod fnlike;
mod fnlike_mat;
mod helpers;
mod named;
mod named_mat;
mod objlike;
mod objlike_mat;
mod paste_branch;
mod regular_branch;
mod string_branch;

pub use conditional::{opt_conditional_mask, opt_conditional_mask_with_directives};
pub use dynamic_pass::opt_dynamic_macro_expansion;
pub use named::opt_named_macro_expansion;
pub use named_mat::opt_named_macro_expansion_materialized;

// Re-exports so sibling modules can `use super::*` for the active helper
// surface. Keep this small: shared means used by multiple active builders.
