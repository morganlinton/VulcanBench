use super::parse_cache::{cache_key as lexer_cache_key, lexer_output_cache, CachedLexerOutputs};
use super::*;

enum CachedTypeColumn {
    Owned(Vec<u8>),
    Shared(std::sync::Arc<[u8]>),
}

impl CachedTypeColumn {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes.as_slice(),
            Self::Shared(bytes) => bytes.as_ref(),
        }
    }

    fn into_owned(self) -> Vec<u8> {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared(bytes) => bytes.to_vec(),
        }
    }
}

pub(super) fn parse_c11_source_with_backend(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &str,
    target_abi: CTargetAbi,
) -> Result<CParseSummary, String> {
    let trace = std::env::var("VYRE_STAGE_TRACE").is_ok();
    let stage_start = std::time::Instant::now();
    let mut last_t = stage_start;
    let mut log = |label: &str| {
        if trace {
            let now = std::time::Instant::now();
            let stage = now.duration_since(last_t).as_micros();
            let total = now.duration_since(stage_start).as_micros();
            eprintln!("[stage-trace] +{stage}us (total {total}us): parser-sema {label}");
            last_t = now;
        }
    };
    reject_c11_source_diagnostics(path, source)?;
    log("source diagnostics");
    let mut expanded_haystack_cache = None;
    let mut dcfg = DispatchConfig::default();

    // Check the lex output cache. The dense single-thread c11_lexer scans ~200
    // KB of preprocessed source on one GPU thread (~550 ms median). When the
    // same translation unit is parsed twice (warm path / multi-pass parser /
    // bench loop), the lex outputs are deterministic in `(backend_id, source)`
    //  -  cache them so the second call skips the lex dispatch entirely.
    let cache_key = lexer_cache_key(backend.id(), source);
    let cached = lexer_output_cache()
        .lock()
        .map_err(|error| {
            format!(
                "vyre-frontend-c lexer output cache lock poisoned before lookup: {error}. Fix: investigate the previous parser panic instead of silently bypassing the cache."
            )
        })?
        .lookup(&cache_key);
    let mut cache_miss = false;
    let (types, starts, lens, counts, n_tokens, keyword_promoted, cuda_keyword_haystack) =
        if let Some(hit) = cached {
            log("dispatch c11_lexer (cache hit)");
            (
                CachedTypeColumn::Shared(hit.types),
                hit.starts,
                hit.lens,
                hit.counts,
                hit.n_tokens,
                hit.keyword_promoted,
                hit.cuda_keyword_haystack,
            )
        } else {
            cache_miss = true;
            let lexed = lex_c11_tokens(
                backend,
                source,
                &mut dcfg,
                &mut expanded_haystack_cache,
                "vyre-frontend-c parser-sema lexer",
                "vyre-frontend-c parser-sema lexer",
                |stage| log(stage),
            )?;
            let starts = std::sync::Arc::<[u8]>::from(lexed.starts);
            let lens = std::sync::Arc::<[u8]>::from(lexed.lens);
            let counts = std::sync::Arc::<[u8]>::from(lexed.counts);
            let cuda_keyword_haystack = lexed
                .cuda_keyword_haystack
                .map(|(bytes, len)| (std::sync::Arc::<[u8]>::from(bytes), len));
            (
                CachedTypeColumn::Owned(lexed.types),
                starts,
                lens,
                counts,
                lexed.n_tokens,
                lexed.keyword_promoted,
                cuda_keyword_haystack,
            )
        };

    let cuda_keyword_haystack_ref = cuda_keyword_haystack
        .as_ref()
        .map(|(bytes, len)| (bytes.as_ref(), *len));

    let mut unpromoted_types = Some(types);
    let mut promoted_types = None;
    let types_for_decode = if keyword_promoted {
        log("skip c_keyword; lexer promoted keywords");
        let Some(types) = unpromoted_types.as_ref() else {
            return Err(
                "parser-sema keyword cache lost promoted token types before decode. Fix: preserve cached lexer output ownership until decode completes."
                    .to_string(),
            );
        };
        types.as_slice()
    } else {
        let Some(types) = unpromoted_types.take() else {
            return Err(
                "parser-sema keyword promotion lost unpromoted token types. Fix: keep lexer output ownership until c_keyword promotion consumes it."
                    .to_string(),
            );
        };
        let mut keyword_types = types.into_owned();
        promote_c11_keywords(
            backend,
            source,
            &mut dcfg,
            &mut expanded_haystack_cache,
            &mut keyword_types,
            starts.as_ref(),
            lens.as_ref(),
            counts.as_ref(),
            n_tokens,
            keyword_promoted,
            cuda_keyword_haystack_ref,
            "vyre-frontend-c parser-sema keyword",
            |stage| log(stage),
        )?;
        promoted_types = Some(keyword_types);
        let Some(types) = promoted_types.as_ref() else {
            return Err(
                "parser-sema keyword promotion returned without promoted token types. Fix: c_keyword must produce a complete token type column before decode."
                    .to_string(),
            );
        };
        types.as_slice()
    };

    let decoded = decode_c11_tokens(
        path,
        source,
        types_for_decode,
        starts.as_ref(),
        lens.as_ref(),
        n_tokens,
        |stage| log(stage),
    )?;
    if cache_miss {
        let cached_types = if let Some(promoted_types) = promoted_types.take() {
            std::sync::Arc::<[u8]>::from(promoted_types)
        } else if let Some(unpromoted_types) = unpromoted_types.take() {
            std::sync::Arc::<[u8]>::from(unpromoted_types.into_owned())
        } else {
            return Err(
                "parser-sema cache insertion lost token types after decode. Fix: preserve the token type column until warm-cache materialization completes."
                    .to_string(),
            );
        };
        let cache_entry = CachedLexerOutputs {
            types: cached_types,
            starts: std::sync::Arc::clone(&starts),
            lens: std::sync::Arc::clone(&lens),
            counts: std::sync::Arc::clone(&counts),
            n_tokens,
            keyword_promoted: true,
            cuda_keyword_haystack: cuda_keyword_haystack
                .as_ref()
                .map(|(bytes, len)| (std::sync::Arc::clone(bytes), *len)),
        };
        let mut lexer_cache = lexer_output_cache()
            .lock()
            .map_err(|error| {
                format!(
                    "vyre-frontend-c lexer output cache lock poisoned before insert: {error}. Fix: investigate the previous parser panic instead of silently dropping the warm-cache entry."
                )
            })?;
        parse_cache::insert_lexer_output_cache(&mut lexer_cache, cache_key, cache_entry);
    }
    let DecodedC11Tokens {
        tok_types,
        start_words,
        len_words,
        starts_logical,
        lens_logical,
        types_logical,
        n_tokens: _,
        nt,
    } = decoded;
    let semantic_haystack = select_semantic_haystack(
        source,
        &mut expanded_haystack_cache,
        cuda_keyword_haystack_ref,
        |stage| log(stage),
    )?;
    let semantic_features =
        build_semantic_feature_inputs(source.as_bytes(), &tok_types, &start_words, &len_words);

    let structure = build_c11_structure_stage(
        backend,
        &tok_types,
        &types_logical,
        n_tokens,
        &mut dcfg,
        "vyre-frontend-c parser-sema c11-brackets",
        "vyre-frontend-c parser-sema structure",
        |stage| log(stage),
    )?;
    let fn_records = structure.fn_records;
    let call_records = structure.call_records;

    let ast_stage = build_c11_full_ast_stage(
        backend,
        &tok_types,
        &types_logical,
        n_tokens,
        nt,
        C11AstReadback::CountOnly,
        &mut dcfg,
        "vyre-frontend-c parser-sema statement-bounds",
        "vyre-frontend-c parser-sema ast",
        |stage| log(stage),
    )?;
    let ast_bytes = u64::from(ast_stage.ast_capacity)
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_mul(4))
        .and_then(|bytes| bytes.checked_add(4))
        .and_then(|bytes| u64::from(ast_stage.num_stmt).checked_mul(4).and_then(|stmt| bytes.checked_add(stmt)))
        .ok_or_else(|| {
            format!(
                "parser-sema ast_shunting_yard byte accounting overflows u64 for ast_capacity={}, num_stmt={}. Fix: shard AST construction before semantic analysis.",
                ast_stage.ast_capacity,
                ast_stage.num_stmt
            )
        })?;

    let ast_words = read_u32_at(&ast_stage.outputs[0], 0).map_err(|error| {
        format!("parser-sema ast_shunting_yard node-count output decode failed: {error}")
    })?;
    let ast_node_count = (ast_words / 4).max(1);

    let abi_stage = build_c11_abi_stage(
        backend,
        target_abi,
        &tok_types,
        &mut dcfg,
        "vyre-frontend-c parser-sema abi",
        |stage| log(stage),
    )?;
    let abi_layout_bytes = abi_stage.byte_len;

    let semantic_graphs = build_c11_semantic_graphs(
        backend,
        path,
        source.as_bytes(),
        &tok_types,
        &start_words,
        &len_words,
        &types_logical,
        &starts_logical,
        &lens_logical,
        &semantic_haystack,
        nt,
        false,
        &semantic_features,
        |stage| log(stage),
    )?;

    let summary = CParseSummary {
        source_bytes: source.len() as u64,
        token_count: n_tokens,
        ast_bytes,
        ast_node_count,
        vast_bytes: semantic_graphs.vast_bytes,
        abi_layout_bytes,
        expression_shape_bytes: semantic_graphs.expression_shape_bytes,
        program_graph_bytes: semantic_graphs.program_graph_bytes,
        semantic_node_bytes: semantic_graphs.semantic_node_bytes,
        semantic_edge_bytes: semantic_graphs.semantic_edge_bytes,
        sema_scope_bytes: semantic_graphs.sema_scope_bytes,
        function_record_bytes: fn_records.len() as u64,
        call_record_bytes: call_records.len() as u64,
    };
    require_full_semantic_summary(summary, "parser/sema dispatch")?;
    Ok(summary)
}
