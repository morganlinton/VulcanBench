//! Backend-neutral reservation policy adapters.
//!
//! Concrete backends own their wording, but hot dispatch paths should share one
//! reservation policy for Vec, SmallVec, hash collections, and output slots.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hash};

use smallvec::{Array, SmallVec};

use crate::BackendError;

/// Domain wording for a family of bounded reservations.
#[derive(Clone, Copy, Debug)]
pub struct ReservationPolicy {
    context: &'static str,
    fix: &'static str,
}

impl ReservationPolicy {
    /// Create a reservation policy with a stable error context and fix.
    #[must_use]
    pub const fn new(context: &'static str, fix: &'static str) -> Self {
        Self { context, fix }
    }

    /// Ensure a Vec reaches `target_capacity` without changing length.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the Vec cannot reserve memory.
    pub fn reserve_vec_to_capacity<T>(
        self,
        vec: &mut Vec<T>,
        target_capacity: usize,
        item: &'static str,
    ) -> Result<(), BackendError> {
        crate::allocation::reserve_vec_to_capacity(
            vec,
            target_capacity,
            self.context,
            item,
            self.fix,
        )
    }

    /// Allocate an empty Vec with `target_capacity` reserved.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the Vec cannot reserve memory.
    pub fn reserved_vec<T>(
        self,
        target_capacity: usize,
        item: &'static str,
    ) -> Result<Vec<T>, BackendError> {
        let mut vec = Vec::new();
        self.reserve_vec_to_capacity(&mut vec, target_capacity, item)?;
        Ok(vec)
    }

    /// Reserve `additional` more Vec elements without changing length.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the Vec cannot reserve memory.
    pub fn reserve_vec_additional<T>(
        self,
        vec: &mut Vec<T>,
        additional: usize,
        item: &'static str,
    ) -> Result<(), BackendError> {
        crate::allocation::reserve_vec_additional(vec, additional, self.context, item, self.fix)
    }

    /// Reserve enough Vec storage for `target_len` elements without resizing.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the Vec cannot reserve memory.
    pub fn reserve_vec_exact_for_len<T>(
        self,
        vec: &mut Vec<T>,
        target_len: usize,
        item: &'static str,
    ) -> Result<(), BackendError> {
        crate::output_slots::reserve_vec_exact_for_len(
            vec,
            target_len,
            self.context,
            item,
            self.fix,
        )
    }

    /// Ensure a Vec of output slots has at least `slot_count` slots.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the outer Vec cannot reserve memory.
    pub fn ensure_vec_slots_at_least<T>(
        self,
        slots: &mut Vec<Vec<T>>,
        slot_count: usize,
        item: &'static str,
    ) -> Result<(), BackendError> {
        crate::output_slots::ensure_vec_slots_at_least(
            slots,
            slot_count,
            self.context,
            item,
            self.fix,
        )
    }

    /// Resize a Vec of output slots while preserving existing prefixes.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the outer Vec cannot reserve memory.
    pub fn resize_vec_slots<T>(
        self,
        slots: &mut Vec<Vec<T>>,
        slot_count: usize,
        item: &'static str,
    ) -> Result<(), BackendError> {
        crate::output_slots::resize_vec_slots(slots, slot_count, self.context, item, self.fix)
    }

    /// Clear inner output buffers without changing slot count.
    pub fn clear_vec_slots<T>(slots: &mut [Vec<T>]) {
        crate::output_slots::clear_vec_slots(slots);
    }

    /// Ensure a SmallVec reaches `target_capacity` without changing length.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the SmallVec cannot reserve memory.
    pub fn reserve_smallvec_to_capacity<A>(
        self,
        vec: &mut SmallVec<A>,
        target_capacity: usize,
        item: &'static str,
    ) -> Result<(), BackendError>
    where
        A: Array,
    {
        crate::allocation::reserve_smallvec_to_capacity(
            vec,
            target_capacity,
            self.context,
            item,
            self.fix,
        )
    }

