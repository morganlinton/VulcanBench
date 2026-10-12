use std::path::Path;

use vyre_libs::parsing::c::lex::diagnostics::{first_c11_lexer_diagnostic, C11LexerDiagnosticKind};

use super::read_u32_stream;

pub(super) fn reject_decoded_c11_lexer_diagnostics(
    path: &Path,
    tok_types: &[u32],
    starts_buf: &[u8],
    lens_buf: &[u8],
) -> Result<(), String> {
    let tok_starts = read_u32_stream(starts_buf, tok_types.len(), "lexer diagnostic starts")?;
    let tok_lens = read_u32_stream(lens_buf, tok_types.len(), "lexer diagnostic lengths")?;
    let diag = first_c11_lexer_diagnostic(tok_types, &tok_starts, &tok_lens).ok_or_else(|| {
        format!(
            "C lexer emitted an error token for {}, but no diagnostic decoded from token buffers. \
             Fix: keep token kind/start/length buffers aligned before parser entry.",
            path.display()
        )
    })?;
    let token_kind = tok_types
        .get(diag.token_index as usize)
        .copied()
        .ok_or_else(|| {
            format!(
                "C lexer diagnostic token index {} is outside {} decoded token kinds for {}. \
                 Fix: keep diagnostic token indices aligned with compact lexer outputs.",
                diag.token_index,
                tok_types.len(),
                path.display()
            )
        })?;
    let detail = match diag.kind {
        C11LexerDiagnosticKind::UnterminatedString => "unterminated string literal",
        C11LexerDiagnosticKind::UnterminatedChar => "unterminated character literal",
        C11LexerDiagnosticKind::UnterminatedBlockComment => "unterminated block comment",
        C11LexerDiagnosticKind::InvalidEscape => "invalid string or character escape",
    };
    Err(format!(
        "C lexer rejected {}: {detail} ({:?}, token kind {token_kind}) at token index {}, \
         byte span [{}..{}), length {}. Fix: correct the malformed C token before parser, VAST, \
         or ProgramGraph lowering.",
        path.display(),
        diag.kind,
        diag.token_index,
        diag.byte_start,
        diag.byte_start.saturating_add(diag.byte_len),
        diag.byte_len
    ))
}

#[cfg(test)]
mod tests {
    use super::reject_decoded_c11_lexer_diagnostics;
    use std::path::Path;
    use vyre_libs::parsing::c::lex::tokens::{
        TOK_ERR_INVALID_ESCAPE, TOK_ERR_UNTERMINATED_CHAR, TOK_ERR_UNTERMINATED_COMMENT,
        TOK_ERR_UNTERMINATED_STRING,
    };

    fn u32_bytes(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn generated_lexer_diagnostic_report_covers_all_error_kinds() {
        let cases = [
            (TOK_ERR_UNTERMINATED_STRING, "unterminated string literal"),
            (TOK_ERR_UNTERMINATED_CHAR, "unterminated character literal"),
            (TOK_ERR_UNTERMINATED_COMMENT, "unterminated block comment"),
            (TOK_ERR_INVALID_ESCAPE, "invalid string or character escape"),
        ];

        for (token, detail) in cases {
            let err = reject_decoded_c11_lexer_diagnostics(
                Path::new("bad.c"),
                &[token],
                &u32_bytes(&[12]),
                &u32_bytes(&[5]),
            )
            .unwrap_err();

            assert!(err.contains("C lexer rejected bad.c"));
            assert!(err.contains(detail));
            assert!(err.contains("byte span [12..17), length 5"));
        }
    }
}
