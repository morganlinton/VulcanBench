use super::super::scan::{inclusive_prefix_scan_u32_into, PrefixScanScratch};
use super::compact::{compact_comment_filtered_bytes, CommentCompactScratch};
use super::line_programs::{
    simple_line_comment_masks_program, simple_line_comment_starts_program,
    simple_line_newline_flags_program,
};
use super::scratch::{write_fill_bytes, write_zero_bytes};
use super::FilteredBytes;
use crate::parsing::c::preprocess::gpu_pipeline::GpuDispatcher;

#[derive(Default)]
pub(super) struct SimpleLineScratch {
    zero_words: Vec<u8>,
    scalar_ff: Vec<u8>,
    newline_flags_out: Vec<Vec<u8>>,
    newline_scan: Vec<u8>,
    row_comment_out: Vec<Vec<u8>>,
    masks_out: Vec<Vec<u8>>,
    compact: CommentCompactScratch,
}

impl SimpleLineScratch {
    fn prepare(&mut self, n_bucket: u32, byte_buf_pad: usize) -> Result<(), String> {
        let word_bytes = (n_bucket as usize).checked_mul(4).ok_or_else(|| {
            "simple line comments scratch byte size overflowed usize. Fix: reduce batch size."
                .to_string()
        })?;
        write_zero_bytes(
            &mut self.zero_words,
            word_bytes,
            "simple line comments zero words",
        )?;
        write_fill_bytes(
            &mut self.scalar_ff,
            word_bytes,
            0xFF,
            "simple line comments scalar ff",
        )?;
        self.compact.prepare(byte_buf_pad)
    }
}

pub(super) fn gpu_filter_simple_line_comments(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    bytes_in: &[u8],
    n_bucket: u32,
    byte_buf_pad: usize,
    n_real_buf: &[u8],
    scratch: &mut SimpleLineScratch,
    scan_scratch: &mut PrefixScanScratch,
) -> Result<FilteredBytes, String> {
    scratch.prepare(n_bucket, byte_buf_pad)?;
    let newline_flags_prog = simple_line_newline_flags_program(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &newline_flags_prog,
            &[bytes_in, scratch.zero_words.as_slice(), n_real_buf],
            &mut scratch.newline_flags_out,
        )
        .map_err(|e| format!("simple line comments newline flags: {e}"))?;
    if scratch.newline_flags_out.len() != 1 {
        return Err(format!(
            "simple line comments newline flags: expected exactly 1 output, got {}. Fix: backend must return only newline_flags.",
            scratch.newline_flags_out.len()
        ));
    }
    inclusive_prefix_scan_u32_into(
        dispatcher,
        &scratch.newline_flags_out[0],
        n_bucket,
        scan_scratch,
        &mut scratch.newline_scan,
    )
    .map_err(|e| format!("simple line comments newline scan: {e}"))?;

    let row_comment_prog = simple_line_comment_starts_program(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &row_comment_prog,
            &[
                bytes_in,
                scratch.newline_flags_out[0].as_slice(),
                scratch.newline_scan.as_slice(),
                scratch.scalar_ff.as_slice(),
                n_real_buf,
            ],
            &mut scratch.row_comment_out,
        )
        .map_err(|e| format!("simple line comments row starts: {e}"))?;
    if scratch.row_comment_out.len() != 1 {
        return Err(format!(
            "simple line comments row starts: expected exactly 1 output, got {}. Fix: backend must return only row_comment_starts.",
            scratch.row_comment_out.len()
        ));
    }
    let masks_prog = simple_line_comment_masks_program(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &masks_prog,
            &[
                bytes_in,
                scratch.newline_flags_out[0].as_slice(),
                scratch.newline_scan.as_slice(),
                scratch.row_comment_out[0].as_slice(),
                scratch.zero_words.as_slice(),
                scratch.zero_words.as_slice(),
                n_real_buf,
            ],
            &mut scratch.masks_out,
        )
        .map_err(|e| format!("simple line comments masks: {e}"))?;
    if scratch.masks_out.len() != 2 {
        return Err(format!(
            "simple line comments masks: expected exactly 2 outputs, got {}. Fix: backend must return final_keep/comment_mask and no extras.",
            scratch.masks_out.len()
        ));
    }
    compact_comment_filtered_bytes(
        dispatcher,
        "simple line comments",
        raw,
        bytes_in,
        scratch.masks_out[0].as_slice(),
        scratch.masks_out[1].as_slice(),
        n_bucket,
        &mut scratch.compact,
        scan_scratch,
    )
}