    /// Reserve `additional` more SmallVec elements without changing length.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the SmallVec cannot reserve memory.
    pub fn reserve_smallvec_additional<A>(
        self,
        vec: &mut SmallVec<A>,
        additional: usize,
        item: &'static str,
    ) -> Result<(), BackendError>
    where
        A: Array,
    {
        crate::allocation::reserve_smallvec_additional(
            vec,
            additional,
            self.context,
            item,
            self.fix,
        )
    }

    /// Ensure a HashSet reaches `target_capacity` without changing length.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the HashSet cannot reserve memory.
    pub fn reserve_hash_set_to_capacity<T, S>(
        self,
        set: &mut HashSet<T, S>,
        target_capacity: usize,
        item: &'static str,
    ) -> Result<(), BackendError>
    where
        T: Eq + Hash,
        S: BuildHasher,
    {
        crate::allocation::reserve_hash_set_to_capacity(
            set,
            target_capacity,
            self.context,
            item,
            self.fix,
        )
    }

    /// Ensure a HashMap reaches `target_capacity` without changing length.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the HashMap cannot reserve memory.
    pub fn reserve_hash_map_to_capacity<K, V, S>(
        self,
        map: &mut HashMap<K, V, S>,
        target_capacity: usize,
        item: &'static str,
    ) -> Result<(), BackendError>
    where
        K: Eq + Hash,
        S: BuildHasher,
    {
        crate::allocation::reserve_hash_map_to_capacity(
            map,
            target_capacity,
            self.context,
            item,
            self.fix,
        )
    }
}

/// Convert a shared reservation failure into a caller-domain error.
pub type StagingReservationFailureAdapter<E> = fn(&'static str, usize, String) -> E;

/// Reserve Vec capacity and map failures into a caller-domain typed error.
///
/// # Errors
///
/// Returns `E` when the Vec cannot reserve memory.
pub fn reserve_typed_vec_to_capacity<T, E>(
    policy: ReservationPolicy,
    vec: &mut Vec<T>,
    target_capacity: usize,
    item: &'static str,
    failure: StagingReservationFailureAdapter<E>,
) -> Result<(), E> {
    policy
        .reserve_vec_to_capacity(vec, target_capacity, item)
        .map_err(|error| failure(item, target_capacity, error.to_string()))
}

/// Allocate an empty Vec with reserved capacity and typed failure mapping.
///
/// # Errors
///
/// Returns `E` when the Vec cannot reserve memory.
pub fn reserved_typed_vec<T, E>(
    policy: ReservationPolicy,
    target_capacity: usize,
    item: &'static str,
    failure: StagingReservationFailureAdapter<E>,
) -> Result<Vec<T>, E> {
    let mut vec = Vec::new();
    reserve_typed_vec_to_capacity(policy, &mut vec, target_capacity, item, failure)?;
    Ok(vec)
}

/// Reserve HashSet capacity and map failures into a caller-domain typed error.
///
/// # Errors
///
/// Returns `E` when the HashSet cannot reserve memory.
pub fn reserve_typed_hash_set_to_capacity<T, S, E>(
    policy: ReservationPolicy,
    set: &mut HashSet<T, S>,
    target_capacity: usize,
    item: &'static str,
    failure: StagingReservationFailureAdapter<E>,
) -> Result<(), E>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    policy
        .reserve_hash_set_to_capacity(set, target_capacity, item)
        .map_err(|error| failure(item, target_capacity, error.to_string()))
}

/// Reserve HashMap capacity and map failures into a caller-domain typed error.
///
/// # Errors
///
/// Returns `E` when the HashMap cannot reserve memory.
pub fn reserve_typed_hash_map_to_capacity<K, V, S, E>(
    policy: ReservationPolicy,
    map: &mut HashMap<K, V, S>,
    target_capacity: usize,
    item: &'static str,
    failure: StagingReservationFailureAdapter<E>,
) -> Result<(), E>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    policy
        .reserve_hash_map_to_capacity(map, target_capacity, item)
        .map_err(|error| failure(item, target_capacity, error.to_string()))
}

