//! Shared fallible scratch allocation helpers for self-substrate release paths.
//!
//! Dispatch wrappers in this crate reuse caller-owned buffers heavily. Keeping
//! reservation policy here prevents each domain from growing its own unchecked
//! `Vec::reserve` variant and keeps allocation failures actionable.

use crate::optimizer::dispatcher::DispatchError;
use std::collections::HashSet;
use std::hash::{BuildHasher, Hash};

pub(crate) fn try_reserve_vec_capacity<T>(
    buffer: &mut Vec<T>,
    capacity: usize,
) -> Result<(), String> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(buffer, capacity)
        .map_err(|error| error.to_string())
}

pub(crate) fn reserve_vec<T>(
    buffer: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), DispatchError> {
    if additional == 0 {
        return Ok(());
    }
    buffer.try_reserve_exact(additional).map_err(|error| {
        DispatchError::BackendError(format!(
            "Fix: {context} could not reserve {additional} additional scratch slot(s): {error}. Split the dispatch window before retrying."
        ))
    })
}

pub(crate) fn reserve_vec_capacity<T>(
    buffer: &mut Vec<T>,
    capacity: usize,
    context: &'static str,
) -> Result<(), DispatchError> {
    try_reserve_vec_capacity(buffer, capacity).map_err(|message| {
        DispatchError::BackendError(format!(
            "Fix: {context} could not reserve scratch capacity for {capacity} item(s): {message}. Split the dispatch window before retrying."
        ))
    })
}

pub(crate) fn reserve_vec_capacity_or_panic<T>(
    buffer: &mut Vec<T>,
    capacity: usize,
    context: &'static str,
) {
    // The name promises a panic on failure; the old body did `let _ = …`,
    // silently swallowing the reservation error (and discarding `context`) —
    // a name/behavior incoherence and a silent fallback (Law 10). Honor the
    // contract: fail loud with context.
    if let Err(message) = try_reserve_vec_capacity(buffer, capacity) {
        panic!("{context} could not reserve scratch capacity for {capacity} item(s): {message}");
    }
}

pub(crate) fn reserve_hash_set<T, S>(
    set: &mut HashSet<T, S>,
    additional: usize,
    context: &'static str,
) -> Result<(), DispatchError>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    if additional == 0 {
        return Ok(());
    }
    let target_capacity = set.len().checked_add(additional).ok_or_else(|| {
        DispatchError::BackendError(format!(
            "Fix: {context} hash scratch reservation overflowed for {additional} additional slot(s). Split the dispatch window before retrying."
        ))
    })?;
    vyre_foundation::allocation::try_reserve_hash_set_to_capacity(set, target_capacity).map_err(|error| {
        DispatchError::BackendError(format!(
            "Fix: {context} could not reserve {additional} additional hash slot(s): {error}. Split the dispatch window before retrying."
        ))
    })
}

#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn reserve_hash_set_capacity_or_panic<T, S>(
    set: &mut HashSet<T, S>,
    capacity: usize,
    context: &'static str,
) where
    T: Eq + Hash,
    S: BuildHasher,
{
    if let Err(error) = vyre_foundation::allocation::try_reserve_hash_set_to_capacity(set, capacity)
    {
        panic!(
            "Fix: {context} could not reserve hash scratch capacity for {capacity} item(s): {error}. Split the analysis window before retrying."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_vec_capacity_reuses_existing_allocation() {
        let mut scratch = Vec::<u32>::with_capacity(8);
        reserve_vec_capacity(&mut scratch, 4, "frontier seed")
            .expect("Fix: scratch grow must reuse capacity; fall back to allocate on hostile zero-cap - existing capacity should be reused");
        assert_eq!(scratch.capacity(), 8);
    }

    #[test]
    fn reserve_vec_capacity_reports_context_on_overflow() {
        let mut scratch = Vec::<u8>::new();
        let err = reserve_vec_capacity(&mut scratch, usize::MAX, "huge frontier")
            .expect_err("oversized reservation should fail");
        let message = err.to_string();
        assert!(message.contains("huge frontier"));
        assert!(message.contains("Fix:"));
    }

    #[test]
    fn reserve_vec_additional_reports_context_on_overflow() {
        let mut scratch = Vec::<u8>::new();
        let err = reserve_vec(&mut scratch, usize::MAX, "huge additional frontier")
            .expect_err("oversized reservation should fail");
        let message = err.to_string();
        assert!(message.contains("huge additional frontier"));
        assert!(message.contains("Fix:"));
    }
}
