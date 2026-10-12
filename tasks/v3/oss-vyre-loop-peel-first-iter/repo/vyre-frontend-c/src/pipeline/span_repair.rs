use vyre_libs::parsing::c::lex::tokens::*;

pub(super) fn repair_token_spans_from_source(
    source: &str,
    tok_types: &[u32],
    starts: &mut [u32],
    lens: &mut [u32],
) -> Result<(), String> {
    if starts.len() != tok_types.len() || lens.len() != tok_types.len() {
        return Err(format!(
            "token span repair received mismatched stream lengths: tok_types={}, starts={}, lens={}. Fix: GPU C lexer outputs must carry one start and one length per token; silent tail truncation is forbidden.",
            tok_types.len(),
            starts.len(),
            lens.len()
        ));
    }
    let bytes = source.as_bytes();
    let mut cursor = 0usize;
    for idx in 0..tok_types.len() {
        let start = starts[idx] as usize;
        let len = lens[idx] as usize;
        if len > 0 && start.saturating_add(len) <= bytes.len() {
            cursor = start.saturating_add(len);
            continue;
        }
        cursor = skip_c_trivia(bytes, cursor);
        let Some((repaired_start, repaired_len)) = span_for_token_at(bytes, cursor, tok_types[idx])
        else {
            return Err(format!(
                "token {idx} has invalid span start={start} len={len} type={}. \
                 Fix: the GPU C lexer must emit spans that match the resident source stream.",
                tok_types[idx]
            ));
        };
        starts[idx] = u32::try_from(repaired_start).map_err(|_| {
            format!(
                "token {idx} repaired start {repaired_start} exceeds u32. \
                 Fix: split the resident translation unit before lexing."
            )
        })?;
        lens[idx] = u32::try_from(repaired_len).map_err(|_| {
            format!(
                "token {idx} repaired length {repaired_len} exceeds u32. \
                 Fix: split the resident translation unit before lexing."
            )
        })?;
        cursor = repaired_start.saturating_add(repaired_len);
    }
    Ok(())
}

fn skip_c_trivia(bytes: &[u8], mut cursor: usize) -> usize {
    loop {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor..cursor.saturating_add(2)) == Some(b"//") {
            cursor += 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            continue;
        }
        if bytes.get(cursor..cursor.saturating_add(2)) == Some(b"/*") {
            cursor += 2;
            while cursor + 1 < bytes.len() && bytes.get(cursor..cursor + 2) != Some(b"*/") {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            continue;
        }
        return cursor;
    }
}

fn span_for_token_at(bytes: &[u8], start: usize, token: u32) -> Option<(usize, usize)> {
    if start >= bytes.len() {
        return None;
    }
    if token == TOK_PREPROC {
        if bytes[start] != b'#' {
            return None;
        }
        let mut end = start;
        while end < bytes.len() && !matches!(bytes[end], b'\n' | b'\r') {
            end += 1;
        }
        return (end > start).then_some((start, end - start));
    }
    if is_identifier_like_token(token) {
        return scan_identifier(bytes, start);
    }
    if token == TOK_INTEGER || token == TOK_FLOAT {
        return scan_number(bytes, start);
    }
    if token == TOK_STRING || token == TOK_CHAR {
        return scan_quoted_token(bytes, start);
    }
    punct_len(bytes, start, token).map(|len| (start, len))
}

fn is_identifier_like_token(token: u32) -> bool {
    token == TOK_IDENTIFIER || (TOK_IF..=TOK_GNU_LABEL).contains(&token)
}

