use std::path::Path;

use vyre_libs::parsing::c::lex::tokens::is_c_lexer_error_token;

use super::{lexer_diagnostic_report::reject_decoded_c11_lexer_diagnostics, read_u32_stream};

pub(crate) fn token_types_from_lex(types_buf: &[u8], n_tokens: u32) -> Result<Vec<u32>, String> {
    read_u32_stream(types_buf, n_tokens as usize, "token type buffer")
}

pub(crate) fn reject_c11_lexer_diagnostics(
    path: &Path,
    tok_types: &[u32],
    starts_buf: &[u8],
    lens_buf: &[u8],
) -> Result<(), String> {
    if !tok_types.iter().copied().any(is_c_lexer_error_token) {
        return Ok(());
    }
    reject_decoded_c11_lexer_diagnostics(path, tok_types, starts_buf, lens_buf)
}
