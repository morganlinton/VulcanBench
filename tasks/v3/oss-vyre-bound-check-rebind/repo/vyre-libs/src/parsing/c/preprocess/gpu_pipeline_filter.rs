use super::{buffers::checked_gpu_u32, scan::PrefixScanScratch, GpuDispatcher};
use crate::parsing::c::preprocess::gpu_pipeline::bucket_pow2;

#[path = "gpu_filter/block_programs.rs"]
mod block_programs;
#[path = "gpu_filter/compact.rs"]
mod compact;
#[path = "gpu_filter/full_comment.rs"]
mod full_comment;
#[path = "gpu_filter/host.rs"]
mod host;
#[path = "gpu_filter/line_programs.rs"]
mod line_programs;
#[path = "gpu_filter/preflight.rs"]
mod preflight;
#[path = "gpu_filter/program_helpers.rs"]
mod program_helpers;
#[path = "gpu_filter/scratch.rs"]
mod scratch;
#[path = "gpu_filter/simple_block.rs"]
mod simple_block;
#[path = "gpu_filter/simple_line.rs"]
mod simple_line;
#[path = "gpu_filter/splice_only.rs"]
mod splice_only;

const TRANSFORM_LINE_COMMENT: u32 = 1;
const TRANSFORM_BLOCK_COMMENT: u32 = 2;
const TRANSFORM_LINE_SPLICE: u32 = 4;
const TRANSFORM_LITERAL_QUOTE: u32 = 8;

/// Output of the byte-filter stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteredBytes {
    /// The post-phase-2, comment-free byte stream ready to feed the
    /// lexer. Length is the number of survivor bytes, not the input
    /// length.
    pub bytes: Vec<u8>,
}

#[derive(Default)]
pub(super) struct FilterScratch {
    n_real_buf: Vec<u8>,
    preflight_zero: Vec<u8>,
    preflight_outputs: Vec<Vec<u8>>,
    full_comment: full_comment::FullCommentScratch,
    simple_line: simple_line::SimpleLineScratch,
    simple_block: simple_block::SimpleBlockScratch,
    splice_only: splice_only::SpliceOnlyScratch,
    scan: PrefixScanScratch,
}

impl FilterScratch {
    fn prepare_n_real(&mut self, n: u32) {
        self.n_real_buf.clear();
        self.n_real_buf.extend_from_slice(&n.to_le_bytes());
    }

    fn prepare_preflight_zero(&mut self, byte_len: usize) -> Result<(), String> {
        scratch::write_zero_bytes(&mut self.preflight_zero, byte_len, "filter preflight zero")
    }
}

/// Orchestrate the GPU byte-filter stages over raw C source bytes.
pub fn gpu_filter_source_bytes(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
) -> Result<FilteredBytes, String> {
    let mut scratch = FilterScratch::default();
    gpu_filter_source_bytes_with_scratch(dispatcher, raw, &mut scratch)
}

pub(super) fn gpu_filter_source_bytes_with_scratch(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    scratch: &mut FilterScratch,
) -> Result<FilteredBytes, String> {
    if raw.is_empty() {
        return Ok(FilteredBytes { bytes: Vec::new() });
    }
    let n = checked_gpu_u32("filter source length", raw.len())?;
    let cap = n.max(1) as usize;
    let n_bucket_usize = bucket_pow2(cap, 1024);
    let n_bucket = checked_gpu_u32("filter bucketed length", n_bucket_usize)?;
    let cap_bucket = n_bucket as usize;
    let byte_buf_pad = cap_bucket
        .div_ceil(4)
        .checked_mul(4)
        .ok_or_else(|| {
            "filter byte buffer padding overflowed usize. Fix: reduce batch size.".to_string()
        })?
        .max(4);
    let word_buf_bytes = cap_bucket
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            "filter word buffer padding overflowed usize. Fix: reduce batch size.".to_string()
        })?;
    scratch.prepare_n_real(n);
    scratch.prepare_preflight_zero(word_buf_bytes)?;

    dispatcher
        .dispatch_borrowed_into(
            &preflight::transform_candidate_program(n_bucket),
            &[
                raw,
                scratch.preflight_zero.as_slice(),
                scratch.n_real_buf.as_slice(),
            ],
            &mut scratch.preflight_outputs,
        )
        .map_err(|e| format!("filter transform preflight: {e}"))?;
    if scratch.preflight_outputs.len() != 1 {
        return Err(format!(
            "filter transform preflight: expected exactly 1 output, got {}. Fix: backend must return transform_flag only.",
            scratch.preflight_outputs.len()
        ));
    }
    let transform_candidate = &scratch.preflight_outputs[0];
    if transform_candidate.len() < 4 {
        return Err(format!(
            "filter transform preflight: malformed flag output: expected at least 4 bytes, got {}. Fix: backend must emit transform_flag[0].",
            transform_candidate.len()
        ));
    }
    let transform_flags = u32::from_le_bytes([
        transform_candidate[0],
        transform_candidate[1],
        transform_candidate[2],
        transform_candidate[3],
    ]);

    if transform_flags & (TRANSFORM_LINE_COMMENT | TRANSFORM_BLOCK_COMMENT | TRANSFORM_LINE_SPLICE)
        == 0
    {
        return Ok(FilteredBytes {
            bytes: scratch::copy_output_bytes(raw, "filter no-transform output")?,
        });
    }
    if transform_flags == TRANSFORM_LINE_COMMENT {
        return simple_line::gpu_filter_simple_line_comments(
            dispatcher,
            raw,
            raw,
            n_bucket,
            byte_buf_pad,
            &scratch.n_real_buf,
            &mut scratch.simple_line,
            &mut scratch.scan,
        );
    }
    if transform_flags == TRANSFORM_LINE_SPLICE {
        if splice_only::line_splices_can_create_comment(
            dispatcher,
            raw,
            n_bucket,
            &scratch.n_real_buf,
            &mut scratch.splice_only,
        )? {
            return full_comment::gpu_filter_full_comment_state(
                dispatcher,
                raw,
                raw,
                n_bucket,
                cap_bucket,
                byte_buf_pad,
                &scratch.n_real_buf,
                &mut scratch.full_comment,
                &mut scratch.scan,
            );
        }
        return splice_only::gpu_filter_line_splices(
            dispatcher,
            raw,
            raw,
            n_bucket,
            byte_buf_pad,
            &scratch.n_real_buf,
            &mut scratch.splice_only,
            &mut scratch.scan,
        );
    }
    if transform_flags == TRANSFORM_BLOCK_COMMENT {
        return simple_block::gpu_filter_simple_block_comments(
            dispatcher,
            raw,
            raw,
            n_bucket,
            cap_bucket,
            byte_buf_pad,
            &scratch.n_real_buf,
            &mut scratch.simple_block,
            &mut scratch.full_comment,
            &mut scratch.scan,
        );
    }

    full_comment::gpu_filter_full_comment_state(
        dispatcher,
        raw,
        raw,
        n_bucket,
        cap_bucket,
        byte_buf_pad,
        &scratch.n_real_buf,
        &mut scratch.full_comment,
        &mut scratch.scan,
    )
}
