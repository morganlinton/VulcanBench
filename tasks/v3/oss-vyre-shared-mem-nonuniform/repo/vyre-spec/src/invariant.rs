//! Frozen invariant catalog entries used by conformance tooling.

use core::fmt;

use crate::{
    engine_invariant::InvariantId, invariant_category::InvariantCategory,
    test_descriptor::TestDescriptor,
};

/// A single invariant entry in the frozen data contract catalog.
///
/// Example: the catalog entry for `InvariantId::I1` names determinism,
/// describes the byte-identical output requirement, and points at its tests.
#[derive(Clone)]
pub struct Invariant {
    /// Stable id.
    pub id: InvariantId,
    /// Short human-readable name.
    pub name: &'static str,
    /// Full description of the backend contract.
    pub description: &'static str,
    /// Category grouping.
    pub category: InvariantCategory,
    /// Returns the test descriptors for this invariant.
    pub test_family: fn() -> &'static [TestDescriptor],
}

impl fmt::Debug for Invariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Invariant")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("category", &self.category)
            .finish_non_exhaustive()
    }
}
