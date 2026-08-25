pub(super) fn checked_gpu_u32(label: &str, value: usize) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| {
        format!(
            "vyre-libs::gpu_pipeline: {label} {value} exceeds the current u32 GPU index space. Fix: shard the translation unit before preprocessing."
        )
    })
}

// =================================================================
// Phase 18b: gpu_tokenize_and_classify
// =================================================================

pub(super) fn unpack_u32_words_prefix(bytes: &[u8], count: usize) -> Result<Vec<u32>, String> {
    // The wire primitive errors when bytes < count*4; the prefix variant
    // is permissive by contract (truncates), so clamp count down before
    // delegating.
    let available = bytes.len() / 4;
    let take = count.min(available);
    let mut out = Vec::new();
    out.try_reserve_exact(take).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {take} prefix u32 decode words: {error:?}. Fix: shard GPU preprocess decode before collecting prefix rows."
        )
    })?;
    vyre_primitives::wire::unpack_u32_slice_into(bytes, take, "gpu_pipeline.prefix", &mut out)
        .map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: clamped prefix u32 unpack failed: {error}. Fix: repair GPU output table encoding before prefix decode."
            )
        })?;
    Ok(out)
}

pub(super) fn unpack_u32_words_exact_into(
    bytes: &[u8],
    count: usize,
    label: &str,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let expected = count
        .checked_mul(4)
        .ok_or_else(|| format!("{label}: expected byte count overflows usize"))?;
    if bytes.len() != expected {
        return Err(format!(
            "{label}: malformed u32 table: expected exactly {expected} bytes for {count} rows, got {}. Fix: backend must emit the declared table shape and no trailing bytes.",
            bytes.len()
        ));
    }
    // Trailing-byte rejection above guarantees `bytes.len() == count*4`,
    // so the wire primitive's LE bytemuck fast path applies cleanly.
    vyre_primitives::wire::unpack_u32_slice_into(bytes, count, label, out)
}

pub(super) fn unpack_u32_words_prefix_exact(
    bytes: &[u8],
    prefix_count: usize,
    table_words: usize,
    label: &str,
) -> Result<Vec<u32>, String> {
    let expected = table_words
        .checked_mul(4)
        .ok_or_else(|| format!("{label}: expected byte count overflows usize"))?;
    if bytes.len() != expected {
        return Err(format!(
            "{label}: malformed u32 table: expected exactly {expected} bytes for {table_words} rows, got {}. Fix: backend must emit the declared table shape and no trailing bytes.",
            bytes.len()
        ));
    }
    unpack_u32_words_prefix(bytes, prefix_count)
}

pub(super) fn u32_word_byte_len(word_count: usize, label: &str) -> Result<usize, String> {
    word_count.checked_mul(4).ok_or_else(|| {
        format!(
            "vyre-libs::gpu_pipeline: {label} word count {word_count} overflows host byte sizing. Fix: shard preprocessing before GPU staging."
        )
    })
}

pub(super) fn reserve_gpu_staging_bytes(
    out: &mut Vec<u8>,
    additional: usize,
    label: &str,
) -> Result<(), String> {
    out.try_reserve_exact(additional).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {additional} {label} bytes: {error:?}. Fix: shard preprocessing before GPU staging."
        )
    })
}

pub(super) fn pack_u32_words(words: &[u32], pad_len: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    pack_u32_words_into(&mut out, words, pad_len)?;
    Ok(out)
}

pub(super) fn pack_u32_words_into(
    out: &mut Vec<u8>,
    words: &[u32],
    pad_len: usize,
) -> Result<(), String> {
    // Route through the canonical `vyre-primitives::wire` LEGO primitive:
    // it does the same clear/resize/copy_from_slice with one allocation,
    // an endian-aware `bytemuck::cast_slice` fast path on LE hosts, and
    // surfaces a structured `Result` for overflow/sizing errors. The pad
    // contract here is "exact `pad_len` words long with input as prefix",
    // which is exactly what `pack_u32_slice_min_words_into` provides via
    // `min_words = max(pad_len, words.len())`. Local code panicked
    // through `resize` when `words.len() > pad_len`, so we preserve that
    // contract by clamping `min_words` to at least `words.len()` so the
    // canonical primitive's overflow check is the new behavior.
    let min_words = pad_len.max(words.len());
    if let Ok(min_words_u32) = u32::try_from(min_words) {
        if vyre_primitives::wire::pack_u32_slice_min_words_into(words, min_words_u32, out).is_ok() {
            return Ok(());
        }
    }

    out.clear();
    let byte_len = u32_word_byte_len(min_words, "packed u32 table")?;
    reserve_gpu_staging_bytes(out, byte_len, "packed u32 table")?;
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.resize(byte_len, 0);
    Ok(())
}

