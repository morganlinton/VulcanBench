//! Go structural extraction passes.

/// Go-specific AST-shaped operations: goroutines, channels, defer.
pub mod ast_ops;
/// Declaration/package/import extraction.
pub mod structure;
mod token_predicates;
