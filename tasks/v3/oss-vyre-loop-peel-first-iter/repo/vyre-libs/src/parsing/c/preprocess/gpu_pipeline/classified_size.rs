use super::ClassifiedTokens;

pub(super) fn classified_tokens_bytes(classified: &ClassifiedTokens) -> usize {
    let word_columns = classified
        .tok_types
        .len()
        .checked_add(classified.tok_starts.len())
        .and_then(|value| value.checked_add(classified.tok_lens.len()))
        .and_then(|value| value.checked_add(classified.directive_kinds.len()))
        .and_then(|value| value.checked_mul(std::mem::size_of::<u32>()))
        .unwrap_or(usize::MAX);
    word_columns
        .checked_add(classified.source.len())
        .unwrap_or(usize::MAX)
}
