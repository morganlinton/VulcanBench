use super::*;

pub(crate) fn flush_active_macro_segment(
    dispatcher: &dyn GpuDispatcher,
    file_path: &std::path::Path,
    include_stack: &[std::path::PathBuf],
    parent_classified: &ClassifiedTokens,
    segment_start: usize,
    macros: &[MacroDef],
    macro_events: &[MacroEvent],
    macro_expansion_cache: &mut MacroExpansionCache,
    segment: &mut Vec<u8>,
    output: &mut Vec<u8>,
    macro_expansion_events: &mut Vec<MacroExpansionEvent>,
    token_provenance_events: &mut Vec<TokenProvenanceEvent>,
) -> Result<(), String> {
    flush_active_macro_segment_inner(
        dispatcher,
        file_path,
        include_stack,
        parent_classified,
        segment_start,
        macros,
        macro_events,
        macro_expansion_cache,
        segment,
        output,
        macro_expansion_events,
        token_provenance_events,
        0,
    )
}

pub(crate) fn flush_active_macro_segment_inner(
    dispatcher: &dyn GpuDispatcher,
    file_path: &std::path::Path,
    include_stack: &[std::path::PathBuf],
    parent_classified: &ClassifiedTokens,
    segment_start: usize,
    macros: &[MacroDef],
    macro_events: &[MacroEvent],
    macro_expansion_cache: &mut MacroExpansionCache,
    segment: &mut Vec<u8>,
    output: &mut Vec<u8>,
    macro_expansion_events: &mut Vec<MacroExpansionEvent>,
    token_provenance_events: &mut Vec<TokenProvenanceEvent>,
    rescan_depth: usize,
) -> Result<(), String> {
    if rescan_depth > MACRO_RESCAN_DEPTH_LIMIT {
        return Err(format!(
            "vyre-libs::gpu_pipeline: macro rescan depth exceeded {MACRO_RESCAN_DEPTH_LIMIT} in {}. Fix: preserve the C disabled-macro expansion set across recursive rescans.",
            file_path.display()
        ));
    }
    if segment.is_empty() {
        return Ok(());
    }
    let trace = std::env::var_os("VYRE_STAGE_TRACE").is_some();
    let flush_start = std::time::Instant::now();
    if segment.iter().all(|byte| byte.is_ascii_whitespace()) {
        output.extend_from_slice(segment);
        if trace {
            tracing::debug!(
                "[stage-trace] macro-flush direct whitespace file={} segment_bytes={} elapsed_us={}",
                file_path.display(),
                segment.len(),
                flush_start.elapsed().as_micros()
            );
        }
        segment.clear();
        return Ok(());
    }
    if macros.is_empty() {
        let classified = classified_segment(parent_classified, segment_start, segment.len())?;
        record_direct_token_provenance(
            file_path,
            include_stack,
            &classified,
            output.len(),
            token_provenance_events,
        )?;
        output.extend_from_slice(segment);
        if trace {
            tracing::debug!(
                "[stage-trace] macro-flush direct no-macros file={} segment_bytes={} elapsed_us={}",
                file_path.display(),
                segment.len(),
                flush_start.elapsed().as_micros()
            );
        }
        segment.clear();
        return Ok(());
    }

    let classified = classified_segment(parent_classified, segment_start, segment.len())?;
    if classified.tok_types.is_empty() {
        output.extend_from_slice(segment);
        if trace {
            tracing::debug!(
                "[stage-trace] macro-flush direct no-tokens file={} segment_bytes={} elapsed_us={}",
                file_path.display(),
                segment.len(),
                flush_start.elapsed().as_micros()
            );
        }
        segment.clear();
        return Ok(());
    }
    let mut segment_macros = live_macro_defs_for_segment(
        macros,
        &classified,
        &mut macro_expansion_cache.live_macro_lookup,
    )?;
    if segment_macros.is_empty() {
        record_direct_token_provenance(
            file_path,
            include_stack,
            &classified,
            output.len(),
            token_provenance_events,
        )?;
        output.extend_from_slice(segment);
        if trace {
            tracing::debug!(
                "[stage-trace] macro-flush direct no-live-use file={} segment_bytes={} tokens={} live_macros={} elapsed_us={}",
                file_path.display(),
                segment.len(),
                classified.tok_types.len(),
                macros.len(),
                flush_start.elapsed().as_micros()
            );
        }
        segment.clear();
        return Ok(());
    }
    if let Some(prescan_macros) = function_argument_prescan_macros(
        &classified,
        &segment_macros,
        macros,
        &mut macro_expansion_cache.live_macro_lookup,
    )? {
        segment_macros = prescan_macros;
    }
    if let Some(ranges) = macro_use_statement_ranges(&classified, &segment_macros)? {
        flush_macro_segment_ranges(
            dispatcher,
            file_path,
            include_stack,
            parent_classified,
            segment_start,
            macros,
            macro_events,
            macro_expansion_cache,
            segment,
            output,
            macro_expansion_events,
            token_provenance_events,
            ranges,
        )?;
        return Ok(());
    }
    if let Some(ranges) = macro_segment_shard_ranges(&classified)? {
        flush_macro_segment_ranges(
            dispatcher,
            file_path,
            include_stack,
            parent_classified,
            segment_start,
            macros,
            macro_events,
            macro_expansion_cache,
            segment,
            output,
            macro_expansion_events,
            token_provenance_events,
            ranges,
        )?;
        return Ok(());
    }
    let output_base = output.len();
    let cache_key = macro_segment_cache_key(&classified, &segment_macros);
    record_macro_expansions(
        file_path,
        include_stack,
        &segment_macros,
        &classified,
        macro_expansion_events,
    )?;
    if let Some(cached) = macro_expansion_cache.cached_expanded_segment(&cache_key) {
        output.extend_from_slice(&cached.bytes);
        record_macro_token_provenance(
            dispatcher,
            file_path,
            include_stack,
            &segment_macros,
            macro_events,
            &classified,
            &cached.classified,
            output_base,
            token_provenance_events,
        )?;
        if trace {
            tracing::debug!(
                "[stage-trace] macro-flush expanded-cache-hit file={} segment_bytes={} out_bytes={} out_tokens={} elapsed_us={}",
                file_path.display(),
                segment.len(),
                cached.bytes.len(),
                cached.classified.tok_types.len(),
                flush_start.elapsed().as_micros()
            );
        }
        segment.clear();
        return Ok(());
    }
    if trace {
        let macro_names = segment_macros
            .iter()
            .take(4)
            .map(|mac| String::from_utf8_lossy(&mac.name).into_owned())
            .collect::<Vec<_>>()
            .join(",");
        tracing::debug!(
            "[stage-trace] macro-flush expand-start file={} segment_bytes={} tokens={} live_macros={} segment_macros={} macro_names={} elapsed_us={}",
            file_path.display(),
            segment.len(),
            classified.tok_types.len(),
            macros.len(),
            segment_macros.len(),
            macro_names,
            flush_start.elapsed().as_micros()
        );
    }

    let (table, dispatch_scratch) = macro_expansion_cache
        .packed_macro_table_with_dispatch_scratch(cache_key.macro_hash, &segment_macros)?;
    let token_count_actual = classified.tok_types.len().max(1);
    let source_len_actual = classified.source.len();
    let token_count_bucket = bucket_pow2(token_count_actual, 512);
    let token_count = checked_gpu_u32("macro expansion token count", token_count_actual)?;
    let source_len = checked_gpu_u32("macro expansion source length", source_len_actual)?;
    let max_body_len = segment_macros
        .iter()
        .map(|mac| mac.body.len())
        .max()
        .unwrap_or(0);
    let max_out_source_bytes = checked_gpu_u32(
        "macro expansion output source capacity",
        bucket_pow2(
            classified
                .source
                .len()
                .max(source_len_actual)
                .saturating_add(classified.tok_types.len().saturating_mul(max_body_len))
                .saturating_add(classified.tok_types.len())
                .max(1),
            MACRO_EXPANSION_MIN_OUTPUT_SOURCE_BYTES,
        ),
    )?;
    let max_out_tokens = checked_gpu_u32(
        "macro expansion output token capacity",
        bucket_pow2(
            classified
                .tok_types
                .len()
                .saturating_mul(table.expansion_max_replacement_tokens.max(1) as usize)
                .max(token_count_actual),
            MACRO_EXPANSION_MIN_OUTPUT_TOKENS,
        ),
    )?;

    let replacement_source_len = checked_gpu_u32(
        "macro expansion replacement source length",
        table.expansion_replacement_source_len.max(1) as usize,
    )?;
    let mut program = opt_named_macro_expansion_materialized(
        "in_tok_types",
        "in_tok_starts",
        "in_tok_lens",
        "source_words",
        "macro_name_hashes",
        "macro_name_starts",
        "macro_name_lens",
        "macro_name_words",
        "macro_vals",
        "macro_sizes",
        "macro_kinds",
        "macro_param_counts",
        "macro_replacement_params",
        "macro_replacement_starts",
        "macro_replacement_lens",
        "macro_replacement_words",
        "runtime_counts",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_source_words",
        "out_tok_counts",
        "out_source_counts",
        Expr::load("runtime_counts", Expr::u32(0)),
        Expr::load("runtime_counts", Expr::u32(1)),
        Expr::load("runtime_counts", Expr::u32(2)),
        checked_gpu_u32("macro expansion input token capacity", token_count_bucket)?,
        source_len.max(1),
        checked_gpu_u32(
            "macro expansion replacement source capacity",
            table.expansion_replacement_bytes.len().max(1),
        )?,
        max_out_tokens,
        max_out_source_bytes,
    );
    if !dispatcher.requires_output_inputs() {
        program = materialized_output_program(program);
    }
    dispatch_scratch.ensure_input_buffers(12);
    pack_u32_words_into(
        dispatch_scratch.input_buffer_mut(0),
        &classified.tok_types,
        token_count_bucket,
    )?;
    pack_u32_words_into(
        dispatch_scratch.input_buffer_mut(1),
        &classified.tok_starts,
        token_count_bucket,
    )?;
    pack_u32_words_into(
        dispatch_scratch.input_buffer_mut(2),
        &classified.tok_lens,
        token_count_bucket,
    )?;
    dispatch_scratch.write_zero_bytes(
        4,
        checked_staging_word_bytes(
            max_out_tokens as usize,
            "macro expansion output token types",
        )?,
    )?;
    dispatch_scratch.write_zero_bytes(
        5,
        checked_staging_word_bytes(
            max_out_tokens as usize,
            "macro expansion output token starts",
        )?,
    )?;
    dispatch_scratch.write_zero_bytes(
        6,
        checked_staging_word_bytes(
            max_out_tokens as usize,
            "macro expansion output token lengths",
        )?,
    )?;
    dispatch_scratch.write_zero_bytes(
        7,
        checked_staging_word_bytes(
            max_out_source_bytes as usize,
            "macro expansion output source bytes",
        )?,
    )?;
    dispatch_scratch.write_zero_bytes(8, 4)?;
    dispatch_scratch.write_zero_bytes(9, 4)?;
    dispatch_scratch.write_zero_bytes(
        10,
        checked_staging_word_bytes(token_count_bucket, "macro expansion replacement starts")?,
    )?;
    dispatch_scratch.write_zero_bytes(
        11,
        checked_staging_word_bytes(token_count_bucket, "macro expansion replacement lengths")?,
    )?;
    dispatch_scratch.write_runtime_counts(token_count, source_len, replacement_source_len);

    if dispatcher.requires_output_inputs() {
        let input_buffers = &dispatch_scratch.input_buffers;
        let outputs = &mut dispatch_scratch.outputs;
        let input_refs = [
            input_buffers[0].as_slice(),
            input_buffers[1].as_slice(),
            input_buffers[2].as_slice(),
            classified.source.as_ref(),
            table.expansion_name_hashes_le.as_slice(),
            table.expansion_name_starts_le.as_slice(),
            table.expansion_name_lens_le.as_slice(),
            table.expansion_name_bytes.as_slice(),
            table.expansion_vals_le.as_slice(),
            table.expansion_sizes_le.as_slice(),
            table.expansion_kinds_le.as_slice(),
            table.expansion_param_counts_le.as_slice(),
            table.expansion_replacement_params_le.as_slice(),
            table.expansion_replacement_starts_le.as_slice(),
            table.expansion_replacement_lens_le.as_slice(),
            table.expansion_replacement_bytes.as_slice(),
            dispatch_scratch.runtime_counts.as_slice(),
            input_buffers[4].as_slice(),
            input_buffers[5].as_slice(),
            input_buffers[6].as_slice(),
            input_buffers[7].as_slice(),
            input_buffers[8].as_slice(),
            input_buffers[9].as_slice(),
            input_buffers[10].as_slice(),
            input_buffers[11].as_slice(),
        ];
        dispatcher
            .dispatch_borrowed_into(&program, &input_refs, outputs)
            .map_err(|error| format!("named macro expansion materialization: {error}"))?;
    } else {
        let input_buffers = &dispatch_scratch.input_buffers;
        let outputs = &mut dispatch_scratch.outputs;
        let input_refs = [
            input_buffers[0].as_slice(),
            input_buffers[1].as_slice(),
            input_buffers[2].as_slice(),
            classified.source.as_ref(),
            table.expansion_name_hashes_le.as_slice(),
            table.expansion_name_starts_le.as_slice(),
            table.expansion_name_lens_le.as_slice(),
            table.expansion_name_bytes.as_slice(),
            table.expansion_vals_le.as_slice(),
            table.expansion_sizes_le.as_slice(),
            table.expansion_kinds_le.as_slice(),
            table.expansion_param_counts_le.as_slice(),
            table.expansion_replacement_params_le.as_slice(),
            table.expansion_replacement_starts_le.as_slice(),
            table.expansion_replacement_lens_le.as_slice(),
            table.expansion_replacement_bytes.as_slice(),
            dispatch_scratch.runtime_counts.as_slice(),
            input_buffers[10].as_slice(),
            input_buffers[11].as_slice(),
        ];
        dispatcher
            .dispatch_borrowed_into(&program, &input_refs, outputs)
            .map_err(|error| format!("named macro expansion materialization: {error}"))?;
    }
    let expanded = &dispatch_scratch.outputs;
    if trace {
        tracing::debug!(
            "[stage-trace] macro-flush expand-dispatched file={} segment_bytes={} elapsed_us={}",
            file_path.display(),
            segment.len(),
            flush_start.elapsed().as_micros()
        );
    }
    if expanded.len() < 6 {
        return Err(format!(
            "named macro expansion materialization: expected at least 6 outputs, got {}. Fix: backend must return the declared macro expansion ABI outputs.",
            expanded.len()
        ));
    }
    let source_words = &expanded[3];
    let token_counts = &expanded[4];
    let source_counts = &expanded[5];
    let expanded_token_count =
        read_u32_scalar_exact(token_counts, "named macro expansion token count")? as usize;
    let source_count =
        read_u32_scalar_exact(source_counts, "named macro expansion source count")? as usize;
    let token_capacity = expanded[0].len() / 4;
    if expanded_token_count > token_capacity {
        return Err(format!(
            "named macro expansion token count {expanded_token_count} exceeds output token capacity {token_capacity}. Fix: backend must keep out_tok_counts within token column capacity."
        ));
    }
    let source_capacity = source_words.len() / 4;
    if source_count > source_capacity {
        return Err(format!(
            "named macro expansion source count {source_count} exceeds output source capacity {source_capacity}. Fix: backend must keep out_source_counts within out_source_words capacity."
        ));
    }
    let output_base_before_push = output.len();
    output.try_reserve_exact(source_count).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {source_count} expanded macro source bytes: {error:?}. Fix: shard macro expansion materialization before GPU preprocessing."
        )
    })?;
    output.extend(
        source_words
            .chunks_exact(4)
            .take(source_count)
            .map(|word| word[0]),
    );
    let expanded_classified = expanded_classified_from_materialized_outputs(
        &expanded,
        expanded_token_count,
        &output[output_base_before_push..output_base_before_push + source_count],
    )?;
    let expanded_range_start = output_base_before_push;
    let expanded_range_end = output_base_before_push + source_count;
    let expansion_made_progress = output[expanded_range_start..expanded_range_end] != *segment;
    let disabled_rescan_names = disabled_self_recursive_macro_names(&segment_macros);
    let should_rescan = expansion_made_progress
        && has_live_macro_for_segment_excluding(
            macros,
            &expanded_classified,
            &disabled_rescan_names,
            &mut macro_expansion_cache.live_macro_lookup,
        )?;
    let mut rescan_live_macros = if should_rescan {
        live_macro_defs_for_segment(
            macros,
            &expanded_classified,
            &mut macro_expansion_cache.live_macro_lookup,
        )?
    } else {
        Vec::new()
    };
    if !disabled_rescan_names.is_empty() {
        rescan_live_macros.retain(|mac| !disabled_rescan_names.contains(mac.name.as_slice()));
    }
    if !rescan_live_macros.is_empty() {
        let mut rescanned_segment = macro_expansion_cache.take_rescan_segment_scratch();
        rescanned_segment.clear();
        rescanned_segment.extend_from_slice(&output[expanded_range_start..expanded_range_end]);
        output.truncate(output_base_before_push);
        let available_rescan_macros;
        let recursive_macros = if disabled_rescan_names.is_empty() {
            macros
        } else {
            available_rescan_macros = macros
                .iter()
                .filter(|mac| !disabled_rescan_names.contains(mac.name.as_slice()))
                .cloned()
                .collect::<Vec<_>>();
            available_rescan_macros.as_slice()
        };
        let rescan_result = flush_active_macro_segment_inner(
            dispatcher,
            file_path,
            include_stack,
            &expanded_classified,
            0,
            recursive_macros,
            macro_events,
            macro_expansion_cache,
            &mut rescanned_segment,
            output,
            macro_expansion_events,
            token_provenance_events,
            rescan_depth + 1,
        );
        segment.clear();
        macro_expansion_cache.store_rescan_segment_scratch(rescanned_segment);
        rescan_result?;
        return Ok(());
    }
    if trace {
        tracing::debug!(
            "[stage-trace] macro-flush expanded-columns-decoded file={} segment_bytes={} out_bytes={} out_tokens={} elapsed_us={}",
            file_path.display(),
            segment.len(),
            source_count,
            expanded_token_count,
            flush_start.elapsed().as_micros()
        );
    }
    record_macro_token_provenance(
        dispatcher,
        file_path,
        include_stack,
        &segment_macros,
        macro_events,
        &classified,
        &expanded_classified,
        output_base,
        token_provenance_events,
    )?;
    let expanded_bytes = output[expanded_range_start..expanded_range_end].to_vec();
    macro_expansion_cache.insert_expanded_segment(
        cache_key,
        CachedExpandedSegment {
            bytes: expanded_bytes,
            classified: expanded_classified,
        },
    );
    if trace {
        tracing::debug!(
            "[stage-trace] macro-flush provenance-recorded file={} segment_bytes={} elapsed_us={}",
            file_path.display(),
            segment.len(),
            flush_start.elapsed().as_micros()
        );
    }
    if trace {
        tracing::debug!(
            "[stage-trace] macro-flush expanded file={} segment_bytes={} tokens={} live_macros={} segment_macros={} out_bytes={} elapsed_us={}",
            file_path.display(),
            segment.len(),
            classified.tok_types.len(),
            macros.len(),
            segment_macros.len(),
            source_count,
            flush_start.elapsed().as_micros()
        );
    }
    segment.clear();
    Ok(())
}
