//! Frozen verification-evidence records attached to declared algebraic laws.

use crate::float_type::FloatType;

/// Evidence attached to a declared law in the frozen data contract.
///
/// Example: `Verification::WitnessedU32 { seed: 7, count: 1024 }` records a
/// deterministic witness run for a full-width integer law.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Verification {
    /// Exhaustively verified over the `u8` input domain.
    ExhaustiveU8,
    /// Exhaustively verified over the `u16` input domain.
    ExhaustiveU16,
    /// Witnessed over `u32` using a deterministic seed and count.
    WitnessedU32 {
        /// Deterministic random seed used by the proof engine.
        seed: u64,
        /// Number of witnesses checked; coverage rejects zero.
        count: u64,
    },
    /// Exhaustively verified for a restricted floating-point type.
    ExhaustiveFloat {
        /// Floating-point format covered by the exhaustive pass.
        typ: FloatType,
    },
}

impl Verification {
    /// Return the witness count for witnessed variants, or `None` for
    /// exhaustive variants.
    #[must_use]
    pub fn witness_count(&self) -> Option<u64> {
        match self {
            Self::WitnessedU32 { count, .. } => Some(*count),
            _ => None,
        }
    }
}
