use super::*;

pub(crate) struct ReplacementTokenView {
    pub(crate) spelling_start: u32,
    pub(crate) spelling_len: u32,
}

pub(crate) fn collect_replacement_token_views(
    replacement_tokens: &ClassifiedTokens,
) -> SmallVec<[ReplacementTokenView; 8]> {
    let mut out = SmallVec::new();
    for idx in 0..replacement_tokens.tok_types.len() {
        if replacement_tokens.tok_types[idx] == 0 {
            continue;
        }
        let Ok(start) = token_start(replacement_tokens, idx) else {
            continue;
        };
        let Ok(len) = token_len(replacement_tokens, idx) else {
            continue;
        };
        out.push(ReplacementTokenView {
            spelling_start: start,
            spelling_len: len,
        });
    }
    out
}

pub(crate) fn replacement_token_count(replacement_tokens: &ClassifiedTokens) -> usize {
    replacement_tokens
        .tok_types
        .iter()
        .filter(|kind| **kind != 0)
        .count()
}
