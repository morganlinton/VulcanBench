use super::*;

pub(crate) fn macro_segment_cache_key(
    classified: &ClassifiedTokens,
    macros: &[MacroDef],
) -> MacroSegmentCacheKey {
    MacroSegmentCacheKey {
        source_len: classified.source.len(),
        source_hash: hash_bytes16(&classified.source),
        macro_hash: hash_macro_defs16(macros),
    }
}

pub(crate) fn hash_macro_defs16(macros: &[MacroDef]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(macros.len() as u64).to_le_bytes());
    for mac in macros {
        update_hash_bytes(&mut hasher, &mac.name);
        update_hash_bytes(&mut hasher, &mac.args);
        update_hash_bytes(&mut hasher, &mac.body);
        hasher.update(&[u8::from(mac.is_function_like)]);
    }
    finish_hash16(hasher)
}

pub(crate) fn hash_bytes16(bytes: &[u8]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    finish_hash16(hasher)
}

pub(crate) fn update_hash_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

pub(crate) fn finish_hash16(hasher: blake3::Hasher) -> [u8; 16] {
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}
