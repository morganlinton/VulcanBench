use std::mem;

use vyre::{DispatchConfig, VyreBackend};
use vyre_libs::parsing::c::lex::lexer::c11_compact_sparse_tokens;

use super::buffers::{pad_dispatch_input_refs, read_u32_at};
use super::prefix_scan_dispatch::{
    dispatch_borrowed_prefix_scan_u32_into, PrefixScanDispatchScratch,
};
use super::sparse_prefix_programs::prefix_scan_nonzero_workgroup;
use super::{dispatch_borrowed_cached_into, validate_internal_stage};

mod programs;
use programs::block_totals_nonzero_scan;
pub(super) use programs::pass_c_rescan_compact_sparse_tokens_with_capacity;

#[derive(Default)]
pub(super) struct SparseCompactionScratch {
    pass_a_outputs: Vec<Vec<u8>>,
    compact_outputs: Vec<Vec<u8>>,
    compact_padding: Vec<Vec<u8>>,
    prefix_scan: PrefixScanDispatchScratch,
    block_totals: Vec<u8>,
    block_totals_scanned: Vec<u8>,
    offsets: Vec<u8>,
    compact_types: Vec<u8>,
    compact_starts: Vec<u8>,
    compact_lens: Vec<u8>,
    compact_counts: Vec<u8>,
}

#[allow(clippy::type_complexity)]
pub(super) fn compact_sparse_lexer_outputs_gpu_with_scratch(
    backend: &dyn VyreBackend,
    types_sparse: Vec<u8>,
    starts_sparse: Vec<u8>,
    lens_sparse: Vec<u8>,
    _flags: Vec<u8>,
    haystack_len: u32,
    config: &mut DispatchConfig,
    label: &str,
    scratch: &mut SparseCompactionScratch,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, u32), String> {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let sparse_logical_bytes = sparse_logical_byte_len(haystack_len, label)?;
    let types_sparse = sparse_logical_slice(&types_sparse, sparse_logical_bytes, label, "types")?;
    let starts_sparse =
        sparse_logical_slice(&starts_sparse, sparse_logical_bytes, label, "starts")?;
    let lens_sparse = sparse_logical_slice(&lens_sparse, sparse_logical_bytes, label, "lens")?;

    if haystack_len > BLOCK_LANES {
        let num_blocks = haystack_len.div_ceil(BLOCK_LANES);
        let pass_a =
            block_totals_nonzero_scan("sparse_types", "block_totals", haystack_len, num_blocks);
        validate_internal_stage(&pass_a, "sparse_token_compact_pass_a_nonzero")?;
        config.label = Some(format!("{label} sparse-compact-pass-a"));
        dispatch_borrowed_cached_into(
            backend,
            &pass_a,
            &[types_sparse],
            config,
            &mut scratch.pass_a_outputs,
        )
        .map_err(|e| format!("{label} sparse token compact pass A dispatch failed: {e}"))?;
        take_single_output_into(
            &mut scratch.pass_a_outputs,
            &mut scratch.block_totals,
            || format!("{label} sparse token compact pass A: missing block totals"),
        )?;

        dispatch_borrowed_prefix_scan_u32_into(
            backend,
            &scratch.block_totals,
            num_blocks,
            config,
            label,
            "sparse_token_compact_pass_b",
            &mut scratch.block_totals_scanned,
            &mut scratch.prefix_scan,
        )?;
        let compact_capacity = compact_output_capacity_from_inclusive_offsets(
            &scratch.block_totals_scanned,
            num_blocks,
            label,
            "sparse token compact pass B",
        )?;

        let compact_prog = pass_c_rescan_compact_sparse_tokens_with_capacity(
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
        );
        validate_internal_stage(&compact_prog, "sparse_token_compact_pass_c_rescan")?;
        config.label = Some(format!("{label} sparse-compact-pass-c"));
        let compact_refs = pad_dispatch_input_refs(
            &compact_prog,
            vec![
                scratch.block_totals_scanned.as_slice(),
                types_sparse,
                starts_sparse,
                lens_sparse,
            ],
            &mut scratch.compact_padding,
        );
        dispatch_borrowed_cached_into(
            backend,
            &compact_prog,
            &compact_refs,
            config,
            &mut scratch.compact_outputs,
        )
        .map_err(|e| format!("{label} sparse token compact pass C dispatch failed: {e}"))?;
        if scratch.compact_outputs.len() != 4 {
            return Err(format!(
                "{label} sparse token compact pass C: expected exactly 4 outputs, got {}. Fix: backend must return token type/start/len/count buffers and no extras.",
                scratch.compact_outputs.len()
            ));
        }
        let (types, starts, lens, counts) = take_four_outputs(
            &mut scratch.compact_outputs,
            &format!("{label} sparse token compact pass C"),
        )?;
        let n_tokens =
            read_u32_at(&counts, 0).map_err(|e| format!("{label} sparse token count: {e}"))?;
        return Ok((types, starts, lens, counts, n_tokens));
    }

    sparse_prefix_offsets_gpu_into(
        backend,
        types_sparse,
        haystack_len,
        config,
        label,
        &mut scratch.offsets,
        &mut scratch.pass_a_outputs,
    )?;
    let compact_capacity = compact_output_capacity_from_inclusive_offsets(
        &scratch.offsets,
        haystack_len,
        label,
        "sparse token compact prefix scan",
    )?;

    let compact_prog = c11_compact_sparse_tokens(
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
    validate_internal_stage(&compact_prog, "c11_compact_sparse_tokens")?;
    let compact_bytes = sparse_logical_byte_len(compact_capacity, label)?;
    scratch.compact_types.clear();
    scratch.compact_types.resize(compact_bytes, 0);
    scratch.compact_starts.clear();
    scratch.compact_starts.resize(compact_bytes, 0);
    scratch.compact_lens.clear();
    scratch.compact_lens.resize(compact_bytes, 0);
    scratch.compact_counts.clear();
    scratch.compact_counts.resize(4, 0);
    config.label = Some(format!("{label} sparse-compact"));
    dispatch_borrowed_cached_into(
        backend,
        &compact_prog,
        &[
            types_sparse,
            starts_sparse,
            lens_sparse,
            &scratch.offsets,
            &scratch.compact_types,
            &scratch.compact_starts,
            &scratch.compact_lens,
            &scratch.compact_counts,
        ],
        config,
        &mut scratch.compact_outputs,
    )
    .map_err(|e| format!("{label} sparse token compact dispatch failed: {e}"))?;
    if scratch.compact_outputs.len() != 4 {
        return Err(format!(
            "{label} sparse token compact: expected exactly 4 outputs, got {}. Fix: backend must return token type/start/len/count buffers and no extras.",
            scratch.compact_outputs.len()
        ));
    }
    let (types, starts, lens, counts) = take_four_outputs(
        &mut scratch.compact_outputs,
        &format!("{label} sparse token compact"),
    )?;
    let n_tokens =
        read_u32_at(&counts, 0).map_err(|e| format!("{label} sparse token count: {e}"))?;
    Ok((types, starts, lens, counts, n_tokens))
}

fn sparse_logical_byte_len(haystack_len: u32, label: &str) -> Result<usize, String> {
    usize::try_from(haystack_len.max(1))
        .ok()
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "{label} sparse token compact byte length overflowed for {haystack_len} lanes. Fix: shard the translation unit before sparse lexer compaction."
            )
        })
}

