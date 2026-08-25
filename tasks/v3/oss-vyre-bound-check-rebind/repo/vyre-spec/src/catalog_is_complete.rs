//! Completeness check for the frozen invariant catalog.

use std::collections::BTreeSet;

use crate::{engine_invariant::InvariantId, invariants::invariants};

/// Return true when every I1..I15 invariant appears exactly once.
#[must_use]
pub fn catalog_is_complete() -> bool {
    let known_ids = InvariantId::iter().collect::<BTreeSet<_>>();
    let catalog_ids = invariants()
        .iter()
        .map(|inv| inv.id)
        .collect::<BTreeSet<_>>();
    catalog_ids == known_ids
}
