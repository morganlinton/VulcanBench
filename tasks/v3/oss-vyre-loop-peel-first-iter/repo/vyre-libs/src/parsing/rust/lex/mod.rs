//! GPU DFA lexer pipeline for Rust source text.
//!
//! Reuses `vyre-primitives::text` byte-classifiers and the sparse-dispatch
//! pattern proven in the C11 lexer.  The CPU reference (`core::lex`)
//! is validated token-for-token against `rustc_lexer`.

/// Post-lex keyword promotion.
pub mod keyword;
/// Lexer kernels (CPU reference + GPU plan builder).
pub mod lexer;
/// Token constants and predicates.
pub mod tokens;