/// Reserve paired duplicate-detection and stable-order buffers with one typed failure adapter.
///
/// # Errors
///
/// Returns `E` when either staging collection cannot reserve memory.
pub fn reserve_typed_hash_set_and_vec_to_capacity<K, V, S, E>(
    policy: ReservationPolicy,
    set: &mut HashSet<K, S>,
    vec: &mut Vec<V>,
    target_capacity: usize,
    set_item: &'static str,
    vec_item: &'static str,
    failure: StagingReservationFailureAdapter<E>,
) -> Result<(), E>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    reserve_typed_hash_set_to_capacity(policy, set, target_capacity, set_item, failure)?;
    reserve_typed_vec_to_capacity(policy, vec, target_capacity, vec_item, failure)
}

/// Reusable duplicate-detection plus stable-order scratch for planner hot paths.
pub struct ReusableIndexScratch<K, S = RandomState> {
    seen: HashSet<K, S>,
    ordered_indices: Vec<usize>,
}

impl<K, S> std::fmt::Debug for ReusableIndexScratch<K, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReusableIndexScratch")
            .field("seen_capacity", &self.seen.capacity())
            .field("ordered_index_capacity", &self.ordered_indices.capacity())
            .finish()
    }
}

impl<K, S> Default for ReusableIndexScratch<K, S>
where
    S: Default,
{
    fn default() -> Self {
        Self {
            seen: HashSet::with_hasher(S::default()),
            ordered_indices: Vec::new(),
        }
    }
}

impl<K, S> ReusableIndexScratch<K, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    /// Create empty reusable index scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear retained scratch entries without releasing retained capacity.
    pub fn clear(&mut self) {
        self.seen.clear();
        self.ordered_indices.clear();
    }

    /// Reserve duplicate-detection and ordering scratch to the requested capacity.
    ///
    /// # Errors
    ///
    /// Returns `E` when either retained scratch collection cannot reserve memory.
    pub fn try_reserve_with<E>(
        &mut self,
        policy: ReservationPolicy,
        capacity: usize,
        seen_item: &'static str,
        ordered_indices_item: &'static str,
        failure: StagingReservationFailureAdapter<E>,
    ) -> Result<(), E> {
        reserve_typed_hash_set_and_vec_to_capacity(
            policy,
            &mut self.seen,
            &mut self.ordered_indices,
            capacity,
            seen_item,
            ordered_indices_item,
            failure,
        )
    }

    /// Insert a duplicate-detection key.
    pub fn insert_seen(&mut self, key: K) -> bool {
        self.seen.insert(key)
    }

    /// Append an input index to the reusable ordering buffer.
    pub fn push_index(&mut self, index: usize) {
        self.ordered_indices.push(index);
    }

    /// Mutable ordering buffer for planner-specific sort keys.
    pub fn ordered_indices_mut(&mut self) -> &mut Vec<usize> {
        &mut self.ordered_indices
    }

    /// Sort ordered indices only when the current key order is not already monotonic.
    pub fn sort_indices_unstable_by_key_if_needed<Key, F>(&mut self, mut key: F)
    where
        Key: Ord,
        F: FnMut(usize) -> Key,
    {
        let needs_sort = self
            .ordered_indices
            .windows(2)
            .any(|pair| key(pair[0]) > key(pair[1]));
        if needs_sort {
            self.ordered_indices
                .sort_unstable_by_key(|&index| key(index));
        }
    }

    /// Ordered input indices after planner-specific sorting.
    #[must_use]
    pub fn ordered_indices(&self) -> &[usize] {
        &self.ordered_indices
    }

    /// Retained duplicate-detection capacity.
    #[must_use]
    pub fn seen_capacity(&self) -> usize {
        self.seen.capacity()
    }

    /// Retained ordering capacity.
    #[must_use]
    pub fn ordered_index_capacity(&self) -> usize {
        self.ordered_indices.capacity()
    }
}

#[cfg(test)]

mod tests {
    use std::cell::Cell;
    use std::collections::{HashMap, HashSet};

