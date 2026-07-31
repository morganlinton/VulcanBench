use super::buffers::{
    bucket_pow2, checked_gpu_u32, pack_u32_words_into, read_u32_scalar_exact,
    reserve_gpu_staging_bytes, u32_word_byte_len, unpack_u32_words_prefix_exact,
};
use super::dispatch::GpuDispatcher;
use super::scan::{inclusive_prefix_scan_u32_into, PrefixScanScratch};
use crate::parsing::c::lex::lexer::{
    c11_compact_sparse_tokens, c11_compact_sparse_tokens_output,
    c11_lexer_regular_sparse_u8_haystack_with_flags,
};
use crate::parsing::c::lex::tokens::{TOK_PP_ELIF, TOK_PP_IF};
use crate::parsing::c::preprocess::gpu_directive_metadata::gpu_directive_metadata_u8;
use crate::parsing::c::preprocess::gpu_if_expression_abi::INVALID_EXPR_VALUE;
use std::sync::Arc;

/// Output of the lex+classify stage.
///
/// All four columns are dense (one entry per emitted token, length =
/// `n_tokens`). `directive_kinds[i]` is `0` for any token whose type
/// is not `TOK_PREPROC`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedTokens {
    /// Token kind id per token (TOK_* constants from `parsing::c::lex::tokens`).
    pub tok_types: Vec<u32>,
    /// Per-token byte offset into the source buffer.
    pub tok_starts: Vec<u32>,
    /// Per-token byte length in the source buffer.
    pub tok_lens: Vec<u32>,
    /// Per-token directive kind (TOK_PP_* constants); `0` for non-PREPROC.
    pub directive_kinds: Vec<u32>,
    /// Number of non-zero directive rows in `directive_kinds`.
    ///
    /// Construct with [`ClassifiedTokens::from_parts`] so this cached
    /// aggregate cannot drift from the GPU-produced directive column.
    pub directive_count: u32,
    /// The source bytes the tokens index into. Held alongside the
    /// columns so downstream stages don't have to re-pass it.
    pub source: Arc<[u8]>,
}

impl ClassifiedTokens {
    /// Build classified token columns and cache the directive-row count once.
    pub fn from_parts(
        tok_types: Vec<u32>,
        tok_starts: Vec<u32>,
        tok_lens: Vec<u32>,
        directive_kinds: Vec<u32>,
        source: Arc<[u8]>,
    ) -> Self {
        let directive_count = count_directives(&directive_kinds);
        Self {
            tok_types,
            tok_starts,
            tok_lens,
            directive_kinds,
            directive_count,
            source,
        }
    }

    /// True when at least one token row is a preprocessor directive.
    #[must_use]
    pub fn has_directives(&self) -> bool {
        self.directive_count != 0
    }

    /// Iterate `(index, kind)` over directive rows whose kind is non-zero.
    pub fn directive_rows(&self) -> impl Iterator<Item = (usize, u32)> + '_ {
        self.directive_kinds
            .iter()
            .enumerate()
            .filter_map(|(i, &k)| if k == 0 { None } else { Some((i, k)) })
    }
}

