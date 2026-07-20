use super::*;
pub(crate) fn cuda_sparse_lexer_strategy(
    backend: &dyn VyreBackend,
    source: &[u8],
) -> Result<CudaSparseLexerStrategy, String> {
    if backend.id() != "cuda" || source.is_empty() {
        return Ok(CudaSparseLexerStrategy::None);
    }
    match classify_regular_sparse_lexer_source(source) {
        SparseLexerSourceClass::Rejected => Ok(CudaSparseLexerStrategy::None),
        SparseLexerSourceClass::FastNoLiterals => Ok(CudaSparseLexerStrategy::FastNoLiterals),
        SparseLexerSourceClass::Megakernel => Ok(CudaSparseLexerStrategy::Megakernel),
    }
}
