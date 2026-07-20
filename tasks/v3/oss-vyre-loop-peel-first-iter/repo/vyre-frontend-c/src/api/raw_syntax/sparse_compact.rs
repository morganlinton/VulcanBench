use std::cell::RefCell;
use std::mem;

use super::*;

#[derive(Default)]
struct RawSparseCompactScratch {
    compact_outputs: Vec<Vec<u8>>,
}

thread_local! {
    static RAW_SPARSE_COMPACT_SCRATCH: RefCell<RawSparseCompactScratch> =
        RefCell::new(RawSparseCompactScratch::default());
}

pub(super) fn compact_sparse_tokens_ordered_gpu(
    backend: &dyn vyre::VyreBackend,
    sparse_types: crate::pipeline::ResidentBlob,
    sparse_starts: crate::pipeline::ResidentBlob,
    sparse_lens: crate::pipeline::ResidentBlob,
    count: u32,
    config: &mut DispatchConfig,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    RAW_SPARSE_COMPACT_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "raw sparse token compaction scratch was re-entered on the same thread. Fix: call raw sparse compaction from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        compact_sparse_tokens_ordered_gpu_with_scratch(
            backend,
            sparse_types,
            sparse_starts,
            sparse_lens,
            count,
            config,
            &mut scratch,
        )
    })
}

fn compact_sparse_tokens_ordered_gpu_with_scratch(
    backend: &dyn vyre::VyreBackend,
    sparse_types: crate::pipeline::ResidentBlob,
    sparse_starts: crate::pipeline::ResidentBlob,
    sparse_lens: crate::pipeline::ResidentBlob,
    count: u32,
    config: &mut DispatchConfig,
    scratch: &mut RawSparseCompactScratch,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let mut cleanup = ResidentBlobCleanup::new(backend);
    cleanup.push(sparse_types);
    cleanup.push(sparse_starts);
    cleanup.push(sparse_lens);
    let num_blocks = count.div_ceil(BLOCK_LANES).max(1);
    config.label = Some("vyre-frontend-c raw-byte sparse block totals".to_string());
    let totals = crate::pipeline::dispatch_resident_stage_cached(
        backend,
        crate::pipeline::stage_pipeline_cache_key(
            "raw_sparse_token_block_totals",
            &[u64::from(count), u64::from(num_blocks)],
        ),
        || {
            Ok(sparse_token_block_totals_program(
                "sparse_types",
                "block_totals",
                count,
                num_blocks,
            ))
        },
        &[crate::pipeline::ResidentStageInput::Resident(
            &cleanup.blobs[0],
        )],
        config,
    )
    .map_err(|e| format!("raw-byte sparse block-total dispatch failed: {e}"))?;
    if totals.len() != 1 {
        let actual = totals.len();
        let _ = crate::pipeline::free_resident_blobs(backend, totals);
        return Err(format!(
            "raw-byte sparse block-total dispatch returned {actual} resident outputs, expected exactly block_totals. Fix: backend must return the declared GPU block-total ABI resource and no extras."
        ));
    }
    cleanup.extend(totals);
    config.label = Some("vyre-frontend-c raw-byte sparse block scan".to_string());
    let scanned = crate::pipeline::dispatch_resident_stage_cached(
        backend,
        crate::pipeline::stage_pipeline_cache_key(
            "raw_sparse_token_block_scan",
            &[u64::from(num_blocks)],
        ),
        || {
            Ok(
                vyre_primitives::reduce::multi_block_prefix_scan::multi_block_prefix_scan_sum_u32(
                    "block_totals",
                    "block_totals_scanned",
                    num_blocks,
                ),
            )
        },
        &[crate::pipeline::ResidentStageInput::Resident(
            &cleanup.blobs[3],
        )],
        config,
    )
    .map_err(|e| format!("raw-byte sparse block scan dispatch failed: {e}"))?;
    if scanned.len() != 1 {
        let actual = scanned.len();
        let _ = crate::pipeline::free_resident_blobs(backend, scanned);
        return Err(format!(
            "raw-byte sparse block scan returned {actual} resident outputs, expected exactly block_totals_scanned. Fix: backend must return the declared GPU prefix-scan ABI resource and no extras."
        ));
    }
    cleanup.extend(scanned);
    config.label = Some("vyre-frontend-c raw-byte sparse block compact".to_string());
    scratch.compact_outputs.clear();
    crate::pipeline::dispatch_resident_stage_readback_cached_into(
        backend,
        crate::pipeline::stage_pipeline_cache_key(
            "raw_sparse_token_block_compact",
            &[u64::from(count), u64::from(num_blocks)],
        ),
        || {
            Ok(sparse_token_block_compact_program(
                "block_totals_scanned",
                "sparse_types",
                "sparse_starts",
                "sparse_lens",
                "out_tok_triplets_and_count",
                count,
                num_blocks,
            ))
        },
        &[
            crate::pipeline::ResidentStageInput::Resident(&cleanup.blobs[4]),
            crate::pipeline::ResidentStageInput::Resident(&cleanup.blobs[0]),
            crate::pipeline::ResidentStageInput::Resident(&cleanup.blobs[1]),
            crate::pipeline::ResidentStageInput::Resident(&cleanup.blobs[2]),
        ],
        config,
        &mut scratch.compact_outputs,
    )
    .map_err(|e| format!("raw-byte sparse block compact dispatch failed: {e}"))?;
    if scratch.compact_outputs.len() != 1 {
        return Err(format!(
            "raw-byte sparse block compact returned {} outputs, expected exactly one packed token-triplet/count buffer. Fix: backend must return the declared GPU compaction ABI output and no extras.",
            scratch.compact_outputs.len()
        ));
    }
    let mut packed = Vec::new();
    mem::swap(&mut packed, &mut scratch.compact_outputs[0]);
    let (dense_types, counts) = unpack_dense_types_from_token_triplets(&packed)?;
    Ok((dense_types, counts))
}