    use smallvec::SmallVec;

    use super::{
        reserve_typed_hash_map_to_capacity, reserve_typed_hash_set_and_vec_to_capacity,
        reserve_typed_hash_set_to_capacity, reserve_typed_vec_to_capacity, reserved_typed_vec,
        ReservationPolicy, ReusableIndexScratch,
    };

    const TEST_POLICY: ReservationPolicy =
        ReservationPolicy::new("generated staging reserve", "split generated dispatch");

    #[derive(Debug, Eq, PartialEq)]
    enum TypedReserveError {
        Reserve {
            field: &'static str,
            requested: usize,
            message: String,
        },
    }

    fn typed_reserve_error(
        field: &'static str,
        requested: usize,
        message: String,
    ) -> TypedReserveError {
        TypedReserveError::Reserve {
            field,
            requested,
            message,
        }
    }

    #[test]
    fn policy_reserves_vec_smallvec_and_hash_collections_to_target_capacity() {
        let mut vec = Vec::<u8>::with_capacity(4);
        let mut small = SmallVec::<[u8; 2]>::new();
        let mut map = HashMap::<u32, u32>::with_capacity(4);
        let mut set = HashSet::<u32>::with_capacity(4);

        vec.extend_from_slice(&[1, 2, 3, 4]);
        small.extend_from_slice(&[1, 2, 3, 4]);
        for value in 0..4 {
            map.insert(value, value);
            set.insert(value);
        }
        vec.clear();
        small.clear();
        map.clear();
        set.clear();

        TEST_POLICY
            .reserve_vec_to_capacity(&mut vec, 32, "byte")
            .expect("Fix: Vec target reservation should grow");
        TEST_POLICY
            .reserve_smallvec_to_capacity(&mut small, 32, "byte")
            .expect("Fix: SmallVec target reservation should grow");
        TEST_POLICY
            .reserve_hash_map_to_capacity(&mut map, 32, "entry")
            .expect("Fix: HashMap target reservation should grow");
        TEST_POLICY
            .reserve_hash_set_to_capacity(&mut set, 32, "entry")
            .expect("Fix: HashSet target reservation should grow");

        assert!(vec.capacity() >= 32);
        assert!(small.capacity() >= 32);
        assert!(map.capacity() >= 32);
        assert!(set.capacity() >= 32);
        assert!(vec.is_empty());
        assert!(small.is_empty());
        assert!(map.is_empty());
        assert!(set.is_empty());
    }

