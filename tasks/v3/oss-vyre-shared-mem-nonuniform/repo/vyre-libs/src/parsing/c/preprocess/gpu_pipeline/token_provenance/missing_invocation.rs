use super::*;

pub(crate) fn record_missing_invocation_provenance(
    dispatcher: &dyn GpuDispatcher,
    file_path: &std::path::Path,
    include_stack: &[std::path::PathBuf],
    macros_by_name: &HashMap<&[u8], MacroBucket<'_>>,
    macro_events: &[MacroEvent],
    classified: &ClassifiedTokens,
    output_base: usize,
    dedupe_start: usize,
    token_provenance_events: &mut Vec<TokenProvenanceEvent>,
) -> Result<(), String> {
    for (idx, token_kind) in classified.tok_types.iter().enumerate() {
        if *token_kind == 0 {
            continue;
        }
        let start = token_start(classified, idx)? as usize;
        let len = token_len(classified, idx)? as usize;
        let end = start.checked_add(len).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: function invocation provenance token range overflow. Fix: shard preprocessing before provenance export.".to_string()
        })?;
        let Some(token) = classified.source.get(start..end) else {
            continue;
        };
        let Some(candidate_macros) = macros_by_name.get(token) else {
            continue;
        };
        for mac in candidate_macros {
            let consumed_end = if mac.is_function_like {
                let Some(invocation_end) = function_like_invocation_end(&classified.source, end)
                else {
                    continue;
                };
                invocation_end
            } else {
                end
            };
            let params = parse_param_names(&mac.args);
            let arg_spans = if mac.is_function_like {
                invocation_arg_spans(&classified.source, end).unwrap_or_default()
            } else {
                SmallVec::new()
            };
            let symbol_id = stable_macro_symbol_id(&mac.name);
            let replacement_tokens = cached_replacement_tokens(dispatcher, mac, symbol_id)?;
            let (_spelling_file, spelling_start, _spelling_len) =
                macro_spelling_origin(macro_events, symbol_id, file_path, start as u32, len as u32);
            record_missing_object_replacement_provenance(
                file_path,
                include_stack,
                &replacement_tokens,
                spelling_start,
                output_base,
                start,
                consumed_end,
                symbol_id,
                &mac.name,
                dedupe_start,
                token_provenance_events,
            )?;
            if !mac.is_function_like {
                continue;
            }
            let mut recorded_arg_spans = SpanDedupe::try_from_iter(
                token_provenance_events[dedupe_start..]
                    .iter()
                    .filter_map(|event| {
                        if event.macro_name == mac.name && event.expansion_start as usize == start {
                            Some((event.spelling_start as usize, event.spelling_len as usize))
                        } else {
                            None
                        }
                    }),
            )?;
            record_missing_parameter_substitution_provenance(
                file_path,
                include_stack,
                &replacement_tokens,
                &params,
                &arg_spans,
                output_base,
                start,
                start,
                consumed_end,
                symbol_id,
                &mac.name,
                0,
                token_provenance_events,
            )?;
            reserve_token_provenance_events(
                token_provenance_events,
                arg_spans.len(),
                "function invocation argument provenance",
            )?;
            for (arg_start, arg_len) in arg_spans {
                if !recorded_arg_spans.insert((arg_start, arg_len))? {
                    continue;
                }
                token_provenance_events.push(TokenProvenanceEvent {
                    file: file_path.to_path_buf(),
                    output_start: checked_output_offset(
                        output_base,
                        checked_usize_to_u32(start, "function invocation argument output start")?,
                        "function invocation argument output start",
                    )?,
                    output_len: checked_usize_to_u32(
                        arg_len,
                        "function invocation argument output length",
                    )?,
                    spelling_file: file_path.to_path_buf(),
                    spelling_start: checked_usize_to_u32(
                        arg_start,
                        "function invocation argument spelling start",
                    )?,
                    spelling_len: checked_usize_to_u32(
                        arg_len,
                        "function invocation argument spelling length",
                    )?,
                    expansion_file: file_path.to_path_buf(),
                    expansion_start: checked_usize_to_u32(
                        start,
                        "function invocation argument expansion start",
                    )?,
                    expansion_len: checked_usize_to_u32(
                        consumed_end.saturating_sub(start),
                        "function invocation argument expansion length",
                    )?,
                    include_stack: include_stack.to_vec(),
                    macro_symbol_id: Some(symbol_id),
                    macro_name: mac.name.clone(),
                    gpu_resident: true,
                });
            }
        }
    }
    Ok(())
}