pub(super) fn compact_output_capacity_from_inclusive_offsets(
    offsets: &[u8],
    logical_items: u32,
    label: &str,
    stage: &str,
) -> Result<u32, String> {
    if logical_items == 0 {
        return Ok(1);
    }
    let last_index = usize::try_from(logical_items - 1).map_err(|error| {
        format!(
            "{label} {stage} logical item count {logical_items} does not fit usize: {error}. Fix: shard the sparse lexer compaction input."
        )
    })?;
    let last_byte = last_index.checked_mul(4).ok_or_else(|| {
        format!(
            "{label} {stage} final offset byte index overflows for logical item {last_index}. Fix: shard the sparse lexer compaction input."
        )
    })?;
    let token_count = read_u32_at(offsets, last_byte)
        .map_err(|error| format!("{label} {stage} token count readback: {error}"))?;
    Ok(token_count.max(1))
}

fn sparse_logical_slice<'a>(
    bytes: &'a [u8],
    logical_bytes: usize,
    label: &str,
    stream: &str,
) -> Result<&'a [u8], String> {
    bytes.get(..logical_bytes).ok_or_else(|| {
        format!(
            "{label} sparse token compact {stream} stream has {} bytes but needs {logical_bytes} logical bytes. Fix: lexer sparse output must cover every source lane before compaction.",
            bytes.len()
        )
    })
}

