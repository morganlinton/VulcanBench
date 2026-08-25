//! Backend-neutral device-side diagnostic aggregation planning.
//!
//! Frontend diagnostics are sparse relative to token/fact streams. Reading the
//! whole candidate stream back to the host and filtering on CPU is release-path
//! wrong: it moves bytes that the device already proved irrelevant. This module
//! plans resident counter and compact-record slabs for device-side diagnostic
//! aggregation, then final-only readback of counters and compact records.

use crate::accounting::{
    checked_add_u64_count as checked_add, checked_add_usize_count as checked_add_usize,
    checked_mul_u64_count as checked_mul, ArithmeticOverflow,
};
use crate::numeric::BackendNumericPolicy;
use crate::reservation_policy::{
    reserved_typed_vec as reserved_vec, ReservationPolicy, ReusableIndexScratch,
};

const DEVICE_DIAGNOSTIC_AGGREGATION_RESERVATION: ReservationPolicy = ReservationPolicy::new(
    "device diagnostic aggregation",
    "shard diagnostic aggregation before launch planning",
);

const DEVICE_DIAGNOSTIC_AGGREGATION_NUMERIC: BackendNumericPolicy =
    BackendNumericPolicy::new("device diagnostic aggregation");

/// One device-resident diagnostic shard before aggregation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticShard {
    /// Stable shard id.
    pub shard: u32,
    /// Candidate token/fact items inspected by the device.
    pub candidate_items: u64,
    /// Diagnostics emitted by the device for this shard.
    pub emitted_diagnostics: u64,
    /// Bytes per candidate item in the unaggregated stream.
    pub raw_item_bytes: u64,
    /// Bytes per compact diagnostic record.
    pub diagnostic_record_bytes: u64,
    /// Bytes for device-side counters and overflow flags for this shard.
    pub counter_bytes: u64,
    /// Non-zero severity/category mask represented by emitted diagnostics.
    pub severity_mask: u32,
}

/// One compact diagnostic readback range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticCompactRange {
    /// Source shard id.
    pub shard: u32,
    /// Offset in the compact diagnostic slab.
    pub compact_offset: u64,
    /// Diagnostics represented in this range.
    pub records: u64,
    /// Bytes copied into the compact diagnostic slab.
    pub bytes: u64,
}

/// Device diagnostic aggregation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticAggregationPlan {
    /// Compact readback ranges ordered by shard id.
    pub compact_ranges: Vec<DiagnosticCompactRange>,
    /// Total counter/overflow bytes read by the host.
    pub counter_readback_bytes: u64,
    /// Total compact diagnostic record bytes read by the host.
    pub compact_readback_bytes: u64,
    /// Total host readback bytes after device aggregation.
    pub host_readback_bytes: u64,
    /// Bytes that would have been read by a raw candidate-stream readback.
    pub raw_candidate_readback_bytes: u64,
    /// Bytes avoided by aggregating on device.
    pub avoided_readback_bytes: u64,
    /// Aggregate compression ratio in basis points.
    pub compression_ratio_bps: u32,
    /// Diagnostics omitted because per-shard caps were reached.
    pub overflow_records: u64,
    /// Whether any shard needs a device-side overflow flag.
    pub requires_overflow_flag: bool,
    /// Whether aggregation requires a device-side prefix scan over records.
    pub requires_device_prefix_scan: bool,
    /// This plan never requires host participation before final readback.
    pub final_only_host_readback: bool,
}

/// Caller-owned scratch for repeated device diagnostic aggregation planning.
#[derive(Debug, Default)]
pub struct DiagnosticAggregationScratch {
    index_scratch: ReusableIndexScratch<u32>,
}

