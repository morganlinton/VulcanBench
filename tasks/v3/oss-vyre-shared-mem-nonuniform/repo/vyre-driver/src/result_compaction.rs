//! Backend-neutral compact result readback planning.

use crate::accounting::{
    checked_add_u64_count as checked_add, checked_add_usize_count as checked_add_usize,
    checked_sub_u64_count as checked_sub, ArithmeticOverflow,
};
use crate::numeric::BackendNumericPolicy;
use crate::reservation_policy::{
    reserved_typed_vec as reserved_vec, ReservationPolicy, ReusableIndexScratch,
};

const RESULT_COMPACTION_RESERVATION: ReservationPolicy = ReservationPolicy::new(
    "result compaction",
    "shard result readback planning before launch",
);

const RESULT_COMPACTION_NUMERIC: BackendNumericPolicy =
    BackendNumericPolicy::new("result compaction");

/// One output slot before result compaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultSlot {
    /// Stable output slot id.
    pub slot: u32,
    /// Meaningful bytes produced by the kernel.
    pub meaningful_bytes: u64,
    /// Allocated/readback capacity for the output slot.
    pub capacity_bytes: u64,
}

/// One compact readback record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactResultRecord {
    /// Source output slot id.
    pub slot: u32,
    /// Offset in the compact readback slab.
    pub compact_offset: u64,
    /// Meaningful bytes copied into the slab.
    pub bytes: u64,
}

/// Compact result readback plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultCompactionPlan {
    /// Records copied into the compact slab.
    pub compact_records: Vec<CompactResultRecord>,
    /// Output slots left as direct readback ranges.
    pub direct_slots: Vec<u32>,
    /// Total allocated/readback capacity across all output slots.
    pub full_capacity_bytes: u64,
    /// Total compact slab bytes.
    pub compact_bytes: u64,
    /// Total direct readback bytes.
    pub direct_bytes: u64,
    /// Total bytes actually selected for readback after compaction planning.
    pub selected_readback_bytes: u64,
    /// Bytes avoided compared with reading full output capacities.
    pub avoided_readback_bytes: u64,
    /// Avoided readback as floor basis points of full capacity.
    pub avoided_readback_basis_points: u32,
}

/// Caller-owned scratch for repeated result-compaction planning.
#[derive(Debug, Default)]
pub struct ResultCompactionScratch {
    index_scratch: ReusableIndexScratch<u32>,
}

impl ResultCompactionScratch {
    /// Allocate empty reusable compaction scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate reusable compaction scratch for a known output-slot count.
    ///
    /// # Errors
    ///
    /// Returns [`ResultCompactionError`] when scratch storage cannot be reserved.
    pub fn try_with_capacity(slot_count: usize) -> Result<Self, ResultCompactionError> {
        let mut scratch = Self::default();
        scratch.try_reserve_slots(slot_count)?;
        Ok(scratch)
    }

    /// Reserve reusable compaction scratch for a known output-slot count.
    ///
    /// # Errors
    ///
    /// Returns [`ResultCompactionError`] when scratch storage cannot be reserved.
    pub fn try_reserve_slots(&mut self, slot_count: usize) -> Result<(), ResultCompactionError> {
        self.index_scratch.try_reserve_with(
            RESULT_COMPACTION_RESERVATION,
            slot_count,
            "scratch.ids",
            "scratch.ordered_indices",
            storage_reserve_failed,
        )
    }

    /// Retained duplicate-detection capacity.
    #[must_use]
    pub fn id_capacity(&self) -> usize {
        self.index_scratch.seen_capacity()
    }

    /// Retained slot-ordering capacity.
    #[must_use]
    pub fn ordered_index_capacity(&self) -> usize {
        self.index_scratch.ordered_index_capacity()
    }
}

/// Result compaction errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResultCompactionError {
    /// Duplicate output slot id.
    DuplicateSlot {
        /// Duplicate slot.
        slot: u32,
    },
    /// Meaningful bytes exceed allocated slot capacity.
    MeaningfulExceedsCapacity {
        /// Output slot.
        slot: u32,
        /// Meaningful bytes.
        meaningful_bytes: u64,
        /// Slot capacity.
        capacity_bytes: u64,
    },
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// Scratch or result-vector storage reservation failed before launch planning.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Requested total capacity.
        requested: usize,
        /// Allocator failure details.
        message: String,
    },
}

impl ArithmeticOverflow for ResultCompactionError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