fn take_single_output_into<F>(
    outputs: &mut Vec<Vec<u8>>,
    output: &mut Vec<u8>,
    missing: F,
) -> Result<(), String>
where
    F: FnOnce() -> String,
{
    let mut next = outputs.pop().ok_or_else(missing)?;
    outputs.clear();
    mem::swap(output, &mut next);
    outputs.push(next);
    Ok(())
}

fn take_four_outputs(
    outputs: &mut Vec<Vec<u8>>,
    stage: &str,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>), String> {
    let mut iter = outputs.drain(..);
    let types = iter
        .next()
        .ok_or_else(|| format!("{stage}: missing types"))?;
    let starts = iter
        .next()
        .ok_or_else(|| format!("{stage}: missing starts"))?;
    let lens = iter
        .next()
        .ok_or_else(|| format!("{stage}: missing lens"))?;
    let counts = iter
        .next()
        .ok_or_else(|| format!("{stage}: missing counts"))?;
    Ok((types, starts, lens, counts))
}

fn sparse_prefix_offsets_gpu_into(
    backend: &dyn VyreBackend,
    sparse_types: &[u8],
    haystack_len: u32,
    config: &mut DispatchConfig,
    label: &str,
    offsets: &mut Vec<u8>,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    if haystack_len <= BLOCK_LANES {
        let scan_prog = prefix_scan_nonzero_workgroup("sparse_types", "offsets", haystack_len);
        validate_internal_stage(&scan_prog, "prefix_scan_nonzero")?;
        config.label = Some(format!("{label} sparse-prefix"));
        dispatch_borrowed_cached_into(backend, &scan_prog, &[sparse_types], config, outputs)
            .map_err(|e| format!("{label} sparse prefix scan dispatch failed: {e}"))?;
        return take_single_output_into(outputs, offsets, || {
            format!("{label} sparse prefix scan: missing offsets output")
        });
    }
    Err(format!(
        "{label} sparse prefix requested {haystack_len} lanes through the small-prefix path; \
         large sparse compaction must use the block-total rescan path"
    ))
}

#[cfg(test)]
mod tests {
    use super::compact_output_capacity_from_inclusive_offsets;

    fn words(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn compact_capacity_reads_final_inclusive_offset() {
        let offsets = words(&[0, 1, 1, 7]);

        assert_eq!(
            compact_output_capacity_from_inclusive_offsets(&offsets, 4, "test", "scan")
                .expect("Fix: valid inclusive offsets should size compact output"),
            7
        );
    }

    #[test]
    fn compact_capacity_keeps_one_physical_slot_for_empty_streams() {
        assert_eq!(
            compact_output_capacity_from_inclusive_offsets(&[], 0, "test", "scan")
                .expect("Fix: empty logical offset stream should still size output ABI"),
            1
        );
        assert_eq!(
            compact_output_capacity_from_inclusive_offsets(&words(&[0]), 1, "test", "scan")
                .expect("Fix: zero-token offset stream should still size output ABI"),
            1
        );
    }

    #[test]
    fn compact_capacity_rejects_truncated_offset_readback() {
        let err = compact_output_capacity_from_inclusive_offsets(&words(&[1]), 2, "test", "scan")
            .expect_err("truncated inclusive offsets must fail before compact dispatch");

        assert!(
            err.contains("token count readback"),
            "capacity error should point at the scanned offset readback: {err}"
        );
    }

    #[test]
    fn compact_capacity_matches_generated_sparse_profiles() {
        for case in 0..10_000_u32 {
            let logical_items = (case % 257) + 1;
            let mut running_tokens = 0_u32;
            let mut offsets = Vec::with_capacity(logical_items as usize * 4);
            for lane in 0..logical_items {
                let mixed = case
                    .wrapping_mul(1_664_525)
                    .wrapping_add(lane.wrapping_mul(1_013_904_223))
                    .rotate_left(lane % 17);
                if mixed.count_ones() % 5 == 0 {
                    running_tokens += 1;
                }
                offsets.extend_from_slice(&running_tokens.to_le_bytes());
            }

            assert_eq!(
                compact_output_capacity_from_inclusive_offsets(
                    &offsets,
                    logical_items,
                    "generated",
                    "scan"
                )
                .expect("Fix: generated inclusive offsets should size compact output"),
                running_tokens.max(1),
                "case {case} logical_items {logical_items}"
            );
        }
    }
}
