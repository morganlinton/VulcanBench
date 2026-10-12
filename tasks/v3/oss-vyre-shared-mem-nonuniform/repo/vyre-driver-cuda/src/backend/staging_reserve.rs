//! Shared fallible staging reservations for CUDA backend hot paths.

use std::hash::Hash;

use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use smallvec::{Array, SmallVec};
use vyre_driver::{
    reservation_policy::{
        reserve_typed_hash_map_to_capacity, reserve_typed_hash_set_and_vec_to_capacity,
        reserve_typed_hash_set_to_capacity, reserve_typed_vec_to_capacity,
        reserved_typed_vec as driver_reserved_typed_vec, ReservationPolicy, ReusableIndexScratch,
    },
    BackendError,
};

const CUDA_STAGING: ReservationPolicy = ReservationPolicy::new(
    "CUDA backend staging",
    "split the dispatch batch or lower CUDA staging fan-out before retrying",
);

pub(crate) fn reserve_vec<T>(
    vec: &mut Vec<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), BackendError> {
    CUDA_STAGING.reserve_vec_to_capacity(vec, capacity, field)
}

pub(crate) fn reserved_vec<T>(
    capacity: usize,
    field: &'static str,
) -> Result<Vec<T>, BackendError> {
    let mut vec = Vec::new();
    reserve_vec(&mut vec, capacity, field)?;
    Ok(vec)
}

pub(crate) fn ensure_vec_slots_at_least<T>(
    slots: &mut Vec<Vec<T>>,
    slot_count: usize,
    field: &'static str,
) -> Result<(), BackendError> {
    CUDA_STAGING.ensure_vec_slots_at_least(slots, slot_count, field)
}

pub(crate) fn resize_vec_slots<T>(
    slots: &mut Vec<Vec<T>>,
    slot_count: usize,
    field: &'static str,
) -> Result<(), BackendError> {
    CUDA_STAGING.resize_vec_slots(slots, slot_count, field)
}

pub(crate) fn clear_vec_slots<T>(slots: &mut [Vec<T>]) {
    ReservationPolicy::clear_vec_slots(slots);
}

pub(crate) fn reserve_smallvec<A>(
    vec: &mut SmallVec<A>,
    capacity: usize,
    field: &'static str,
) -> Result<(), BackendError>
where
    A: Array,
{
    CUDA_STAGING.reserve_smallvec_to_capacity(vec, capacity, field)
}

pub(crate) fn reserve_hash_set<T>(
    set: &mut FxHashSet<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), BackendError>
where
    T: Eq + Hash,
{
    CUDA_STAGING.reserve_hash_set_to_capacity(set, capacity, field)
}

pub(crate) fn reserve_hash_map<K, V>(
    map: &mut FxHashMap<K, V>,
    capacity: usize,
    field: &'static str,
) -> Result<(), BackendError>
where
    K: Eq + Hash,
{
    CUDA_STAGING.reserve_hash_map_to_capacity(map, capacity, field)
}

