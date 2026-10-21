//! Batched GPU lexer dispatch for many Rust frontend source buffers.

use vyre_libs::parsing::rust::lex::lexer::core::Token;
use vyre_libs::parsing::rust::lex::lexer::plan::RustLexerPlan;

use crate::RustFrontendError;

use super::{
    decode_token_window, decode_u32_words, dispatch_gpu_lexer_cached, shared_gpu_lexer_backend,
    source_words, u32s_to_bytes, RUST_GPU_LEXER_WORKGROUP_SIZE,
};

pub(super) fn lex_gpu_batch(
    sources: &[&[u8]],
    plan: &RustLexerPlan,
) -> Result<Vec<Result<Vec<Token>, RustFrontendError>>, RustFrontendError> {
    let layout = BatchLexerLayout::from_sources(sources);
    if layout.source_map.is_empty() {
        return Ok(layout.results);
    }

    let source_count = u32::try_from(layout.source_map.len()).map_err(|_| {
        RustFrontendError::Backend(format!(
            "Rust GPU batch lexer received {} valid sources, exceeding the u32 dispatch limit",
            layout.source_map.len()
        ))
    })?;
    let haystack_len = u32::try_from(layout.packed_source.len()).map_err(|_| {
        RustFrontendError::Backend(format!(
            "Rust GPU batch lexer packed source has {} bytes, exceeding the u32 address limit",
            layout.packed_source.len()
        ))
    })?;
    let token_stride = u32::try_from(layout.token_stride).map_err(|_| {
        RustFrontendError::Backend(format!(
            "Rust GPU batch lexer token stride {} exceeds the u32 layout limit",
            layout.token_stride
        ))
    })?;
    let program = plan.build_batch_for_layout(haystack_len, source_count, token_stride);
    let inputs = batch_lexer_inputs(&layout)?;
    let input_refs = inputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let backend = shared_gpu_lexer_backend()?;
    let outputs = dispatch_gpu_lexer_cached(
        backend.as_ref(),
        &program,
        &input_refs,
        [
            source_count.div_ceil(RUST_GPU_LEXER_WORKGROUP_SIZE).max(1),
            1,
            1,
        ],
        "rust_frontend_gpu_lexer_batch",
    )?;
    decode_gpu_token_batch(outputs, layout)
}

struct BatchLexerLayout {
    results: Vec<Result<Vec<Token>, RustFrontendError>>,
    packed_source: Vec<u8>,
    offsets: Vec<u32>,
    lens: Vec<u32>,
    source_map: Vec<usize>,
    token_stride: usize,
}

impl BatchLexerLayout {
    fn from_sources(sources: &[&[u8]]) -> Self {
        let mut results = vec![Ok(Vec::new()); sources.len()];
        let mut packed_source = Vec::new();
        let mut offsets = Vec::new();
        let mut lens = Vec::new();
        let mut source_map = Vec::new();
        let mut token_stride = 1usize;

        for (source_idx, source) in sources.iter().enumerate() {
            if let Err(error) = std::str::from_utf8(source) {
                results[source_idx] = Err(RustFrontendError::Lex(error.valid_up_to()));
                continue;
            }
            let Ok(source_len) = u32::try_from(source.len()) else {
                results[source_idx] = Err(RustFrontendError::Lex(u32::MAX as usize));
                continue;
            };
            let Ok(offset) = u32::try_from(packed_source.len()) else {
                results[source_idx] = Err(RustFrontendError::Backend(format!(
                    "Rust GPU batch lexer source {source_idx} starts beyond the u32 address range"
                )));
                continue;
            };
            offsets.push(offset);
            lens.push(source_len);
            source_map.push(source_idx);
            token_stride = token_stride.max(source.len().saturating_add(1).max(1));
            packed_source.extend_from_slice(source);
        }

        Self {
            results,
            packed_source,
            offsets,
            lens,
            source_map,
            token_stride,
        }
    }
}

fn batch_lexer_inputs(layout: &BatchLexerLayout) -> Result<Vec<Vec<u8>>, RustFrontendError> {
    let token_slots = layout
        .source_map
        .len()
        .max(1)
        .checked_mul(layout.token_stride)
        .ok_or_else(|| {
            RustFrontendError::Backend(
                "Rust GPU batch lexer token output layout overflows host usize".to_string(),
            )
        })?;
    let zero_tokens = vec![0u8; token_slots * std::mem::size_of::<u32>()];
    Ok(vec![
        u32s_to_bytes(&source_words(&layout.packed_source)),
        u32s_to_bytes(&layout.offsets),
        u32s_to_bytes(&layout.lens),
        zero_tokens.clone(),
        zero_tokens.clone(),
        zero_tokens,
        vec![0u8; layout.source_map.len().max(1) * std::mem::size_of::<u32>()],
    ])
}

