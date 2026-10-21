//! Backend-neutral fallible allocation reservation helpers.
//!
//! Concrete backends still own domain wording, but the arithmetic for
//! "reserve additional" and "reserve up to target capacity" must not drift.

use std::collections::{HashMap, HashSet, TryReserveError};
use std::hash::{BuildHasher, Hash};

use smallvec::{Array, SmallVec};

use crate::BackendError;

fn reserve_error(
    context: &'static str,
    requested: usize,
    item: &'static str,
    source: impl std::fmt::Display,
    fix: &'static str,
) -> BackendError {
    BackendError::new(format!(
        "{context} could not reserve {requested} {item}(s): {source}. Fix: {fix}."
    ))
}

/// Reserve additional capacity for a [`Vec`] without changing its length.
///
/// # Errors
///
/// Returns [`BackendError`] when allocation fails.
pub fn reserve_vec_additional<T>(
    vec: &mut Vec<T>,
    additional: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError> {
    vec.try_reserve(additional)
        .map_err(|source| reserve_error(context, additional, item, source, fix))
}

/// Ensure a [`Vec`] can hold `target_capacity` items without changing length,
/// returning the standard allocation error for domain-specific adapters.
///
/// # Errors
///
/// Returns [`TryReserveError`] when allocation fails.
pub fn try_reserve_vec_to_capacity<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
) -> Result<(), TryReserveError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(vec, target_capacity)
}

/// Ensure a [`Vec`] can hold `target_capacity` items without changing length.
///
/// Uses `target_capacity - len`, not `target_capacity - capacity`, so a vector
/// that was cleared after holding many elements still grows to the requested
/// target if its retained capacity is too small.
///
/// # Errors
///
/// Returns [`BackendError`] when allocation fails.
pub fn reserve_vec_to_capacity<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError> {
    try_reserve_vec_to_capacity(vec, target_capacity)
        .map_err(|source| reserve_error(context, target_capacity, item, source, fix))
}

/// Reserve additional capacity for a [`SmallVec`] without changing its length.
///
/// # Errors
///
/// Returns [`BackendError`] when allocation fails.
pub fn reserve_smallvec_additional<A>(
    vec: &mut SmallVec<A>,
    additional: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError>
where
    A: Array,
{
    vec.try_reserve(additional)
        .map_err(|source| reserve_error(context, additional, item, source, fix))
}

/// Ensure a [`SmallVec`] can hold `target_capacity` items without changing
/// length.
///
/// # Errors
///
/// Returns [`BackendError`] when allocation fails.
pub fn reserve_smallvec_to_capacity<A>(
    vec: &mut SmallVec<A>,
    target_capacity: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError>
where
    A: Array,
{
    vyre_foundation::allocation::try_reserve_smallvec_to_capacity(vec, target_capacity)
        .map_err(|source| reserve_error(context, target_capacity, item, source, fix))
}

/// Ensure a [`HashMap`] can hold `target_capacity` entries without changing
/// length, returning the standard allocation error for domain-specific
/// adapters.
///
/// # Errors
///
/// Returns [`TryReserveError`] when allocation fails.
pub fn try_reserve_hash_map_to_capacity<K, V, S>(
    map: &mut HashMap<K, V, S>,
    target_capacity: usize,
) -> Result<(), TryReserveError>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    vyre_foundation::allocation::try_reserve_hash_map_to_capacity(map, target_capacity)
}

/// Ensure a [`HashSet`] can hold `target_capacity` entries without changing
/// length, returning the standard allocation error for domain-specific
/// adapters.
///
/// # Errors
///
/// Returns [`TryReserveError`] when allocation fails.
pub fn try_reserve_hash_set_to_capacity<T, S>(
    set: &mut HashSet<T, S>,
    target_capacity: usize,
) -> Result<(), TryReserveError>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    vyre_foundation::allocation::try_reserve_hash_set_to_capacity(set, target_capacity)
}

