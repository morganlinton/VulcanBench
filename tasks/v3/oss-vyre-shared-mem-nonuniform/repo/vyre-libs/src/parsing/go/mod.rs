//! Go 1.21 parser modules  -  lexer + structural extraction passes.

/// Byte-oriented GPU lexer for the Go frontend.
pub mod lex;
/// Structural extraction passes for Go declarations and AST-shaped ops.
pub mod parse;