fn decode_gpu_token_batch(
    outputs: Vec<Vec<u8>>,
    mut layout: BatchLexerLayout,
) -> Result<Vec<Result<Vec<Token>, RustFrontendError>>, RustFrontendError> {
    if outputs.len() != 4 {
        return Err(RustFrontendError::Backend(format!(
            "Rust GPU batch lexer returned {} output buffers, expected 4 token columns [types, starts, lens, counts]",
            outputs.len()
        )));
    }
    let kinds = decode_u32_words(&outputs[0], "batch token types")?;
    let starts = decode_u32_words(&outputs[1], "batch token starts")?;
    let lens = decode_u32_words(&outputs[2], "batch token lengths")?;
    let counts = decode_u32_words(&outputs[3], "batch token counts")?;
    let expected_slots = layout
        .source_map
        .len()
        .checked_mul(layout.token_stride)
        .ok_or_else(|| {
            RustFrontendError::Backend(
                "Rust GPU batch lexer token output layout overflows host usize during decode"
                    .to_string(),
            )
        })?;
    if counts.len() < layout.source_map.len()
        || kinds.len() < expected_slots
        || starts.len() < expected_slots
        || lens.len() < expected_slots
    {
        return Err(RustFrontendError::Backend(format!(
            "Rust GPU batch lexer output shape mismatch: valid_sources={}, stride={}, counts={}, types={}, starts={}, lens={}",
            layout.source_map.len(),
            layout.token_stride,
            counts.len(),
            kinds.len(),
            starts.len(),
            lens.len()
        )));
    }

    for (local_idx, source_idx) in layout.source_map.iter().copied().enumerate() {
        let count = counts[local_idx] as usize;
        let base = local_idx * layout.token_stride;
        layout.results[source_idx] = if count == 0 || count > layout.token_stride {
            Err(RustFrontendError::Backend(format!(
                "Rust GPU batch lexer emitted token count {count} for source {source_idx}, outside stride {}",
                layout.token_stride
            )))
        } else {
            decode_token_window(&kinds, &starts, &lens, base, count)
        };
    }

    Ok(layout.results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_libs::parsing::rust::lex::tokens::{EOF, ERROR, KW_FN, KW_I32, KW_LET};

    #[test]
    fn gpu_batch_token_decoder_reconstructs_per_source_streams() {
        let layout = BatchLexerLayout {
            results: vec![Ok(Vec::new()), Ok(Vec::new())],
            packed_source: Vec::new(),
            offsets: vec![0, 16],
            lens: vec![8, 12],
            source_map: vec![0, 1],
            token_stride: 4,
        };
        let outputs = vec![
            u32s_to_bytes(&[
                u32::from(KW_FN),
                u32::from(EOF),
                0,
                0,
                u32::from(KW_LET),
                u32::from(KW_I32),
                u32::from(EOF),
                0,
            ]),
            u32s_to_bytes(&[0, 2, 0, 0, 0, 8, 11, 0]),
            u32s_to_bytes(&[2, 0, 0, 0, 3, 3, 0, 0]),
            u32s_to_bytes(&[2, 3]),
        ];

        let decoded = decode_gpu_token_batch(outputs, layout).expect("valid batch columns decode");
        let source0 = decoded[0].as_ref().expect("source 0 decodes");
        let source1 = decoded[1].as_ref().expect("source 1 decodes");
        assert_eq!(source0.len(), 2);
        assert_eq!(source0[0].kind, KW_FN);
        assert_eq!(source1.len(), 3);
        assert_eq!(source1[1].kind, KW_I32);
        assert_eq!(source1[2].kind, EOF);
    }

    #[test]
    fn gpu_batch_token_decoder_keeps_error_source_isolated() {
        let layout = BatchLexerLayout {
            results: vec![Ok(Vec::new()), Ok(Vec::new()), Ok(Vec::new())],
            packed_source: Vec::new(),
            offsets: vec![0, 16, 32],
            lens: vec![8, 8, 8],
            source_map: vec![0, 1, 2],
            token_stride: 2,
        };
        let outputs = vec![
            u32s_to_bytes(&[
                u32::from(KW_FN),
                u32::from(EOF),
                u32::from(ERROR),
                u32::from(EOF),
                u32::from(KW_LET),
                u32::from(EOF),
            ]),
            u32s_to_bytes(&[0, 2, 9, 10, 0, 3]),
            u32s_to_bytes(&[2, 0, 1, 0, 3, 0]),
            u32s_to_bytes(&[2, 2, 2]),
        ];

        let decoded = decode_gpu_token_batch(outputs, layout).expect("batch columns decode");
        assert!(decoded[0].is_ok(), "source 0 should remain usable");
        assert!(matches!(decoded[1], Err(RustFrontendError::Lex(9))));
        assert!(decoded[2].is_ok(), "source 2 should remain usable");
    }
}
