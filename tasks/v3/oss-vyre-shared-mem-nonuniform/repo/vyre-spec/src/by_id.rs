//! Identifier lookup helpers for the frozen invariant catalog.

use crate::{engine_invariant::InvariantId, invariant::Invariant, invariants::invariants};

/// Look up an invariant by id.
#[must_use]
pub fn by_id(id: InvariantId) -> Option<&'static Invariant> {
    invariants().iter().find(|inv| inv.id == id)
}
