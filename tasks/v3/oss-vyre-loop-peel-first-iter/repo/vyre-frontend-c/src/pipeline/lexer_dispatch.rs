use std::cell::RefCell;

use super::lexer_sparse_cuda::{
    dispatch_cuda_sparse_lexer_if_available_with_scratch, CudaSparseLexerScratch,
};
use super::sparse_compaction::{
    compact_sparse_lexer_outputs_gpu_with_scratch, SparseCompactionScratch,
};
use super::*;

#[derive(Default)]
pub(in crate::pipeline) struct LexDispatchScratch {
    lex_haystack_padded: Vec<u8>,
    lex_padding: Vec<Vec<u8>>,
    lex_out: Vec<Vec<u8>>,
    sparse_compaction: SparseCompactionScratch,
    cuda_sparse: CudaSparseLexerScratch,
}

pub(in crate::pipeline) struct C11LexTokens {
    pub(in crate::pipeline) types: Vec<u8>,
    pub(in crate::pipeline) starts: Vec<u8>,
    pub(in crate::pipeline) lens: Vec<u8>,
    pub(in crate::pipeline) counts: Vec<u8>,
    pub(in crate::pipeline) n_tokens: u32,
    pub(in crate::pipeline) keyword_promoted: bool,
    pub(in crate::pipeline) cuda_keyword_haystack: Option<(Vec<u8>, u32)>,
}

thread_local! {
    static LEX_DISPATCH_SCRATCH: RefCell<LexDispatchScratch> =
        RefCell::new(LexDispatchScratch::default());
}

pub(in crate::pipeline) fn lex_c11_tokens(
    backend: &dyn VyreBackend,
    source: &str,
    dcfg: &mut DispatchConfig,
    expanded_haystack_cache: &mut Option<(Vec<u8>, u32)>,
    dispatch_label: &str,
    mismatch_label: &str,
    mut log: impl FnMut(&str),
) -> Result<C11LexTokens, String> {
    LEX_DISPATCH_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "c11 lexer dispatch scratch was re-entered on the same thread. Fix: call lex_c11_tokens_with_scratch with an explicit caller-owned LexDispatchScratch for nested parser use.".to_string()
        })?;
        lex_c11_tokens_with_scratch(
            backend,
            source,
            dcfg,
            expanded_haystack_cache,
            dispatch_label,
            mismatch_label,
            &mut log,
            &mut scratch,
        )
    })
}

pub(in crate::pipeline) fn lex_c11_tokens_with_scratch(
    backend: &dyn VyreBackend,
    source: &str,
    dcfg: &mut DispatchConfig,
    expanded_haystack_cache: &mut Option<(Vec<u8>, u32)>,
    dispatch_label: &str,
    mismatch_label: &str,
    mut log: impl FnMut(&str),
    scratch: &mut LexDispatchScratch,
) -> Result<C11LexTokens, String> {
    let mut cuda_keyword_haystack = None;
    let (mut types, mut starts, mut lens, counts, n_tokens, keyword_promoted) = if let Some(
        cuda_lex,
    ) =
        dispatch_cuda_sparse_lexer_if_available_with_scratch(
            backend,
            source.as_bytes(),
            dcfg,
            dispatch_label,
            &mut scratch.cuda_sparse,
        )? {
        log("dispatch CUDA sparse lexer megakernel");
        cuda_keyword_haystack = Some((cuda_lex.packed_haystack, cuda_lex.haystack_len));
        (
            cuda_lex.types,
            cuda_lex.starts,
            cuda_lex.lens,
            cuda_lex.counts,
            cuda_lex.n_tokens,
            cuda_lex.keyword_promoted,
        )
    } else {
        let was_packed = expanded_haystack_cache.is_none();
        let (dense_haystack, dense_haystack_len) =
            expanded_haystack(expanded_haystack_cache, source)?;
        if was_packed {
            log("pack_haystack");
        }
        let (lex_haystack, lex_haystack_bucket) = bucketed_dense_lex_haystack(
            dense_haystack,
            dense_haystack_len,
            &mut scratch.lex_haystack_padded,
        );
        let lex_plan = c11_lex_program_for_source(
            source,
            lex_haystack_bucket,
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
        );
        let lex_prog = &lex_plan.program;
        validate_internal_stage(lex_prog, "c11_lexer")?;
        let lex_refs =
            pad_dispatch_input_refs(lex_prog, vec![lex_haystack], &mut scratch.lex_padding);
        dcfg.label = Some(dispatch_label.to_string());
        dispatch_borrowed_cached_into(backend, lex_prog, &lex_refs, dcfg, &mut scratch.lex_out)
            .map_err(|error| format!("{dispatch_label} c11_lexer dispatch failed: {error}"))?;
        log("dispatch c11_lexer");
        if scratch.lex_out.len() != 4 {
            return Err(format!(
                    "{dispatch_label}: expected exactly 4 lexer output buffers, got {}. Fix: backend must return token type/start/len/count buffers and no extras.",
                    scratch.lex_out.len()
                ));
        }
        let mut lex_outputs = scratch.lex_out.drain(..);
        let types_raw = lex_outputs
            .next()
            .ok_or_else(|| format!("{dispatch_label}: missing lexer types output"))?;
        let starts_raw = lex_outputs
            .next()
            .ok_or_else(|| format!("{dispatch_label}: missing lexer starts output"))?;
        let lens_raw = lex_outputs
            .next()
            .ok_or_else(|| format!("{dispatch_label}: missing lexer lens output"))?;
        let counts_raw = lex_outputs
            .next()
            .ok_or_else(|| format!("{dispatch_label}: missing lexer counts output"))?;
        if lex_plan.sparse_output {
            let (types, starts, lens, counts, n_tokens) =
                compact_sparse_lexer_outputs_gpu_with_scratch(
                    backend,
                    types_raw,
                    starts_raw,
                    lens_raw,
                    counts_raw,
                    lex_haystack_bucket,
                    dcfg,
                    dispatch_label,
                    &mut scratch.sparse_compaction,
                )?;
            (
                types,
                starts,
                lens,
                counts,
                n_tokens,
                lex_plan.keyword_promoted,
            )
        } else {
            let n_tokens =
                read_u32_at(&counts_raw, 0).map_err(|error| format!("lexer count: {error}"))?;
            (
                types_raw,
                starts_raw,
                lens_raw,
                counts_raw,
                n_tokens,
                lex_plan.keyword_promoted,
            )
        }
    };
    truncate_lexer_outputs_to_logical_tokens(&mut types, &mut starts, &mut lens, n_tokens)?;
    if cuda_keyword_haystack.is_some() {
        reject_sparse_dense_lexer_mismatch(
            backend,
            source,
            &types,
            &starts,
            &lens,
            n_tokens,
            dcfg,
            mismatch_label,
        )?;
    }
    Ok(C11LexTokens {
        types,
        starts,
        lens,
        counts,
        n_tokens,
        keyword_promoted,
        cuda_keyword_haystack,
    })
}