pub(super) fn compact_sparse_token_types_ordered_gpu(
    backend: &dyn vyre::VyreBackend,
    sparse_types: crate::pipeline::ResidentBlob,
    count: u32,
    config: &mut DispatchConfig,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    RAW_SPARSE_COMPACT_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "raw sparse type compaction scratch was re-entered on the same thread. Fix: call raw sparse compaction from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        compact_sparse_token_types_ordered_gpu_with_scratch(
            backend,
            sparse_types,
            count,
            config,
            &mut scratch,
        )
    })
}

fn compact_sparse_token_types_ordered_gpu_with_scratch(
    backend: &dyn vyre::VyreBackend,
    sparse_types: crate::pipeline::ResidentBlob,
    count: u32,
    config: &mut DispatchConfig,
    scratch: &mut RawSparseCompactScratch,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

    let mut cleanup = ResidentBlobCleanup::new(backend);
    cleanup.push(sparse_types);
    let num_blocks = count.div_ceil(BLOCK_LANES).max(1);
    config.label = Some("vyre-frontend-c raw-byte sparse type block totals".to_string());
    let totals = crate::pipeline::dispatch_resident_stage_cached(
        backend,
        crate::pipeline::stage_pipeline_cache_key(
            "raw_sparse_type_block_totals",
            &[u64::from(count), u64::from(num_blocks)],
        ),
        || {
            Ok(sparse_token_block_totals_program(
                "sparse_types",
                "block_totals",
                count,
                num_blocks,
            ))
        },
        &[crate::pipeline::ResidentStageInput::Resident(
            &cleanup.blobs[0],
        )],
        config,
    )
    .map_err(|e| format!("raw-byte sparse type block-total dispatch failed: {e}"))?;
    if totals.len() != 1 {
        let actual = totals.len();
        let _ = crate::pipeline::free_resident_blobs(backend, totals);
        return Err(format!(
            "raw-byte sparse type block-total dispatch returned {actual} resident outputs, expected exactly block_totals. Fix: backend must return the declared GPU block-total ABI resource and no extras."
        ));
    }
    cleanup.extend(totals);
    config.label = Some("vyre-frontend-c raw-byte sparse type block scan".to_string());
    let scanned = crate::pipeline::dispatch_resident_stage_cached(
        backend,
        crate::pipeline::stage_pipeline_cache_key(
            "raw_sparse_type_block_scan",
            &[u64::from(num_blocks)],
        ),
        || {
            Ok(
                vyre_primitives::reduce::multi_block_prefix_scan::multi_block_prefix_scan_sum_u32(
                    "block_totals",
                    "block_totals_scanned",
                    num_blocks,
                ),
            )
        },
        &[crate::pipeline::ResidentStageInput::Resident(
            &cleanup.blobs[1],
        )],
        config,
    )
    .map_err(|e| format!("raw-byte sparse type block scan dispatch failed: {e}"))?;
    if scanned.len() != 1 {
        let actual = scanned.len();
        let _ = crate::pipeline::free_resident_blobs(backend, scanned);
        return Err(format!(
            "raw-byte sparse type block scan returned {actual} resident outputs, expected exactly block_totals_scanned. Fix: backend must return the declared GPU prefix-scan ABI resource and no extras."
        ));
    }
    cleanup.extend(scanned);
    config.label = Some("vyre-frontend-c raw-byte sparse type block compact".to_string());
    scratch.compact_outputs.clear();
    crate::pipeline::dispatch_resident_stage_readback_cached_into(
        backend,
        crate::pipeline::stage_pipeline_cache_key(
            "raw_sparse_type_block_compact",
            &[u64::from(count), u64::from(num_blocks)],
        ),
        || {
            Ok(sparse_token_type_block_compact_program(
                "block_totals_scanned",
                "sparse_types",
                "out_tok_types_and_count",
                count,
                num_blocks,
            ))
        },
        &[
            crate::pipeline::ResidentStageInput::Resident(&cleanup.blobs[2]),
            crate::pipeline::ResidentStageInput::Resident(&cleanup.blobs[0]),
        ],
        config,
        &mut scratch.compact_outputs,
    )
    .map_err(|e| format!("raw-byte sparse type block compact dispatch failed: {e}"))?;
    if scratch.compact_outputs.len() != 1 {
        return Err(format!(
            "raw-byte sparse type block compact returned {} outputs, expected exactly one packed dense-token/count buffer. Fix: backend must return the declared GPU type-compaction ABI output and no extras.",
            scratch.compact_outputs.len()
        ));
    }
    let mut packed = Vec::new();
    mem::swap(&mut packed, &mut scratch.compact_outputs[0]);
    if packed.len() < 4 {
        return Err(format!(
            "raw-byte sparse type block compact returned {} packed bytes, expected at least a u32 token count. Fix: backend must preserve the packed compaction ABI.",
            packed.len()
        ));
    }
    let dense_types = packed.split_off(4);
    let counts = packed;
    Ok((dense_types, counts))
}

