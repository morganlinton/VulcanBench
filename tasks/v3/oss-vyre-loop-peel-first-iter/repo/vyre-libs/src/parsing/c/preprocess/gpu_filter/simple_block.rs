use super::super::scan::{inclusive_prefix_scan_u32_into, PrefixScanScratch};
use super::block_programs::{
    simple_block_comment_marks_program, simple_block_comment_masks_program,
    simple_block_comment_topology_program,
};
use super::compact::{compact_comment_filtered_bytes, CommentCompactScratch};
use super::full_comment::gpu_filter_full_comment_state;
use super::host::read_output_u32;
use super::scratch::write_zero_bytes;
use super::FilteredBytes;
use crate::parsing::c::preprocess::gpu_pipeline::GpuDispatcher;

#[derive(Default)]
pub(super) struct SimpleBlockScratch {
    zero_words: Vec<u8>,
    marks_out: Vec<Vec<u8>>,
    open_scan: Vec<u8>,
    close_after_scan: Vec<u8>,
    topology_out: Vec<Vec<u8>>,
    masks_out: Vec<Vec<u8>>,
    compact: CommentCompactScratch,
}

impl SimpleBlockScratch {
    fn prepare(&mut self, n_bucket: u32, byte_buf_pad: usize) -> Result<(), String> {
        let word_bytes = (n_bucket as usize).checked_mul(4).ok_or_else(|| {
            "simple block comments scratch byte size overflowed usize. Fix: reduce batch size."
                .to_string()
        })?;
        write_zero_bytes(
            &mut self.zero_words,
            word_bytes,
            "simple block comments zero words",
        )?;
        self.compact.prepare(byte_buf_pad)
    }
}

pub(super) fn gpu_filter_simple_block_comments(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    bytes_in: &[u8],
    n_bucket: u32,
    cap_bucket: usize,
    byte_buf_pad: usize,
    n_real_buf: &[u8],
    scratch: &mut SimpleBlockScratch,
    full_scratch: &mut super::full_comment::FullCommentScratch,
    scan_scratch: &mut PrefixScanScratch,
) -> Result<FilteredBytes, String> {
    scratch.prepare(n_bucket, byte_buf_pad)?;
    let marks_prog = simple_block_comment_marks_program(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &marks_prog,
            &[
                bytes_in,
                scratch.zero_words.as_slice(),
                scratch.zero_words.as_slice(),
                n_real_buf,
            ],
            &mut scratch.marks_out,
        )
        .map_err(|e| format!("simple block comments marks: {e}"))?;
    if scratch.marks_out.len() != 2 {
        return Err(format!(
            "simple block comments marks: expected exactly 2 outputs, got {}. Fix: backend must return open_flags/close_after_flags and no extras.",
            scratch.marks_out.len()
        ));
    }
    inclusive_prefix_scan_u32_into(
        dispatcher,
        &scratch.marks_out[0],
        n_bucket,
        scan_scratch,
        &mut scratch.open_scan,
    )
    .map_err(|e| format!("simple block comments open scan: {e}"))?;
    inclusive_prefix_scan_u32_into(
        dispatcher,
        &scratch.marks_out[1],
        n_bucket,
        scan_scratch,
        &mut scratch.close_after_scan,
    )
    .map_err(|e| format!("simple block comments close-after scan: {e}"))?;

    let topology_prog = simple_block_comment_topology_program(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &topology_prog,
            &[
                scratch.marks_out[0].as_slice(),
                scratch.marks_out[1].as_slice(),
                scratch.open_scan.as_slice(),
                scratch.close_after_scan.as_slice(),
                scratch.compact.scalar_zero(),
                n_real_buf,
            ],
            &mut scratch.topology_out,
        )
        .map_err(|e| format!("simple block comments topology: {e}"))?;
    if scratch.topology_out.len() != 1 {
        return Err(format!(
            "simple block comments topology: expected exactly 1 output, got {}. Fix: backend must return only the invalid flag.",
            scratch.topology_out.len()
        ));
    }
    let topology_flag = scratch
        .topology_out
        .first()
        .ok_or_else(|| "simple block comments topology: missing invalid flag".to_string())?;
    if read_output_u32(
        &topology_flag,
        "simple block comments topology invalid flag",
    )? != 0
    {
        return gpu_filter_full_comment_state(
            dispatcher,
            raw,
            raw,
            n_bucket,
            cap_bucket,
            byte_buf_pad,
            n_real_buf,
            full_scratch,
            scan_scratch,
        );
    }

    let masks_prog = simple_block_comment_masks_program(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &masks_prog,
            &[
                scratch.marks_out[0].as_slice(),
                scratch.open_scan.as_slice(),
                scratch.close_after_scan.as_slice(),
                scratch.zero_words.as_slice(),
                scratch.zero_words.as_slice(),
                n_real_buf,
            ],
            &mut scratch.masks_out,
        )
        .map_err(|e| format!("simple block comments masks: {e}"))?;
    if scratch.masks_out.len() != 2 {
        return Err(format!(
            "simple block comments masks: expected exactly 2 outputs, got {}. Fix: backend must return final_keep/comment_mask and no extras.",
            scratch.masks_out.len()
        ));
    }
    compact_comment_filtered_bytes(
        dispatcher,
        "simple block comments",
        raw,
        bytes_in,
        scratch.masks_out[0].as_slice(),
        scratch.masks_out[1].as_slice(),
        n_bucket,
        &mut scratch.compact,
        scan_scratch,
    )
}
