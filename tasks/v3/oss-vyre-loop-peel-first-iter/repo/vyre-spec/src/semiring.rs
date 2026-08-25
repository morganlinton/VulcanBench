//! Canonical semiring selector shared by optimizer, lowered rewrites, and primitives.

/// Closed semiring choice used by semiring-GEMM-style dataflow and algebraic kernels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Semiring {
    /// `(x, +)` standard linear algebra over encoded u32 lanes.
    Real,
    /// `(+, min)` shortest-path / min-plus tropical semiring.
    MinPlus,
    /// `(+, max)` longest-path / max-plus tropical semiring.
    MaxPlus,
    /// `(x, max)` Viterbi-style max-product semiring.
    MaxTimes,
    /// `(AND, OR)` boolean reachability over u32 lanes.
    BoolOr,
    /// `(OR, AND)` boolean covering / all-paths over u32 lanes.
    BoolAnd,
    /// `(AND, XOR)` GF(2) linear algebra over u32 lanes.
    Gf2,
    /// `(OR, OR)` lineage / fact-provenance semiring.
    Lineage,
}

impl Semiring {
    /// Additive identity used to initialize accumulators.
    #[must_use]
    pub const fn identity(self) -> u32 {
        match self {
            Self::Real
            | Self::MaxPlus
            | Self::MaxTimes
            | Self::BoolOr
            | Self::Gf2
            | Self::Lineage => 0,
            Self::MinPlus | Self::BoolAnd => u32::MAX,
        }
    }
}
