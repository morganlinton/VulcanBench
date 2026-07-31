use vyre::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, Program};
use vyre_libs::parsing::c::lex::lexer::{
    c11_compact_sparse_tokens_output, c11_lexer_regular_sparse_packed_haystack_with_block_totals,
    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives,
    c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives_no_backscan,
};
use vyre_primitives::math::prefix_scan::{prefix_scan, ScanKind};
use vyre_primitives::reduce::multi_block_prefix_scan::{
    pass_a_local_scan, pass_c_broadcast_offsets, BLOCK_LANES,
};

use super::buffers::pad_dispatch_input_refs;
use super::prefix_scan_dispatch::{
    dispatch_borrowed_prefix_scan_u32_into, PrefixScanDispatchScratch,
};
use super::sparse_compaction::{
    compact_output_capacity_from_inclusive_offsets,
    pass_c_rescan_compact_sparse_tokens_with_capacity,
};
mod output_collect;
mod resident_stages;

use output_collect::{
    collect_compact_lexer_output_drain, collect_resident_compact_lexer_output_exact_readback,
    mark_output_buffers, resident_output_pairs, returned_buffer_names, take_resident_blob,
    zero_readback_buffers,
};
use resident_stages::{dispatch_sparse_lexer_cached_stages_resident, workgroup_prefix_scan_u32};

use super::backend_select::dispatch_borrowed_stage_cached_into;
use super::{
    dispatch_resident_stage_cached, free_resident_blobs, read_u32_at, stage_pipeline_cache_key,
    validate_internal_stage, ResidentBlob, ResidentStageInput,
};

#[derive(Default)]
pub(super) struct SparseLexerMegakernelScratch {
    sparse_padding: Vec<Vec<u8>>,
    sparse_outputs: Vec<Vec<u8>>,
    compact_padding: Vec<Vec<u8>>,
    compact_outputs: Vec<Vec<u8>>,
    input_padding: Vec<Vec<u8>>,
    fused_outputs: Vec<Vec<u8>>,
    resident_compact_outputs: Vec<Vec<u8>>,
    resident_count_readback: Vec<u8>,
    prefix_scan: PrefixScanDispatchScratch,
    block_totals_scanned: Vec<u8>,
    offsets: Vec<u8>,
}

pub(super) struct SparseLexerMegakernelOutput {
    pub types: Vec<u8>,
    pub starts: Vec<u8>,
    pub lens: Vec<u8>,
    pub counts: Vec<u8>,
    pub n_tokens: u32,
}

pub(super) fn dispatch_sparse_lexer_block_totals_megakernel_with_scratch(
    backend: &dyn VyreBackend,
    haystack_bytes: &[u8],
    haystack_len: u32,
    config: &mut DispatchConfig,
    label: &str,
    scratch: &mut SparseLexerMegakernelScratch,
) -> Result<SparseLexerMegakernelOutput, String> {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let num_blocks = haystack_len.div_ceil(BLOCK_LANES).max(1);
    let sparse = c11_lexer_regular_sparse_packed_haystack_with_block_totals(
        "haystack",
        "sparse_types",
        "sparse_starts",
        "sparse_lens",
        "scratch_counts",
        "block_totals",
        haystack_len,
    );
    validate_internal_stage(&sparse, "syntax_sparse_block_total_stage_sparse")?;
    let sparse_refs =
        pad_dispatch_input_refs(&sparse, vec![haystack_bytes], &mut scratch.sparse_padding);
    config.label = Some(format!("{label} sparse-block-total-stage-sparse"));
    let sparse_key = stage_pipeline_cache_key(
        "syntax_sparse_block_total_stage_sparse",
        &[haystack_len as u64, num_blocks as u64],
    );
    let cached_sparse = sparse.clone();
    dispatch_borrowed_stage_cached_into(
        backend,
        sparse_key,
        || Ok(cached_sparse),
        &sparse_refs,
        config,
        &mut scratch.sparse_outputs,
    )
    .map_err(|e| format!("{label} sparse block-total stage sparse dispatch failed: {e}"))?;
    let sparse_names = returned_buffer_names(
        &sparse,
        scratch.sparse_outputs.len(),
        label,
        "block-total stage sparse",
    )?;
    let mut sparse_types = None;
    let mut sparse_starts = None;
    let mut sparse_lens = None;
    let mut block_totals = None;
    for (name, value) in sparse_names
        .into_iter()
        .zip(scratch.sparse_outputs.drain(..))
    {
        match name.as_str() {
            "sparse_types" => sparse_types = Some(value),
            "sparse_starts" => sparse_starts = Some(value),
            "sparse_lens" => sparse_lens = Some(value),
            "block_totals" => block_totals = Some(value),
            _ => {}
        }
    }
    let sparse_types = sparse_types
        .ok_or_else(|| format!("{label} sparse block-total stage sparse missing sparse_types"))?;
    let sparse_starts = sparse_starts
        .ok_or_else(|| format!("{label} sparse block-total stage sparse missing sparse_starts"))?;
    let sparse_lens = sparse_lens
        .ok_or_else(|| format!("{label} sparse block-total stage sparse missing sparse_lens"))?;
    let block_totals = block_totals
        .ok_or_else(|| format!("{label} sparse block-total stage sparse missing block_totals"))?;

    dispatch_borrowed_prefix_scan_u32_into(
        backend,
        &block_totals,
        num_blocks,
        config,
        label,
        "syntax_sparse_block_total_stage_scan",
        &mut scratch.block_totals_scanned,
        &mut scratch.prefix_scan,
    )?;
    let compact_capacity = compact_output_capacity_from_inclusive_offsets(
        &scratch.block_totals_scanned,
        num_blocks,
        label,
        "sparse block-total stage scan",
    )?;

    let compact = mark_output_buffers(
        pass_c_rescan_compact_sparse_tokens_with_capacity(
            "block_totals_scanned",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            haystack_len,
            num_blocks,
            compact_capacity,
        ),
        &[
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
        ],
    );
    validate_internal_stage(&compact, "syntax_sparse_block_total_stage_compact")?;
    let compact_refs = pad_dispatch_input_refs(
        &compact,
        vec![
            scratch.block_totals_scanned.as_slice(),
            sparse_types.as_slice(),
            sparse_starts.as_slice(),
            sparse_lens.as_slice(),
        ],
        &mut scratch.compact_padding,
    );
    config.label = Some(format!("{label} sparse-block-total-stage-compact"));
    let key = stage_pipeline_cache_key(
        "syntax_sparse_block_total_stage_compact",
        &[
            haystack_len as u64,
            num_blocks as u64,
            compact_capacity as u64,
        ],
    );
    let cached_compact = compact.clone();
    dispatch_borrowed_stage_cached_into(
        backend,
        key,
        || Ok(cached_compact),
        &compact_refs,
        config,
        &mut scratch.compact_outputs,
    )
    .map_err(|e| format!("{label} sparse block-total stage compact dispatch failed: {e}"))?;
    collect_compact_lexer_output_drain(
        &compact,
        &mut scratch.compact_outputs,
        label,
        "block-total stages",
    )
}