    #[test]
    fn policy_manages_output_slot_vectors_without_dropping_live_prefixes() {
        let mut slots = vec![vec![1_u8], vec![2, 3]];

        TEST_POLICY
            .ensure_vec_slots_at_least(&mut slots, 4, "slot")
            .expect("Fix: slot reservation should grow");
        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0], vec![1]);
        assert_eq!(slots[1], vec![2, 3]);

        TEST_POLICY
            .resize_vec_slots(&mut slots, 1, "slot")
            .expect("Fix: slot resize should truncate without allocation");
        assert_eq!(slots, vec![vec![1]]);

        ReservationPolicy::clear_vec_slots(&mut slots);
        assert_eq!(slots, vec![Vec::<u8>::new()]);
    }

    #[test]
    fn typed_policy_reservations_share_vec_set_and_map_growth() {
        let mut vec = Vec::<u8>::new();
        let mut set = HashSet::<u32>::new();
        let mut map = HashMap::<u32, u32>::new();

        reserve_typed_vec_to_capacity(TEST_POLICY, &mut vec, 32, "typed byte", typed_reserve_error)
            .expect("Fix: typed Vec reservation should grow");
        reserve_typed_hash_set_to_capacity(
            TEST_POLICY,
            &mut set,
            32,
            "typed set entry",
            typed_reserve_error,
        )
        .expect("Fix: typed HashSet reservation should grow");
        reserve_typed_hash_map_to_capacity(
            TEST_POLICY,
            &mut map,
            32,
            "typed map entry",
            typed_reserve_error,
        )
        .expect("Fix: typed HashMap reservation should grow");
        reserve_typed_hash_set_and_vec_to_capacity(
            TEST_POLICY,
            &mut set,
            &mut vec,
            64,
            "paired set entry",
            "paired byte",
            typed_reserve_error,
        )
        .expect("Fix: paired typed reservations should share one adapter");
        let reserved =
            reserved_typed_vec::<u16, _>(TEST_POLICY, 16, "typed word", typed_reserve_error)
                .expect("Fix: typed Vec allocation should reserve");

        assert!(vec.capacity() >= 64);
        assert!(set.capacity() >= 64);
        assert!(map.capacity() >= 32);
        assert!(reserved.capacity() >= 16);
        assert!(reserved.is_empty());
    }

    #[test]
    fn typed_policy_reservation_reports_domain_failure_on_overflow() {
        let mut bytes = Vec::<u8>::new();
        let err = reserve_typed_vec_to_capacity(
            TEST_POLICY,
            &mut bytes,
            usize::MAX,
            "oversized typed byte",
            typed_reserve_error,
        )
        .expect_err("oversized typed reservation should fail");

        match err {
            TypedReserveError::Reserve {
                field,
                requested,
                message,
            } => {
                assert_eq!(field, "oversized typed byte");
                assert_eq!(requested, usize::MAX);
                assert!(message.contains("oversized typed byte"));
                assert!(message.contains("Fix:"));
            }
        }
    }

    #[test]
    fn reusable_index_scratch_preserves_capacity_and_orders_only_when_needed() {
        let mut scratch = ReusableIndexScratch::<u32>::new();

        scratch
            .try_reserve_with(
                TEST_POLICY,
                64,
                "scratch seen",
                "scratch ordered",
                typed_reserve_error,
            )
            .expect("Fix: reusable scratch should reserve through shared policy");
        assert!(scratch.insert_seen(7));
        assert!(!scratch.insert_seen(7));
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
            "Fix: monotonic planner indices must skip sort_unstable_by_key."
        );

        let seen_capacity = scratch.seen_capacity();
        let ordered_capacity = scratch.ordered_index_capacity();
        scratch.clear();
        scratch.push_index(2);
        scratch.push_index(0);
        scratch.push_index(1);
        scratch.sort_indices_unstable_by_key_if_needed(|index| [10_u32, 20, 30][index]);

        assert_eq!(scratch.ordered_indices(), &[0, 1, 2]);
        assert!(scratch.seen_capacity() >= seen_capacity);
        assert!(scratch.ordered_index_capacity() >= ordered_capacity);
    }

    #[test]
    fn generated_reusable_index_scratch_matrix_keeps_exact_order_contract() {
        for len in 0..=96 {
            let mut scratch = ReusableIndexScratch::<usize>::new();
            scratch
                .try_reserve_with(
                    TEST_POLICY,
                    len,
                    "generated seen",
                    "generated ordered",
                    typed_reserve_error,
                )
                .expect("Fix: generated scratch reservation should succeed");

            for index in (0..len).rev() {
                assert!(scratch.insert_seen(index));
                scratch.push_index(index);
            }
            scratch.sort_indices_unstable_by_key_if_needed(|index| index);

            assert_eq!(scratch.ordered_indices().len(), len);
            for (expected, actual) in scratch.ordered_indices().iter().copied().enumerate() {
                assert_eq!(actual, expected);
            }
            let seen_capacity = scratch.seen_capacity();
            let ordered_capacity = scratch.ordered_index_capacity();
            scratch.clear();
            scratch
                .try_reserve_with(
                    TEST_POLICY,
                    len / 2,
                    "generated seen shrink",
                    "generated ordered shrink",
                    typed_reserve_error,
                )
                .expect("Fix: generated scratch reuse should keep retained storage");
            assert!(scratch.seen_capacity() >= seen_capacity);
            assert!(scratch.ordered_index_capacity() >= ordered_capacity);
            assert!(scratch.ordered_indices().is_empty());
        }
    }
}