pub(super) fn count_directives(directive_kinds: &[u32]) -> u32 {
    directive_kinds
        .iter()
        .filter(|&&kind| kind != 0)
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod classified_token_tests {
    use super::*;

    #[test]
    fn from_parts_caches_directive_count_once() {
        let classified = ClassifiedTokens::from_parts(
            vec![1, 2, 3, 4],
            vec![0, 1, 2, 3],
            vec![1, 1, 1, 1],
            vec![0, 7, 0, 9],
            Arc::from(b"abcd".as_slice()),
        );

        assert_eq!(classified.directive_count, 2);
        assert!(classified.has_directives());
        assert_eq!(
            classified.directive_rows().collect::<Vec<_>>(),
            vec![(1, 7), (3, 9)]
        );
    }

    #[test]
    fn from_parts_keeps_directive_free_fast_path_o1() {
        let classified = ClassifiedTokens::from_parts(
            vec![1, 2, 3],
            vec![0, 1, 2],
            vec![1, 1, 1],
            vec![0, 0, 0],
            Arc::from(b"abc".as_slice()),
        );

        assert_eq!(classified.directive_count, 0);
        assert!(!classified.has_directives());
        assert_eq!(classified.directive_rows().next(), None);
    }
}

#[derive(Default)]
pub(super) struct TokenizationScratch {
    tok_types_b: Vec<u8>,
    tok_starts_b: Vec<u8>,
    tok_lens_b: Vec<u8>,
    directive_zero: Vec<u8>,
    directive_outputs: Vec<Vec<u8>>,
    sparse_zero: Vec<u8>,
    sparse_outputs: Vec<Vec<u8>>,
    prefix_scan: PrefixScanScratch,
    offsets: Vec<u8>,
    compact_zero: Vec<u8>,
    compact_count_zero: Vec<u8>,
    compact_outputs: Vec<Vec<u8>>,
}

impl TokenizationScratch {
    fn prepare_zero(out: &mut Vec<u8>, byte_len: usize) -> Result<(), String> {
        out.clear();
        reserve_gpu_staging_bytes(out, byte_len, "tokenization zero staging")?;
        out.resize(byte_len, 0);
        Ok(())
    }
}

pub(super) fn reject_invalid_if_expression_values(
    values: &[u32],
    classified: &ClassifiedTokens,
) -> Result<(), String> {
    if !classified.has_directives() {
        return Ok(());
    }
    for (idx, kind) in classified.directive_rows() {
        if !matches!(kind, TOK_PP_IF | TOK_PP_ELIF) {
            continue;
        }
        if values.get(idx).copied() == Some(INVALID_EXPR_VALUE) {
            return Err(format!(
                "gpu_if_expression rejected malformed #if/#elif expression at token {idx}. Fix: repair division/modulo-by-zero or malformed arithmetic before preprocessing."
            ));
        }
    }
    Ok(())
}

/// Run lex + directive classify on `raw` source bytes.
///
/// Dispatches:
///   1. `c11_lexer` (existing GPU kernel) → `(types, starts, lens, n_tokens)`.
///      One output entry per byte position; first `n_tokens` slots are
///      the dense token list, the remainder are zero-padded.
///   2. `gpu_directive_metadata` (17a) → directive kinds per token.
///
/// `raw` should be the post-byte-filter stream (output of
/// `gpu_filter_source_bytes`), but the function works on any byte
/// slice  -  no preprocessing is required for the lexer to produce a
/// valid token list.
///
/// # Errors
/// Returns the dispatcher error verbatim if any stage fails.
pub fn gpu_tokenize_and_classify(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
) -> Result<ClassifiedTokens, String> {
    let mut scratch = TokenizationScratch::default();
    gpu_tokenize_and_classify_with_scratch(dispatcher, raw, &mut scratch)
}

pub(super) fn gpu_tokenize_and_classify_with_scratch(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    scratch: &mut TokenizationScratch,
) -> Result<ClassifiedTokens, String> {
    if raw.is_empty() {
        return Ok(ClassifiedTokens::from_parts(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Arc::from([]),
        ));
    }
    let n_bytes = raw.len() as u32;

    // ---- Stage 1: lex ----
    let (tok_types, tok_starts, tok_lens) = sparse_tokenize(dispatcher, raw, n_bytes, scratch)?;
    let n_tokens = tok_types.len();

    // ---- Stage 2: directive classify ----
    if n_tokens == 0 {
        return Ok(ClassifiedTokens::from_parts(
            tok_types,
            tok_starts,
            tok_lens,
            Vec::new(),
            Arc::from(raw),
        ));
    }
    // Bucket n_tokens so the directive-metadata kernel hits the process-wide
    // pipeline cache across files. Source bytes are runtime-sized now, so they
    // are passed exactly and no longer drive shader shape or padding copies.
    let n_bucket = bucket_pow2(n_tokens.max(1), 64);
    let dm_prog = gpu_directive_metadata_u8(n_bucket as u32, 0);
    let n_pad = n_bucket;
    let _ = n_bytes;
    pack_u32_words_into(&mut scratch.tok_types_b, &tok_types, n_pad)?;
    pack_u32_words_into(&mut scratch.tok_starts_b, &tok_starts, n_pad)?;
    pack_u32_words_into(&mut scratch.tok_lens_b, &tok_lens, n_pad)?;
    TokenizationScratch::prepare_zero(
        &mut scratch.directive_zero,
        u32_word_byte_len(n_pad, "directive metadata zero staging")?,
    )?;
    let dm_inputs: [&[u8]; 6] = [
        scratch.tok_types_b.as_slice(),
        scratch.tok_starts_b.as_slice(),
        scratch.tok_lens_b.as_slice(),
        raw,
        scratch.directive_zero.as_slice(),
        scratch.directive_zero.as_slice(),
    ];
    dispatcher
        .dispatch_borrowed_into(&dm_prog, &dm_inputs, &mut scratch.directive_outputs)
        .map_err(|e| format!("gpu_directive_metadata: {e}"))?;
    if scratch.directive_outputs.len() != 2 {
        return Err(format!(
            "gpu_directive_metadata: expected exactly 2 outputs, got {}. Fix: backend must return directive_kinds/directive_values and no extras.",
            scratch.directive_outputs.len()
        ));
    }
    let directive_kinds = unpack_u32_words_prefix_exact(
        &scratch.directive_outputs[0],
        n_tokens,
        n_pad,
        "gpu_directive_metadata directive_kinds",
    )?;

    Ok(ClassifiedTokens::from_parts(
        tok_types,
        tok_starts,
        tok_lens,
        directive_kinds,
        Arc::from(raw),
    ))
}

pub(super) fn gpu_tokenize_without_directive_metadata(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
) -> Result<ClassifiedTokens, String> {
    let mut scratch = TokenizationScratch::default();
    gpu_tokenize_without_directive_metadata_with_scratch(dispatcher, raw, &mut scratch)
}

pub(super) fn gpu_tokenize_without_directive_metadata_with_scratch(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    scratch: &mut TokenizationScratch,
) -> Result<ClassifiedTokens, String> {
    if raw.is_empty() {
        return Ok(ClassifiedTokens::from_parts(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Arc::from([]),
        ));
    }
    let n_bytes = raw.len() as u32;
    let (tok_types, tok_starts, tok_lens) = sparse_tokenize(dispatcher, raw, n_bytes, scratch)?;
    let directive_kinds = vec![0; tok_types.len()];
    Ok(ClassifiedTokens::from_parts(
        tok_types,
        tok_starts,
        tok_lens,
        directive_kinds,
        Arc::from(raw),
    ))
}

fn sparse_tokenize(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    n_bytes: u32,
    scratch: &mut TokenizationScratch,
) -> Result<(Vec<u32>, Vec<u32>, Vec<u32>), String> {
    // PERF: bucket n_bytes so the sparse lexer + prefix-scan + compact
    // kernel triplet hits the dispatcher's pipeline cache across files.
    // Without bucketing every distinct file size produced a unique
    // program fingerprint and paid a ~2 second cold native-compile per
    // dispatch. Soundness: the lexer's default classification for any
    // byte is `tok_type=TOK_WHITESPACE, emit=0`. The raw-U8 haystack
    // stays runtime-sized; the bucketed sparse lexer reads zero beyond the
    // host-supplied raw length. Padding positions therefore get emit=0,
    // contribute zero to the prefix scan's running sum, and never scatter into
    // the compact output. n_tokens at the end reflects only real-source tokens.
    // PERF 2026-05-10: floor 1024  -  same as gpu_extract_directive_payloads
    // and gpu_tokenize_and_classify. The lexer kernel was previously
    // unbucketed (every distinct file size was its own program); now it
    // shares a small set of bucket programs with the rest of the pipeline.
    let n_bytes_bucket = checked_gpu_u32(
        "sparse tokenizer bucket byte count",
        bucket_pow2(raw.len().max(1), 1024),
    )?;
    let token_capacity = n_bytes_bucket as usize;

    let sparse = c11_lexer_regular_sparse_u8_haystack_with_flags(
        "haystack",
        "sparse_types",
        "sparse_starts",
        "sparse_lens",
        "sparse_flags",
        n_bytes_bucket,
    );
    TokenizationScratch::prepare_zero(
        &mut scratch.sparse_zero,
        u32_word_byte_len(token_capacity, "sparse tokenizer zero staging")?,
    )?;
    let sparse_inputs: [&[u8]; 5] = [
        raw,
        scratch.sparse_zero.as_slice(),
        scratch.sparse_zero.as_slice(),
        scratch.sparse_zero.as_slice(),
        scratch.sparse_zero.as_slice(),
    ];
    dispatcher
        .dispatch_borrowed_into(&sparse, &sparse_inputs, &mut scratch.sparse_outputs)
        .map_err(|e| format!("c11_sparse_lexer preprocess: {e}"))?;
    if scratch.sparse_outputs.len() != 4 {
        return Err(format!(
            "c11_sparse_lexer preprocess: expected exactly 4 output buffers, got {}. Fix: backend must return sparse type/start/len/flag buffers and no extras.",
            scratch.sparse_outputs.len()
        ));
    }
    let _ = n_bytes;

    inclusive_prefix_scan_u32_into(
        dispatcher,
        &scratch.sparse_outputs[3],
        n_bytes_bucket,
        &mut scratch.prefix_scan,
        &mut scratch.offsets,
    )
    .map_err(|e| format!("c11_sparse_lexer preprocess prefix scan: {e}"))?;

    let requires_output_inputs = dispatcher.requires_output_inputs();
    let compact = if requires_output_inputs {
        c11_compact_sparse_tokens(
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "offsets",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            n_bytes_bucket,
        )
    } else {
        c11_compact_sparse_tokens_output(
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "offsets",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            n_bytes_bucket,
        )
    };
    TokenizationScratch::prepare_zero(
        &mut scratch.compact_zero,
        u32_word_byte_len(token_capacity, "compact tokenizer zero staging")?,
    )?;
    TokenizationScratch::prepare_zero(&mut scratch.compact_count_zero, 4)?;
    if requires_output_inputs {
        let compact_inputs: [&[u8]; 8] = [
            scratch.sparse_outputs[0].as_slice(),
            scratch.sparse_outputs[1].as_slice(),
            scratch.sparse_outputs[2].as_slice(),
            scratch.offsets.as_slice(),
            scratch.compact_zero.as_slice(),
            scratch.compact_zero.as_slice(),
            scratch.compact_zero.as_slice(),
            scratch.compact_count_zero.as_slice(),
        ];
        dispatcher
            .dispatch_borrowed_into(&compact, &compact_inputs, &mut scratch.compact_outputs)
            .map_err(|e| format!("c11_sparse_lexer preprocess compact: {e}"))?;
    } else {
        let compact_inputs: [&[u8]; 4] = [
            scratch.sparse_outputs[0].as_slice(),
            scratch.sparse_outputs[1].as_slice(),
            scratch.sparse_outputs[2].as_slice(),
            scratch.offsets.as_slice(),
        ];
        dispatcher
            .dispatch_borrowed_into(&compact, &compact_inputs, &mut scratch.compact_outputs)
            .map_err(|e| format!("c11_sparse_lexer preprocess compact: {e}"))?;
    }
    if scratch.compact_outputs.len() != 4 {
        return Err(format!(
            "c11_sparse_lexer preprocess compact: expected exactly 4 output buffers, got {}. Fix: backend must return dense type/start/len/count buffers and no extras.",
            scratch.compact_outputs.len()
        ));
    }
    let count_buf = &scratch.compact_outputs[3];
    let n_tokens =
        read_u32_scalar_exact(count_buf, "c11_sparse_lexer preprocess compact token count")?
            as usize;
    if n_tokens > token_capacity {
        return Err(format!(
            "c11_sparse_lexer preprocess compact: token count {n_tokens} exceeds output capacity {token_capacity}. Fix: backend must keep out_counts within the dense token table capacity."
        ));
    }
    Ok((
        unpack_u32_words_prefix_exact(
            &scratch.compact_outputs[0],
            n_tokens,
            token_capacity,
            "sparse compact tok_types",
        )?,
        unpack_u32_words_prefix_exact(
            &scratch.compact_outputs[1],
            n_tokens,
            token_capacity,
            "sparse compact tok_starts",
        )?,
        unpack_u32_words_prefix_exact(
            &scratch.compact_outputs[2],
            n_tokens,
            token_capacity,
            "sparse compact tok_lens",
        )?,
    ))
}

#[cfg(test)]

mod tests {
    use super::*;
    use vyre::ir::Program;

    struct SparsePathSentinel;

    impl GpuDispatcher for SparsePathSentinel {
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
        ) -> Result<Vec<Vec<u8>>, String> {
            Err("entered dispatcher".to_string())
        }

        fn requires_output_inputs(&self) -> bool {
            true
        }
    }

    #[test]
    fn sparse_tokenizer_handles_sources_past_legacy_64k_gate() {
        let mut raw = Vec::with_capacity(70_000);
        for i in 0..2500u32 {
            raw.extend_from_slice(format!("#define LONG_GATE_{i} {i}\n").as_bytes());
        }
        assert!(
            raw.len() > 65_536,
            "fixture must exceed the removed sparse-tokenizer byte gate"
        );

        let mut scratch = TokenizationScratch::default();
        let error = sparse_tokenize(&SparsePathSentinel, &raw, raw.len() as u32, &mut scratch)
            .expect_err("large inputs must enter sparse dispatch");
        assert!(
            error.contains("c11_sparse_lexer preprocess: entered dispatcher"),
            "large input must attempt sparse lexer dispatch; got {error}"
        );
    }
}
