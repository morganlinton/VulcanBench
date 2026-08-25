use std::cell::RefCell;

use super::sparse_lexer_megakernel::{
    dispatch_sparse_lexer_megakernel_with_scratch,
    dispatch_sparse_lexer_no_literal_backscan_with_scratch, SparseLexerMegakernelScratch,
};
use super::*;

#[derive(Default)]
pub(super) struct CudaSparseLexerScratch {
    sparse_lexer: SparseLexerMegakernelScratch,
}

pub(super) struct CudaSparseLexerDispatch {
    pub(super) types: Vec<u8>,
    pub(super) starts: Vec<u8>,
    pub(super) lens: Vec<u8>,
    pub(super) counts: Vec<u8>,
    pub(super) n_tokens: u32,
    pub(super) packed_haystack: Vec<u8>,
    pub(super) haystack_len: u32,
    pub(super) keyword_promoted: bool,
}

#[derive(Default)]
struct DenseLexerDiffScratch {
    haystack: Vec<u8>,
    padding: Vec<Vec<u8>>,
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static DENSE_LEXER_DIFF_SCRATCH: RefCell<DenseLexerDiffScratch> =
        RefCell::new(DenseLexerDiffScratch::default());
}

pub(super) fn dispatch_cuda_sparse_lexer_if_available_with_scratch(
    backend: &dyn VyreBackend,
    source: &[u8],
    config: &mut DispatchConfig,
    label: &str,
    scratch: &mut CudaSparseLexerScratch,
) -> Result<Option<CudaSparseLexerDispatch>, String> {
    let strategy = cuda_sparse_lexer_strategy(backend, source)?;
    if strategy == CudaSparseLexerStrategy::None {
        return Ok(None);
    }
    let (haystack, haystack_len) = cuda_lexer_haystack_view(source)?;
    let out = match strategy {
        CudaSparseLexerStrategy::None => unreachable!(),
        CudaSparseLexerStrategy::FastNoLiterals => {
            dispatch_sparse_lexer_no_literal_backscan_with_scratch(
                backend,
                &haystack,
                haystack_len,
                config,
                label,
                &mut scratch.sparse_lexer,
            )?
        }
        CudaSparseLexerStrategy::Megakernel => dispatch_sparse_lexer_megakernel_with_scratch(
            backend,
            &haystack,
            haystack_len,
            config,
            label,
            &mut scratch.sparse_lexer,
        )?,
    };
    Ok(Some(CudaSparseLexerDispatch {
        types: out.types,
        starts: out.starts,
        lens: out.lens,
        counts: out.counts,
        n_tokens: out.n_tokens,
        packed_haystack: haystack,
        haystack_len,
        keyword_promoted: false,
    }))
}

pub(super) fn dispatch_dense_lexer_for_sparse_diff(
    backend: &dyn VyreBackend,
    source: &str,
    config: &mut DispatchConfig,
    label: &str,
) -> Result<(Vec<u32>, Vec<u32>, Vec<u32>), String> {
    DENSE_LEXER_DIFF_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "dense lexer diff dispatch scratch was re-entered on the same thread. Fix: call sparse/dense diffing from a non-nested lexer context or add explicit caller-owned scratch.".to_string()
        })?;
        dispatch_dense_lexer_for_sparse_diff_with_scratch(backend, source, config, label, &mut scratch)
    })
}

fn dispatch_dense_lexer_for_sparse_diff_with_scratch(
    backend: &dyn VyreBackend,
    source: &str,
    config: &mut DispatchConfig,
    label: &str,
    scratch: &mut DenseLexerDiffScratch,
) -> Result<(Vec<u32>, Vec<u32>, Vec<u32>), String> {
    let haystack_len = pack_dense_diff_haystack_into(source, &mut scratch.haystack)?;
    let lex_prog = c11_lex_single_pass(
        "haystack",
        "dense_tok_types",
        "dense_tok_starts",
        "dense_tok_lens",
        "dense_counts",
        haystack_len,
        haystack_len.max(1),
    );
    validate_internal_stage(&lex_prog, "c11_lexer")?;
    let refs = pad_dispatch_input_refs(
        &lex_prog,
        vec![scratch.haystack.as_slice()],
        &mut scratch.padding,
    );
    config.label = Some(format!("{label} dense-diff"));
    dispatch_borrowed_cached_into(backend, &lex_prog, &refs, config, &mut scratch.outputs)
        .map_err(|e| format!("{label} dense lexer diff dispatch failed: {e}"))?;
    if scratch.outputs.len() != 4 {
        return Err(format!(
            "{label} dense lexer diff expected exactly 4 output buffers, got {}. Fix: backend must return token type/start/len/count buffers and no extras.",
            scratch.outputs.len()
        ));
    }
    let types = &scratch.outputs[0];
    let starts = &scratch.outputs[1];
    let lens = &scratch.outputs[2];
    let counts = &scratch.outputs[3];
    let n_tokens = read_u32_at(&counts, 0)
        .map_err(|e| format!("{label} dense lexer diff count decode failed: {e}"))?;
    Ok((
        token_types_from_lex(&types, n_tokens)?,
        read_u32_stream(&starts, n_tokens as usize, "dense lexer diff starts")?,
        read_u32_stream(&lens, n_tokens as usize, "dense lexer diff lengths")?,
    ))
}

