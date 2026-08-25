use super::*;

pub(crate) fn record_missing_object_replacement_provenance(
    file_path: &std::path::Path,
    include_stack: &[std::path::PathBuf],
    replacement_tokens: &ClassifiedTokens,
    spelling_base: u32,
    output_base: usize,
    expansion_start: usize,
    consumed_end: usize,
    symbol_id: [u8; 16],
    macro_name: &[u8],
    dedupe_start: usize,
    token_provenance_events: &mut Vec<TokenProvenanceEvent>,
) -> Result<(), String> {
    let replacement_count = replacement_token_count(replacement_tokens);
    reserve_token_provenance_events(
        token_provenance_events,
        replacement_count,
        "object replacement provenance",
    )?;
    let single_token_len = if replacement_count == 1 {
        checked_usize_to_u32(
            replacement_tokens.source.len(),
            "single object backfill replacement length",
        )?
    } else {
        0
    };
    let mut recorded_replacement_spans =
        SpanDedupe::try_from_iter(token_provenance_events[dedupe_start..].iter().filter_map(
            |event| {
                if event.macro_name == macro_name
                    && event.expansion_start as usize == expansion_start
                {
                    Some((event.spelling_start, event.spelling_len))
                } else {
                    None
                }
            },
        ))?;
    for idx in 0..replacement_tokens.tok_types.len() {
        if replacement_tokens.tok_types[idx] == 0 {
            continue;
        }
        let start = token_start(replacement_tokens, idx)?;
        let len = if single_token_len == 0 {
            token_len(replacement_tokens, idx)?
        } else {
            single_token_len
        };
        if !recorded_replacement_spans.insert((spelling_base.saturating_add(start), len))? {
            continue;
        }
        token_provenance_events.push(TokenProvenanceEvent {
            file: file_path.to_path_buf(),
            output_start: checked_output_offset(
                output_base,
                checked_usize_to_u32(
                    expansion_start.saturating_add(start as usize),
                    "object replacement backfill output start",
                )?,
                "object replacement backfill output start",
            )?,
            output_len: len,
            spelling_file: file_path.to_path_buf(),
            spelling_start: spelling_base.saturating_add(start),
            spelling_len: len,
            expansion_file: file_path.to_path_buf(),
            expansion_start: checked_usize_to_u32(
                expansion_start,
                "object replacement expansion start",
            )?,
            expansion_len: checked_usize_to_u32(
                consumed_end.saturating_sub(expansion_start),
                "object replacement expansion length",
            )?,
            include_stack: include_stack.to_vec(),
            macro_symbol_id: Some(symbol_id),
            macro_name: macro_name.to_vec(),
            gpu_resident: true,
        });
    }
    Ok(())
}
