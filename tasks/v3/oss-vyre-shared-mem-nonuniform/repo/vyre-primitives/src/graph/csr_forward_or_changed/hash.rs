pub(crate) fn csr_forward_or_changed_padded_slice_fingerprint(
    values: &[u32],
    padded_words: usize,
) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in (padded_words as u64).to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for index in 0..padded_words {
        let value = values.get(index).copied().unwrap_or(0);
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}
