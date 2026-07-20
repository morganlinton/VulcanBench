use super::*;
pub(super) fn sparse_string_literal_end(source: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start + 1;
    let mut escaped = false;
    while let Some(byte) = source.get(cursor).copied() {
        if escaped {
            escaped = false;
            cursor += 1;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            cursor += 1;
            continue;
        }
        if byte == b'"' {
            let end = cursor + 1;
            return (end - start <= CUDA_SPARSE_LEX_MAX_TOKEN_SCAN).then_some(end);
        }
        if matches!(byte, b'\n' | b'\r') {
            return None;
        }
        cursor += 1;
    }
    None
}

pub(super) fn sparse_char_literal_end(source: &[u8], start: usize) -> Option<usize> {
    if matches!(
        source.get(start.wrapping_sub(1)).copied(),
        Some(b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9')
    ) {
        return None;
    }
    sparse_char_literal_body_end(source, start)
}

pub(super) fn sparse_prefixed_char_literal_end(source: &[u8], start: usize) -> Option<usize> {
    let quote = if matches!(source.get(start).copied(), Some(b'L' | b'U'))
        && matches!(source.get(start + 1).copied(), Some(b'\''))
    {
        start + 1
    } else if matches!(source.get(start).copied(), Some(b'u'))
        && matches!(source.get(start + 1).copied(), Some(b'\''))
    {
        start + 1
    } else if matches!(source.get(start).copied(), Some(b'u'))
        && matches!(source.get(start + 1).copied(), Some(b'8'))
        && matches!(source.get(start + 2).copied(), Some(b'\''))
    {
        start + 2
    } else {
        return None;
    };
    sparse_char_literal_body_end(source, quote)
        .filter(|end| end.saturating_sub(start) <= CUDA_SPARSE_LEX_MAX_TOKEN_SCAN)
}

fn sparse_char_literal_body_end(source: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start + 1;
    let mut escaped = false;
    while let Some(byte) = source.get(cursor).copied() {
        if escaped {
            escaped = false;
            cursor += 1;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            cursor += 1;
            continue;
        }
        if byte == b'\'' {
            let end = cursor + 1;
            return (end - start <= CUDA_SPARSE_LEX_MAX_TOKEN_SCAN).then_some(end);
        }
        if matches!(byte, b'\n' | b'\r') {
            return None;
        }
        cursor += 1;
    }
    None
}

pub(super) fn sparse_line_comment_end(source: &[u8], start: usize) -> usize {
    let mut cursor = start + 2;
    while !matches!(source.get(cursor).copied(), None | Some(b'\n' | b'\r')) {
        cursor += 1;
    }
    cursor
}

pub(super) fn sparse_block_comment_end(source: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start + 2;
    while let Some(byte) = source.get(cursor).copied() {
        if byte == b'*' && matches!(source.get(cursor + 1).copied(), Some(b'/')) {
            let end = cursor + 2;
            return (end - start <= CUDA_SPARSE_LEX_MAX_TOKEN_SCAN).then_some(end);
        }
        cursor += 1;
    }
    None
}
