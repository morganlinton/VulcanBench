//! Category lookup helpers for the frozen invariant catalog.

use crate::{invariant::Invariant, invariant_category::InvariantCategory, invariants::invariants};

/// Return every invariant in a category.
pub fn by_category(category: InvariantCategory) -> impl Iterator<Item = &'static Invariant> {
    invariants()
        .iter()
        .filter(move |inv| inv.category == category)
}
