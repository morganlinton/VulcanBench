use super::*;
pub(super) const RAW_SPARSE_LEXER_ABI_BUFFERS: &[&str] = &[
    "out_tok_types",
    "out_tok_starts",
    "out_tok_lens",
    "out_counts",
];

pub(super) fn mark_raw_sparse_lexer_outputs(
    mut program: Program,
    readback_names: &[&str],
    live_names: &[&str],
) -> Program {
    for buffer in std::sync::Arc::make_mut(&mut program.buffers) {
        if live_names.iter().any(|name| buffer.name.as_ref() == *name) {
            buffer.access = BufferAccess::ReadWrite;
            buffer.pipeline_live_out = true;
            if readback_names
                .iter()
                .any(|name| buffer.name.as_ref() == *name)
            {
                buffer.is_output = true;
            } else {
                buffer.is_output = false;
                buffer.output_byte_range = Some(0..0);
            }
        }
    }
    program
}

pub(super) fn raw_sparse_lexer_readbacks(quote_free: bool) -> &'static [&'static str] {
    if quote_free {
        &["out_tok_types"]
    } else {
        &["out_tok_types", "out_tok_starts", "out_tok_lens"]
    }
}