/// Domain error adapter for CUDA planners that use typed reservation failures.
pub(crate) trait CudaStorageReserveFailure: Sized {
    /// Build the planner-specific error for a failed staging reservation.
    fn storage_reserve_failed(field: &'static str, requested: usize, message: String) -> Self;
}

pub(crate) fn reserve_typed_vec<T, E>(
    vec: &mut Vec<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), E>
where
    E: CudaStorageReserveFailure,
{
    reserve_typed_vec_to_capacity(
        CUDA_STAGING,
        vec,
        capacity,
        field,
        E::storage_reserve_failed,
    )
}

pub(crate) fn reserved_typed_vec<T, E>(capacity: usize, field: &'static str) -> Result<Vec<T>, E>
where
    E: CudaStorageReserveFailure,
{
    driver_reserved_typed_vec(CUDA_STAGING, capacity, field, E::storage_reserve_failed)
}

pub(crate) fn reserve_typed_hash_set<T, E>(
    set: &mut FxHashSet<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), E>
where
    T: Eq + Hash,
    E: CudaStorageReserveFailure,
{
    reserve_typed_hash_set_to_capacity(
        CUDA_STAGING,
        set,
        capacity,
        field,
        E::storage_reserve_failed,
    )
}

pub(crate) fn reserve_typed_hash_set_and_vec<K, V, E>(
    set: &mut FxHashSet<K>,
    vec: &mut Vec<V>,
    capacity: usize,
    set_field: &'static str,
    vec_field: &'static str,
) -> Result<(), E>
where
    K: Eq + Hash,
    E: CudaStorageReserveFailure,
{
    reserve_typed_hash_set_and_vec_to_capacity(
        CUDA_STAGING,
        set,
        vec,
        capacity,
        set_field,
        vec_field,
        E::storage_reserve_failed,
    )
}

/// Reusable CUDA planner scratch for duplicate detection plus stable index ordering.
pub(crate) type CudaReusableIndexScratch<K> = ReusableIndexScratch<K, FxBuildHasher>;

pub(crate) fn reserve_index_scratch<K, E>(
    scratch: &mut CudaReusableIndexScratch<K>,
    capacity: usize,
    seen_field: &'static str,
    ordered_indices_field: &'static str,
) -> Result<(), E>
where
    K: Eq + Hash,
    E: CudaStorageReserveFailure,
{
    scratch.try_reserve_with(
        CUDA_STAGING,
        capacity,
        seen_field,
        ordered_indices_field,
        E::storage_reserve_failed,
    )
}

pub(crate) fn reserve_typed_hash_map<K, V, E>(
    map: &mut FxHashMap<K, V>,
    capacity: usize,
    field: &'static str,
) -> Result<(), E>
where
    K: Eq + Hash,
    E: CudaStorageReserveFailure,
{
    reserve_typed_hash_map_to_capacity(
        CUDA_STAGING,
        map,
        capacity,
        field,
        E::storage_reserve_failed,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use rustc_hash::{FxHashMap, FxHashSet};
    use smallvec::SmallVec;

    #[test]
    fn cuda_reusable_index_scratch_is_type_alias_not_forwarding_fork() {
        let source = include_str!("staging_reserve.rs");
        assert_eq!(
            source
                .matches(concat!("type ", "CudaReusableIndexScratch"))
                .count(),
            1
        );
        assert!(!source.contains(concat!("struct ", "CudaReusableIndexScratch")));
        assert!(!source.contains(concat!("inner", ": ReusableIndexScratch")));
    }

    use super::{
        reserve_index_scratch, reserve_smallvec, reserve_typed_hash_map, reserve_typed_hash_set,
        reserve_typed_hash_set_and_vec, reserve_typed_vec, reserve_vec, resize_vec_slots,
        CudaReusableIndexScratch, CudaStorageReserveFailure,
    };

    #[derive(Debug, Eq, PartialEq)]
    enum TypedReserveError {
        Reserve {
            field: &'static str,
            requested: usize,
            message: String,
        },
    }

    impl CudaStorageReserveFailure for TypedReserveError {
        fn storage_reserve_failed(field: &'static str, requested: usize, message: String) -> Self {
            Self::Reserve {
                field,
                requested,
                message,
            }
        }
    }

    #[test]
    fn reserve_vec_grows_to_target_capacity_after_clear() {
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&[1_u8; 12]);
        bytes.clear();

        reserve_vec(&mut bytes, 20, "test bytes").unwrap();

        assert!(
            bytes.capacity() >= 20,
            "Fix: reserve_vec must request target_capacity - len, not target_capacity - current_capacity."
        );
    }

    #[test]
    fn reserve_smallvec_grows_to_target_capacity_after_clear() {
        let mut words = SmallVec::<[u32; 4]>::new();
        words.extend_from_slice(&[1, 2, 3, 4]);
        words.clear();

        reserve_smallvec(&mut words, 8, "test words").unwrap();

        assert!(
            words.capacity() >= 8,
            "Fix: reserve_smallvec must request target_capacity - len, not target_capacity - current_capacity."
        );
    }

    #[test]
    fn typed_cuda_reservations_share_vec_set_and_map_growth() {
        let mut bytes = Vec::<u8>::new();
        let mut ids = FxHashSet::<u32>::default();
        let mut map = FxHashMap::<u32, u32>::default();

        reserve_typed_vec::<_, TypedReserveError>(&mut bytes, 32, "typed bytes").unwrap();
        reserve_typed_hash_set::<_, TypedReserveError>(&mut ids, 32, "typed ids").unwrap();
        reserve_typed_hash_map::<_, _, TypedReserveError>(&mut map, 32, "typed map").unwrap();
        reserve_typed_hash_set_and_vec::<_, _, TypedReserveError>(
            &mut ids,
            &mut bytes,
            64,
            "typed paired ids",
            "typed paired bytes",
        )
        .unwrap();

        assert!(bytes.capacity() >= 64);
        assert!(ids.capacity() >= 64);
        assert!(map.capacity() >= 32);
    }

    #[test]
    fn reusable_index_scratch_clears_entries_without_releasing_capacity() {
        let mut scratch = CudaReusableIndexScratch::<u32>::new();

        reserve_index_scratch::<_, TypedReserveError>(
            &mut scratch,
            32,
            "test seen",
            "test ordered indices",
        )
        .unwrap();
        assert!(scratch.insert_seen(7));
        assert!(!scratch.insert_seen(7));
        scratch.push_index(2);
        scratch.push_index(1);
        scratch.ordered_indices_mut().sort_unstable();
        let seen_capacity = scratch.seen_capacity();
        let ordered_capacity = scratch.ordered_index_capacity();

        assert_eq!(scratch.ordered_indices(), &[1, 2]);

        scratch.clear();
        reserve_index_scratch::<_, TypedReserveError>(
            &mut scratch,
            4,
            "test seen",
            "test ordered indices",
        )
        .unwrap();
        assert!(scratch.seen_capacity() >= seen_capacity);
        assert!(scratch.ordered_index_capacity() >= ordered_capacity);
        assert!(scratch.ordered_indices().is_empty());
        assert!(scratch.insert_seen(7));
    }

    #[test]
    fn reusable_index_scratch_skips_sort_when_keys_are_monotonic() {
        let mut scratch = CudaReusableIndexScratch::<u32>::new();
        scratch.push_index(0);
        scratch.push_index(1);
        scratch.push_index(2);

        let key_calls = Cell::new(0);
        scratch.sort_indices_unstable_by_key_if_needed(|index| {
            key_calls.set(key_calls.get() + 1);
            [10_u32, 20, 30][index]
        });

        assert_eq!(scratch.ordered_indices(), &[0, 1, 2]);
        assert_eq!(
            key_calls.get(),
            4,
            "Fix: monotonic CUDA planner indices must not call sort_unstable_by_key on already ordered release-path batches."
        );
    }

    #[test]
    fn reusable_index_scratch_sorts_when_keys_are_not_monotonic() {
        let mut scratch = CudaReusableIndexScratch::<u32>::new();
        scratch.push_index(2);
        scratch.push_index(0);
        scratch.push_index(1);

        scratch.sort_indices_unstable_by_key_if_needed(|index| [10_u32, 20, 30][index]);

        assert_eq!(scratch.ordered_indices(), &[0, 1, 2]);
    }

    #[test]
    fn resize_vec_slots_grows_and_truncates_through_shared_policy() {
        let mut slots = Vec::<Vec<u8>>::with_capacity(4);
        slots.push(vec![1, 2, 3]);
        let outer_ptr = slots.as_ptr();

        resize_vec_slots(&mut slots, 3, "cuda replay outputs").unwrap();
        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0], vec![1, 2, 3]);
        assert!(slots[1].is_empty());
        assert!(slots[2].is_empty());
        assert_eq!(slots.as_ptr(), outer_ptr);

        resize_vec_slots(&mut slots, 1, "cuda replay outputs").unwrap();
        assert_eq!(slots, vec![vec![1, 2, 3]]);
        assert_eq!(slots.as_ptr(), outer_ptr);
    }
}