impl DiagnosticAggregationScratch {
    /// Allocate empty reusable diagnostic aggregation scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate reusable diagnostic aggregation scratch for a known shard count.
    ///
    /// # Errors
    ///
    /// Returns [`DiagnosticAggregationError`] when scratch storage cannot be reserved.
    pub fn try_with_capacity(shard_count: usize) -> Result<Self, DiagnosticAggregationError> {
        let mut scratch = Self::default();
        scratch.try_reserve_shards(shard_count)?;
        Ok(scratch)
    }

    /// Reserve reusable diagnostic aggregation scratch for a known shard count.
    ///
    /// # Errors
    ///
    /// Returns [`DiagnosticAggregationError`] when scratch storage cannot be reserved.
    pub fn try_reserve_shards(
        &mut self,
        shard_count: usize,
    ) -> Result<(), DiagnosticAggregationError> {
        self.index_scratch.try_reserve_with(
            DEVICE_DIAGNOSTIC_AGGREGATION_RESERVATION,
            shard_count,
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

    /// Retained shard-ordering capacity.
    #[must_use]
    pub fn ordered_index_capacity(&self) -> usize {
        self.index_scratch.ordered_index_capacity()
    }
}

/// Device diagnostic aggregation planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticAggregationError {
    /// Duplicate shard id.
    DuplicateShard {
        /// Duplicate shard id.
        shard: u32,
    },
    /// Candidate items cannot be zero for an emitted shard.
    ZeroCandidates {
        /// Invalid shard id.
        shard: u32,
    },
    /// Raw candidate ABI width must be non-zero.
    ZeroRawItemBytes {
        /// Invalid shard id.
        shard: u32,
    },
    /// Compact diagnostic ABI width must be non-zero when diagnostics exist.
    ZeroDiagnosticRecordBytes {
        /// Invalid shard id.
        shard: u32,
    },
    /// Diagnostic count cannot exceed inspected candidate items.
    EmittedExceedsCandidates {
        /// Invalid shard id.
        shard: u32,
        /// Emitted diagnostics.
        emitted_diagnostics: u64,
        /// Candidate items.
        candidate_items: u64,
    },
    /// Non-empty diagnostic shards need a non-zero severity/category mask.
    MissingSeverityMask {
        /// Invalid shard id.
        shard: u32,
    },
    /// Per-shard compact cap cannot be zero.
    ZeroRecordCap,
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// Aggregation slabs exceed the explicit device budget.
    OverBudget {
        /// Required resident/readback bytes.
        required_bytes: u64,
        /// Caller-provided budget.
        budget_bytes: u64,
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

impl ArithmeticOverflow for DiagnosticAggregationError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

impl std::fmt::Display for DiagnosticAggregationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateShard { shard } => write!(
                f,
                "device diagnostic aggregation received duplicate shard {shard}. Fix: assign unique diagnostic shard ids before device compaction."
            ),
            Self::ZeroCandidates { shard } => write!(
                f,
                "device diagnostic shard {shard} emitted diagnostics with zero candidates. Fix: emit diagnostic shards only after device candidate classification."
            ),
            Self::ZeroRawItemBytes { shard } => write!(
                f,
                "device diagnostic shard {shard} has raw_item_bytes=0. Fix: pass the concrete token/fact candidate ABI width."
            ),
            Self::ZeroDiagnosticRecordBytes { shard } => write!(
                f,
                "device diagnostic shard {shard} has diagnostic_record_bytes=0. Fix: pass the compact diagnostic record ABI width."
            ),
            Self::EmittedExceedsCandidates {
                shard,
                emitted_diagnostics,
                candidate_items,
            } => write!(
                f,
                "device diagnostic shard {shard} emitted {emitted_diagnostics} diagnostics from {candidate_items} candidates. Fix: clamp emission to the device candidate count or split the shard."
            ),
            Self::MissingSeverityMask { shard } => write!(
                f,
                "device diagnostic shard {shard} emitted diagnostics without a severity/category mask. Fix: preserve diagnostic class bits during device aggregation."
            ),
            Self::ZeroRecordCap => write!(
                f,
                "device diagnostic aggregation received a zero per-shard record cap. Fix: set an explicit compact diagnostic cap before launch."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "device diagnostic aggregation overflowed while computing {field}. Fix: shard diagnostic aggregation before readback planning."
            ),
            Self::OverBudget {
                required_bytes,
                budget_bytes,
            } => write!(
                f,
                "device diagnostic aggregation requires {required_bytes} bytes but budget allows {budget_bytes}. Fix: reduce per-shard caps, split shards, or raise the explicit device budget."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "device diagnostic aggregation failed to reserve {field} for {requested} entries: {message}. Fix: shard diagnostic aggregation before launch planning."
            ),
        }
    }
}

