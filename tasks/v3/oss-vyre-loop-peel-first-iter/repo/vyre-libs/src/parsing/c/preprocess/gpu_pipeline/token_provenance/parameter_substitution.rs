use super::*;

pub(crate) fn record_missing_parameter_substitution_provenance(
    file_path: &std::path::Path,
    include_stack: &[std::path::PathBuf],
    replacement_tokens: &ClassifiedTokens,
    params: &[&[u8]],
    arg_spans: &[(usize, usize)],
    output_base: usize,
    macro_output_cursor: usize,
    expansion_start: usize,
    consumed_end: usize,
    symbol_id: [u8; 16],
    macro_name: &[u8],
    macro_events_start: usize,
    token_provenance_events: &mut Vec<TokenProvenanceEvent>,
) -> Result<(), String> {
    reserve_token_provenance_events(
        token_provenance_events,
        arg_spans.len(),
        "function parameter substitution provenance",
    )?;
    let mut recorded_arg_spans = SpanDedupe::try_from_iter(
        token_provenance_events[macro_events_start..]
            .iter()
            .map(|event| (event.spelling_start as usize, event.spelling_len as usize)),
    )?;
    for idx in 0..replacement_tokens.tok_types.len() {
        if replacement_tokens.tok_types[idx] == 0 {
            continue;
        }
        let start = token_start(replacement_tokens, idx)?;
        let len = token_len(replacement_tokens, idx)?;
        let token_start_usize = start as usize;
        let token_end = token_start_usize.checked_add(len as usize).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: function parameter provenance token range overflow. Fix: shard preprocessing before provenance export.".to_string()
        })?;
        let Some(token) = replacement_tokens.source.get(token_start_usize..token_end) else {
            continue;
        };
        let Some((arg_start, arg_len)) = param_argument_span(token, params, arg_spans) else {
            continue;
        };
        if !recorded_arg_spans.insert((arg_start, arg_len))? {
            continue;
        }
        token_provenance_events.push(TokenProvenanceEvent {
            file: file_path.to_path_buf(),
            output_start: checked_output_offset(
                output_base,
                checked_usize_to_u32(
                    macro_output_cursor.saturating_add(start as usize),
                    "function parameter provenance output start",
                )?,
                "function parameter provenance output start",
            )?,
            output_len: len,
            spelling_file: file_path.to_path_buf(),
            spelling_start: checked_usize_to_u32(arg_start, "function parameter spelling start")?,
            spelling_len: checked_usize_to_u32(arg_len, "function parameter spelling length")?,
            expansion_file: file_path.to_path_buf(),
            expansion_start: checked_usize_to_u32(
                expansion_start,
                "function parameter expansion start",
            )?,
            expansion_len: checked_usize_to_u32(
                consumed_end.saturating_sub(expansion_start),
                "function parameter expansion length",
            )?,
            include_stack: include_stack.to_vec(),
            macro_symbol_id: Some(symbol_id),
            macro_name: macro_name.to_vec(),
            gpu_resident: true,
        });
    }
    Ok(())
}
