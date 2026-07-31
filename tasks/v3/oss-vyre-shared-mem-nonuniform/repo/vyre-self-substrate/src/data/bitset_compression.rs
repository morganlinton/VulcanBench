//! Bitset compression planning for CUDA-resident dataflow facts.

/// Runtime bitset representation selected for a dataflow fact set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitsetCompressionEncoding {
    /// Dense fixed-width words.
    DenseWords,
    /// Sorted active-bit indices.
    SparseIndices,
}

/// Input profile for one fact bitset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitsetCompressionProfile {
    /// Universe size in bits.
    pub universe_bits: u64,
    /// Active bits in the universe.
    pub active_bits: u64,
    /// Bytes per sparse active-bit index.
    pub index_bytes: u64,
    /// Maximum sparse density accepted in basis points.
    pub max_sparse_density_bps: u32,
}

/// Selected bitset compression plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitsetCompressionPlan {
    /// Selected runtime representation.
    pub encoding: BitsetCompressionEncoding,
    /// Dense byte count for the bitset.
    pub dense_bytes: u64,
    /// Sparse byte count for active indices.
    pub sparse_bytes: u64,
    /// Selected encoded byte count.
    pub encoded_bytes: u64,
    /// Bytes avoided compared with dense words.
    pub avoided_dense_bytes: u64,
    /// Active/universe density in basis points.
    pub density_bps: u32,
}

/// Bitset compression planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BitsetCompressionError {
    /// Active bit count exceeds the universe.
    ActiveExceedsUniverse {
        /// Universe size in bits.
        universe_bits: u64,
        /// Active bits.
        active_bits: u64,
    },
    /// Sparse index width must be explicit and non-zero.
    ZeroIndexBytes,
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
}

impl std::fmt::Display for BitsetCompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActiveExceedsUniverse {
                universe_bits,
                active_bits,
            } => write!(
                f,
                "bitset compression active_bits={active_bits} exceeds universe_bits={universe_bits}. Fix: compute dataflow fact cardinality before choosing a CUDA bitset representation."
            ),
            Self::ZeroIndexBytes => write!(
                f,
                "bitset compression received zero index_bytes. Fix: pass the concrete CUDA sparse-index ABI width."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "bitset compression overflowed while computing {field}. Fix: shard the fact universe before CUDA resident planning."
            ),
        }
    }
}

impl std::error::Error for BitsetCompressionError {}

/// Select a bitset representation from density and byte cost.
pub fn plan_bitset_compression(
    profile: BitsetCompressionProfile,
) -> Result<BitsetCompressionPlan, BitsetCompressionError> {
    if profile.index_bytes == 0 {
        return Err(BitsetCompressionError::ZeroIndexBytes);
    }
    if profile.active_bits > profile.universe_bits {
        return Err(BitsetCompressionError::ActiveExceedsUniverse {
            universe_bits: profile.universe_bits,
            active_bits: profile.active_bits,
        });
    }

    let word_count =
        profile
            .universe_bits
            .checked_add(63)
            .ok_or(BitsetCompressionError::ByteCountOverflow {
                field: "dense word count",
            })?
            / 64;
    let dense_bytes = checked_mul(word_count, 8, "dense bytes")?;
    let sparse_bytes = checked_mul(profile.active_bits, profile.index_bytes, "sparse bytes")?;
    let density_bps = if profile.universe_bits == 0 {
        0
    } else {
        ((profile.active_bits.saturating_mul(10_000)) / profile.universe_bits) as u32
    };

    let use_sparse = profile.active_bits == 0
        || (density_bps <= profile.max_sparse_density_bps && sparse_bytes < dense_bytes);
    let (encoding, encoded_bytes) = if use_sparse {
        (BitsetCompressionEncoding::SparseIndices, sparse_bytes)
    } else {
        (BitsetCompressionEncoding::DenseWords, dense_bytes)
    };

    Ok(BitsetCompressionPlan {
        encoding,
        dense_bytes,
        sparse_bytes,
        encoded_bytes,
        avoided_dense_bytes: dense_bytes.saturating_sub(encoded_bytes),
        density_bps,
    })
}

fn checked_mul(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, BitsetCompressionError> {
    lhs.checked_mul(rhs)
        .ok_or(BitsetCompressionError::ByteCountOverflow { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_density_bitsets_use_sparse_indices() {
        let plan = plan_bitset_compression(BitsetCompressionProfile {
            universe_bits: 8_192,
            active_bits: 64,
            index_bytes: 4,
            max_sparse_density_bps: 1_250,
        })
        .expect("Fix: low-density bitset should plan");

        assert_eq!(plan.encoding, BitsetCompressionEncoding::SparseIndices);
        assert_eq!(plan.dense_bytes, 1_024);
        assert_eq!(plan.sparse_bytes, 256);
        assert_eq!(plan.encoded_bytes, 256);
        assert_eq!(plan.avoided_dense_bytes, 768);
        assert_eq!(plan.density_bps, 78);
    }

    #[test]
    fn dense_or_expensive_sparse_bitsets_keep_dense_words() {
        let dense = plan_bitset_compression(BitsetCompressionProfile {
            universe_bits: 1_024,
            active_bits: 512,
            index_bytes: 4,
            max_sparse_density_bps: 1_250,
        })
        .expect("Fix: dense bitset should plan");
        assert_eq!(dense.encoding, BitsetCompressionEncoding::DenseWords);
        assert_eq!(dense.encoded_bytes, dense.dense_bytes);

        let expensive_sparse = plan_bitset_compression(BitsetCompressionProfile {
            universe_bits: 64,
            active_bits: 8,
            index_bytes: 32,
            max_sparse_density_bps: 2_000,
        })
        .expect("Fix: expensive sparse bitset should plan");
        assert_eq!(
            expensive_sparse.encoding,
            BitsetCompressionEncoding::DenseWords
        );
    }

    #[test]
    fn bitset_compression_rejects_invalid_profiles() {
        assert_eq!(
            plan_bitset_compression(BitsetCompressionProfile {
                universe_bits: 4,
                active_bits: 5,
                index_bytes: 4,
                max_sparse_density_bps: 1_250,
            })
            .expect_err("active above universe should fail"),
            BitsetCompressionError::ActiveExceedsUniverse {
                universe_bits: 4,
                active_bits: 5,
            }
        );
        assert_eq!(
            plan_bitset_compression(BitsetCompressionProfile {
                universe_bits: 4,
                active_bits: 1,
                index_bytes: 0,
                max_sparse_density_bps: 1_250,
            })
            .expect_err("zero sparse index width should fail"),
            BitsetCompressionError::ZeroIndexBytes
        );
    }
}
