//! Substrate-decision telemetry (P-OBS-2).
//!
//! Where [`super::observability`] tracks *call counts* per substrate
//! module, this module tracks the *decisions* substrates make: which
//! pass got reordered, which fusion candidate was picked, which
//! cache entry got evicted, etc.
//!
//! Use cases:
//! - Audit trails: "the matroid scheduler kept items 0 and 5 because
//!   the augmenting path through 3 was blocked by …"
//! - Regression detection: when a new substrate version starts
//!   making different decisions, dashboards spot it.
//! - Differential debugging: compare decisions across two runs.
//!
//! Decisions are emitted via `tracing::info!` events (no allocation
//! when `tracing` is filtered out) and counted in atomic histograms
//! for dashboard consumption.

use std::sync::atomic::{AtomicU64, Ordering};

/// Fusion-grouping bucket: the chosen subset was empty (selected_count == 0).
pub static FUSION_SELECTED_ZERO: AtomicU64 = AtomicU64::new(0);
/// Fusion-grouping bucket: selected_count in 1..=4.
pub static FUSION_SELECTED_ONE_TO_FOUR: AtomicU64 = AtomicU64::new(0);
/// Fusion-grouping bucket: selected_count in 5..=16.
pub static FUSION_SELECTED_FIVE_TO_SIXTEEN: AtomicU64 = AtomicU64::new(0);
/// Fusion-grouping bucket: selected_count >= 17.
pub static FUSION_SELECTED_SEVENTEEN_PLUS: AtomicU64 = AtomicU64::new(0);

/// Cache-eviction bucket: nothing evicted; the cache fit within the budget.
pub static EVICTION_KEPT_ALL: AtomicU64 = AtomicU64::new(0);
/// Cache-eviction bucket: <= 25% of entries dropped.
pub static EVICTION_DROPPED_LE_QUARTER: AtomicU64 = AtomicU64::new(0);
/// Cache-eviction bucket: <= 50% of entries dropped.
pub static EVICTION_DROPPED_LE_HALF: AtomicU64 = AtomicU64::new(0);
/// Cache-eviction bucket: > 50% of entries dropped.
pub static EVICTION_DROPPED_GT_HALF: AtomicU64 = AtomicU64::new(0);

/// Provenance-closure bucket: zero output cells had non-empty lineage.
pub static PROVENANCE_EMPTY: AtomicU64 = AtomicU64::new(0);
/// Provenance-closure bucket: a strict subset of output cells carried lineage.
pub static PROVENANCE_PARTIAL: AtomicU64 = AtomicU64::new(0);
/// Provenance-closure bucket: every output cell carried lineage.
pub static PROVENANCE_FULL: AtomicU64 = AtomicU64::new(0);

/// Autotune-step bucket: chosen knob equals the policy default.
pub static AUTOTUNE_DELTA_NONE: AtomicU64 = AtomicU64::new(0);
/// Autotune-step bucket: chosen knob diverges from the policy default by a small amount.
pub static AUTOTUNE_DELTA_SMALL: AtomicU64 = AtomicU64::new(0);
/// Autotune-step bucket: chosen knob diverges substantially from the policy default.
pub static AUTOTUNE_DELTA_LARGE: AtomicU64 = AtomicU64::new(0);

/// Fusion-rate bucket: heuristic matched the exact solver fully (rate == 1.0).
pub static FUSION_RATE_FULL_OPTIMAL: AtomicU64 = AtomicU64::new(0);
/// Fusion-rate bucket: heuristic recovered <= 50% of the exact solver's selection.
pub static FUSION_RATE_LE_HALF: AtomicU64 = AtomicU64::new(0);
/// Fusion-rate bucket: heuristic recovered < 25% of the exact solver's selection.
pub static FUSION_RATE_BELOW_QUARTER: AtomicU64 = AtomicU64::new(0);

/// Record one fusion-grouping decision. `selected_count` is the
/// number of items in the chosen subset; `total` is the input batch
/// size.
pub fn record_fusion(selected_count: u32, total: u32) {
    let bucket = match selected_count {
        0 => &FUSION_SELECTED_ZERO,
        1..=4 => &FUSION_SELECTED_ONE_TO_FOUR,
        5..=16 => &FUSION_SELECTED_FIVE_TO_SIXTEEN,
        _ => &FUSION_SELECTED_SEVENTEEN_PLUS,
    };
    bucket.fetch_add(1, Ordering::Relaxed);
    tracing::trace!(
        target: "vyre.substrate.fusion",
        selected = selected_count,
        total,
        "matroid scheduler decision",
    );
}

