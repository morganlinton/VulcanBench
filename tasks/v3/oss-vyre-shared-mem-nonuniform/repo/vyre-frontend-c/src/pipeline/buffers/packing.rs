pub(crate) fn fast_pack_u32_le(words: &[u32]) -> Vec<u8> {
    // Canonical LEGO: vyre-primitives::wire owns the LE host bytemuck
    // fast path. Local duplicate removed 2026-05-23 (Cargo grep confirmed
    // wire primitive is on the same dep edge).
    vyre_primitives::wire::pack_u32_slice(words)
}

pub(crate) fn read_u32_at(buf: &[u8], off: usize) -> Result<u32, String> {
    // Canonical LEGO: byte-offset LE u32 read lives once in vyre-primitives::wire
    // (same checked bounds + diagnostics). Local duplicate removed.
    vyre_primitives::wire::read_u32_le_at(buf, off, "parser buffer")
}

pub(crate) fn pack_haystack(source: &str) -> Result<(Vec<u8>, u32), String> {
    // Canonical LEGO: vyre-primitives::wire::pack_bytes_as_u32_slice_min_words
    // owns the lane-per-byte u32 layout (byte at lane[0], lanes[1..3] = 0)
    // padded to at least 1 word. The frontend just translates the
    // word-count to the u32 GPU index space.
    let _ = u32::try_from(source.len()).map_err(|_| {
        format!(
            "C frontend source length {} exceeds the u32 GPU index space. Fix: shard the translation unit before packing the haystack.",
            source.len()
        )
    })?;
    let (bytes, words) =
        vyre_primitives::wire::pack_bytes_as_u32_slice_min_words(source.as_bytes(), 1)?;
    let count = u32::try_from(words).map_err(|_| {
        format!(
            "C frontend haystack word count {words} exceeds the u32 GPU index space. Fix: shard the translation unit before packing."
        )
    })?;
    Ok((bytes, count))
}

pub(crate) fn cuda_lexer_haystack_view(source: &[u8]) -> Result<(Vec<u8>, u32), String> {
    let logical_len = u32::try_from(source.len()).map_err(|_| {
        format!(
            "CUDA lexer source length {} exceeds the current u32 GPU index space. Fix: shard the translation unit before CUDA sparse lexing.",
            source.len()
        )
    })?;
    let packed_words = logical_len.max(1).div_ceil(4).max(1) as usize;
    let packed_bytes = packed_words.checked_mul(4).ok_or_else(|| {
        format!(
            "CUDA lexer packed word count {packed_words} overflows byte length. Fix: shard the translation unit before CUDA sparse lexing."
        )
    })?;
    let mut packed = vec![0u8; packed_bytes];
    packed[..source.len()].copy_from_slice(source);
    Ok((packed, logical_len))
}
pub(crate) fn read_u32_stream(buf: &[u8], words: usize, label: &str) -> Result<Vec<u32>, String> {
    // Canonical LEGO: vyre-primitives::wire::unpack_u32_slice_into owns
    // the LE host fast path (one `bytemuck::cast_slice_mut` copy on LE).
    let mut out = Vec::with_capacity(words);
    vyre_primitives::wire::unpack_u32_slice_into(buf, words, label, &mut out)?;
    Ok(out)
}

pub(crate) fn vec_u32_le_bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

pub(crate) fn vec_u32_le_bytes_min_words(words: &[u32], min_words: u32) -> Result<Vec<u8>, String> {
    // Canonical LEGO: vyre-primitives::wire::pack_u32_slice_min_words_into
    // owns the padded-pack fast path. Wrapper allocates the owned Vec
    // because every existing caller takes `Vec<u8>` by value; the `_into`
    // variant is still the right choice when the caller can reuse storage.
    let mut out = Vec::new();
    vyre_primitives::wire::pack_u32_slice_min_words_into(words, min_words, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{fast_pack_u32_le, vec_u32_le_bytes_min_words};

    #[test]
    fn fast_pack_u32_le_preserves_little_endian_wire_order() {
        assert_eq!(
            fast_pack_u32_le(&[0x0102_0304, 0xa0b0_c0d0]),
            vec![0x04, 0x03, 0x02, 0x01, 0xd0, 0xc0, 0xb0, 0xa0]
        );
    }

    #[test]
    fn vec_u32_le_bytes_min_words_pads_without_reordering_words() {
        assert_eq!(
            vec_u32_le_bytes_min_words(&[0x1122_3344], 3).unwrap(),
            vec![0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}
