use super::super::scan::PrefixScanScratch;
use super::compact::{compact_comment_filtered_bytes, CommentCompactScratch};
use super::program_helpers::{
    combine_keep_mask_program, singleton_u32_read_buffer, source_byte_load_or_zero,
    source_bytes_input_buffer, u32_rw_buffer, wrap_gpu_filter_program,
};
use super::scratch::write_zero_bytes;
use super::FilteredBytes;
use crate::parsing::c::preprocess::gpu_pipeline::GpuDispatcher;
use vyre::ir::{Expr, Node, Program};
use vyre_primitives::parsing::line_splice_classify::line_splice_classify_u8;

#[derive(Default)]
pub(super) struct SpliceOnlyScratch {
    zero_words: Vec<u8>,
    comment_candidate_out: Vec<Vec<u8>>,
    splice_out: Vec<Vec<u8>>,
    combine_out: Vec<Vec<u8>>,
    compact: CommentCompactScratch,
}

impl SpliceOnlyScratch {
    fn prepare_spliced_comment_preflight(&mut self) -> Result<(), String> {
        write_zero_bytes(
            &mut self.zero_words,
            std::mem::size_of::<u32>(),
            "line-splice comment preflight zero words",
        )
    }

    fn prepare(&mut self, n_bucket: u32, byte_buf_pad: usize) -> Result<(), String> {
        let word_bytes = (n_bucket as usize).checked_mul(4).ok_or_else(|| {
            "line-splice-only scratch byte size overflowed usize. Fix: reduce batch size."
                .to_string()
        })?;
        write_zero_bytes(
            &mut self.zero_words,
            word_bytes,
            "line-splice-only zero words",
        )?;
        self.compact.prepare(byte_buf_pad)
    }
}

pub(super) fn line_splices_can_create_comment(
    dispatcher: &dyn GpuDispatcher,
    bytes_in: &[u8],
    n_bucket: u32,
    n_real_buf: &[u8],
    scratch: &mut SpliceOnlyScratch,
) -> Result<bool, String> {
    scratch.prepare_spliced_comment_preflight()?;
    dispatcher
        .dispatch_borrowed_into(
            &spliced_comment_candidate_program(n_bucket),
            &[bytes_in, scratch.zero_words.as_slice(), n_real_buf],
            &mut scratch.comment_candidate_out,
        )
        .map_err(|e| format!("line-splice comment preflight: {e}"))?;
    if scratch.comment_candidate_out.len() != 1 {
        return Err(format!(
            "line-splice comment preflight: expected exactly 1 output, got {}. Fix: backend must return only spliced_comment_flag.",
            scratch.comment_candidate_out.len()
        ));
    }
    let flag_buf = scratch
        .comment_candidate_out
        .first()
        .ok_or_else(|| "line-splice comment preflight: missing flag output".to_string())?;
    if flag_buf.len() < 4 {
        return Err(format!(
            "line-splice comment preflight flag: malformed output: expected at least 4 bytes, got {}. Fix: backend must emit spliced_comment_flag[0].",
            flag_buf.len()
        ));
    }
    Ok(u32::from_le_bytes([flag_buf[0], flag_buf[1], flag_buf[2], flag_buf[3]]) != 0)
}

pub(super) fn gpu_filter_line_splices(
    dispatcher: &dyn GpuDispatcher,
    raw: &[u8],
    bytes_in: &[u8],
    n_bucket: u32,
    byte_buf_pad: usize,
    n_real_buf: &[u8],
    scratch: &mut SpliceOnlyScratch,
    scan_scratch: &mut PrefixScanScratch,
) -> Result<FilteredBytes, String> {
    scratch.prepare(n_bucket, byte_buf_pad)?;

    let splice_prog = line_splice_classify_u8(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &splice_prog,
            &[bytes_in, scratch.zero_words.as_slice()],
            &mut scratch.splice_out,
        )
        .map_err(|e| format!("line-splice-only classify: {e}"))?;
    if scratch.splice_out.len() != 1 {
        return Err(format!(
            "line-splice-only classify: expected exactly 1 output, got {}. Fix: backend must return only kept_mask_out.",
            scratch.splice_out.len()
        ));
    }

    let combine_prog = combine_keep_mask_program(n_bucket);
    dispatcher
        .dispatch_borrowed_into(
            &combine_prog,
            &[
                scratch.splice_out[0].as_slice(),
                scratch.zero_words.as_slice(),
                scratch.zero_words.as_slice(),
                n_real_buf,
            ],
            &mut scratch.combine_out,
        )
        .map_err(|e| format!("line-splice-only real-length gate: {e}"))?;
    if scratch.combine_out.len() != 1 {
        return Err(format!(
            "line-splice-only real-length gate: expected exactly 1 output, got {}. Fix: backend must return only final_keep.",
            scratch.combine_out.len()
        ));
    }

    compact_comment_filtered_bytes(
        dispatcher,
        "line-splice-only",
        raw,
        bytes_in,
        scratch.combine_out[0].as_slice(),
        scratch.zero_words.as_slice(),
        n_bucket,
        &mut scratch.compact,
        scan_scratch,
    )
}

