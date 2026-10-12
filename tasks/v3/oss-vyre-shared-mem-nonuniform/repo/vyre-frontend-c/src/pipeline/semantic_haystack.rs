use super::*;

pub(in crate::pipeline) struct SemanticHaystack<'a> {
    pub(in crate::pipeline) bytes: &'a [u8],
    pub(in crate::pipeline) len: u32,
    pub(in crate::pipeline) packed: bool,
}

pub(in crate::pipeline) fn select_semantic_haystack<'a>(
    source: &str,
    expanded_haystack_cache: &'a mut Option<(Vec<u8>, u32)>,
    cuda_keyword_haystack: Option<(&'a [u8], u32)>,
    mut log: impl FnMut(&str),
) -> Result<SemanticHaystack<'a>, String> {
    if let Some((packed_haystack, packed_len)) = cuda_keyword_haystack {
        return Ok(SemanticHaystack {
            bytes: packed_haystack,
            len: packed_len,
            packed: true,
        });
    }

    let was_packed = expanded_haystack_cache.is_none();
    let (dense_haystack, dense_haystack_len) = expanded_haystack(expanded_haystack_cache, source)?;
    if was_packed {
        log("pack_haystack");
    }
    Ok(SemanticHaystack {
        bytes: dense_haystack,
        len: dense_haystack_len,
        packed: false,
    })
}
