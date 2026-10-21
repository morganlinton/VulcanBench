//! Python 3.12 structural frontend.
//!
//! The frontend keeps the runtime path GPU-native: the lexer and every
//! structural extractor return `vyre::Program` kernels over raw source
//! bytes. Tests may use host helpers for reference expectations, but the
//! shipping path never links a CPU parser.

/// Python byte lexer.
pub mod lex;
/// Python structural extractors.
pub mod parse;
/// Content-keyed pipeline Program cache.
pub mod source_cache;

#[cfg(test)]
mod tests;

pub(crate) const INVALID_POS: u32 = u32::MAX;

pub(crate) const MAX_DOTTED_SEGMENTS: u32 = 8;

pub(crate) const DEF_RECORD_WORDS: u32 = 6;
pub(crate) const IMPORT_RECORD_WORDS: u32 = 6;
pub(crate) const WITH_RECORD_WORDS: u32 = 6;
pub(crate) const CALL_RECORD_WORDS: u32 = 7;
pub(crate) const KWARG_RECORD_WORDS: u32 = 2;
pub(crate) const DECORATOR_RECORD_WORDS: u32 = 6;
