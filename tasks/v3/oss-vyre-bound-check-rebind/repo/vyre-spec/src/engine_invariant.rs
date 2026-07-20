//! Frozen engine-invariant identifiers used by conformance certificates.

use core::fmt;

/// Stable engine invariant identifier in the frozen data contract.
///
/// The numeric value matches the `I{N}` naming in the vyre specification.
/// These numbers are permanent: new invariants add new variants, while
/// existing variants never change meaning. Example: `EngineInvariant::I4`
/// identifies the IR wire-format round-trip invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum EngineInvariant {
    /// I1 Determinism.
    I1 = 1,
    /// I2 Composition commutativity with lowering.
    I2 = 2,
    /// I3 Backend equivalence.
    I3 = 3,
    /// I4 IR wire-format round-trip.
    I4 = 4,
    /// I5 Validation soundness.
    I5 = 5,
    /// I6 Validation completeness, partial.
    I6 = 6,
    /// I7 Law monotonicity under composition.
    I7 = 7,
    /// I8 Reference agreement.
    I8 = 8,
    /// I9 Law falsifiability.
    I9 = 9,
    /// I10 Bounded allocation.
    I10 = 10,
    /// I11 No panic.
    I11 = 11,
    /// I12 No undefined behaviour.
    I12 = 12,
    /// I13 Userspace stability.
    I13 = 13,
    /// I14 Non-exhaustive discipline.
    I14 = 14,
    /// I15 Certificate stability.
    I15 = 15,
}

/// Public alias for stable invariant identifiers.
pub type InvariantId = EngineInvariant;

impl fmt::Display for EngineInvariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "I{}", self.ordinal())
    }
}

impl EngineInvariant {
    /// Stable one-based invariant number.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        match self {
            Self::I1 => 1,
            Self::I2 => 2,
            Self::I3 => 3,
            Self::I4 => 4,
            Self::I5 => 5,
            Self::I6 => 6,
            Self::I7 => 7,
            Self::I8 => 8,
            Self::I9 => 9,
            Self::I10 => 10,
            Self::I11 => 11,
            Self::I12 => 12,
            Self::I13 => 13,
            Self::I14 => 14,
            Self::I15 => 15,
        }
    }

    /// Iterate over every known invariant id in declaration order.
    pub fn iter() -> impl Iterator<Item = Self> {
        [
            Self::I1,
            Self::I2,
            Self::I3,
            Self::I4,
            Self::I5,
            Self::I6,
            Self::I7,
            Self::I8,
            Self::I9,
            Self::I10,
            Self::I11,
            Self::I12,
            Self::I13,
            Self::I14,
            Self::I15,
        ]
        .into_iter()
    }
}
