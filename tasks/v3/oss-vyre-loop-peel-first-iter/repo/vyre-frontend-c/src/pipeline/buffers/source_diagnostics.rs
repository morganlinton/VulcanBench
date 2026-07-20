use super::*;
pub(crate) fn reject_c11_source_diagnostics(path: &Path, source: &str) -> Result<(), String> {
    let bytes = source.as_bytes();
    let mut token_index = 0u32;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                if let Some((kind, start, len)) = scan_quoted(bytes, i, b'"') {
                    return Err(format_source_diagnostic(
                        path,
                        kind,
                        token_index,
                        start,
                        len,
                    ));
                }
                token_index = token_index.saturating_add(1);
                i = skip_quoted(bytes, i, b'"');
            }
            b'\'' => {
                if let Some((kind, start, len)) = scan_quoted(bytes, i, b'\'') {
                    return Err(format_source_diagnostic(
                        path,
                        kind,
                        token_index,
                        start,
                        len,
                    ));
                }
                token_index = token_index.saturating_add(1);
                i = skip_quoted(bytes, i, b'\'');
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                if let Some(end) = find_block_comment_end(bytes, i + 2) {
                    token_index = token_index.saturating_add(1);
                    i = end;
                } else {
                    return Err(format_source_diagnostic(
                        path,
                        C11LexerDiagnosticKind::UnterminatedBlockComment,
                        token_index,
                        i,
                        bytes.len().saturating_sub(i),
                    ));
                }
            }
            byte if byte.is_ascii_whitespace() => {
                i += 1;
            }
            b'_' if identifier_starts_at(bytes, i) => {
                let ident_end = identifier_end(bytes, i);
                if is_gnu_attribute_ident(&bytes[i..ident_end]) {
                    i = validate_gnu_attribute_introducer(path, bytes, i, token_index)?;
                } else {
                    i = ident_end;
                }
                token_index = token_index.saturating_add(1);
            }
            _ => {
                token_index = token_index.saturating_add(1);
                i += 1;
            }
        }
    }
    Ok(())
}

fn identifier_starts_at(bytes: &[u8], start: usize) -> bool {
    bytes
        .get(start)
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        && start
            .checked_sub(1)
            .and_then(|prev| bytes.get(prev))
            .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
}

fn identifier_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end += 1;
    }
    end
}

fn is_gnu_attribute_ident(ident: &[u8]) -> bool {
    matches!(ident, b"__attribute" | b"__attribute__")
}

fn validate_gnu_attribute_introducer(
    path: &Path,
    bytes: &[u8],
    attr_start: usize,
    token_index: u32,
) -> Result<usize, String> {
    let ident_end = identifier_end(bytes, attr_start);
    let outer_open = skip_ascii_whitespace(bytes, ident_end);
    if bytes.get(outer_open) != Some(&b'(') {
        return Err(format_gnu_attribute_diagnostic(
            path,
            token_index,
            attr_start,
            ident_end.saturating_sub(attr_start),
            "__attribute__ must be followed by `(`",
        ));
    }
    let first_inner = skip_ascii_whitespace(bytes, outer_open + 1);
    if bytes.get(first_inner) == Some(&b')') {
        return Ok(first_inner + 1);
    }
    if bytes.get(first_inner) != Some(&b'(') {
        return Err(format_gnu_attribute_diagnostic(
            path,
            token_index,
            attr_start,
            first_inner.saturating_sub(attr_start).saturating_add(1),
            "__attribute__ payload must use GNU double parentheses `((...))` or empty `()`",
        ));
    }
    let inner_end = skip_balanced_parentheses(bytes, first_inner).ok_or_else(|| {
        format_gnu_attribute_diagnostic(
            path,
            token_index,
            attr_start,
            bytes.len().saturating_sub(attr_start),
            "__attribute__ inner payload is missing its closing `)`",
        )
    })?;
    let outer_close = skip_ascii_whitespace(bytes, inner_end);
    if bytes.get(outer_close) != Some(&b')') {
        return Err(format_gnu_attribute_diagnostic(
            path,
            token_index,
            attr_start,
            outer_close.saturating_sub(attr_start).saturating_add(1),
            "__attribute__ outer wrapper is missing its closing `)`",
        ));
    }
    Ok(outer_close + 1)
}

fn skip_ascii_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

fn skip_balanced_parentheses(bytes: &[u8], open: usize) -> Option<usize> {
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0u32;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => {
                depth = depth.checked_add(1)?;
                i += 1;
            }
            b')' => {
                depth = depth.checked_sub(1)?;
                i += 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'"' => i = skip_quoted(bytes, i, b'"'),
            b'\'' => i = skip_quoted(bytes, i, b'\''),
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = find_block_comment_end(bytes, i + 2)?;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i = bytes[i + 2..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |offset| i + 2 + offset + 1);
            }
            _ => i += 1,
        }
    }
    None
}