pub(super) fn dispatch_sparse_lexer_megakernel_with_scratch(
    backend: &dyn VyreBackend,
    haystack_bytes: &[u8],
    haystack_len: u32,
    config: &mut DispatchConfig,
    label: &str,
    scratch: &mut SparseLexerMegakernelScratch,
) -> Result<SparseLexerMegakernelOutput, String> {
    if backend.id() == "cuda" {
        return dispatch_sparse_lexer_cached_stages_with_scratch(
            backend,
            haystack_bytes,
            haystack_len,
            config,
            label,
            false,
            scratch,
        );
    }
    if haystack_len > BLOCK_LANES {
        return dispatch_sparse_lexer_block_totals_megakernel_with_scratch(
            backend,
            haystack_bytes,
            haystack_len,
            config,
            label,
            scratch,
        );
    }
    let sparse = zero_readback_buffers(
        c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives(
            "haystack",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "sparse_flags",
            haystack_len,
        ),
        &[
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "sparse_flags",
        ],
    );
    let scan = zero_readback_buffers(
        workgroup_prefix_scan_u32("sparse_flags", "offsets", haystack_len),
        &["offsets"],
    );
    let compact = c11_compact_sparse_tokens_output(
        "sparse_types",
        "sparse_starts",
        "sparse_lens",
        "offsets",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        haystack_len,
    );
    let fused = vyre_foundation::execution_plan::fusion::fuse_programs(&[sparse, scan, compact])
        .map_err(|error| format!("{label} sparse lexer megakernel fusion failed: {error}"))?
        .with_entry_op_id("vyre-frontend-c::syntax_sparse_lexer_megakernel");
    validate_internal_stage(&fused, "syntax_sparse_lexer_megakernel")?;

    let refs = pad_dispatch_input_refs(&fused, vec![haystack_bytes], &mut scratch.input_padding);
    config.label = Some(format!("{label} sparse-lexer-megakernel"));
    let key = stage_pipeline_cache_key("syntax_sparse_lexer_megakernel", &[haystack_len as u64]);
    let cached_fused = fused.clone();
    dispatch_borrowed_stage_cached_into(
        backend,
        key,
        || Ok(cached_fused),
        &refs,
        config,
        &mut scratch.fused_outputs,
    )
    .map_err(|e| format!("{label} sparse lexer megakernel dispatch failed: {e}"))?;

    collect_compact_lexer_output_drain(&fused, &mut scratch.fused_outputs, label, "megakernel")
}

pub(super) fn dispatch_sparse_lexer_no_literal_backscan_with_scratch(
    backend: &dyn VyreBackend,
    haystack_bytes: &[u8],
    haystack_len: u32,
    config: &mut DispatchConfig,
    label: &str,
    scratch: &mut SparseLexerMegakernelScratch,
) -> Result<SparseLexerMegakernelOutput, String> {
    dispatch_sparse_lexer_cached_stages_with_scratch(
        backend,
        haystack_bytes,
        haystack_len,
        config,
        label,
        true,
        scratch,
    )
}