impl std::error::Error for DiagnosticAggregationError {}

/// Plan device-side diagnostic aggregation and final-only compact readback.
///
/// # Errors
///
/// Returns [`DiagnosticAggregationError`] when shards are invalid, byte
/// accounting overflows, the explicit budget is exceeded, or planner storage
/// cannot be reserved.
pub fn plan_device_diagnostic_aggregation(
    shards: &[DiagnosticShard],
    max_records_per_shard: u64,
    budget_bytes: u64,
) -> Result<DiagnosticAggregationPlan, DiagnosticAggregationError> {
    let mut scratch = DiagnosticAggregationScratch::try_with_capacity(shards.len())?;
    plan_device_diagnostic_aggregation_with_scratch(
        shards,
        max_records_per_shard,
        budget_bytes,
        &mut scratch,
    )
}

/// Plan device-side diagnostic aggregation using caller-owned temporary storage.
///
/// # Errors
///
/// Returns [`DiagnosticAggregationError`] when shards are invalid, byte
/// accounting overflows, the explicit budget is exceeded, or planner storage
/// cannot be reserved.
pub fn plan_device_diagnostic_aggregation_with_scratch(
    shards: &[DiagnosticShard],
    max_records_per_shard: u64,
    budget_bytes: u64,
    scratch: &mut DiagnosticAggregationScratch,
) -> Result<DiagnosticAggregationPlan, DiagnosticAggregationError> {
    if max_records_per_shard == 0 {
        return Err(DiagnosticAggregationError::ZeroRecordCap);
    }

    scratch.index_scratch.clear();
    scratch.try_reserve_shards(shards.len())?;
    let mut counter_readback_bytes = 0_u64;
    let mut compact_readback_bytes = 0_u64;
    let mut raw_candidate_readback_bytes = 0_u64;
    let mut overflow_records = 0_u64;
    let mut non_empty_diagnostic_shards = 0usize;

    for (index, shard) in shards.iter().copied().enumerate() {
        validate_shard(shard, &mut scratch.index_scratch)?;

        let raw_bytes = checked_mul(
            shard.candidate_items,
            shard.raw_item_bytes,
            "raw candidate readback bytes",
        )?;
        raw_candidate_readback_bytes = checked_add(
            raw_candidate_readback_bytes,
            raw_bytes,
            "total raw candidate readback bytes",
        )?;
        counter_readback_bytes = checked_add(
            counter_readback_bytes,
            shard.counter_bytes,
            "counter readback bytes",
        )?;
        if shard.emitted_diagnostics != 0 {
            non_empty_diagnostic_shards = checked_add_usize(
                non_empty_diagnostic_shards,
                1,
                "non-empty diagnostic shard count",
            )?;
        }
        scratch.index_scratch.push_index(index);
    }
    scratch
        .index_scratch
        .sort_indices_unstable_by_key_if_needed(|index| shards[index].shard);

    let mut compact_ranges =
        reserved_aggregation_vec(non_empty_diagnostic_shards, "compact_ranges")?;

    for &index in scratch.index_scratch.ordered_indices() {
        let shard = shards[index];
        if shard.emitted_diagnostics == 0 {
            continue;
        }

        let compact_records = shard.emitted_diagnostics.min(max_records_per_shard);
        let omitted = shard.emitted_diagnostics - compact_records;
        overflow_records = checked_add(overflow_records, omitted, "overflow records")?;
        let compact_bytes = checked_mul(
            compact_records,
            shard.diagnostic_record_bytes,
            "compact diagnostic bytes",
        )?;
        compact_ranges.push(DiagnosticCompactRange {
            shard: shard.shard,
            compact_offset: compact_readback_bytes,
            records: compact_records,
            bytes: compact_bytes,
        });
        compact_readback_bytes = checked_add(
            compact_readback_bytes,
            compact_bytes,
            "total compact diagnostic bytes",
        )?;
    }

    let host_readback_bytes = checked_add(
        counter_readback_bytes,
        compact_readback_bytes,
        "host diagnostic readback bytes",
    )?;
    if host_readback_bytes > budget_bytes {
        return Err(DiagnosticAggregationError::OverBudget {
            required_bytes: host_readback_bytes,
            budget_bytes,
        });
    }
    let compression_ratio_bps =
        diagnostic_compression_ratio_bps(host_readback_bytes, raw_candidate_readback_bytes);

    Ok(DiagnosticAggregationPlan {
        compact_ranges,
        counter_readback_bytes,
        compact_readback_bytes,
        host_readback_bytes,
        raw_candidate_readback_bytes,
        avoided_readback_bytes: avoided_readback_bytes(
            raw_candidate_readback_bytes,
            host_readback_bytes,
        ),
        compression_ratio_bps,
        overflow_records,
        requires_overflow_flag: overflow_records != 0,
        requires_device_prefix_scan: non_empty_diagnostic_shards > 1,
        final_only_host_readback: true,
    })
}

