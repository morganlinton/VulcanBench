//! IR mutation classification.
//!
//! Every optimizer pass declares a `MutationClass`  -  a frozen tag that says
//! *what kind of change this pass is allowed to make*. The conformance
//! harness uses the class to decide how strictly the result must match the
//! reference interpreter:
//!
//! - `Cosmetic`: re-names a local, collapses aliases. Output must match the
//!   reference **byte-for-byte** on every witness input.
//! - `Structural`: reshapes the IR (CSE, DCE, node flattening) without
//!   changing observable semantics. Output must match byte-for-byte.
//! - `Semantic`: may change IR observable semantics under a declared
//!   precondition (e.g. fast-math reassociation assumes no NaNs). The
//!   conform gate must verify the precondition holds on the witness set.
//! - `Lowering`: backend-specific emission. Output is allowed to differ in
//!   shape but must satisfy every `AlgebraicLaw` declared on the op.

/// Frozen classification of IR-mutating passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MutationClass {
    /// Renames and alias collapse only. Byte-exact output required.
    Cosmetic,
    /// Reshape without semantic change (CSE, DCE, flatten, inline). Byte-exact.
    Structural,
    /// Semantic change under a declared precondition. Requires witness proof.
    Semantic,
    /// Backend lowering. Output checked against declared algebraic laws, not
    /// against byte-for-byte reference output.
    Lowering,
}

impl MutationClass {
    /// `true` when the conform gate must verify byte-for-byte parity with the
    /// reference interpreter after this class of mutation.
    #[must_use]
    pub const fn requires_byte_parity(self) -> bool {
        matches!(self, Self::Cosmetic | Self::Structural)
    }

    /// `true` when the conform gate verifies declared `AlgebraicLaw`s rather
    /// than byte-for-byte equivalence.
    #[must_use]
    pub const fn uses_law_proof(self) -> bool {
        matches!(self, Self::Lowering)
    }
}
