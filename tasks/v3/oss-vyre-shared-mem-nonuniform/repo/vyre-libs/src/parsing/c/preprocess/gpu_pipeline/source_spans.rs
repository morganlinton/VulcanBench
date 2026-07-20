use super::ClassifiedTokens;

pub(super) fn token_row_bytes(
    classified: &ClassifiedTokens,
    token_index: usize,
) -> Result<&[u8], String> {
    let (start, end) = token_row_span(classified, token_index)?;
    classified.source.get(start..end).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: token row {token_index} span {start}..{end} is outside source length {}. Fix: repair GPU lexer span emission.",
            classified.source.len()
        )
    })
}

pub(super) fn token_row_span(
    classified: &ClassifiedTokens,
    token_index: usize,
) -> Result<(usize, usize), String> {
    let start = classified
        .tok_starts
        .get(token_index)
        .copied()
        .ok_or_else(|| {
            format!(
                "vyre-libs::gpu_pipeline: token row {token_index} is missing a start offset. Fix: repair GPU lexer output cardinality."
            )
        })? as usize;
    let len = classified
        .tok_lens
        .get(token_index)
        .copied()
        .ok_or_else(|| {
            format!(
                "vyre-libs::gpu_pipeline: token row {token_index} is missing a length. Fix: repair GPU lexer output cardinality."
            )
        })? as usize;
    let end = start.checked_add(len).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: token row {token_index} span overflows usize. Fix: repair GPU lexer span emission."
        )
    })?;
    if end > classified.source.len() {
        return Err(format!(
            "vyre-libs::gpu_pipeline: token row {token_index} span {start}..{end} is outside source length {}. Fix: repair GPU lexer span emission.",
            classified.source.len()
        ));
    }
    Ok((start, end))
}

pub(super) fn checked_source_range<'a>(
    source: &'a [u8],
    start: usize,
    end: usize,
    context: &str,
) -> Result<&'a [u8], String> {
    source.get(start..end).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: source range {start}..{end} for {context} is outside source length {}. Fix: repair GPU lexer span emission.",
            source.len()
        )
    })
}
