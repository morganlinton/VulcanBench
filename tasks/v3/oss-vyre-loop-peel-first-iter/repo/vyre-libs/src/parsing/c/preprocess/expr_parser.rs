//! C preprocessor `#if` / `#elif` expression parser, extracted from
//! `mod.rs` so the file stays under the 500-LOC source cap.

use super::{is_c_ident_start, is_directive_ident_continue, macro_is_defined, CPreprocessorError};
mod cursor;
mod entry;
mod literals;
mod model;
mod operators;
mod precedence;
#[cfg(test)]
mod tests;

pub(super) use model::{PreprocessorExprParser, MAX_PP_EXPR_DEPTH};
pub use operators::is_reserved_preprocessor_identifier;