fn unpack_dense_types_from_token_triplets(packed: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let count_bytes = packed.get(0..4).ok_or_else(|| {
        format!(
            "raw-byte sparse block compact returned {} packed bytes, expected at least a u32 token count. Fix: backend must preserve the packed token-triplet ABI.",
            packed.len()
        )
    })?;
    let token_count =
        u32::from_le_bytes(count_bytes.try_into().map_err(|_| {
            "raw-byte sparse block compact count slice had invalid width".to_string()
        })?) as usize;
    let payload_bytes = token_count.checked_mul(12).ok_or_else(|| {
        "raw-byte sparse block compact token triplet byte count overflows usize. Fix: shard parser input."
            .to_string()
    })?;
    let payload = packed.get(4..4 + payload_bytes).ok_or_else(|| {
        format!(
            "raw-byte sparse block compact returned {} packed bytes, expected {} bytes for {token_count} token triplets. Fix: backend must preserve the packed token-triplet ABI.",
            packed.len(),
            4usize.saturating_add(payload_bytes)
        )
    })?;
    let mut dense_types = Vec::with_capacity(token_count.saturating_mul(4));
    for triplet in payload.chunks_exact(12) {
        dense_types.extend_from_slice(&triplet[..4]);
    }
    Ok((dense_types, count_bytes.to_vec()))
}

struct ResidentBlobCleanup<'a> {
    backend: &'a dyn vyre::VyreBackend,
    blobs: Vec<crate::pipeline::ResidentBlob>,
}

impl<'a> ResidentBlobCleanup<'a> {
    fn new(backend: &'a dyn vyre::VyreBackend) -> Self {
        Self {
            backend,
            blobs: Vec::new(),
        }
    }

    fn push(&mut self, blob: crate::pipeline::ResidentBlob) {
        self.blobs.push(blob);
    }

    fn extend(&mut self, blobs: Vec<crate::pipeline::ResidentBlob>) {
        self.blobs.extend(blobs);
    }
}

impl Drop for ResidentBlobCleanup<'_> {
    fn drop(&mut self) {
        let blobs = mem::take(&mut self.blobs);
        let _ = crate::pipeline::free_resident_blobs(self.backend, blobs);
    }
}