fn pack_dense_diff_haystack_into(source: &str, bytes: &mut Vec<u8>) -> Result<u32, String> {
    let haystack_u32_count = u32::try_from(source.len())
        .map_err(|_| {
            format!(
                "C frontend source length {} exceeds the u32 GPU index space. Fix: shard the translation unit before packing the haystack.",
                source.len()
            )
        })?
        .max(1);
    let byte_len = (haystack_u32_count as usize).checked_mul(4).ok_or_else(|| {
        format!(
            "dense lexer diff haystack word count {haystack_u32_count} overflows host byte length. Fix: shard the translation unit before packing."
        )
    })?;
    bytes.clear();
    bytes.resize(byte_len, 0);
    for (index, byte) in source.bytes().enumerate() {
        bytes[index * 4] = byte;
    }
    Ok(haystack_u32_count)
}

pub(super) fn reject_sparse_dense_lexer_mismatch(
    backend: &dyn VyreBackend,
    source: &str,
    sparse_types: &[u8],
    sparse_starts: &[u8],
    sparse_lens: &[u8],
    sparse_n_tokens: u32,
    config: &mut DispatchConfig,
    label: &str,
) -> Result<(), String> {
    if std::env::var_os("VYRE_FRONTEND_C_SPARSE_DENSE_DIFF").is_none() {
        return Ok(());
    }
    let sparse_types = token_types_from_lex(sparse_types, sparse_n_tokens)?;
    let sparse_starts = read_u32_stream(
        sparse_starts,
        sparse_n_tokens as usize,
        "sparse lexer diff starts",
    )?;
    let sparse_lens = read_u32_stream(
        sparse_lens,
        sparse_n_tokens as usize,
        "sparse lexer diff lengths",
    )?;
    let (dense_types, dense_starts, dense_lens) =
        dispatch_dense_lexer_for_sparse_diff(backend, source, config, label)?;
    let common = dense_types.len().min(sparse_types.len());
    for idx in 0..common {
        if dense_types[idx] != sparse_types[idx]
            || dense_starts[idx] != sparse_starts[idx]
            || dense_lens[idx] != sparse_lens[idx]
        {
            return Err(format!(
                "{label} sparse/dense lexer mismatch at token {idx}: dense(type={}, start={}, len={}, text={:?}, context={:?}) sparse(type={}, start={}, len={}, text={:?}, context={:?}). Fix: keep CUDA sparse lexer byte-for-byte equivalent to dense lexer before enabling it as a release path.",
                dense_types[idx],
                dense_starts[idx],
                dense_lens[idx],
                lexer_debug_snippet(source, dense_starts[idx], dense_lens[idx]),
                lexer_debug_context(source, dense_starts[idx]),
                sparse_types[idx],
                sparse_starts[idx],
                sparse_lens[idx],
                lexer_debug_snippet(source, sparse_starts[idx], sparse_lens[idx]),
                lexer_debug_context(source, sparse_starts[idx]),
            ));
        }
    }
    if dense_types.len() != sparse_types.len() {
        let idx = common;
        let dense_tail = dense_types.get(idx).map(|kind| {
            format!(
                "type={}, start={}, len={}, text={:?}",
                kind,
                dense_starts[idx],
                dense_lens[idx],
                lexer_debug_snippet(source, dense_starts[idx], dense_lens[idx])
            )
        });
        let sparse_tail = sparse_types.get(idx).map(|kind| {
            format!(
                "type={}, start={}, len={}, text={:?}",
                kind,
                sparse_starts[idx],
                sparse_lens[idx],
                lexer_debug_snippet(source, sparse_starts[idx], sparse_lens[idx])
            )
        });
        return Err(format!(
            "{label} sparse/dense lexer token-count mismatch: dense={} sparse={} first_tail_index={} dense_tail={:?} sparse_tail={:?}. Fix: keep CUDA sparse lexer token compaction equivalent to dense lexer.",
            dense_types.len(),
            sparse_types.len(),
            idx,
            dense_tail,
            sparse_tail,
        ));
    }
    Ok(())
}

pub(super) fn lexer_debug_snippet(source: &str, start: u32, len: u32) -> String {
    let start = start as usize;
    let end = source.len().min(start.saturating_add(len as usize));
    source
        .get(start..end)
        .unwrap_or("<invalid utf8 boundary>")
        .chars()
        .take(80)
        .collect()
}

pub(super) fn lexer_debug_context(source: &str, start: u32) -> String {
    let center = start as usize;
    let start = center.saturating_sub(96);
    let end = source.len().min(center.saturating_add(96));
    source
        .get(start..end)
        .unwrap_or("<invalid utf8 boundary>")
        .chars()
        .collect()
}