impl std::fmt::Display for ResultCompactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateSlot { slot } => write!(
                f,
                "result compaction received duplicate output slot {slot}. Fix: assign unique output slots before readback planning."
            ),
            Self::MeaningfulExceedsCapacity {
                slot,
                meaningful_bytes,
                capacity_bytes,
            } => write!(
                f,
                "result slot {slot} has meaningful_bytes={meaningful_bytes} above capacity_bytes={capacity_bytes}. Fix: compute compact result sizes before dispatch readback."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "result compaction overflowed while computing {field}. Fix: shard compact result readback before launch."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "result compaction failed to reserve {field} for {requested} entries: {message}. Fix: shard result readback planning before launch."
            ),
        }
    }
}

impl std::error::Error for ResultCompactionError {}

/// Plan compact readback for small outputs.
///
/// # Errors
///
/// Returns [`ResultCompactionError`] when slots are invalid, byte accounting
/// overflows, or result storage cannot be reserved.
pub fn plan_result_compaction(
    slots: &[ResultSlot],
    max_compact_record_bytes: u64,
) -> Result<ResultCompactionPlan, ResultCompactionError> {
    let mut scratch = ResultCompactionScratch::try_with_capacity(slots.len())?;
    plan_result_compaction_with_scratch(slots, max_compact_record_bytes, &mut scratch)
}

/// Plan compact readback using caller-owned temporary storage.
///
/// # Errors
///
/// Returns [`ResultCompactionError`] when slots are invalid, byte accounting
/// overflows, or result storage cannot be reserved.
pub fn plan_result_compaction_with_scratch(
    slots: &[ResultSlot],
    max_compact_record_bytes: u64,
    scratch: &mut ResultCompactionScratch,
) -> Result<ResultCompactionPlan, ResultCompactionError> {
    scratch.index_scratch.clear();
    scratch.try_reserve_slots(slots.len())?;
    let mut full_capacity_bytes = 0_u64;
    let mut compact_record_count = 0usize;
    let mut direct_slot_count = 0usize;

    for (index, slot) in slots.iter().copied().enumerate() {
        if !scratch.index_scratch.insert_seen(slot.slot) {
            return Err(ResultCompactionError::DuplicateSlot { slot: slot.slot });
        }
        if slot.meaningful_bytes > slot.capacity_bytes {
            return Err(ResultCompactionError::MeaningfulExceedsCapacity {
                slot: slot.slot,
                meaningful_bytes: slot.meaningful_bytes,
                capacity_bytes: slot.capacity_bytes,
            });
        }
        full_capacity_bytes = checked_add(
            full_capacity_bytes,
            slot.capacity_bytes,
            "full capacity bytes",
        )?;
        if slot.meaningful_bytes != 0 {
            if slot.meaningful_bytes <= max_compact_record_bytes {
                compact_record_count =
                    checked_add_usize(compact_record_count, 1, "compact record count")?;
            } else {
                direct_slot_count = checked_add_usize(direct_slot_count, 1, "direct slot count")?;
            }
        }
        scratch.index_scratch.push_index(index);
    }
    scratch
        .index_scratch
        .sort_indices_unstable_by_key_if_needed(|index| slots[index].slot);

    let mut compact_records = reserved_result_vec(compact_record_count, "compact_records")?;
    let mut direct_slots = reserved_result_vec(direct_slot_count, "direct_slots")?;
    let mut compact_bytes = 0_u64;
    let mut direct_bytes = 0_u64;

    for &index in scratch.index_scratch.ordered_indices() {
        let slot = slots[index];
        if slot.meaningful_bytes == 0 {
            continue;
        }
        if slot.meaningful_bytes <= max_compact_record_bytes {
            compact_records.push(CompactResultRecord {
                slot: slot.slot,
                compact_offset: compact_bytes,
                bytes: slot.meaningful_bytes,
            });
            compact_bytes = checked_add(compact_bytes, slot.meaningful_bytes, "compact bytes")?;
        } else {
            direct_slots.push(slot.slot);
            direct_bytes = checked_add(direct_bytes, slot.meaningful_bytes, "direct bytes")?;
        }
    }

    let selected_readback_bytes =
        checked_add(compact_bytes, direct_bytes, "selected readback bytes")?;
    let avoided_readback_bytes = checked_sub(
        full_capacity_bytes,
        selected_readback_bytes,
        "avoided readback bytes",
    )?;

    Ok(ResultCompactionPlan {
        compact_records,
        direct_slots,
        full_capacity_bytes,
        compact_bytes,
        direct_bytes,
        selected_readback_bytes,
        avoided_readback_bytes,
        avoided_readback_basis_points: RESULT_COMPACTION_NUMERIC.ratio_basis_points_u64(
            avoided_readback_bytes,
            full_capacity_bytes,
            0,
            "result-compaction avoided-readback",
        ),
    })
}

