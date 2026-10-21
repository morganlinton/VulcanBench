use super::*;

pub(crate) fn macro_output_end(
    input_source: &[u8],
    expanded_source: &[u8],
    consumed_end: usize,
    output_cursor: usize,
) -> usize {
    let Some(anchor) = next_anchor_token(input_source, consumed_end) else {
        return expanded_source.len();
    };
    find_subslice(expanded_source, anchor, output_cursor).unwrap_or(expanded_source.len())
}

pub(crate) fn next_anchor_token(source: &[u8], from: usize) -> Option<&[u8]> {
    let mut pos = from;
    while source
        .get(pos)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        pos += 1;
    }
    let first = *source.get(pos)?;
    if first.is_ascii_alphabetic() || first == b'_' {
        let start = pos;
        pos += 1;
        while source
            .get(pos)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            pos += 1;
        }
        return source.get(start..pos);
    }
    if first.is_ascii_digit() {
        let start = pos;
        pos += 1;
        while source.get(pos).is_some_and(|byte| byte.is_ascii_digit()) {
            pos += 1;
        }
        return source.get(start..pos);
    }
    source.get(pos..pos + 1)
}

pub(crate) fn first_expanded_token_at_or_after(
    expanded: &ClassifiedTokens,
    output_cursor: usize,
    search_index: &mut usize,
) -> Result<Option<usize>, String> {
    while *search_index < expanded.tok_types.len() {
        if expanded.tok_types[*search_index] == 0 {
            *search_index += 1;
            continue;
        }
        let start = token_start(expanded, *search_index)? as usize;
        if start >= output_cursor {
            return Ok(Some(*search_index));
        }
        *search_index += 1;
    }
    Ok(None)
}

pub(crate) fn find_expanded_token_at_or_after_from(
    expanded: &ClassifiedTokens,
    token: &[u8],
    output_cursor: usize,
    search_index: &mut usize,
) -> Result<Option<(u32, u32)>, String> {
    while let Some(idx) = first_expanded_token_at_or_after(expanded, output_cursor, search_index)? {
        let start = token_start(expanded, idx)?;
        let len = token_len(expanded, idx)?;
        let end = (start as usize).checked_add(len as usize).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: expanded provenance token range overflow. Fix: shard preprocessing before provenance export.".to_string()
        })?;
        if expanded.source.get(start as usize..end) == Some(token) {
            *search_index = idx + 1;
            return Ok(Some((start, len)));
        }
        *search_index = idx + 1;
    }
    Ok(None)
}

pub(crate) fn find_subslice(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(haystack.len()));
    }
    if needle.len() == 1 {
        let target = needle[0];
        return haystack
            .get(from..)?
            .iter()
            .position(|byte| *byte == target)
            .map(|offset| from + offset);
    }
    let first = needle[0];
    let search = haystack.get(from..)?;
    if needle.len() > search.len() {
        return None;
    }
    let last_start = search.len() - needle.len();
    let mut offset = 0usize;
    while offset <= last_start {
        let Some(next) = search[offset..=last_start]
            .iter()
            .position(|byte| *byte == first)
        else {
            return None;
        };
        offset += next;
        if search.get(offset..offset + needle.len()) == Some(needle) {
            return Some(from + offset);
        }
        offset += 1;
    }
    None
}