fn dispatch_sparse_lexer_cached_stages_with_scratch(
    backend: &dyn VyreBackend,
    haystack_bytes: &[u8],
    haystack_len: u32,
    config: &mut DispatchConfig,
    label: &str,
    no_literal_backscan: bool,
    scratch: &mut SparseLexerMegakernelScratch,
) -> Result<SparseLexerMegakernelOutput, String> {
    if backend.id() == "cuda" {
        return dispatch_sparse_lexer_cached_stages_resident(
            backend,
            haystack_bytes,
            haystack_len,
            config,
            label,
            no_literal_backscan,
            scratch,
        );
    }
    if !no_literal_backscan && haystack_len > BLOCK_LANES {
        return dispatch_sparse_lexer_block_totals_megakernel_with_scratch(
            backend,
            haystack_bytes,
            haystack_len,
            config,
            label,
            scratch,
        );
    }

    let sparse = if no_literal_backscan {
        c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives_no_backscan(
            "haystack",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "sparse_flags",
            haystack_len,
        )
    } else {
        c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives(
            "haystack",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "sparse_flags",
            haystack_len,
        )
    };
    validate_internal_stage(&sparse, "syntax_sparse_lexer_stage_sparse")?;
    let sparse_refs =
        pad_dispatch_input_refs(&sparse, vec![haystack_bytes], &mut scratch.sparse_padding);
    config.label = Some(format!("{label} sparse-lexer-stage-sparse"));
    let sparse_key = stage_pipeline_cache_key(
        if no_literal_backscan {
            "syntax_sparse_lexer_stage_sparse_no_literal_backscan"
        } else {
            "syntax_sparse_lexer_stage_sparse"
        },
        &[haystack_len as u64],
    );
    let cached_sparse = sparse.clone();
    dispatch_borrowed_stage_cached_into(
        backend,
        sparse_key,
        || Ok(cached_sparse),
        &sparse_refs,
        config,
        &mut scratch.sparse_outputs,
    )
    .map_err(|e| format!("{label} sparse lexer stage sparse dispatch failed: {e}"))?;
    let sparse_names =
        returned_buffer_names(&sparse, scratch.sparse_outputs.len(), label, "stage sparse")?;
    let mut sparse_types = None;
    let mut sparse_starts = None;
    let mut sparse_lens = None;
    let mut sparse_flags = None;
    for (name, value) in sparse_names
        .into_iter()
        .zip(scratch.sparse_outputs.drain(..))
    {
        match name.as_str() {
            "sparse_types" => sparse_types = Some(value),
            "sparse_starts" => sparse_starts = Some(value),
            "sparse_lens" => sparse_lens = Some(value),
            "sparse_flags" => sparse_flags = Some(value),
            _ => {}
        }
    }
    let sparse_types = sparse_types
        .ok_or_else(|| format!("{label} sparse lexer stage sparse missing sparse_types"))?;
    let sparse_starts = sparse_starts
        .ok_or_else(|| format!("{label} sparse lexer stage sparse missing sparse_starts"))?;
    let sparse_lens = sparse_lens
        .ok_or_else(|| format!("{label} sparse lexer stage sparse missing sparse_lens"))?;
    let sparse_flags = sparse_flags
        .ok_or_else(|| format!("{label} sparse lexer stage sparse missing sparse_flags"))?;

    dispatch_borrowed_prefix_scan_u32_into(
        backend,
        &sparse_flags,
        haystack_len,
        config,
        label,
        "syntax_sparse_lexer_stage_scan",
        &mut scratch.offsets,
        &mut scratch.prefix_scan,
    )?;
    let compact_capacity = compact_output_capacity_from_inclusive_offsets(
        &scratch.offsets,
        haystack_len,
        label,
        "sparse lexer stage scan",
    )?;

    let compact = c11_compact_sparse_tokens_output(
        "sparse_types",
        "sparse_starts",
        "sparse_lens",
        "offsets",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        compact_capacity,
    );
    validate_internal_stage(&compact, "syntax_sparse_lexer_stage_compact")?;
    let compact_refs = pad_dispatch_input_refs(
        &compact,
        vec![
            sparse_types.as_slice(),
            sparse_starts.as_slice(),
            sparse_lens.as_slice(),
            scratch.offsets.as_slice(),
        ],
        &mut scratch.compact_padding,
    );
    config.label = Some(format!("{label} sparse-lexer-stage-compact"));
    let compact_key = stage_pipeline_cache_key(
        "syntax_sparse_lexer_stage_compact",
        &[haystack_len as u64, compact_capacity as u64],
    );
    let cached_compact = compact.clone();
    dispatch_borrowed_stage_cached_into(
        backend,
        compact_key,
        || Ok(cached_compact),
        &compact_refs,
        config,
        &mut scratch.compact_outputs,
    )
    .map_err(|e| format!("{label} sparse lexer stage compact dispatch failed: {e}"))?;
    collect_compact_lexer_output_drain(
        &compact,
        &mut scratch.compact_outputs,
        label,
        "cached stages",
    )
}
