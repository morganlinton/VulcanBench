use super::sparse_programs::{
    sparse_token_block_compact_program, sparse_token_type_block_compact_program,
};
use super::{
    mark_raw_sparse_lexer_outputs, raw_sparse_lexer_readbacks, RAW_SPARSE_LEXER_ABI_BUFFERS,
};
use vyre_libs::parsing::c::lex::lexer::c11_lexer_regular_sparse;

#[test]
fn raw_sparse_lexer_readbacks_exclude_counts_for_gpu_compaction() {
    assert_eq!(raw_sparse_lexer_readbacks(true), ["out_tok_types"]);
    assert_eq!(
        raw_sparse_lexer_readbacks(false),
        ["out_tok_types", "out_tok_starts", "out_tok_lens"]
    );
}

#[test]
fn raw_sparse_lexer_abi_live_out_buffers_are_not_host_inputs() {
    let program = c11_lexer_regular_sparse(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        16,
    );
    let program = mark_raw_sparse_lexer_outputs(
        program,
        raw_sparse_lexer_readbacks(false),
        RAW_SPARSE_LEXER_ABI_BUFFERS,
    );
    let readback_count = program
        .buffers
        .iter()
        .filter(|buffer| buffer.is_output)
        .count();
    let input_count = program
        .buffers
        .iter()
        .filter(|buffer| {
            !buffer.is_output
                && !(buffer.pipeline_live_out
                    && buffer.access == vyre_foundation::ir::BufferAccess::ReadWrite)
                && matches!(
                    buffer.access,
                    vyre_foundation::ir::BufferAccess::ReadOnly
                        | vyre_foundation::ir::BufferAccess::ReadWrite
                        | vyre_foundation::ir::BufferAccess::Uniform
                )
        })
        .count();
    assert_eq!(readback_count, 3);
    assert_eq!(input_count, 1);
}

#[test]
fn raw_sparse_compaction_programs_emit_one_packed_output_buffer() {
    let full = sparse_token_block_compact_program(
        "block_totals_scanned",
        "sparse_types",
        "sparse_starts",
        "sparse_lens",
        "out_tok_triplets_and_count",
        64,
        1,
    );
    let type_only = sparse_token_type_block_compact_program(
        "block_totals_scanned",
        "sparse_types",
        "out_tok_types_and_count",
        64,
        1,
    );
    for program in [full, type_only] {
        let outputs = program
            .buffers
            .iter()
            .filter(|buffer| buffer.is_output)
            .count();
        assert_eq!(
            outputs, 1,
            "raw syntax compaction must preserve the single-output backend ABI"
        );
    }
}