fn validate_shard(
    shard: DiagnosticShard,
    scratch: &mut ReusableIndexScratch<u32>,
) -> Result<(), DiagnosticAggregationError> {
    if !scratch.insert_seen(shard.shard) {
        return Err(DiagnosticAggregationError::DuplicateShard { shard: shard.shard });
    }
    if shard.raw_item_bytes == 0 {
        return Err(DiagnosticAggregationError::ZeroRawItemBytes { shard: shard.shard });
    }
    if shard.emitted_diagnostics > shard.candidate_items {
        return Err(DiagnosticAggregationError::EmittedExceedsCandidates {
            shard: shard.shard,
            emitted_diagnostics: shard.emitted_diagnostics,
            candidate_items: shard.candidate_items,
        });
    }
    if shard.emitted_diagnostics != 0 && shard.candidate_items == 0 {
        return Err(DiagnosticAggregationError::ZeroCandidates { shard: shard.shard });
    }
    if shard.emitted_diagnostics != 0 && shard.diagnostic_record_bytes == 0 {
        return Err(DiagnosticAggregationError::ZeroDiagnosticRecordBytes { shard: shard.shard });
    }
    if shard.emitted_diagnostics != 0 && shard.severity_mask == 0 {
        return Err(DiagnosticAggregationError::MissingSeverityMask { shard: shard.shard });
    }
    Ok(())
}

/// Compression ratio of compact diagnostic readback relative to raw candidate readback.
#[must_use]
pub fn diagnostic_compression_ratio_bps(
    host_readback_bytes: u64,
    raw_candidate_readback_bytes: u64,
) -> u32 {
    DEVICE_DIAGNOSTIC_AGGREGATION_NUMERIC.ratio_basis_points_u64(
        host_readback_bytes,
        raw_candidate_readback_bytes,
        0,
        "diagnostic compression ratio",
    )
}

fn avoided_readback_bytes(raw_candidate_readback_bytes: u64, host_readback_bytes: u64) -> u64 {
    if raw_candidate_readback_bytes >= host_readback_bytes {
        raw_candidate_readback_bytes - host_readback_bytes
    } else {
        0
    }
}