fn reserved_result_vec<T>(
    capacity: usize,
    field: &'static str,
) -> Result<Vec<T>, ResultCompactionError> {
    reserved_vec(
        RESULT_COMPACTION_RESERVATION,
        capacity,
        field,
        storage_reserve_failed,
    )
}

fn storage_reserve_failed(
    field: &'static str,
    requested: usize,
    message: String,
) -> ResultCompactionError {
    ResultCompactionError::StorageReserveFailed {
        field,
        requested,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_compaction_packs_small_outputs_and_skips_empty_slots() {
        let plan =
            plan_result_compaction(&[slot(2, 0, 128), slot(1, 12, 128), slot(3, 24, 256)], 32)
                .expect("Fix: small outputs should compact");

        assert_eq!(
            plan.compact_records,
            vec![
                CompactResultRecord {
                    slot: 1,
                    compact_offset: 0,
                    bytes: 12,
                },
                CompactResultRecord {
                    slot: 3,
                    compact_offset: 12,
                    bytes: 24,
                },
            ]
        );
        assert_eq!(plan.direct_slots, Vec::<u32>::new());
        assert_eq!(plan.full_capacity_bytes, 512);
        assert_eq!(plan.compact_bytes, 36);
        assert_eq!(plan.direct_bytes, 0);
        assert_eq!(plan.selected_readback_bytes, 36);
        assert_eq!(plan.avoided_readback_bytes, 476);
        assert_eq!(plan.avoided_readback_basis_points, 9_296);
    }

    #[test]
    fn result_compaction_keeps_large_outputs_direct() {
        let plan = plan_result_compaction(&[slot(1, 64, 128), slot(2, 512, 1_024)], 128)
            .expect("Fix: mixed outputs should plan");

        assert_eq!(plan.compact_records.len(), 1);
        assert_eq!(plan.direct_slots, vec![2]);
        assert_eq!(plan.full_capacity_bytes, 1_152);
        assert_eq!(plan.compact_bytes, 64);
        assert_eq!(plan.direct_bytes, 512);
        assert_eq!(plan.selected_readback_bytes, 576);
        assert_eq!(plan.avoided_readback_bytes, 576);
        assert_eq!(plan.avoided_readback_basis_points, 5_000);
    }

    #[test]
    fn result_compaction_reports_zero_work_telemetry_without_division() {
        let plan = plan_result_compaction(&[slot(4, 0, 0), slot(9, 0, 0)], 128)
            .expect("Fix: zero-capacity outputs should plan");

        assert!(plan.compact_records.is_empty());
        assert!(plan.direct_slots.is_empty());
        assert_eq!(plan.full_capacity_bytes, 0);
        assert_eq!(plan.compact_bytes, 0);
        assert_eq!(plan.direct_bytes, 0);
        assert_eq!(plan.selected_readback_bytes, 0);
        assert_eq!(plan.avoided_readback_bytes, 0);
        assert_eq!(plan.avoided_readback_basis_points, 0);
    }

    #[test]
    fn result_compaction_rejects_invalid_slots() {
        assert_eq!(
            plan_result_compaction(&[slot(1, 1, 8), slot(1, 1, 8)], 4)
                .expect_err("duplicate slots should fail"),
            ResultCompactionError::DuplicateSlot { slot: 1 }
        );
        assert_eq!(
            plan_result_compaction(&[slot(2, 9, 8)], 4)
                .expect_err("meaningful bytes above capacity should fail"),
            ResultCompactionError::MeaningfulExceedsCapacity {
                slot: 2,
                meaningful_bytes: 9,
                capacity_bytes: 8,
            }
        );
    }

    #[test]
    fn result_compaction_avoids_tree_sets_and_slot_vector_copies() {
        let src = include_str!("result_compaction.rs");
        assert!(
            !src.contains(concat!("BTree", "Set")),
            "Fix: result compaction duplicate detection should use a hash set; slot ordering should be a final index sort."
        );
        assert!(
            !src.contains(concat!("slots", ".to_vec()")),
            "Fix: result compaction should sort slot indices rather than copying every slot before planning readback."
        );
        assert!(
            !src.contains(concat!(".", "saturating_sub")),
            "Fix: result compaction avoided-readback accounting must be exact, not saturating."
        );
        assert!(
            !src.contains(concat!(" as ", "f32")) && !src.contains(concat!(" as ", "f64")),
            "Fix: result compaction efficiency telemetry must use integer arithmetic, not lossy floats."
        );
        assert!(
            src.contains("pub full_capacity_bytes: u64")
                && src.contains("pub selected_readback_bytes: u64")
                && src.contains("pub avoided_readback_basis_points: u32"),
            "Fix: result compaction plans must expose checked capacity and integer reduction telemetry."
        );
        assert!(src.contains("RESULT_COMPACTION_NUMERIC.ratio_basis_points_u64"));
        assert!(
            !src.contains(concat!("fn ", "ratio_basis_points(")),
            "Fix: result compaction must not carry a local numeric wrapper around the shared numeric policy."
        );
        assert!(
            src.contains("ResultCompactionScratch::try_with_capacity(slots.len())?"),
            "Fix: result compaction must stage scratch with fallible release-path allocation."
        );
        assert!(
            src.contains("scratch.try_reserve_slots(slots.len())?"),
            "Fix: caller-owned result compaction scratch must grow through fallible reservation."
        );
        assert!(
            src.contains("ReusableIndexScratch"),
            "Fix: result compaction duplicate detection and ordering scratch must share the paired typed fallible reservation helper."
        );
        assert!(
            src.contains("StorageReserveFailed"),
            "Fix: result compaction allocation failures must surface as actionable launch-planning errors."
        );
        assert!(
            !src.contains(concat!("FxHashSet::with_capacity", "_and_hasher")),
            "Fix: result compaction scratch hash storage must not allocate infallibly."
        );
        assert!(
            !src.contains(concat!("Vec::with_capacity", "(slot_count)"))
                && !src.contains(concat!("Vec::with_capacity", "(slots.len())")),
            "Fix: result compaction scratch/result vectors must not allocate infallibly."
        );
    }

    #[test]
    fn result_compaction_reuses_caller_owned_slot_planning_scratch() {
        let mut scratch =
            ResultCompactionScratch::try_with_capacity(96).expect("Fix: scratch capacity");
        let wide = (0..96)
            .rev()
            .map(|index| slot(index, 8, 64))
            .collect::<Vec<_>>();
        let first = plan_result_compaction_with_scratch(&wide, 16, &mut scratch)
            .expect("Fix: wide compact result set should plan with reusable scratch");
        let id_capacity = scratch.id_capacity();
        let ordered_index_capacity = scratch.ordered_index_capacity();

        assert_eq!(first.compact_records.len(), 96);
        assert_eq!(first.compact_records[0].slot, 0);

        let second = plan_result_compaction_with_scratch(
            &[slot(7, 0, 128), slot(3, 512, 1_024), slot(5, 16, 128)],
            32,
            &mut scratch,
        )
        .expect("Fix: smaller mixed result set should reuse previous scratch");

        assert_eq!(second.compact_records[0].slot, 5);
        assert_eq!(second.direct_slots, vec![3]);
        assert!(scratch.id_capacity() >= id_capacity);
        assert!(scratch.ordered_index_capacity() >= ordered_index_capacity);
    }

    #[test]
    fn generated_result_compaction_profiles_preserve_exact_telemetry_for_4096_shapes() {
        let mut scratch = ResultCompactionScratch::default();
        for slot_count in 1u32..=128 {
            for compact_threshold in 0u64..32 {
                let slots = (0..slot_count)
                    .rev()
                    .map(|slot_id| {
                        let meaningful = u64::from((slot_id % 17) + 1);
                        ResultSlot {
                            slot: slot_id,
                            meaningful_bytes: meaningful,
                            capacity_bytes: meaningful + compact_threshold + 8,
                        }
                    })
                    .collect::<Vec<_>>();

                let plan =
                    plan_result_compaction_with_scratch(&slots, compact_threshold, &mut scratch)
                        .expect("Fix: generated result compaction profile should plan");

                let expected_full = slots.iter().map(|slot| slot.capacity_bytes).sum::<u64>();
                let expected_selected = slots.iter().map(|slot| slot.meaningful_bytes).sum::<u64>();
                assert_eq!(plan.full_capacity_bytes, expected_full);
                assert_eq!(plan.selected_readback_bytes, expected_selected);
                assert_eq!(
                    plan.avoided_readback_bytes,
                    expected_full - expected_selected
                );
                assert!(plan
                    .compact_records
                    .windows(2)
                    .all(|pair| pair[0].slot < pair[1].slot));
                assert!(plan.direct_slots.windows(2).all(|pair| pair[0] < pair[1]));
            }
        }
    }

    fn slot(slot: u32, meaningful_bytes: u64, capacity_bytes: u64) -> ResultSlot {
        ResultSlot {
            slot,
            meaningful_bytes,
            capacity_bytes,
        }
    }
}
