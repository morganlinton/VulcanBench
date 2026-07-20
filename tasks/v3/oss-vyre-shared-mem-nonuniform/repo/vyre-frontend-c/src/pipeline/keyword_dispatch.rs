use std::cell::RefCell;
use std::mem;

use super::*;

#[derive(Default)]
struct KeywordDispatchScratch {
    outputs: Vec<Vec<u8>>,
}

thread_local! {
    static KEYWORD_DISPATCH_SCRATCH: RefCell<KeywordDispatchScratch> =
        RefCell::new(KeywordDispatchScratch::default());
}

pub(in crate::pipeline) fn promote_c11_keywords(
    backend: &dyn VyreBackend,
    source: &str,
    dcfg: &mut DispatchConfig,
    expanded_haystack_cache: &mut Option<(Vec<u8>, u32)>,
    types: &mut Vec<u8>,
    starts: &[u8],
    lens: &[u8],
    counts: &[u8],
    n_tokens: u32,
    keyword_promoted: bool,
    cuda_keyword_haystack: Option<(&[u8], u32)>,
    dispatch_label: &str,
    mut log: impl FnMut(&str),
) -> Result<(), String> {
    KEYWORD_DISPATCH_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "c_keyword dispatch scratch was re-entered on the same thread. Fix: call keyword promotion from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        promote_c11_keywords_with_scratch(
            backend,
            source,
            dcfg,
            expanded_haystack_cache,
            types,
            starts,
            lens,
            counts,
            n_tokens,
            keyword_promoted,
            cuda_keyword_haystack,
            dispatch_label,
            &mut log,
            &mut scratch,
        )
    })
}

fn promote_c11_keywords_with_scratch(
    backend: &dyn VyreBackend,
    source: &str,
    dcfg: &mut DispatchConfig,
    expanded_haystack_cache: &mut Option<(Vec<u8>, u32)>,
    types: &mut Vec<u8>,
    starts: &[u8],
    lens: &[u8],
    counts: &[u8],
    n_tokens: u32,
    keyword_promoted: bool,
    cuda_keyword_haystack: Option<(&[u8], u32)>,
    dispatch_label: &str,
    mut log: impl FnMut(&str),
    scratch: &mut KeywordDispatchScratch,
) -> Result<(), String> {
    if keyword_promoted {
        log("skip c_keyword; lexer promoted keywords");
        return Ok(());
    }

    let keyword_map_bytes = keyword_map_bytes_cached();
    let (keyword_prog, keyword_haystack, keyword_stage) =
        if let Some((packed_haystack, packed_len)) = cuda_keyword_haystack {
            (
                c_keyword_packed_haystack(
                    "tok_types",
                    "tok_starts",
                    "tok_lens",
                    "counts",
                    "haystack",
                    "keyword_map",
                    n_tokens.max(1),
                    C_KEYWORDS.len() as u32,
                    packed_len.max(1),
                ),
                packed_haystack,
                "c_keyword_packed_haystack",
            )
        } else {
            let was_packed = expanded_haystack_cache.is_none();
            let (dense_haystack, dense_haystack_len) =
                expanded_haystack(expanded_haystack_cache, source)?;
            if was_packed {
                log("pack_haystack");
            }
            (
                c_keyword(
                    "tok_types",
                    "tok_starts",
                    "tok_lens",
                    "counts",
                    "haystack",
                    "keyword_map",
                    n_tokens.max(1),
                    C_KEYWORDS.len() as u32,
                    dense_haystack_len.max(1),
                ),
                dense_haystack,
                "c_keyword",
            )
        };
    validate_internal_stage(&keyword_prog, keyword_stage)?;
    dcfg.label = Some(dispatch_label.to_string());
    dispatch_borrowed_cached_into(
        backend,
        &keyword_prog,
        &[
            types,
            starts,
            lens,
            counts,
            keyword_haystack,
            keyword_map_bytes,
        ],
        dcfg,
        &mut scratch.outputs,
    )
    .map_err(|error| format!("{dispatch_label} c_keyword dispatch failed: {error}"))?;
    log("dispatch c_keyword");
    if scratch.outputs.len() != 1 {
        return Err(format!(
            "{dispatch_label}: expected exactly 1 keyword output buffer, got {}. Fix: backend must return promoted token types and no extras.",
            scratch.outputs.len()
        ));
    }
    preserve_unpromoted_token_types(types, &mut scratch.outputs[0], n_tokens)?;
    mem::swap(types, &mut scratch.outputs[0]);
    Ok(())
}

fn preserve_unpromoted_token_types(
    original: &[u8],
    promoted: &mut [u8],
    n_tokens: u32,
) -> Result<(), String> {
    let logical_bytes = usize::try_from(n_tokens)
        .ok()
        .and_then(|tokens| tokens.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            format!(
                "keyword promotion token count {n_tokens} overflows host byte indexing. Fix: shard token promotion."
            )
        })?;
    if original.len() < logical_bytes || promoted.len() < logical_bytes {
        return Err(format!(
            "keyword promotion buffers are too short for {n_tokens} token(s): original={} promoted={}. Fix: keep keyword promotion output aligned with lexer output.",
            original.len(),
            promoted.len()
        ));
    }
    for offset in (0..logical_bytes).step_by(std::mem::size_of::<u32>()) {
        let original_word = u32::from_le_bytes(
            original[offset..offset + 4]
                .try_into()
                .map_err(|_| "keyword promotion original word decode failed".to_string())?,
        );
        let promoted_word = u32::from_le_bytes(
            promoted[offset..offset + 4]
                .try_into()
                .map_err(|_| "keyword promotion promoted word decode failed".to_string())?,
        );
        if promoted_word == 0 && original_word != 0 {
            promoted[offset..offset + 4].copy_from_slice(&original[offset..offset + 4]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::preserve_unpromoted_token_types;

    #[test]
    fn preserve_unpromoted_token_types_restores_zeroed_non_keywords() {
        let original = pack_words(&[10, 20, 30]);
        let mut promoted = pack_words(&[10, 0, 99]);

        preserve_unpromoted_token_types(&original, &mut promoted, 3).unwrap();

        assert_eq!(promoted, pack_words(&[10, 20, 99]));
    }

    #[test]
    fn preserve_unpromoted_token_types_rejects_short_output() {
        let original = pack_words(&[10, 20]);
        let mut promoted = pack_words(&[10]);

        let error = preserve_unpromoted_token_types(&original, &mut promoted, 2)
            .expect_err("Fix: short keyword outputs must be rejected.");
        assert!(error.contains("too short"));
    }

    // Canonical LE-pack — no hand-rolled reimplementation (matches the production
    // path's `vyre_primitives::wire` alias and uses wire's bytemuck fast-path).
    use vyre_primitives::wire::pack_u32_slice as pack_words;
}
