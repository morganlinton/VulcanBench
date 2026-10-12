pub(super) fn checked_count_u64(count: usize, label: &str) -> Result<u64, String> {
    u64::try_from(count).map_err(|_| {
        format!(
            "vyre-frontend-c {label} exceeds u64. Fix: shard the object before decoding summary metadata."
        )
    })
}

pub(super) fn decode_u32_words(bytes: &[u8]) -> Result<Vec<u32>, String> {
    crate::api::word_decode::decode_u32_words_for_section(bytes, "object section payload")
}

pub(super) fn decode_u32_words_for_section(
    bytes: &[u8],
    section_name: &str,
) -> Result<Vec<u32>, String> {
    crate::api::word_decode::decode_u32_words_for_section(bytes, section_name)
}