/// Ensure a [`HashMap`] can hold `target_capacity` entries without changing
/// length.
///
/// # Errors
///
/// Returns [`BackendError`] when allocation fails.
pub fn reserve_hash_map_to_capacity<K, V, S>(
    map: &mut HashMap<K, V, S>,
    target_capacity: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    try_reserve_hash_map_to_capacity(map, target_capacity)
        .map_err(|source| reserve_error(context, target_capacity, item, source, fix))
}

/// Ensure a [`HashSet`] can hold `target_capacity` entries without changing
/// length.
///
/// # Errors
///
/// Returns [`BackendError`] when allocation fails.
pub fn reserve_hash_set_to_capacity<T, S>(
    set: &mut HashSet<T, S>,
    target_capacity: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    try_reserve_hash_set_to_capacity(set, target_capacity)
        .map_err(|source| reserve_error(context, target_capacity, item, source, fix))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use smallvec::SmallVec;

    use super::{
        reserve_hash_map_to_capacity, reserve_hash_set_to_capacity, reserve_smallvec_additional,
        reserve_smallvec_to_capacity, reserve_vec_additional, reserve_vec_to_capacity,
    };

    #[test]
    fn reserve_vec_to_capacity_grows_after_clear() {
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&[1_u8; 12]);
        bytes.clear();

        reserve_vec_to_capacity(
            &mut bytes,
            20,
            "generated reserve test",
            "byte",
            "split generated dispatch",
        )
        .expect("Fix: reserve_vec_to_capacity should grow cleared vectors");

        assert!(bytes.capacity() >= 20);
        assert!(bytes.is_empty());
    }

    #[test]
    fn reserve_smallvec_to_capacity_grows_after_clear() {
        let mut words = SmallVec::<[u32; 4]>::new();
        words.extend_from_slice(&[1, 2, 3, 4]);
        words.clear();

        reserve_smallvec_to_capacity(
            &mut words,
            8,
            "generated reserve test",
            "word",
            "split generated dispatch",
        )
        .expect("Fix: reserve_smallvec_to_capacity should grow cleared smallvecs");

        assert!(words.capacity() >= 8);
        assert!(words.is_empty());
    }

    #[test]
    fn additional_reservations_preserve_length() {
        let mut bytes = vec![1_u8, 2, 3];
        reserve_vec_additional(
            &mut bytes,
            10,
            "generated reserve test",
            "byte",
            "split generated dispatch",
        )
        .expect("Fix: reserve_vec_additional should not mutate length");
        assert_eq!(bytes, vec![1, 2, 3]);

        let mut small = SmallVec::<[u8; 2]>::new();
        small.push(9);
        reserve_smallvec_additional(
            &mut small,
            10,
            "generated reserve test",
            "byte",
            "split generated dispatch",
        )
        .expect("Fix: reserve_smallvec_additional should not mutate length");
        assert_eq!(small.as_slice(), &[9]);
    }

    #[test]
    fn hash_collection_reservations_grow_after_clear_without_reinserting() {
        let mut map = HashMap::<u32, u32>::with_capacity(4);
        let mut set = HashSet::<u32>::with_capacity(4);
        for value in 0..4 {
            map.insert(value, value * 10);
            set.insert(value);
        }
        map.clear();
        set.clear();

        for target in [8, 32, 128, 1024] {
            reserve_hash_map_to_capacity(
                &mut map,
                target,
                "generated reserve test",
                "entry",
                "split generated dispatch",
            )
            .expect("Fix: hash map target reservation should grow cleared maps");
            reserve_hash_set_to_capacity(
                &mut set,
                target,
                "generated reserve test",
                "entry",
                "split generated dispatch",
            )
            .expect("Fix: hash set target reservation should grow cleared sets");

            assert!(map.capacity() >= target);
            assert!(set.capacity() >= target);
            assert!(map.is_empty());
            assert!(set.is_empty());
        }
    }
}
