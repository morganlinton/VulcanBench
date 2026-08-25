use vyre::VyreBackend;

const CUDA_SPARSE_LEX_MAX_TOKEN_SCAN: usize = 65_536;

mod literals;
mod numerics;
mod source_scan;
mod strategy;

use literals::{
    sparse_block_comment_end, sparse_char_literal_end, sparse_line_comment_end,
    sparse_prefixed_char_literal_end, sparse_string_literal_end,
};
use numerics::{sparse_numeric_literal_end, sparse_numeric_literal_supported};
pub(super) use source_scan::{
    classify_regular_sparse_lexer_source, source_can_use_regular_sparse_lexer,
    SparseLexerSourceClass,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CudaSparseLexerStrategy {
    None,
    FastNoLiterals,
    Megakernel,
}

pub(super) use strategy::cuda_sparse_lexer_strategy;
