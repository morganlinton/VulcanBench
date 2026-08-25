use super::super::scan::{inclusive_prefix_scan_u32_into, PrefixScanScratch};
use super::host::read_output_u32;
use super::program_helpers::{byte_compact_program, combine_keep_mask_program};
use super::scratch::{copy_output_bytes, write_zero_bytes};
use super::FilteredBytes;
use crate::parsing::c::preprocess::gpu_comment_strip_mask::gpu_comment_strip_mask;
use crate::parsing::c::preprocess::gpu_pipeline::GpuDispatcher;
use vyre_primitives::parsing::line_splice_classify::line_splice_classify;

#[derive(Default)]
pub(super) struct FullCommentScratch {
    zero_words: Vec<u8>,
    source_words: Vec<u8>,
    splice_out: Vec<Vec<u8>>,
    comment_out: Vec<Vec<u8>>,
    combine_out: Vec<Vec<u8>>,
    offsets_bytes: Vec<u8>,
    compact_init: Vec<u8>,
    live_init: Vec<u8>,
    compact_out: Vec<Vec<u8>>,
}

impl FullCommentScratch {
    fn prepare_zero_words(&mut self, byte_len: usize) -> Result<(), String> {
        write_zero_bytes(&mut self.zero_words, byte_len, "full comment zero words")
    }

    fn prepare_source_words(&mut self, bytes_in: &[u8], byte_len: usize) -> Result<(), String> {
        write_zero_bytes(
            &mut self.source_words,
            byte_len,
            "full comment packed source words",
        )?;
        let end = bytes_in.len().min(self.source_words.len());
        self.source_words[..end].copy_from_slice(&bytes_in[..end]);
        Ok(())
    }

    fn prepare_compact_inputs(&mut self, compact_len: usize) -> Result<(), String> {
        write_zero_bytes(
            &mut self.compact_init,
            compact_len,
            "full comment compact init",
        )?;
        write_zero_bytes(&mut self.live_init, 4, "full comment live init")
    }
}

pub(super) fn gpu_filter_full_comment_state(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    bytes_in: &[u8],
    n_bucket: u32,
    cap_bucket: usize,
    byte_buf_pad: usize,
    n_real_buf: &[u8],
    scratch: &mut FullCommentScratch,
    scan_scratch: &mut PrefixScanScratch,
) -> Result<FilteredBytes, String> {
    // ---- Stages 1+2+3: line_splice + comment_strip + combine ----
    // Keep this path explicitly staged. Fused-program output ordering is not
    // a stable contract across backends, and swapping kept/comment masks turns
    // opening comment bytes into live `/` tokens. The staged path is still
    // fully GPU-resident and only used for complex comment/splice inputs; the
    // hot simple-line/simple-block paths stay specialized above.
    let splice_prog = line_splice_classify(n_bucket);
    let comment_prog = gpu_comment_strip_mask(n_bucket);
    let combine_prog = combine_keep_mask_program(n_bucket);
    let zero_word_bytes = cap_bucket.checked_mul(4).ok_or_else(|| {
        "full comment zero words overflowed usize. Fix: reduce batch size.".to_string()
    })?;
    scratch.prepare_zero_words(zero_word_bytes)?;
    scratch.prepare_source_words(bytes_in, byte_buf_pad)?;
    dispatcher
        .dispatch_borrowed_into(
            &splice_prog,
            &[
                scratch.source_words.as_slice(),
                scratch.zero_words.as_slice(),
            ],
            &mut scratch.splice_out,
        )
        .map_err(|e| format!("filter line_splice_classify: {e}"))?;
    if scratch.splice_out.len() != 1 {
        return Err(format!(
            "filter line_splice_classify: expected exactly 1 output, got {}. Fix: backend must return kept_mask_out only.",
            scratch.splice_out.len()
        ));
    }
    dispatcher
        .dispatch_borrowed_into(
            &comment_prog,
            &[
                scratch.source_words.as_slice(),
                scratch.zero_words.as_slice(),
            ],
            &mut scratch.comment_out,
        )
        .map_err(|e| format!("filter gpu_comment_strip_mask: {e}"))?;
    if scratch.comment_out.len() != 1 {
        return Err(format!(
            "filter gpu_comment_strip_mask: expected exactly 1 output, got {}. Fix: backend must return comment_mask_out only.",
            scratch.comment_out.len()
        ));
    }
    dispatcher
        .dispatch_borrowed_into(
            &combine_prog,
            &[
                scratch.splice_out[0].as_slice(),
                scratch.comment_out[0].as_slice(),
                scratch.zero_words.as_slice(),
                n_real_buf,
            ],
            &mut scratch.combine_out,
        )
        .map_err(|e| format!("filter combine_keep_mask: {e}"))?;
    if scratch.combine_out.len() != 1 {
        return Err(format!(
            "filter combine_keep_mask: expected exactly 1 output, got {}. Fix: backend must return final_keep only.",
            scratch.combine_out.len()
        ));
    }

    // ---- Stage 4: parallel inclusive prefix scan over keep mask ----
    // Run the scan over the full bucketed extent so the prefix-scan
    // kernel itself buckets identically across files. Padding entries
    // contribute zero to the running sum (gated to 0 by the combine
    // kernel) so the live count at the last real position equals the
    // total byte_compact will write.
    inclusive_prefix_scan_u32_into(
        dispatcher,
        &scratch.combine_out[0],
        n_bucket,
        scan_scratch,
        &mut scratch.offsets_bytes,
    )
    .map_err(|e| format!("filter keep-mask prefix scan: {e}"))?;

    // ---- Stage 5: scatter-compact bytes by offsets ----
    let compact_prog = byte_compact_program(n_bucket);
    // byte_compact dispatches one thread per *output word* (the
    // kernel iterates `w = InvocationId.x` over `0..ceil(n/4)`,
    // unrolling 4 input bytes per thread). compacted_out is sized
    // at `ceil(n/4)` u32 words = `byte_buf_pad` bytes  -  no over-
    // allocation, and the inferred grid (output_word_count =
    // ceil(n/4)) matches the kernel's logical extent exactly. The
    // host MUST zero-init compacted_out because the kernel
    // accumulates via `atomic_or`.
    scratch.prepare_compact_inputs(byte_buf_pad)?;
    dispatcher
        .dispatch_borrowed_into(
            &compact_prog,
            &[
                bytes_in,
                scratch.combine_out[0].as_slice(),
                scratch.comment_out[0].as_slice(),
                scratch.offsets_bytes.as_slice(),
                scratch.compact_init.as_slice(),
                scratch.live_init.as_slice(),
            ],
            &mut scratch.compact_out,
        )
        .map_err(|e| format!("byte_compact: {e}"))?;
    if scratch.compact_out.len() != 2 {
        return Err(format!(
            "byte_compact: expected exactly 2 outputs, got {}. Fix: backend must return compacted/live_count and no extras.",
            scratch.compact_out.len()
        ));
    }
    let compacted_buf = scratch
        .compact_out
        .first()
        .ok_or_else(|| "byte_compact: missing compacted output".to_string())?;
    let live_buf = scratch
        .compact_out
        .get(1)
        .ok_or_else(|| "byte_compact: missing live_count output".to_string())?;
    let live = read_output_u32(&live_buf, "byte_compact live_count")? as usize;
    let byte_len = live.min(raw.len()).min(compacted_buf.len());
    Ok(FilteredBytes {
        bytes: copy_output_bytes(&compacted_buf[..byte_len], "full comment byte_compact")?,
    })
}
