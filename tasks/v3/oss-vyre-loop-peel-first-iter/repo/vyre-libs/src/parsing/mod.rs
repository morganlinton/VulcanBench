//! The `vyre-libs` parsing and AST building domain library.
//!
//! Exposes registered `OpEntry` definitions for structural analysis and
//! full-grammar Shunting-Yard AST generation entirely on GPU.
//!
//! Architected as disjoint, language-isolated registered passes:
//!
//! - `core`  -  substrate-neutral parsing primitives (AST node kinds,
//!   delimiter handling, grammar table walkers).
//! - `c`  -  C11 pipeline: lex / preprocess / parse / sema / lower.
//!   Feature-gated behind `c-parser`.
//! - `python`  -  Python 3.12 sparse lex + structural extraction.
//!   Feature-gated behind `python-parser`.

/// Substrate-neutral parsing primitives (AST, delimiter, grammar).
pub mod core;

/// Content-hash LRU cache for parsed source artifacts. ROADMAP L2 / E2
/// substrate; language-specific parse pipelines opt in via
/// `ParsedSourceLru::get_or_parse`.
pub mod source_cache;

/// Parallel corpus parse on top of the L2 LRU cache. ROADMAP L3
/// substrate; fans `get_or_parse` across cores with `rayon` while
/// preserving input order.
pub mod parallel_parse;

pub(crate) mod composition;

/// Precomputed LR action/goto tables and CPU reference parser.
pub mod lr_tables;

/// Packed AST (VAST) wire + host walks  -  re-export from `vyre-foundation`.
pub mod vast;

/// C11 pipeline (lex / preprocess / parse / sema / lower).
#[cfg(feature = "c-parser")]
pub mod c;

/// Go 1.21 pipeline (lex / structural parse / AST ops).
#[cfg(feature = "go-parser")]
pub mod go;

/// Python 3.12 pipeline (lex / structural parse / AST ops).
#[cfg(feature = "python-parser")]
pub mod python;

/// Rust pipeline (lex / parse / typeck / borrow).
#[cfg(feature = "rust-parser")]
pub mod rust;
