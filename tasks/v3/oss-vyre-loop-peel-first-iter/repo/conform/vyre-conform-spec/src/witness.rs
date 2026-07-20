//! Witness set enumeration.

/// Canonical u32 witness set: boundary values + deterministic pseudo-random samples.
pub struct U32Witness;

impl U32Witness {
    /// Enumerate the canonical u32 witness set.
    pub fn enumerate() -> Vec<u32> {
        let mut out = vec![
            0u32,
            1,
            2,
            3,
            u32::MAX,
            u32::MAX - 1,
            0x8000_0000,
            0x7FFF_FFFF,
            0xAAAA_AAAA,
            0x5555_5555,
            0xDEAD_BEEF,
            0xCAFE_F00D,
        ];

        // 24 deterministic pseudo-random samples seeded from a fixed blake3 digest.
        let seed = *blake3::hash(b"u32-witness-v1").as_bytes();
        let mut state = u64::from_le_bytes([
            seed[0], seed[1], seed[2], seed[3], seed[4], seed[5], seed[6], seed[7],
        ]);
        for _ in 0..24 {
            // splitmix64
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            out.push((z as u32) ^ ((z >> 32) as u32));
        }
        out
    }

    /// Canonical blake3 fingerprint for this witness set (little-endian encoding).
    #[must_use]
    pub fn fingerprint_canonical() -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        for v in Self::enumerate() {
            hasher.update(&v.to_le_bytes());
        }
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_witness_is_deterministic() {
        let a = U32Witness::enumerate();
        let b = U32Witness::enumerate();
        assert_eq!(a, b, "witness set must be deterministic across calls");
    }

    #[test]
    fn u32_witness_fingerprint_stable() {
        let a = U32Witness::fingerprint_canonical();
        let b = U32Witness::fingerprint_canonical();
        assert_eq!(a, b, "fingerprint must be stable");
    }

    #[test]
    fn u32_witness_includes_boundaries() {
        let w = U32Witness::enumerate();
        assert!(w.contains(&0));
        assert!(w.contains(&u32::MAX));
        assert!(w.contains(&0x8000_0000));
    }
}
