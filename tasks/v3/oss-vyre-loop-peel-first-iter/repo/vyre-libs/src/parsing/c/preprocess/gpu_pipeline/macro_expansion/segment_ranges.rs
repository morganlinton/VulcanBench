use super::flush::flush_active_macro_segment_inner;
use super::*;

pub(crate) fn flush_macro_segment_ranges(
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
    ranges: Vec<(usize, usize)>,
) -> Result<(), String> {
    let mut original = std::mem::take(segment);
    if ranges.len() == 1 {
        let (chunk_start, chunk_end) = ranges[0];
        if chunk_start == 0 && chunk_end == original.len() {
            return flush_active_macro_segment_inner(
                dispatcher,
                file_path,
                include_stack,
                parent_classified,
                segment_start,
                macros,
                macro_events,
                macro_expansion_cache,
                &mut original,
                output,
                macro_expansion_events,
                token_provenance_events,
                0,
            );
        }
    }

    let max_chunk_len = ranges
        .iter()
        .map(|(chunk_start, chunk_end)| chunk_end.saturating_sub(*chunk_start))
        .max()
        .unwrap_or(0);
    let mut chunk = macro_expansion_cache.take_range_chunk_scratch();
    if chunk.capacity() < max_chunk_len {
        let additional = max_chunk_len - chunk.capacity();
        chunk.try_reserve_exact(additional).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {additional} macro segment shard bytes: {error:?}. Fix: shard macro segment ranges before GPU macro expansion."
            )
        })?;
    }
    let result = (|| {
        for (chunk_start, chunk_end) in ranges {
            let chunk_bytes = original
                    .get(chunk_start..chunk_end)
                    .ok_or_else(|| {
                        "vyre-libs::gpu_pipeline: macro segment shard range outside source. Fix: repair shard range generation.".to_string()
                    })?;
            chunk.clear();
            chunk.extend_from_slice(chunk_bytes);
            flush_active_macro_segment_inner(
                dispatcher,
                file_path,
                include_stack,
                parent_classified,
                segment_start + chunk_start,
                macros,
                macro_events,
                macro_expansion_cache,
                &mut chunk,
                output,
                macro_expansion_events,
                token_provenance_events,
                0,
            )?;
        }
        Ok(())
    })();
    macro_expansion_cache.store_range_chunk_scratch(chunk);
    result
}