/// Record one cache-eviction decision. `dropped_fraction` is the
/// fraction in `[0, 1]` of cache entries evicted.
pub fn record_eviction(dropped_fraction: f64) {
    let f = dropped_fraction.clamp(0.0, 1.0);
    let bucket = if f == 0.0 {
        &EVICTION_KEPT_ALL
    } else if f <= 0.25 {
        &EVICTION_DROPPED_LE_QUARTER
    } else if f <= 0.50 {
        &EVICTION_DROPPED_LE_HALF
    } else {
        &EVICTION_DROPPED_GT_HALF
    };
    bucket.fetch_add(1, Ordering::Relaxed);
    tracing::trace!(
        target: "vyre.substrate.eviction",
        dropped_fraction = f,
        "submodular eviction decision",
    );
}

/// Record one autotune-step decision. `relative_delta` is the
/// magnitude of the parameter change as a fraction of the previous
/// value  -  `(new - old).abs() / max(old, eps)`.
pub fn record_autotune(relative_delta: f64) {
    let f = relative_delta.abs();
    let bucket = if f < 1e-3 {
        &AUTOTUNE_DELTA_NONE
    } else if f < 0.1 {
        &AUTOTUNE_DELTA_SMALL
    } else {
        &AUTOTUNE_DELTA_LARGE
    };
    bucket.fetch_add(1, Ordering::Relaxed);
    tracing::trace!(
        target: "vyre.substrate.autotune",
        relative_delta = f,
        "autotune step decision",
    );
}

/// Record one optimal-fusion-rate sample. `rate` is
/// `heuristic_selected / optimal_selected` in `[0, 1]`.
pub fn record_fusion_rate(rate: f64) {
    let f = rate.clamp(0.0, 1.0);
    let bucket = if f >= 0.999 {
        &FUSION_RATE_FULL_OPTIMAL
    } else if f >= 0.5 {
        &FUSION_RATE_LE_HALF
    } else {
        &FUSION_RATE_BELOW_QUARTER
    };
    bucket.fetch_add(1, Ordering::Relaxed);
    tracing::trace!(
        target: "vyre.substrate.fusion_rate",
        rate = f,
        "scheduler optimal-fusion-rate sample",
    );
}

/// Record one provenance-closure decision. `nonempty_fraction` is
/// the fraction in `[0, 1]` of output cells that ended up with
/// non-empty lineage bitsets.
pub fn record_provenance(nonempty_fraction: f64) {
    let f = nonempty_fraction.clamp(0.0, 1.0);
    let bucket = if f == 0.0 {
        &PROVENANCE_EMPTY
    } else if f >= 1.0 {
        &PROVENANCE_FULL
    } else {
        &PROVENANCE_PARTIAL
    };
    bucket.fetch_add(1, Ordering::Relaxed);
    tracing::trace!(
        target: "vyre.substrate.provenance",
        nonempty_fraction = f,
        "scallop provenance decision",
    );
}

