//! Substrate-neutral fallible target-capacity reservation helpers.
//!
//! These helpers are deliberately below driver/runtime/self-substrate crates so
//! hot paths can share the same capacity arithmetic without creating dependency
//! cycles or backend coupling. Domain crates still own their error wording.

use std::collections::{BinaryHeap, HashMap, HashSet, TryReserveError};
use std::hash::{BuildHasher, Hash};

use smallvec::{Array, SmallVec};

/// Ensure a [`Vec`] can hold `target_capacity` items without changing length.
///
/// # Errors
///
/// Returns [`TryReserveError`] when allocation fails.
pub fn try_reserve_vec_to_capacity<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
) -> Result<(), TryReserveError> {
    if target_capacity > vec.capacity() {
        vec.try_reserve_exact(target_capacity - vec.len())?;
    }
    Ok(())
}

/// Ensure a [`String`] can hold `target_capacity` bytes without changing
/// length.
///
/// # Errors
///
/// Returns [`TryReserveError`] when allocation fails.
pub fn try_reserve_string_to_capacity(
    string: &mut String,
    target_capacity: usize,
) -> Result<(), TryReserveError> {
    if target_capacity > string.capacity() {
        string.try_reserve(target_capacity - string.len())?;
    }
    Ok(())
}

/// Ensure a [`SmallVec`] can hold `target_capacity` items without changing
/// length.
///
/// # Errors
///
/// Returns [`TryReserveError`] when allocation fails.
pub fn try_reserve_smallvec_to_capacity<A>(
    vec: &mut SmallVec<A>,
    target_capacity: usize,
) -> Result<(), smallvec::CollectionAllocErr>
where
    A: Array,
{
    if target_capacity > vec.capacity() {
        vec.try_reserve(target_capacity - vec.len())?;
    }
    Ok(())
}

/// Ensure a [`HashMap`] can hold `target_capacity` entries without changing
/// length.
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
    if target_capacity > map.capacity() {
        map.try_reserve(target_capacity - map.len())?;
    }
    Ok(())
}

/// Ensure a [`HashSet`] can hold `target_capacity` entries without changing
/// length.
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
    if target_capacity > set.capacity() {
        set.try_reserve(target_capacity - set.len())?;
    }
    Ok(())
}

/// Ensure a [`BinaryHeap`] can hold `target_capacity` entries without changing
/// length.
///
/// # Errors
///
/// Returns [`TryReserveError`] when allocation fails.
pub fn try_reserve_binary_heap_to_capacity<T>(
    heap: &mut BinaryHeap<T>,
    target_capacity: usize,
) -> Result<(), TryReserveError> {
    if target_capacity > heap.capacity() {
        heap.try_reserve(target_capacity - heap.len())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BinaryHeap, HashMap, HashSet};

    use smallvec::SmallVec;

    use super::{
        try_reserve_binary_heap_to_capacity, try_reserve_hash_map_to_capacity,
        try_reserve_hash_set_to_capacity, try_reserve_smallvec_to_capacity,
        try_reserve_string_to_capacity, try_reserve_vec_to_capacity,
    };

    #[test]
    fn target_capacity_helpers_grow_after_clear_without_mutating_lengths() {
        let mut vec = Vec::<u32>::with_capacity(4);
        let mut small = SmallVec::<[u32; 2]>::new();
        let mut string = String::with_capacity(4);
        let mut map = HashMap::<u32, u32>::with_capacity(4);
        let mut set = HashSet::<u32>::with_capacity(4);
        let mut heap = BinaryHeap::<u32>::with_capacity(4);
        for value in 0..4 {
            vec.push(value);
            small.push(value);
            string.push(char::from(b'a' + value as u8));
            map.insert(value, value * 10);
            set.insert(value);
            heap.push(value);
        }
        vec.clear();
        small.clear();
        string.clear();
        map.clear();
        set.clear();
        heap.clear();

        for target in [8, 32, 128, 1024] {
            try_reserve_vec_to_capacity(&mut vec, target)
                .expect("Fix: foundation Vec target reservation must grow cleared Vecs");
            try_reserve_smallvec_to_capacity(&mut small, target)
                .expect("Fix: foundation SmallVec target reservation must grow cleared SmallVecs");
            try_reserve_string_to_capacity(&mut string, target)
                .expect("Fix: foundation String target reservation must grow cleared strings");
            try_reserve_hash_map_to_capacity(&mut map, target)
                .expect("Fix: foundation HashMap target reservation must grow cleared maps");
            try_reserve_hash_set_to_capacity(&mut set, target)
                .expect("Fix: foundation HashSet target reservation must grow cleared sets");
            try_reserve_binary_heap_to_capacity(&mut heap, target)
                .expect("Fix: foundation BinaryHeap target reservation must grow cleared heaps");

            assert!(vec.capacity() >= target);
            assert!(small.capacity() >= target);
            assert!(string.capacity() >= target);
            assert!(map.capacity() >= target);
            assert!(set.capacity() >= target);
            assert!(heap.capacity() >= target);
            assert!(vec.is_empty());
            assert!(small.is_empty());
            assert!(string.is_empty());
            assert!(map.is_empty());
            assert!(set.is_empty());
            assert!(heap.is_empty());
        }
    }

    #[test]
    fn target_capacity_helpers_reject_usize_max_without_mutating_lengths() {
        let mut vec = vec![1u8, 2, 3];
        let mut small = SmallVec::<[u8; 2]>::from_slice(&[1, 2, 3]);
        let mut string = String::from("abc");
        let mut map = HashMap::<u8, u8>::new();
        let mut set = HashSet::<u8>::new();
        let mut heap = BinaryHeap::<u8>::from([3, 1, 2]);

        assert!(try_reserve_vec_to_capacity(&mut vec, usize::MAX).is_err());
        assert!(try_reserve_smallvec_to_capacity(&mut small, usize::MAX).is_err());
        assert!(try_reserve_string_to_capacity(&mut string, usize::MAX).is_err());
        assert!(try_reserve_hash_map_to_capacity(&mut map, usize::MAX).is_err());
        assert!(try_reserve_hash_set_to_capacity(&mut set, usize::MAX).is_err());
        assert!(try_reserve_binary_heap_to_capacity(&mut heap, usize::MAX).is_err());
        assert_eq!(vec, vec![1, 2, 3]);
        assert_eq!(small.as_slice(), &[1, 2, 3]);
        assert_eq!(string, "abc");
        assert!(map.is_empty());
        assert!(set.is_empty());
        assert_eq!(heap.len(), 3);
    }
}
