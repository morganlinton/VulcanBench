//! Public entry points for the Rust frontend.

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::Module;

use crate::api::parse_summary::ParseSummary;
use crate::RustFrontendError;

/// Parse Rust source bytes into a `ParseSummary`.
///
/// 1. Lexes the source through the reusable Rust lexer substrate.
/// 2. Parses the token stream into the nano-subset AST.
pub fn parse_rust_bytes(source: &[u8]) -> Result<ParseSummary, RustFrontendError> {
    let tokens = lex(source).map_err(RustFrontendError::Lex)?;
    let module = parse_tokens(source, &tokens)?;
    Ok(ParseSummary {
        token_count: tokens.len(),
        module,
        gpu_lex: false,
    })
}

fn parse_tokens(source: &[u8], tokens: &[crate::Token]) -> Result<Module, RustFrontendError> {
    vyre_libs::parsing::rust::parse::parse(source, tokens).map_err(|e| RustFrontendError::Parse {
        message: e.message,
        token_index: e.token_index,
    })
}