/// Snapshot of every decision histogram bucket, formatted for
/// dashboard scrape.
#[must_use]
pub fn snapshot_decisions() -> Vec<(&'static str, u64)> {
    vec![
        (
            "fusion_selected_zero",
            FUSION_SELECTED_ZERO.load(Ordering::Relaxed),
        ),
        (
            "fusion_selected_one_to_four",
            FUSION_SELECTED_ONE_TO_FOUR.load(Ordering::Relaxed),
        ),
        (
            "fusion_selected_five_to_sixteen",
            FUSION_SELECTED_FIVE_TO_SIXTEEN.load(Ordering::Relaxed),
        ),
        (
            "fusion_selected_seventeen_plus",
            FUSION_SELECTED_SEVENTEEN_PLUS.load(Ordering::Relaxed),
        ),
        (
            "eviction_kept_all",
            EVICTION_KEPT_ALL.load(Ordering::Relaxed),
        ),
        (
            "eviction_dropped_le_quarter",
            EVICTION_DROPPED_LE_QUARTER.load(Ordering::Relaxed),
        ),
        (
            "eviction_dropped_le_half",
            EVICTION_DROPPED_LE_HALF.load(Ordering::Relaxed),
        ),
        (
            "eviction_dropped_gt_half",
            EVICTION_DROPPED_GT_HALF.load(Ordering::Relaxed),
        ),
        ("provenance_empty", PROVENANCE_EMPTY.load(Ordering::Relaxed)),
        (
            "provenance_partial",
            PROVENANCE_PARTIAL.load(Ordering::Relaxed),
        ),
        ("provenance_full", PROVENANCE_FULL.load(Ordering::Relaxed)),
        (
            "autotune_delta_none",
            AUTOTUNE_DELTA_NONE.load(Ordering::Relaxed),
        ),
        (
            "autotune_delta_small",
            AUTOTUNE_DELTA_SMALL.load(Ordering::Relaxed),
        ),
        (
            "autotune_delta_large",
            AUTOTUNE_DELTA_LARGE.load(Ordering::Relaxed),
        ),
        (
            "fusion_rate_full_optimal",
            FUSION_RATE_FULL_OPTIMAL.load(Ordering::Relaxed),
        ),
        (
            "fusion_rate_le_half",
            FUSION_RATE_LE_HALF.load(Ordering::Relaxed),
        ),
        (
            "fusion_rate_below_quarter",
            FUSION_RATE_BELOW_QUARTER.load(Ordering::Relaxed),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_fusion_buckets_correctly() {
        let before = FUSION_SELECTED_ONE_TO_FOUR.load(Ordering::Relaxed);
        record_fusion(3, 10);
        assert_eq!(
            FUSION_SELECTED_ONE_TO_FOUR.load(Ordering::Relaxed),
            before + 1
        );
    }

    #[test]
    fn record_fusion_seventeen_plus() {
        let before = FUSION_SELECTED_SEVENTEEN_PLUS.load(Ordering::Relaxed);
        record_fusion(50, 100);
        assert_eq!(
            FUSION_SELECTED_SEVENTEEN_PLUS.load(Ordering::Relaxed),
            before + 1
        );
    }

    #[test]
    fn record_eviction_kept_all() {
        let before = EVICTION_KEPT_ALL.load(Ordering::Relaxed);
        record_eviction(0.0);
        assert_eq!(EVICTION_KEPT_ALL.load(Ordering::Relaxed), before + 1);
    }

    #[test]
    fn record_eviction_clamps_above_one() {
        let before = EVICTION_DROPPED_GT_HALF.load(Ordering::Relaxed);
        record_eviction(2.0);
        assert_eq!(EVICTION_DROPPED_GT_HALF.load(Ordering::Relaxed), before + 1);
    }

    #[test]
    fn snapshot_lists_every_bucket() {
        let snap = snapshot_decisions();
        assert_eq!(snap.len(), 17);
        assert!(snap.iter().any(|(n, _)| *n == "fusion_selected_zero"));
        assert!(snap.iter().any(|(n, _)| *n == "provenance_full"));
        assert!(snap.iter().any(|(n, _)| *n == "autotune_delta_large"));
        assert!(snap.iter().any(|(n, _)| *n == "fusion_rate_full_optimal"));
    }

    #[test]
    fn record_autotune_buckets() {
        let before = AUTOTUNE_DELTA_NONE.load(Ordering::Relaxed);
        record_autotune(0.0);
        assert_eq!(AUTOTUNE_DELTA_NONE.load(Ordering::Relaxed), before + 1);
        let before = AUTOTUNE_DELTA_LARGE.load(Ordering::Relaxed);
        record_autotune(0.5);
        assert_eq!(AUTOTUNE_DELTA_LARGE.load(Ordering::Relaxed), before + 1);
    }

    #[test]
    fn record_fusion_rate_buckets() {
        let before = FUSION_RATE_FULL_OPTIMAL.load(Ordering::Relaxed);
        record_fusion_rate(1.0);
        assert_eq!(FUSION_RATE_FULL_OPTIMAL.load(Ordering::Relaxed), before + 1);
        let before = FUSION_RATE_BELOW_QUARTER.load(Ordering::Relaxed);
        record_fusion_rate(0.1);
        assert_eq!(
            FUSION_RATE_BELOW_QUARTER.load(Ordering::Relaxed),
            before + 1
        );
    }
}
