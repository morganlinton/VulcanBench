use super::*;
/// Parse a C11 source string through the full GPU parser and semantic pipeline.
pub fn parse_c11_source(source: &str) -> Result<CParseSummary, String> {
    let backend = shared_dispatch_backend()?;
    let key = parse_cache::semantic_summary_cache_key(
        backend.id(),
        source,
        crate::api::CTargetOptions::default().cache_tag(),
    );
    {
        let mut guard = parse_cache::summary_cache().lock().map_err(|_| {
            "vyre-frontend-c summary cache mutex is poisoned. Fix: restart the process and investigate the previous parser panic before reusing cached in-memory semantic evidence.".to_string()
        })?;
        if let Some(summary) = guard.lookup(&key) {
            require_full_semantic_summary(summary, "process summary cache")?;
            return Ok(summary);
        }
    }
    if let Some(summary) = parse_cache::load_summary_from_disk(key)? {
        require_full_semantic_summary(summary, "disk summary cache")?;
        let mut guard = parse_cache::summary_cache().lock().map_err(|_| {
            "vyre-frontend-c summary cache mutex is poisoned while promoting in-memory disk cache evidence. Fix: restart the process and investigate the previous parser panic.".to_string()
        })?;
        parse_cache::insert_summary_cache(&mut guard, key, summary);
        return Ok(summary);
    }
    let summary = parse_c11_source_with_backend(
        backend.as_ref(),
        Path::new("memory.c"),
        source,
        CTargetAbi::default(),
    )?;
    require_full_semantic_summary(summary, "fresh parser/sema dispatch")?;
    {
        let mut guard = parse_cache::summary_cache().lock().map_err(|_| {
            "vyre-frontend-c summary cache mutex is poisoned while storing in-memory parser evidence. Fix: restart the process and investigate the previous parser panic.".to_string()
        })?;
        parse_cache::insert_summary_cache(&mut guard, key, summary);
    }
    parse_cache::store_summary_to_disk(key, &summary)?;
    Ok(summary)
}

/// Full GPU parser/sema summary for a real translation-unit path using the same
/// resident preprocessing contract as object compilation, without emitting an
/// ELF/VYRECOB2 carrier.
pub fn parse_c11_translation_unit(
    path: &Path,
    options: &VyreCompileOptions,
) -> Result<CParseSummary, String> {
    let raw_bytes = read_translation_unit_bounded(path)?;
    parse_c11_translation_unit_bytes(path, &raw_bytes, options)
}

/// Full GPU parser/sema summary for already-loaded source bytes. This is the
/// benchmark/fairness path: callers can share one source preload with external
/// comparators while keeping vyre's real path-aware include context.
///
/// Three-layer cache (in `pipeline::parse_cache`):
///   1. process-local `summary_cache()`  -  returns cached summary in <1 µs.
///   2. on-disk summary cache  -  survives process restarts.
///   3. lex output cache (further down inside `parse_c11_source_with_backend`).
pub fn parse_c11_translation_unit_bytes(
    path: &Path,
    raw_bytes: &[u8],
    options: &VyreCompileOptions,
) -> Result<CParseSummary, String> {
    let backend = shared_dispatch_backend()?;
    let prepared = prepare_translation_unit_from_bytes(path, PathBuf::new(), raw_bytes, options)?;
    let key = parse_cache::semantic_summary_cache_key(
        backend.id(),
        &prepared.source,
        options.target.cache_tag(),
    );
    {
        let mut guard = parse_cache::summary_cache().lock().map_err(|_| {
            "vyre-frontend-c summary cache mutex is poisoned. Fix: restart the process and investigate the previous parser panic before reusing cached semantic evidence.".to_string()
        })?;
        if let Some(summary) = guard.lookup(&key) {
            require_full_semantic_summary(summary, "process summary cache")?;
            return Ok(summary);
        }
    }
    if let Some(summary) = parse_cache::load_summary_from_disk(key)? {
        require_full_semantic_summary(summary, "disk summary cache")?;
        let mut guard = parse_cache::summary_cache().lock().map_err(|_| {
            "vyre-frontend-c summary cache mutex is poisoned while promoting disk cache evidence. Fix: restart the process and investigate the previous parser panic.".to_string()
        })?;
        parse_cache::insert_summary_cache(&mut guard, key, summary);
        return Ok(summary);
    }
    let summary = parse_c11_source_with_backend(
        backend.as_ref(),
        prepared.path.as_path(),
        &prepared.source,
        options.target.abi,
    )?;
    require_full_semantic_summary(summary, "fresh parser/sema dispatch")?;
    {
        let mut guard = parse_cache::summary_cache().lock().map_err(|_| {
            "vyre-frontend-c summary cache mutex is poisoned while storing parser evidence. Fix: restart the process and investigate the previous parser panic.".to_string()
        })?;
        parse_cache::insert_summary_cache(&mut guard, key, summary);
    }
    parse_cache::store_summary_to_disk(key, &summary)?;
    Ok(summary)
}

// Syntax-only GPU C11 spine entry point was moved to `syntax_parse`.
