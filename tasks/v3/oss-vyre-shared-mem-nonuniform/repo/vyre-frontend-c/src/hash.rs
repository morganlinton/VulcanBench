//! Deterministic crate-local hashing for cache keys and artifact names.
//!
//! Do not use `DefaultHasher` for persistent parser artifacts or cache keys:
//! its algorithm is not a frontend contract. Adversarial-input cache keys use
//! BLAKE3-128 via [`blake3_128`].

pub(crate) type StableHash128 = [u8; 16];

pub(crate) fn blake3_128(bytes: &[u8]) -> StableHash128 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    blake3_128_from_hasher(&hasher)
}

pub(crate) fn blake3_128_from_hasher(hasher: &blake3::Hasher) -> StableHash128 {
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

pub(crate) fn blake3_128_update_len_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}
