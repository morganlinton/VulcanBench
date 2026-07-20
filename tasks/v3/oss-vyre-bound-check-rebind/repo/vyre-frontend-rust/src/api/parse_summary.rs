//! Result types for Rust frontend entry points.

/// Result of parsing a Rust source file.
#[derive(Debug, Clone)]
pub struct ParseSummary {
    /// The parsed module AST.
    pub module: vyre_libs::parsing::rust::parse::Module,
    /// Number of tokens.
    pub token_count: usize,
    /// Whether GPU fast-path was used for lexing.
    pub gpu_lex: bool,
}
