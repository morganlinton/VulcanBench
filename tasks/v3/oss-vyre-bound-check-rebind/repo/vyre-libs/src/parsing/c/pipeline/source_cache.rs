//! Content-keyed C11 pipeline Program cache.
//!
//! The GPU C frontend builds several `Program` stages from the same source
//! shape. This wrapper opts the C pipeline into the shared `ParsedSourceLru`
//! substrate so repeated translation units with identical bytes reuse one
//! Arc-owned stage bundle instead of rebuilding every Program.

use std::sync::Arc;

use crate::parsing::c::lex::lexer::c11_lex_single_pass;
use crate::parsing::c::parse::structure::{c11_extract_calls, c11_extract_functions};
use crate::parsing::source_cache::{source_len_u32_nonzero, ParsedSourceLru};
use vyre::ir::{Expr, Program};

/// Cached C11 pipeline stage bundle for one source shape.
#[derive(Debug, Clone)]
pub struct C11PipelinePrograms {
    /// Fused lexer + digraph rewrite.
    pub lex: Program,
    /// Function extractor over the token stream.
    pub functions: Program,
    /// Call-site extractor over the token stream and function table.
    pub calls: Program,
}

/// Bounded content-keyed C11 pipeline cache.
pub type C11PipelineCache = ParsedSourceLru<C11PipelinePrograms>;

/// Build or fetch the C11 pipeline bundle for `source`.
///
/// `extra` separates build-flag variants that share the same source bytes
/// but differ in preprocessor or dialect options.
pub fn get_or_build_c11_pipeline(
    cache: &C11PipelineCache,
    source: &[u8],
    extra: &[u8],
) -> Arc<C11PipelinePrograms> {
    cache.get_or_parse(source, extra, |bytes| {
        let len = source_len_u32_nonzero(bytes);
        let token_capacity = len.checked_next_power_of_two().unwrap_or(u32::MAX);
        C11PipelinePrograms {
            lex: c11_lex_single_pass(
                "haystack",
                "tok_types",
                "tok_starts",
                "tok_lens",
                "tok_counts",
                len,
                token_capacity,
            ),
            functions: c11_extract_functions(
                "tok_types",
                "paren_pairs",
                "brace_pairs",
                Expr::u32(token_capacity),
                "functions",
                "function_counts",
            ),
            calls: c11_extract_calls(
                "tok_types",
                "paren_pairs",
                "functions",
                Expr::u32(token_capacity),
                Expr::u32(token_capacity),
                "calls",
                "call_counts",
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c11_pipeline_cache_reuses_same_source_bundle() {
        let cache = C11PipelineCache::with_capacity(4);
        let a = get_or_build_c11_pipeline(&cache, b"int main(void){return 0;}", b"-std=c11");
        let b = get_or_build_c11_pipeline(&cache, b"int main(void){return 0;}", b"-std=c11");
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn c11_pipeline_cache_separates_build_flags() {
        let cache = C11PipelineCache::with_capacity(4);
        let a = get_or_build_c11_pipeline(&cache, b"int x;", b"-DA");
        let b = get_or_build_c11_pipeline(&cache, b"int x;", b"-DB");
        assert!(!Arc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 2);
    }
}