fn reserved_aggregation_vec<T>(
    capacity: usize,
    field: &'static str,
) -> Result<Vec<T>, DiagnosticAggregationError> {
    reserved_vec(
        DEVICE_DIAGNOSTIC_AGGREGATION_RESERVATION,
        capacity,
        field,
        storage_reserve_failed,
    )
}

fn storage_reserve_failed(
    field: &'static str,
    requested: usize,
    message: String,
) -> DiagnosticAggregationError {
    DiagnosticAggregationError::StorageReserveFailed {
        field,
        requested,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_aggregation_compacts_sparse_device_diagnostics() {
        let plan = plan_device_diagnostic_aggregation(
            &[
                shard(2, 2_000, 4, 32, 24, 16, 0b010),
                shard(1, 1_000, 2, 32, 24, 16, 0b001),
                shard(3, 4_000, 0, 32, 24, 16, 0),
            ],
            64,
            1_024,
        )
        .expect("Fix: sparse diagnostics should aggregate on device");

        assert_eq!(
            plan.compact_ranges,
            vec![
                DiagnosticCompactRange {
                    shard: 1,
                    compact_offset: 0,
                    records: 2,
                    bytes: 48,
                },
                DiagnosticCompactRange {
                    shard: 2,
                    compact_offset: 48,
                    records: 4,
                    bytes: 96,
                },
            ]
        );
        assert_eq!(plan.counter_readback_bytes, 48);
        assert_eq!(plan.compact_readback_bytes, 144);
        assert_eq!(plan.host_readback_bytes, 192);
        assert_eq!(plan.raw_candidate_readback_bytes, 224_000);
        assert_eq!(plan.avoided_readback_bytes, 223_808);
        assert!(plan.compression_ratio_bps < 10);
        assert!(plan.requires_device_prefix_scan);
        assert!(plan.final_only_host_readback);
    }

    #[test]
    fn diagnostic_aggregation_caps_overflow_without_host_filtering() {
        let plan =
            plan_device_diagnostic_aggregation(&[shard(7, 1_000, 10, 32, 16, 8, 0b111)], 3, 128)
                .expect("Fix: overflow should be represented by device-side flags");

        assert_eq!(plan.compact_ranges[0].records, 3);
        assert_eq!(plan.overflow_records, 7);
        assert!(plan.requires_overflow_flag);
        assert_eq!(plan.host_readback_bytes, 56);
        assert!(
            !plan.requires_device_prefix_scan,
            "Fix: a single non-empty diagnostic shard has compact offset zero and must not schedule a device prefix scan."
        );
    }

    #[test]
    fn diagnostic_aggregation_ratio_does_not_saturate_before_division() {
        let plan = plan_device_diagnostic_aggregation(
            &[shard(9, u64::MAX / 32, 1, 32, 16, u64::MAX / 20, 0b001)],
            1,
            u64::MAX,
        )
        .expect("Fix: large diagnostic plans must retain exact ratio arithmetic");

        let expected = (((plan.host_readback_bytes as u128) * 10_000)
            / plan.raw_candidate_readback_bytes as u128) as u32;
        assert_eq!(plan.compression_ratio_bps, expected);
        assert!(plan.compression_ratio_bps > 100);
    }

    #[test]
    fn diagnostic_aggregation_rejects_invalid_or_cpu_shaped_inputs() {
        assert_eq!(
            plan_device_diagnostic_aggregation(
                &[shard(1, 8, 1, 32, 24, 8, 1), shard(1, 8, 1, 32, 24, 8, 1)],
                4,
                1_024,
            )
            .expect_err("duplicate shard should fail"),
            DiagnosticAggregationError::DuplicateShard { shard: 1 }
        );
        assert_eq!(
            plan_device_diagnostic_aggregation(&[shard(2, 8, 9, 32, 24, 8, 1)], 4, 1_024)
                .expect_err("emitted diagnostics cannot exceed candidates"),
            DiagnosticAggregationError::EmittedExceedsCandidates {
                shard: 2,
                emitted_diagnostics: 9,
                candidate_items: 8,
            }
        );
        assert_eq!(
            plan_device_diagnostic_aggregation(&[shard(3, 8, 1, 32, 24, 8, 0)], 4, 1_024)
                .expect_err("diagnostics must retain class mask"),
            DiagnosticAggregationError::MissingSeverityMask { shard: 3 }
        );
        assert_eq!(
            plan_device_diagnostic_aggregation(&[shard(4, 8, 1, 32, 24, 8, 1)], 4, 16)
                .expect_err("over budget plan should fail"),
            DiagnosticAggregationError::OverBudget {
                required_bytes: 32,
                budget_bytes: 16,
            }
        );
    }

    #[test]
    fn diagnostic_aggregation_reports_zero_avoided_bytes_when_counters_exceed_raw_stream() {
        let plan = plan_device_diagnostic_aggregation(
            &[shard(1, 1, 0, 1, 8, 64, 0)],
            1,
            128,
        )
        .expect("Fix: diagnostic aggregation should report negative savings as zero avoided bytes, not fail with underflow");

        assert_eq!(plan.raw_candidate_readback_bytes, 1);
        assert_eq!(plan.host_readback_bytes, 64);
        assert_eq!(plan.avoided_readback_bytes, 0);
        assert_eq!(plan.compression_ratio_bps, 640_000);
        assert!(plan.final_only_host_readback);
    }

    #[test]
    fn diagnostic_aggregation_avoids_tree_sets_and_shard_vector_copies() {
        let src = include_str!("device_diagnostic_aggregation.rs");
        assert!(
            !src.contains(concat!("BTree", "Set")),
            "Fix: diagnostic aggregation should hash shard ids and sort compact-readback indices once."
        );
        assert!(
            !src.contains(concat!("shards", ".to_vec()")),
            "Fix: diagnostic aggregation should not copy all shard records before final compact-range ordering."
        );
        assert!(
            src.contains("fn avoided_readback_bytes(")
                && src.contains("raw_candidate_readback_bytes >= host_readback_bytes"),
            "Fix: avoided-readback telemetry must explicitly clamp negative savings to zero after checked host/raw accounting."
        );
        assert!(
            src.contains("DiagnosticAggregationScratch::try_with_capacity(shards.len())?"),
            "Fix: diagnostic aggregation must stage scratch with fallible release-path allocation."
        );
        assert!(
            src.contains("scratch.try_reserve_shards(shards.len())?"),
            "Fix: caller-owned diagnostic aggregation scratch must grow through fallible reservation."
        );
        assert!(
            src.contains("ReusableIndexScratch"),
            "Fix: diagnostic aggregation duplicate detection and ordering scratch must share the paired typed fallible reservation helper."
        );
        assert!(
            src.contains("StorageReserveFailed"),
            "Fix: diagnostic aggregation allocation failures must surface as actionable launch-planning errors."
        );
        assert!(
            !src.contains(concat!("FxHashSet::with_capacity", "_and_hasher")),
            "Fix: diagnostic aggregation scratch hash storage must not allocate infallibly."
        );
        assert!(
            !src.contains(concat!("Vec::with_capacity", "(shard_count)"))
                && !src.contains(concat!("Vec::with_capacity", "(shards.len())")),
            "Fix: diagnostic aggregation scratch/result vectors must not allocate infallibly."
        );
    }

    #[test]
    fn diagnostic_aggregation_reuses_caller_owned_shard_planning_scratch() {
        let mut scratch =
            DiagnosticAggregationScratch::try_with_capacity(128).expect("Fix: scratch capacity");
        let wide = (0..128)
            .rev()
            .map(|index| shard(index, 1_024, 1, 32, 16, 8, 1))
            .collect::<Vec<_>>();
        let first =
            plan_device_diagnostic_aggregation_with_scratch(&wide, 4, 1 << 20, &mut scratch)
                .expect("Fix: wide diagnostic aggregation should plan with reusable scratch");
        let id_capacity = scratch.id_capacity();
        let ordered_index_capacity = scratch.ordered_index_capacity();

        assert_eq!(first.compact_ranges.len(), 128);
        assert_eq!(first.compact_ranges[0].shard, 0);

        let second = plan_device_diagnostic_aggregation_with_scratch(
            &[
                shard(9, 1_000, 0, 32, 24, 16, 0),
                shard(3, 1_000, 7, 32, 24, 16, 1),
            ],
            3,
            1 << 20,
            &mut scratch,
        )
        .expect("Fix: smaller diagnostic aggregation should reuse previous scratch");

        assert_eq!(second.compact_ranges[0].shard, 3);
        assert_eq!(second.overflow_records, 4);
        assert!(scratch.id_capacity() >= id_capacity);
        assert!(scratch.ordered_index_capacity() >= ordered_index_capacity);
    }

    #[test]
    fn generated_diagnostic_aggregation_profiles_preserve_exact_telemetry_for_4096_shapes() {
        let mut scratch = DiagnosticAggregationScratch::default();
        for shard_count in 1u32..=128 {
            for cap in 1u64..=32 {
                let shards = (0..shard_count)
                    .rev()
                    .map(|id| {
                        let candidates = u64::from((id % 19) + 1) * 8;
                        let emitted = u64::from(id % 7);
                        shard(
                            id,
                            candidates,
                            emitted.min(candidates),
                            16,
                            12,
                            8,
                            if emitted == 0 { 0 } else { 1 << (id % 8) },
                        )
                    })
                    .collect::<Vec<_>>();

                let plan = plan_device_diagnostic_aggregation_with_scratch(
                    &shards,
                    cap,
                    u64::MAX,
                    &mut scratch,
                )
                .expect("Fix: generated diagnostic aggregation profile should plan");

                let expected_raw = shards
                    .iter()
                    .map(|shard| shard.candidate_items * shard.raw_item_bytes)
                    .sum::<u64>();
                let expected_counter = shards.iter().map(|shard| shard.counter_bytes).sum::<u64>();
                let expected_compact = shards
                    .iter()
                    .map(|shard| shard.emitted_diagnostics.min(cap) * shard.diagnostic_record_bytes)
                    .sum::<u64>();
                assert_eq!(plan.raw_candidate_readback_bytes, expected_raw);
                assert_eq!(plan.counter_readback_bytes, expected_counter);
                assert_eq!(plan.compact_readback_bytes, expected_compact);
                assert_eq!(
                    plan.host_readback_bytes,
                    expected_counter + expected_compact
                );
                assert!(plan
                    .compact_ranges
                    .windows(2)
                    .all(|pair| pair[0].shard < pair[1].shard));
                assert!(plan.final_only_host_readback);
            }
        }
    }

    #[test]
    fn diagnostic_aggregation_production_ratio_path_does_not_panic() {
        let source = include_str!("device_diagnostic_aggregation.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: diagnostic aggregation source must contain production section");
        assert!(
            !production.contains(".expect(")
                && !production.contains(concat!("panic", "!("))
                && !production.contains(".unwrap_or_else("),
            "Fix: diagnostic aggregation production planning must return errors or bounded telemetry instead of panicking."
        );
        assert_eq!(
            diagnostic_compression_ratio_bps(u64::MAX, 1),
            u32::MAX,
            "Fix: diagnostic compression telemetry must remain bounded when a pathological ratio exceeds export width."
        );
    }

    fn shard(
        shard: u32,
        candidate_items: u64,
        emitted_diagnostics: u64,
        raw_item_bytes: u64,
        diagnostic_record_bytes: u64,
        counter_bytes: u64,
        severity_mask: u32,
    ) -> DiagnosticShard {
        DiagnosticShard {
            shard,
            candidate_items,
            emitted_diagnostics,
            raw_item_bytes,
            diagnostic_record_bytes,
            counter_bytes,
            severity_mask,
        }
    }
}
