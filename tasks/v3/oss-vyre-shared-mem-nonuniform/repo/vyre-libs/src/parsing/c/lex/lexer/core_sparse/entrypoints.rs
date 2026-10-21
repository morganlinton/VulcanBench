use super::*;

pub fn c11_lexer_regular_sparse_packed_haystack_with_flags(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    c11_lexer_regular_sparse_impl(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        haystack_len,
        false,
        true,
        SparseHaystackLayout::PackedU32,
        true,
        true,
        None,
    )
}

pub fn c11_lexer_regular_sparse_u8_haystack_with_flags(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    c11_lexer_regular_sparse_impl(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        haystack_len,
        false,
        true,
        SparseHaystackLayout::RawU8,
        true,
        true,
        None,
    )
}

pub fn c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    c11_lexer_regular_sparse_impl(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        haystack_len,
        false,
        true,
        SparseHaystackLayout::PackedU32,
        false,
        true,
        None,
    )
}

pub fn c11_lexer_regular_sparse_packed_haystack_with_flags_no_directives_no_backscan(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    c11_lexer_regular_sparse_impl(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        haystack_len,
        false,
        true,
        SparseHaystackLayout::PackedU32,
        false,
        false,
        None,
    )
}

pub fn c11_lexer_regular_sparse_no_directives_no_backscan(
    haystack: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    haystack_len: u32,
) -> Program {
    c11_lexer_regular_sparse_impl(
        haystack,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        haystack_len,
        true,
        false,
        SparseHaystackLayout::ExpandedU32,
        false,
        false,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::DataType;

    #[test]
    fn u8_haystack_entrypoint_declares_runtime_sized_raw_byte_source() {
        let program = c11_lexer_regular_sparse_u8_haystack_with_flags(
            "haystack", "types", "starts", "lens", "flags", 17,
        );
        let haystack = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "haystack")
            .expect("sparse lexer must declare haystack input");

        assert_eq!(haystack.element(), DataType::U8);
        assert_eq!(haystack.count(), 0);
    }

    #[test]
    fn packed_haystack_entrypoint_keeps_u32_word_source() {
        let program = c11_lexer_regular_sparse_packed_haystack_with_flags(
            "haystack", "types", "starts", "lens", "flags", 17,
        );
        let haystack = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "haystack")
            .expect("sparse lexer must declare haystack input");

        assert_eq!(haystack.element(), DataType::U32);
        assert_eq!(haystack.count(), 5);
    }

    #[test]
    fn expanded_haystack_entrypoint_keeps_u32_per_byte_source() {
        let program = c11_lexer_regular_sparse_no_directives_no_backscan(
            "haystack", "types", "starts", "lens", "count", 17,
        );
        let haystack = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "haystack")
            .expect("sparse lexer must declare haystack input");

        assert_eq!(haystack.element(), DataType::U32);
        assert_eq!(haystack.count(), 17);
    }
}
