use super::*;
pub(super) fn truncate_lexer_outputs_to_logical_tokens(
    types: &mut Vec<u8>,
    starts: &mut Vec<u8>,
    lens: &mut Vec<u8>,
    n_tokens: u32,
) -> Result<(), String> {
    let logical_bytes = usize::try_from(n_tokens.max(1))
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "lexer logical token byte length overflows host indexing for n_tokens={n_tokens}. Fix: shard lexer output before token truncation."
            )
        })?;
    if n_tokens == 0 {
        types.resize(logical_bytes, 0);
        starts.resize(logical_bytes, 0);
        lens.resize(logical_bytes, 0);
        return Ok(());
    }
    if types.len() < logical_bytes || starts.len() < logical_bytes || lens.len() < logical_bytes {
        return Err(format!(
            "lexer logical token buffers truncated: need {logical_bytes} bytes, have type/start/len {}/{}/{}",
            types.len(),
            starts.len(),
            lens.len()
        ));
    }
    types.truncate(logical_bytes);
    starts.truncate(logical_bytes);
    lens.truncate(logical_bytes);
    Ok(())
}
pub(super) fn expanded_haystack<'a>(
    slot: &'a mut Option<(Vec<u8>, u32)>,
    source: &str,
) -> Result<(&'a [u8], u32), String> {
    if slot.is_none() {
        *slot = Some(pack_haystack(source)?);
    }
    let Some((bytes, len)) = slot.as_ref() else {
        return Err(
            "expanded haystack cache was not initialized. Fix: report this internal frontend state bug."
                .to_string(),
        );
    };
    Ok((bytes.as_slice(), *len))
}

pub(super) fn bucketed_dense_lex_haystack<'a>(
    haystack: &'a [u8],
    haystack_len: u32,
    padded: &'a mut Vec<u8>,
) -> (&'a [u8], u32) {
    // PERF: bucket the construction-time haystack length to a power-of-two
    // range so dense lexer kernels hit the dispatcher's pipeline cache across
    // files. Padding is sound because the C lexer treats zero bytes as
    // non-emitting whitespace, so padded lanes do not create logical tokens.
    let bucket = vyre_libs::parsing::c::preprocess::gpu_pipeline::bucket_pow2(
        haystack_len.max(1) as usize,
        4096,
    ) as u32;
    let bucket_bytes = (bucket as usize).saturating_mul(4);
    if haystack.len() >= bucket_bytes {
        return (haystack, bucket);
    }
    padded.clear();
    padded.reserve(bucket_bytes.saturating_sub(padded.capacity()));
    padded.extend_from_slice(haystack);
    padded.resize(bucket_bytes, 0);
    (padded.as_slice(), bucket)
}

pub(super) fn keyword_map_bytes_cached() -> &'static [u8] {
    static KEYWORD_MAP_BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    KEYWORD_MAP_BYTES
        .get_or_init(|| vec_u32_le_bytes(&c_keyword_map_words()))
        .as_slice()
}
