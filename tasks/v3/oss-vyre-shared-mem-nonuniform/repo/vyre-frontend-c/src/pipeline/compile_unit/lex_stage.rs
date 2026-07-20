use super::*;

pub(super) type ObjectLexTokens = C11LexTokens;

pub(super) fn lex_object_tokens(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &str,
    dcfg: &mut DispatchConfig,
    expanded_haystack_cache: &mut Option<(Vec<u8>, u32)>,
    trace: &mut trace::CompileTrace,
) -> Result<ObjectLexTokens, String> {
    lex_c11_tokens(
        backend,
        source,
        dcfg,
        expanded_haystack_cache,
        &format!("vyre-frontend-c lexer {}", path.display()),
        "vyre-frontend-c object lexer",
        |stage| trace.log(stage),
    )
}
