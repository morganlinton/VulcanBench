//! Frozen monotonic-direction tags attached to monotonic algebraic laws.

/// Direction of monotonicity for the [`crate::AlgebraicLaw::Monotonic`] law.
///
/// Example: `MonotonicDirection::NonDecreasing` records that larger inputs
/// cannot produce smaller outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MonotonicDirection {
    /// `a <= b` implies `f(a) <= f(b)`.
    NonDecreasing,
    /// `a <= b` implies `f(a) >= f(b)`.
    NonIncreasing,
}
