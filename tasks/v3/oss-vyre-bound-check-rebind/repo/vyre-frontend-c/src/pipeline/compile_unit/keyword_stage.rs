use super::*;

pub(super) fn promote_object_keywords(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &str,
    dcfg: &mut DispatchConfig,
    expanded_haystack_cache: &mut Option<(Vec<u8>, u32)>,
    lexed: &mut lex_stage::ObjectLexTokens,
    trace: &mut trace::CompileTrace,
) -> Result<(), String> {
    promote_c11_keywords(
        backend,
        source,
        dcfg,
        expanded_haystack_cache,
        &mut lexed.types,
        &lexed.starts,
        &lexed.lens,
        &lexed.counts,
        lexed.n_tokens,
        lexed.keyword_promoted,
        lexed
            .cuda_keyword_haystack
            .as_ref()
            .map(|(bytes, len)| (bytes.as_slice(), *len)),
        &format!("vyre-frontend-c keyword {}", path.display()),
        |stage| trace.log(stage),
    )
}
