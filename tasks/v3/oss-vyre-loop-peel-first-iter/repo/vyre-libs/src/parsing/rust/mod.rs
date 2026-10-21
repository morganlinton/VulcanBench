//! Rust pipeline modules: lex / parse / sema / lower.
//!
//! Reusable substrate (Tier 3). The lexer, parser, semantic analysis (name
//! resolution, type inference, borrow checking), and lowering to Vyre IR all
//! live here so any consumer gets typed Rust analysis without depending on the
//! `vyre-frontend-rust` driver crate. This mirrors `parsing::c`. The driver
//! owns only orchestration, object/ELF emission, GPU dispatch, and the CLI.

/// DFA lexer pipeline (tokens, keywords, lexer kernels).
pub mod lex;
/// Rust AST to Vyre IR lowering.
pub mod lower;
/// Nano-subset structural parser.
pub mod parse;
/// Semantic analysis: name resolution, type inference, borrow checking.
pub mod sema;
