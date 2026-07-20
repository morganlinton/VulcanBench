use super::*;

/// Records provenance for a segment after GPU macro materialization.
pub(crate) fn record_macro_token_provenance(
    dispatcher: &dyn GpuDispatcher,
    file_path: &std::path::Path,
    include_stack: &[std::path::PathBuf],
    macros: &[MacroDef],
    macro_events: &[MacroEvent],
    classified: &ClassifiedTokens,
    expanded: &ClassifiedTokens,
    output_base: usize,
    token_provenance_events: &mut Vec<TokenProvenanceEvent>,
) -> Result<(), String> {
    let segment_events_start = token_provenance_events.len();
    let mut macros_by_name: HashMap<&[u8], MacroBucket<'_>> = HashMap::default();
    macros_by_name.try_reserve(macros.len()).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {} macro provenance lookup buckets: {error:?}. Fix: shard preprocessing before macro provenance export.",
            macros.len()
        )
    })?;
    for mac in macros {
        macros_by_name
            .entry(mac.name.as_slice())
            .or_default()
            .push(mac);
    }
    let mut input_cursor = 0_usize;
    let mut output_cursor = 0_usize;
    let mut expanded_token_cursor = 0_usize;
    for (idx, token_kind) in classified.tok_types.iter().enumerate() {
        if *token_kind == 0 {
            continue;
        }
        let start = token_start(classified, idx)? as usize;
        let len = token_len(classified, idx)? as usize;
        if start < input_cursor {
            continue;
        }
        output_cursor = output_cursor.checked_add(start - input_cursor).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: provenance output cursor overflow. Fix: shard preprocessing before macro provenance export.".to_string()
        })?;
        let end = start.checked_add(len).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: provenance token range overflow. Fix: shard preprocessing before macro provenance export.".to_string()
        })?;
        let token = classified.source.get(start..end).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: provenance token range outside source. Fix: repair GPU lexer spans before provenance export.".to_string()
        })?;
        let Some(candidate_macros) = macros_by_name.get(token) else {
            input_cursor = end;
            output_cursor = output_cursor.checked_add(len).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: direct provenance output cursor overflow. Fix: shard preprocessing before provenance export.".to_string()
            })?;
            token_provenance_events.push(TokenProvenanceEvent {
                file: file_path.to_path_buf(),
                output_start: checked_output_offset(
                    output_base,
                    checked_usize_to_u32(
                        output_cursor.saturating_sub(len),
                        "direct expanded provenance output start",
                    )?,
                    "direct expanded provenance output start",
                )?,
                output_len: checked_usize_to_u32(len, "direct expanded provenance output length")?,
                spelling_file: file_path.to_path_buf(),
                spelling_start: checked_usize_to_u32(start, "direct provenance spelling start")?,
                spelling_len: checked_usize_to_u32(len, "direct provenance spelling length")?,
                expansion_file: file_path.to_path_buf(),
                expansion_start: checked_usize_to_u32(start, "direct provenance expansion start")?,
                expansion_len: checked_usize_to_u32(len, "direct provenance expansion length")?,
                include_stack: include_stack.to_vec(),
                macro_symbol_id: None,
                macro_name: Vec::new(),
                gpu_resident: true,
            });
            continue;
        };
        if let Some((mac, consumed_end)) =
            matched_macro_invocation(candidate_macros, &classified.source, end)
        {
            let symbol_id = stable_macro_symbol_id(&mac.name);
            let (spelling_file, spelling_start, spelling_len) =
                macro_spelling_origin(macro_events, symbol_id, file_path, start as u32, len as u32);
            let params = parse_param_names(&mac.args);
            let arg_spans = if mac.is_function_like {
                invocation_arg_spans(&classified.source, end).unwrap_or_default()
            } else {
                SmallVec::new()
            };
            let macro_output_end = macro_output_end(
                &classified.source,
                &expanded.source,
                consumed_end,
                output_cursor,
            );
            let replacement_tokens = cached_replacement_tokens(dispatcher, mac, symbol_id)?;
            let replacement_views = collect_replacement_token_views(&replacement_tokens);
            let macro_events_start = token_provenance_events.len();
            let mut macro_token_ordinal = 0_usize;
            while let Some(expanded_idx) = first_expanded_token_at_or_after(
                expanded,
                output_cursor,
                &mut expanded_token_cursor,
            )? {
                let expanded_start = token_start(expanded, expanded_idx)? as usize;
                if expanded_start >= macro_output_end {
                    break;
                }
                let expanded_len = token_len(expanded, expanded_idx)?;
                expanded_token_cursor = expanded_idx + 1;
                let replacement_view = replacement_views.get(macro_token_ordinal);
                let spelling_view = replacement_view.or_else(|| replacement_views.last());
                let (spelling_delta, replacement_len) = spelling_view
                    .map(|view| (view.spelling_start, view.spelling_len))
                    .unwrap_or((0, 0));
                let replacement_token = replacement_view.and_then(|view| {
                    let start = view.spelling_start as usize;
                    let len = view.spelling_len as usize;
                    start
                        .checked_add(len)
                        .and_then(|end| replacement_tokens.source.get(start..end))
                });
                let object_single_token_len = if !mac.is_function_like
                    && replacement_views.len() == 1
                {
                    checked_usize_to_u32(mac.body.len(), "single object replacement body length")?
                } else {
                    replacement_len
                };
                macro_token_ordinal += 1;
                let param_arg_span = replacement_token
                    .and_then(|token| param_argument_span(token, &params, &arg_spans));
                let token_spelling_start = if let Some((arg_start, _arg_len)) = param_arg_span {
                    checked_usize_to_u32(arg_start, "function macro argument spelling start")?
                } else {
                    spelling_start.checked_add(spelling_delta).ok_or_else(|| {
                    "vyre-libs::gpu_pipeline: macro replacement spelling start overflow. Fix: shard preprocessing before provenance export.".to_string()
                    })?
                };
                let token_spelling_len =
                    expanded_len.min(spelling_len.saturating_sub(spelling_delta));
                token_provenance_events.push(TokenProvenanceEvent {
                    file: file_path.to_path_buf(),
                    output_start: checked_output_offset(
                        output_base,
                        checked_usize_to_u32(
                            expanded_start,
                            "macro replacement expanded token start",
                        )?,
                        "macro replacement provenance output start",
                    )?,
                    output_len: if mac.is_function_like {
                        expanded_len
                    } else {
                        object_single_token_len
                    },
                    spelling_file: spelling_file.clone(),
                    spelling_start: token_spelling_start,
                    spelling_len: if mac.is_function_like {
                        if let Some((_arg_start, arg_len)) = param_arg_span {
                            checked_usize_to_u32(
                                arg_len,
                                "function macro argument spelling length",
                            )?
                        } else {
                            token_spelling_len
                        }
                    } else {
                        object_single_token_len
                    },
                    expansion_file: file_path.to_path_buf(),
                    expansion_start: checked_usize_to_u32(
                        start,
                        "macro provenance expansion start",
                    )?,
                    expansion_len: checked_usize_to_u32(
                        consumed_end.saturating_sub(start),
                        "macro provenance expansion length",
                    )?,
                    include_stack: include_stack.to_vec(),
                    macro_symbol_id: Some(symbol_id),
                    macro_name: mac.name.clone(),
                    gpu_resident: true,
                });
            }
            if mac.is_function_like {
                record_missing_parameter_substitution_provenance(
                    file_path,
                    include_stack,
                    &replacement_tokens,
                    &params,
                    &arg_spans,
                    output_base,
                    output_cursor,
                    start,
                    consumed_end,
                    symbol_id,
                    &mac.name,
                    macro_events_start,
                    token_provenance_events,
                )?;
            }
            output_cursor = macro_output_end;
            let _ = mac;
            input_cursor = consumed_end;
        } else {
            let (expanded_start, expanded_len) = if let Some(found) =
                find_expanded_token_at_or_after_from(
                    expanded,
                    &classified.source[start..end],
                    output_cursor,
                    &mut expanded_token_cursor,
                )? {
                found
            } else {
                (
                    checked_usize_to_u32(output_cursor, "direct fallback provenance output start")?,
                    checked_usize_to_u32(len, "direct fallback provenance output length")?,
                )
            };
            token_provenance_events.push(TokenProvenanceEvent {
                file: file_path.to_path_buf(),
                output_start: checked_output_offset(
                    output_base,
                    expanded_start,
                    "direct expanded provenance output start",
                )?,
                output_len: expanded_len,
                spelling_file: file_path.to_path_buf(),
                spelling_start: checked_usize_to_u32(start, "direct provenance spelling start")?,
                spelling_len: checked_usize_to_u32(len, "direct provenance spelling length")?,
                expansion_file: file_path.to_path_buf(),
                expansion_start: checked_usize_to_u32(start, "direct provenance expansion start")?,
                expansion_len: checked_usize_to_u32(len, "direct provenance expansion length")?,
                include_stack: include_stack.to_vec(),
                macro_symbol_id: None,
                macro_name: Vec::new(),
                gpu_resident: true,
            });
            output_cursor = (expanded_start as usize).checked_add(expanded_len as usize).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro provenance output cursor overflow. Fix: shard preprocessing before provenance export.".to_string()
            })?;
            input_cursor = end;
        }
    }
    record_missing_invocation_provenance(
        dispatcher,
        file_path,
        include_stack,
        &macros_by_name,
        macro_events,
        classified,
        output_base,
        segment_events_start,
        token_provenance_events,
    )?;
    Ok(())
}
