use vyre::ir::Program;

pub fn c11_lexer_regular_sparse_packed_haystack_with_block_totals(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    scratch_counts: &str,
    block_totals: &str,
    haystack_len: u32,
) -> Program {
    super::c11_lexer_regular_sparse_impl(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        scratch_counts,
        haystack_len,
        false,
        false,
        super::SparseHaystackLayout::PackedU32,
        true,
        true,
        Some(block_totals),
    )
}

#[cfg(test)]
mod tests {
    use super::c11_lexer_regular_sparse_packed_haystack_with_block_totals;

    #[test]
    fn block_totals_variant_has_no_unused_scratch_counts_binding() {
        let program = c11_lexer_regular_sparse_packed_haystack_with_block_totals(
            "haystack",
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "scratch_counts",
            "block_totals",
            1024,
        );
        assert!(
            program
                .buffers()
                .iter()
                .all(|buffer| buffer.name() != "scratch_counts"),
            "block-total sparse lexer must not declare the obsolete scratch_counts buffer"
        );
        let block_totals = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "block_totals")
            .expect("Fix: block totals output buffer must be declared");
        assert!(block_totals.is_output());
        for name in ["sparse_types", "sparse_starts", "sparse_lens"] {
            let buffer = program
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == name)
                .expect("Fix: sparse token stream must be declared");
            assert!(
                buffer.is_output(),
                "{name} must be an explicit output because the block-total compact pass consumes it"
            );
        }
    }
}