fn format_gnu_attribute_diagnostic(
    path: &Path,
    token_index: u32,
    byte_start: usize,
    byte_len: usize,
    detail: &str,
) -> String {
    format!(
        "C parser rejected {}: malformed-gnu-attribute: {detail} at token index {}, byte span [{}..{}), length {}. Fix: use `__attribute__((...))` or remove the malformed GNU attribute.",
        path.display(),
        token_index,
        byte_start,
        byte_start.saturating_add(byte_len),
        byte_len
    )
}

fn format_source_diagnostic(
    path: &Path,
    kind: C11LexerDiagnosticKind,
    token_index: u32,
    byte_start: usize,
    byte_len: usize,
) -> String {
    let detail = match kind {
        C11LexerDiagnosticKind::UnterminatedString => "unterminated string literal",
        C11LexerDiagnosticKind::UnterminatedChar => "unterminated character literal",
        C11LexerDiagnosticKind::UnterminatedBlockComment => "unterminated block comment",
        C11LexerDiagnosticKind::InvalidEscape => "invalid string or character escape",
    };
    format!(
        "C lexer rejected {}: {detail} ({kind:?}, token kind {kind:?}) at token index {}, \
         byte span [{}..{}), length {}. Fix: correct the malformed C token before parser, VAST, \
         or ProgramGraph lowering.",
        path.display(),
        token_index,
        byte_start,
        byte_start.saturating_add(byte_len),
        byte_len
    )
}

fn scan_quoted(
    bytes: &[u8],
    quote_start: usize,
    quote: u8,
) -> Option<(C11LexerDiagnosticKind, usize, usize)> {
    let mut i = quote_start + 1;
    while i < bytes.len() {
        match bytes[i] {
            byte if byte == quote => return None,
            b'\n' | b'\r' => {
                let kind = if quote == b'"' {
                    C11LexerDiagnosticKind::UnterminatedString
                } else {
                    C11LexerDiagnosticKind::UnterminatedChar
                };
                return Some((kind, quote_start, i.saturating_sub(quote_start)));
            }
            b'\\' => {
                let Some(next) = bytes.get(i + 1).copied() else {
                    return Some((C11LexerDiagnosticKind::InvalidEscape, i, 1));
                };
                match escape_width(bytes, i + 1, next) {
                    Some(width) => i += 1 + width,
                    None => return Some((C11LexerDiagnosticKind::InvalidEscape, i, 2)),
                }
            }
            _ => i += 1,
        }
    }
    let kind = if quote == b'"' {
        C11LexerDiagnosticKind::UnterminatedString
    } else {
        C11LexerDiagnosticKind::UnterminatedChar
    };
    Some((kind, quote_start, bytes.len().saturating_sub(quote_start)))
}

fn skip_quoted(bytes: &[u8], quote_start: usize, quote: u8) -> usize {
    let mut i = quote_start + 1;
    while i < bytes.len() {
        match bytes[i] {
            byte if byte == quote => return i + 1,
            b'\\' => i = i.saturating_add(2),
            _ => i += 1,
        }
    }
    bytes.len()
}

fn escape_width(bytes: &[u8], escape_start: usize, next: u8) -> Option<usize> {
    match next {
        b'\'' | b'"' | b'?' | b'\\' | b'a' | b'b' | b'e' | b'f' | b'n' | b'r' | b't' | b'v' => {
            Some(1)
        }
        b'0'..=b'7' => {
            let mut width = 1usize;
            while width < 3
                && bytes
                    .get(escape_start + width)
                    .is_some_and(u8::is_ascii_digit)
            {
                if !matches!(bytes[escape_start + width], b'0'..=b'7') {
                    break;
                }
                width += 1;
            }
            Some(width)
        }
        b'x' => {
            let mut width = 1usize;
            while bytes
                .get(escape_start + width)
                .is_some_and(u8::is_ascii_hexdigit)
            {
                width += 1;
            }
            (width > 1).then_some(width)
        }
        b'u' | b'U' => {
            let required = if next == b'u' { 4 } else { 8 };
            let end = escape_start + 1 + required;
            bytes
                .get(escape_start + 1..end)
                .filter(|digits| digits.iter().all(u8::is_ascii_hexdigit))
                .map(|_| 1 + required)
        }
        _ => None,
    }
}

fn find_block_comment_end(bytes: &[u8], mut i: usize) -> Option<usize> {
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return Some(i + 2);
        }
        i += 1;
    }
    None
}