fn scan_identifier(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let first = *bytes.get(start)?;
    if !(first == b'_' || first.is_ascii_alphabetic()) {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len() && (bytes[end] == b'_' || bytes[end].is_ascii_alphanumeric()) {
        end += 1;
    }
    Some((start, end - start))
}

fn scan_number(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let first = *bytes.get(start)?;
    if !(first.is_ascii_digit() || first == b'.') {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len()
        && (bytes[end] == b'_'
            || bytes[end] == b'.'
            || bytes[end] == b'+'
            || bytes[end] == b'-'
            || bytes[end].is_ascii_alphanumeric())
    {
        end += 1;
    }
    Some((start, end - start))
}

fn scan_quoted_token(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut quote_at = start;
    if matches!(bytes.get(start), Some(b'L' | b'u' | b'U')) {
        quote_at += 1;
    }
    if bytes.get(start..start.saturating_add(2)) == Some(b"u8") {
        quote_at = start + 2;
    }
    let quote = *bytes.get(quote_at)?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    let mut end = quote_at + 1;
    let mut escaped = false;
    while end < bytes.len() {
        let byte = bytes[end];
        end += 1;
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            continue;
        }
        if byte == quote {
            return Some((start, end - start));
        }
        if byte == b'\n' || byte == b'\r' {
            return Some((start, end - start));
        }
    }
    Some((start, end.saturating_sub(start)))
}

fn punct_len(bytes: &[u8], start: usize, token: u32) -> Option<usize> {
    let candidates: &[&[u8]] = match token {
        TOK_ELLIPSIS => &[b"..."],
        TOK_ARROW => &[b"->"],
        TOK_EQ => &[b"=="],
        TOK_NE => &[b"!="],
        TOK_LE => &[b"<="],
        TOK_GE => &[b">="],
        TOK_AND => &[b"&&"],
        TOK_OR => &[b"||"],
        TOK_LSHIFT => &[b"<<"],
        TOK_RSHIFT => &[b">>"],
        TOK_INC => &[b"++"],
        TOK_DEC => &[b"--"],
        TOK_PLUS_EQ => &[b"+="],
        TOK_MINUS_EQ => &[b"-="],
        TOK_STAR_EQ => &[b"*="],
        TOK_SLASH_EQ => &[b"/="],
        TOK_PERCENT_EQ => &[b"%="],
        TOK_AMP_EQ => &[b"&="],
        TOK_PIPE_EQ => &[b"|="],
        TOK_CARET_EQ => &[b"^="],
        TOK_LSHIFT_EQ => &[b"<<="],
        TOK_RSHIFT_EQ => &[b">>="],
        TOK_HASHHASH => &[b"##", b"%:%:"],
        TOK_LPAREN => &[b"("],
        TOK_RPAREN => &[b")"],
        TOK_LBRACE => &[b"{", b"<%"],
        TOK_RBRACE => &[b"}", b"%>"],
        TOK_LBRACKET => &[b"[", b"<:"],
        TOK_RBRACKET => &[b"]", b":>"],
        TOK_SEMICOLON => &[b";"],
        TOK_COMMA => &[b","],
        TOK_DOT => &[b"."],
        TOK_PLUS => &[b"+"],
        TOK_MINUS => &[b"-"],
        TOK_STAR => &[b"*"],
        TOK_SLASH => &[b"/"],
        TOK_PERCENT => &[b"%"],
        TOK_AMP => &[b"&"],
        TOK_PIPE => &[b"|"],
        TOK_CARET => &[b"^"],
        TOK_TILDE => &[b"~"],
        TOK_BANG => &[b"!"],
        TOK_ASSIGN => &[b"="],
        TOK_LT => &[b"<"],
        TOK_GT => &[b">"],
        TOK_HASH => &[b"#", b"%:"],
        TOK_QUESTION => &[b"?"],
        TOK_COLON => &[b":"],
        _ => return None,
    };
    candidates
        .iter()
        .find(|candidate| bytes.get(start..start + candidate.len()) == Some(**candidate))
        .map(|candidate| candidate.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_token_spans_rejects_mismatched_gpu_lexer_stream_lengths() {
        let mut starts = vec![0u32];
        let mut lens = vec![1u32, 1u32];
        let err = repair_token_spans_from_source(
            "x y",
            &[TOK_IDENTIFIER, TOK_IDENTIFIER],
            &mut starts,
            &mut lens,
        )
        .expect_err("mismatched lexer streams must fail");
        assert!(err.contains("mismatched stream lengths"), "{err}");
        assert!(err.contains("silent tail truncation is forbidden"), "{err}");
    }
}
