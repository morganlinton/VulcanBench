use std::path::Path;

use vyre_libs::parsing::c::lex::tokens::is_c_lexer_error_token;

use super::{lexer_diagnostic_report::reject_decoded_c11_lexer_diagnostics, read_u32_stream};

pub(in crate::pipeline) fn reject_c11_lexer_diagnostics_bytes(
    path: &Path,
    tok_type_bytes: &[u8],
    starts_buf: &[u8],
    lens_buf: &[u8],
    n_tokens: u32,
) -> Result<(), String> {
    let byte_len = n_tokens as usize * 4;
    if byte_len > tok_type_bytes.len() {
        return Err(format!(
            "lexer diagnostic token types: need {byte_len} bytes for {n_tokens} tokens, have {}",
            tok_type_bytes.len()
        ));
    }
    if !tok_type_bytes[..byte_len].chunks_exact(4).any(|chunk| {
        is_c_lexer_error_token(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
    }) {
        return Ok(());
    }
    let tok_types = read_u32_stream(tok_type_bytes, n_tokens as usize, "lexer diagnostic types")?;
    reject_decoded_c11_lexer_diagnostics(path, &tok_types, starts_buf, lens_buf)
}
