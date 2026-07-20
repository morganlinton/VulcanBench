use super::*;

pub(crate) fn token_start(classified: &ClassifiedTokens, idx: usize) -> Result<u32, String> {
    classified.tok_starts.get(idx).copied().ok_or_else(|| {
        "vyre-libs::gpu_pipeline: token provenance start column missing. Fix: repair GPU lexer output column lengths.".to_string()
    })
}

pub(crate) fn token_len(classified: &ClassifiedTokens, idx: usize) -> Result<u32, String> {
    classified.tok_lens.get(idx).copied().ok_or_else(|| {
        "vyre-libs::gpu_pipeline: token provenance length column missing. Fix: repair GPU lexer output column lengths.".to_string()
    })
}
