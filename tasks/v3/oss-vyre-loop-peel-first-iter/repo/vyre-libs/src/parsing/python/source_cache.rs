//! Content-keyed Python 3.12 pipeline Program cache.

use std::sync::Arc;

use crate::parsing::python::lex::python312_lexer;
use crate::parsing::python::parse::calls::python312_extract_calls;
use crate::parsing::python::parse::structure::{
    python312_extract_imports, python312_extract_structure,
};
use crate::parsing::source_cache::{source_len_u32_nonzero, ParsedSourceLru};
use vyre::ir::Program;

/// Cached Python pipeline stage bundle for one source shape.
#[derive(Debug, Clone)]
pub struct Python312PipelinePrograms {
    /// Python lexer Program.
    pub lex: Program,
    /// Definition / class / span structural extractor.
    pub structure: Program,
    /// Import extractor.
    pub imports: Program,
    /// Call-site extractor.
    pub calls: Program,
}

/// Bounded content-keyed Python pipeline cache.
pub type Python312PipelineCache = ParsedSourceLru<Python312PipelinePrograms>;

/// Build or fetch the Python 3.12 pipeline bundle for `source`.
pub fn get_or_build_python312_pipeline(
    cache: &Python312PipelineCache,
    source: &[u8],
    extra: &[u8],
) -> Arc<Python312PipelinePrograms> {
    cache.get_or_parse(source, extra, |bytes| {
        let len = source_len_u32_nonzero(bytes);
        Python312PipelinePrograms {
            lex: python312_lexer(
                "haystack",
                "tok_types",
                "tok_starts",
                "tok_lens",
                "tok_counts",
                len,
            ),
            structure: python312_extract_structure(
                "tok_types",
                "tok_starts",
                "tok_lens",
                "structure_records",
                "structure_counts",
                len,
            ),
            imports: python312_extract_imports(
                "tok_types",
                "tok_starts",
                "tok_lens",
                "import_records",
                "import_counts",
                len,
            ),
            calls: python312_extract_calls(
                "tok_types",
                "tok_starts",
                "tok_lens",
                "call_records",
                "call_counts",
                "kwarg_records",
                "kwarg_counts",
                len,
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_pipeline_cache_reuses_same_source_bundle() {
        let cache = Python312PipelineCache::with_capacity(4);
        let a = get_or_build_python312_pipeline(&cache, b"def f():\n    return 1\n", b"py312");
        let b = get_or_build_python312_pipeline(&cache, b"def f():\n    return 1\n", b"py312");
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn python_pipeline_cache_separates_options() {
        let cache = Python312PipelineCache::with_capacity(4);
        let a = get_or_build_python312_pipeline(&cache, b"x = 1\n", b"mode=a");
        let b = get_or_build_python312_pipeline(&cache, b"x = 1\n", b"mode=b");
        assert!(!Arc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 2);
    }
}