/// Cache the runtime-sized `gpu_ifdef_value(1, 0)` Program so the
/// live-conditional re-eval path does not reconstruct the IR per `#if` row.
pub fn bucket_pow2(value: usize, min: usize) -> usize {
    value.max(min).next_power_of_two()
}

pub(super) fn read_u32_word(buf: &[u8], word_index: usize, label: &str) -> Result<u32, String> {
    vyre_primitives::wire::read_u32_le_word(buf, word_index, label)
}

pub(super) fn read_u32_scalar_exact(buf: &[u8], label: &str) -> Result<u32, String> {
    if buf.len() != 4 {
        return Err(format!(
            "vyre-libs::gpu_pipeline: {label} has malformed byte length: expected exactly 4 bytes, got {}. Fix: backend must emit one u32 scalar and no trailing bytes.",
            buf.len()
        ));
    }
    read_u32_word(buf, 0, label)
}

#[cfg(test)]
mod generated_buffer_codec_tests {
    use super::*;

    fn generated_words(seed: u32, count: usize) -> Vec<u32> {
        let mut state = seed ^ 0x9e37_79b9;
        let mut out = Vec::new();
        out.try_reserve_exact(count)
            .expect("Fix: test fixture must pre-size buffers; increase reserve or shrink generated corpus - generated test word storage must reserve");
        for index in 0..count {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            out.push(state.wrapping_add(index as u32));
        }
        out
    }

    #[test]
    fn generated_u32_pack_prefix_decode_round_trips_4096_shapes() {
        for seed in 0..4096u32 {
            let word_count = (seed as usize) & 31;
            let pad_len = ((seed as usize >> 5) & 31).saturating_sub(seed as usize & 3);
            let words = generated_words(seed, word_count);
            let packed =
                pack_u32_words(&words, pad_len).expect("Fix: packing must not fail on in-bounds generated tables; shrink inputs or grow buffer - generated u32 table packing must not fail");
            let table_words = pad_len.max(word_count);
            assert_eq!(
                packed.len(),
                table_words * 4,
                "seed {seed}: packed byte length must match padded word count"
            );

            let decoded = unpack_u32_words_prefix_exact(
                &packed,
                word_count,
                table_words,
                "generated prefix decode",
            )
            .expect("Fix: reject malformed packed tables in tests; regenerate fixture on mismatch - generated packed table must decode");
            assert_eq!(decoded, words, "seed {seed}: prefix decode must round-trip");

            let over_requested = unpack_u32_words_prefix(&packed, table_words + 7)
                .expect("Fix: prefix decode must clamp to available words; reject over-large prefix requests - over-requested prefix decode must clamp to available words");
            assert_eq!(
                over_requested.len(),
                table_words,
                "seed {seed}: prefix decode must clamp to available complete u32 words"
            );
        }
    }

    #[test]
    fn generated_exact_prefix_decode_rejects_malformed_table_lengths() {
        for seed in 0..1024u32 {
            let word_count = 1 + ((seed as usize) & 15);
            let words = generated_words(seed ^ 0xa5a5_5a5a, word_count);
            let packed = pack_u32_words(&words, word_count)
                .expect("Fix: exact packing requires aligned word counts; reject odd-length hostile inputs - generated exact table packing must not fail");

            let short = &packed[..packed.len() - 1];
            let short_err =
                unpack_u32_words_prefix_exact(short, word_count, word_count, "short table")
                    .expect_err("seed {seed}: short exact table must be rejected");
            assert!(
                short_err.contains("malformed u32 table"),
                "short table error: {short_err}"
            );

            let mut trailing = packed;
            trailing.push(0);
            let trail_err =
                unpack_u32_words_prefix_exact(&trailing, word_count, word_count, "trailing table")
                    .expect_err("seed {seed}: trailing byte must be rejected");
            assert!(
                trail_err.contains("malformed u32 table"),
                "trailing table error: {trail_err}"
            );
        }
    }
}