fn spliced_comment_candidate_program(_n: u32) -> Program {
    let i = Expr::var("i");
    let load = |offset: u32| {
        source_byte_load_or_zero(
            "bytes_in",
            Expr::add(i.clone(), Expr::u32(offset)),
            "spliced_comment_n_real",
        )
    };
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(
                i.clone(),
                Expr::load("spliced_comment_n_real", Expr::u32(0)),
            ),
            vec![
                Node::let_bind("b0", load(0)),
                Node::let_bind("b1", load(1)),
                Node::let_bind("b2", load(2)),
                Node::let_bind("b3", load(3)),
                Node::let_bind("b4", load(4)),
                Node::let_bind(
                    "slash_splice_lf",
                    Expr::and(
                        Expr::and(
                            Expr::eq(Expr::var("b0"), Expr::u32(b'/' as u32)),
                            Expr::eq(Expr::var("b1"), Expr::u32(b'\\' as u32)),
                        ),
                        Expr::eq(Expr::var("b2"), Expr::u32(b'\n' as u32)),
                    ),
                ),
                Node::let_bind(
                    "slash_splice_crlf",
                    Expr::and(
                        Expr::and(
                            Expr::and(
                                Expr::eq(Expr::var("b0"), Expr::u32(b'/' as u32)),
                                Expr::eq(Expr::var("b1"), Expr::u32(b'\\' as u32)),
                            ),
                            Expr::eq(Expr::var("b2"), Expr::u32(b'\r' as u32)),
                        ),
                        Expr::eq(Expr::var("b3"), Expr::u32(b'\n' as u32)),
                    ),
                ),
                Node::let_bind(
                    "forms_line_comment",
                    Expr::or(
                        Expr::and(
                            Expr::var("slash_splice_lf"),
                            Expr::eq(Expr::var("b3"), Expr::u32(b'/' as u32)),
                        ),
                        Expr::and(
                            Expr::var("slash_splice_crlf"),
                            Expr::eq(Expr::var("b4"), Expr::u32(b'/' as u32)),
                        ),
                    ),
                ),
                Node::let_bind(
                    "forms_block_comment",
                    Expr::or(
                        Expr::and(
                            Expr::var("slash_splice_lf"),
                            Expr::eq(Expr::var("b3"), Expr::u32(b'*' as u32)),
                        ),
                        Expr::and(
                            Expr::var("slash_splice_crlf"),
                            Expr::eq(Expr::var("b4"), Expr::u32(b'*' as u32)),
                        ),
                    ),
                ),
                Node::if_then(
                    Expr::or(
                        Expr::var("forms_line_comment"),
                        Expr::var("forms_block_comment"),
                    ),
                    vec![Node::let_bind(
                        "spliced_comment_flag_old",
                        Expr::atomic_or("spliced_comment_flag", Expr::u32(0), Expr::u32(1)),
                    )],
                ),
            ],
        ),
    ];
    wrap_gpu_filter_program(
        "vyre-libs::parsing::c::preprocess::filter_spliced_comment_preflight",
        vec![
            source_bytes_input_buffer("bytes_in", 0, 0),
            u32_rw_buffer("spliced_comment_flag", 1, 1),
            singleton_u32_read_buffer("spliced_comment_n_real", 2),
        ],
        body,
    )
}
