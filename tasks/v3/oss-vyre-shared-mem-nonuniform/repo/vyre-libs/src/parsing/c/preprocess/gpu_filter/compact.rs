use super::super::scan::{inclusive_prefix_scan_u32_into, PrefixScanScratch};
use super::host::read_output_u32;
use super::program_helpers::byte_compact_program;
use super::scratch::{copy_output_bytes, write_zero_bytes};
use super::FilteredBytes;
use crate::parsing::c::preprocess::gpu_pipeline::GpuDispatcher;

#[derive(Default)]
pub(super) struct CommentCompactScratch {
    scalar_zero: Vec<u8>,
    offsets_bytes: Vec<u8>,
    compact_init: Vec<u8>,
    compact_out: Vec<Vec<u8>>,
}

impl CommentCompactScratch {
    pub(super) fn prepare(&mut self, byte_buf_pad: usize) -> Result<(), String> {
        write_zero_bytes(&mut self.scalar_zero, 4, "comment compact scalar zero")?;
        write_zero_bytes(&mut self.compact_init, byte_buf_pad, "comment compact init")
    }

    pub(super) fn scalar_zero(&self) -> &[u8] {
        self.scalar_zero.as_slice()
    }
}

pub(super) fn compact_comment_filtered_bytes(
    dispatcher: &dyn GpuDispatcher,
    stage: &str,
    raw: &[u8],
    bytes_in: &[u8],
    keep_mask: &[u8],
    comment_mask: &[u8],
    n_bucket: u32,
    scratch: &mut CommentCompactScratch,
    scan_scratch: &mut PrefixScanScratch,
) -> Result<FilteredBytes, String> {
    inclusive_prefix_scan_u32_into(
        dispatcher,
        keep_mask,
        n_bucket,
        scan_scratch,
        &mut scratch.offsets_bytes,
    )
    .map_err(|e| format!("{stage} prefix scan: {e}"))?;

    dispatcher
        .dispatch_borrowed_into(
            &byte_compact_program(n_bucket),
            &[
                bytes_in,
                keep_mask,
                comment_mask,
                scratch.offsets_bytes.as_slice(),
                scratch.compact_init.as_slice(),
                scratch.scalar_zero.as_slice(),
            ],
            &mut scratch.compact_out,
        )
        .map_err(|e| format!("{stage} byte_compact: {e}"))?;

    if scratch.compact_out.len() != 2 {
        return Err(format!(
            "{stage} byte_compact: expected exactly 2 outputs, got {}. Fix: backend must return compacted/live_count and no extras.",
            scratch.compact_out.len()
        ));
    }
    let compacted_buf = scratch
        .compact_out
        .first()
        .ok_or_else(|| format!("{stage} byte_compact: missing compacted output"))?;
    let live_buf = scratch
        .compact_out
        .get(1)
        .ok_or_else(|| format!("{stage} byte_compact: missing live_count output"))?;
    let live = read_output_u32(live_buf, &format!("{stage} byte_compact live_count"))? as usize;
    let byte_len = live.min(raw.len()).min(compacted_buf.len());
    Ok(FilteredBytes {
        bytes: copy_output_bytes(&compacted_buf[..byte_len], stage)?,
    })
}
