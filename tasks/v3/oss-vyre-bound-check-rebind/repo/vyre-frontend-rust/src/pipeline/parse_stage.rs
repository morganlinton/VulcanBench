//! Parse stage: token stream → AST.

use vyre_libs::parsing::rust::lex::lexer::core::Token;
use vyre_libs::parsing::rust::parse::Module;

use crate::RustFrontendError;

/// Parse tokens into an AST module.
pub fn parse(source: &[u8], tokens: &[Token]) -> Result<Module, RustFrontendError> {
    vyre_libs::parsing::rust::parse::parse(source, tokens).map_err(|e| RustFrontendError::Parse {
        message: e.message,
        token_index: e.token_index,
    })
}
