//! Frozen generated-test descriptors tied to specific engine invariants.

use crate::engine_invariant::InvariantId;

/// Description of a concrete test the generator will materialize for an invariant.
///
/// Example: an `I4` descriptor can name a wire-format round-trip test and
/// explain the exact invariant it probes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestDescriptor {
    /// Hierarchical test name, used as the generated file stem.
    pub name: &'static str,
    /// One-line human-readable purpose for generated doc comments.
    pub purpose: &'static str,
    /// The invariant this test probes.
    pub invariant: InvariantId,
}
