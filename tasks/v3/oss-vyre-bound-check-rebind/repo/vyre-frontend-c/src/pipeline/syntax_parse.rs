use super::*;
/// Parse a C11 source string through the syntax-only GPU spine.
pub fn parse_c11_syntax_source(source: &str) -> Result<CParseSummary, String> {
    let backend = shared_dispatch_backend()?;
    parse_c11_syntax_source_with_backend(backend.as_ref(), Path::new("memory.c"), source)
}

pub(crate) fn parse_c11_syntax_source_with_backend(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &str,
) -> Result<CParseSummary, String> {
    let trace = std::env::var("VYRE_STAGE_TRACE").is_ok();
    let stage_start = std::time::Instant::now();
    let mut last_t = stage_start;
    let mut log = |label: &str| {
        if trace {
            let now = std::time::Instant::now();
            let stage = now.duration_since(last_t).as_micros();
            let total = now.duration_since(stage_start).as_micros();
            eprintln!("[stage-trace] +{stage}us (total {total}us): syntax-only {label}");
            last_t = now;
        }
    };

    reject_c11_source_diagnostics(path, source)?;
    log("source diagnostics");
    let mut expanded_haystack_cache = None;

    let mut dcfg = DispatchConfig::default();
    let lexed = lex_c11_tokens(
        backend,
        source,
        &mut dcfg,
        &mut expanded_haystack_cache,
        "vyre-frontend-c syntax-only lexer",
        "vyre-frontend-c syntax-only lexer",
        |stage| log(stage),
    )?;
    let C11LexTokens {
        mut types,
        starts,
        lens,
        counts,
        n_tokens,
        keyword_promoted,
        cuda_keyword_haystack,
    } = lexed;

    promote_c11_keywords(
        backend,
        source,
        &mut dcfg,
        &mut expanded_haystack_cache,
        &mut types,
        &starts,
        &lens,
        &counts,
        n_tokens,
        keyword_promoted,
        cuda_keyword_haystack
            .as_ref()
            .map(|(bytes, len)| (bytes.as_slice(), *len)),
        "vyre-frontend-c syntax-only keyword",
        |stage| log(stage),
    )?;

    let decoded = decode_c11_tokens(path, source, &types, &starts, &lens, n_tokens, |stage| {
        log(stage)
    })?;
    let ast =
        build_c11_syntax_ast_stage(backend, &decoded.tok_types, n_tokens, &mut dcfg, |stage| {
            log(stage)
        })?;

    Ok(CParseSummary::syntax_only(
        source.len() as u64,
        n_tokens,
        ast.ast_bytes,
        ast.ast_node_count,
        0,
        0,
    ))
}
