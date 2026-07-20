//! Backend-neutral cache eviction policy.
//!
//! Concrete drivers use this for pipeline/module caches without depending
//! on domain or self-substrate crates. Inputs are caller-owned marginal
//! gains; output is a 0/1 retention vector where `1` means keep.

/// Compute the retention set: `k` entries to keep from `n` gains.
///
/// The algorithm is greedy argmax over caller-provided marginal gains.
/// The selected entry's gain is zeroed after each pick so it cannot be
/// selected twice. Callers that model correlated entries should update
/// `gains` between calls before invoking this helper.
#[must_use]
pub fn select_retention_set(gains: &mut [u32], n: u32, k: u32) -> Vec<u32> {
    match try_select_retention_set(gains, n, k) {
        Ok(picked) => picked,
        Err(error) => {
            tracing::error!("{error}");
            Vec::new()
        }
    }
}

/// Compute the retention set with fallible result storage allocation.
pub fn try_select_retention_set(
    gains: &mut [u32],
    n: u32,
    k: u32,
) -> Result<Vec<u32>, CacheEvictionAllocationError> {
    let effective_n = effective_len(gains, n);
    let mut picked = Vec::new();
    reserve_picked(&mut picked, effective_n)?;
    select_retention_set_into(gains, n, k, &mut picked);
    Ok(picked)
}

/// Compute the retention set into caller-owned storage.
pub fn select_retention_set_into(gains: &mut [u32], n: u32, k: u32, picked: &mut Vec<u32>) {
    let effective_n = effective_len(gains, n);
    let keep_limit = (k as usize).min(effective_n);
    picked.clear();
    picked.resize(effective_n, 0);
    let mut keep_count = 0usize;
    while keep_count < keep_limit {
        let Some(winner) = argmax_unpicked(&gains[..effective_n], picked) else {
            break;
        };
        picked[winner] = 1;
        gains[winner] = 0;
        keep_count += 1;
    }
}

/// Compute the retention set into caller-owned fallible storage.
pub fn try_select_retention_set_into(
    gains: &mut [u32],
    n: u32,
    k: u32,
    picked: &mut Vec<u32>,
) -> Result<(), CacheEvictionAllocationError> {
    let effective_n = effective_len(gains, n);
    reserve_picked(picked, effective_n)?;
    select_retention_set_into(gains, n, k, picked);
    Ok(())
}

/// Retention-vector storage allocation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheEvictionAllocationError {
    /// Requested retention-vector entries.
    pub requested: usize,
    /// Allocator failure details.
    pub message: String,
}

impl std::fmt::Display for CacheEvictionAllocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cache eviction failed to reserve {} retention entries: {}. Fix: shard cache eviction or lower cache soft caps before eviction planning.",
            self.requested, self.message
        )
    }
}

impl std::error::Error for CacheEvictionAllocationError {}

/// Record one eviction decision in the driver metrics/log stream.
pub fn record_eviction(dropped_fraction: f64) {
    let dropped_fraction = if dropped_fraction.is_finite() {
        dropped_fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    tracing::trace!(
        target: "vyre.driver.eviction",
        dropped_fraction,
        "cache eviction decision",
    );
}

/// Record one eviction decision from exact entry counts.
pub fn record_eviction_counts(dropped_entries: usize, total_entries: usize) {
    let dropped_basis_points = eviction_basis_points(dropped_entries, total_entries);
    tracing::trace!(
        target: "vyre.driver.eviction",
        dropped_entries,
        total_entries,
        dropped_basis_points,
        dropped_fraction = f64::from(dropped_basis_points) / 10_000.0,
        "cache eviction decision",
    );
}

/// Compute exact floor basis points for an eviction decision.
#[must_use]
pub fn eviction_basis_points(dropped_entries: usize, total_entries: usize) -> u32 {
    if total_entries == 0 {
        return 0;
    }
    let bounded_dropped = dropped_entries.min(total_entries);
    let dropped = u64::try_from(bounded_dropped).unwrap_or(u64::MAX);
    let total = u64::try_from(total_entries).unwrap_or(u64::MAX);
    crate::numeric::ratio_basis_points_u64(
        dropped,
        total,
        0,
        "cache eviction dropped entries",
        "driver",
    )
    .min(10_000)
}

fn effective_len(gains: &[u32], n: u32) -> usize {
    gains.len().min(n as usize)
}

fn reserve_picked(
    picked: &mut Vec<u32>,
    effective_n: usize,
) -> Result<(), CacheEvictionAllocationError> {
    crate::allocation::try_reserve_vec_to_capacity(picked, effective_n).map_err(|error| {
        CacheEvictionAllocationError {
            requested: effective_n,
            message: error.to_string(),
        }
    })
}

fn argmax_unpicked(gains: &[u32], picked: &[u32]) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (idx, gain) in gains.iter().copied().enumerate() {
        if picked.get(idx).copied().unwrap_or(0) != 0 || gain == 0 {
            continue;
        }
        match best {
            Some((_, current)) if gain <= current => {}
            _ => best = Some((idx, gain)),
        }
    }
    best.map(|(idx, _)| idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_top_k_gains() {
        let mut gains = vec![3, 10, 2, 8, 1];
        let picked = select_retention_set(&mut gains, 5, 2);
        assert_eq!(picked, vec![0, 1, 0, 1, 0]);
    }

    #[test]
    fn zero_k_evicts_all() {
        let mut gains = vec![3, 10, 2];
        let picked = select_retention_set(&mut gains, 3, 0);
        assert_eq!(picked, vec![0, 0, 0]);
    }

    #[test]
    fn k_equal_n_keeps_positive_gain_entries() {
        let mut gains = vec![3, 0, 2];
        let picked = select_retention_set(&mut gains, 3, 3);
        assert_eq!(picked, vec![1, 0, 1]);
    }

    #[test]
    fn into_reuses_storage() {
        let mut gains = vec![1, 9, 4];
        let mut picked = Vec::with_capacity(8);
        let ptr = picked.as_ptr();
        select_retention_set_into(&mut gains, 3, 2, &mut picked);
        assert_eq!(picked, vec![0, 1, 1]);
        assert_eq!(picked.as_ptr(), ptr);
    }

    #[test]
    fn try_into_reuses_storage() {
        let mut gains = vec![1, 9, 4];
        let mut picked = Vec::with_capacity(8);
        let ptr = picked.as_ptr();
        try_select_retention_set_into(&mut gains, 3, 2, &mut picked)
            .expect("Fix: retention scratch should be reusable");
        assert_eq!(picked, vec![0, 1, 1]);
        assert_eq!(picked.as_ptr(), ptr);
    }

    #[test]
    fn invalid_sizing_is_clamped_not_panicked() {
        let mut gains = vec![5, 1];
        let picked = select_retention_set(&mut gains, 99, 99);
        assert_eq!(picked, vec![1, 1]);
    }

    #[test]
    fn eviction_basis_points_are_exact_and_bounded() {
        assert_eq!(eviction_basis_points(0, 0), 0);
        assert_eq!(eviction_basis_points(1, 2), 5_000);
        assert_eq!(eviction_basis_points(476, 512), 9_296);
        assert_eq!(eviction_basis_points(9, 3), 10_000);
        assert_eq!(eviction_basis_points(usize::MAX, usize::MAX), 10_000);
    }

    #[test]
    fn eviction_recording_accepts_hostile_ratios() {
        record_eviction(f64::NAN);
        record_eviction(f64::INFINITY);
        record_eviction(f64::NEG_INFINITY);
        record_eviction_counts(usize::MAX, 1);
    }

    #[test]
    fn release_eviction_selector_exposes_fallible_allocation_path() {
        let source = include_str!("cache_eviction.rs");
        assert!(
            source.contains("pub fn try_select_retention_set")
                && source.contains("pub fn try_select_retention_set_into")
                && source.contains("try_reserve_vec_to_capacity"),
            "Fix: release cache eviction callers need a fallible selector path instead of infallible Vec allocation."
        );
        assert!(
            !source.contains(concat!("Vec::with_capacity", "(effective_len")),
            "Fix: cache eviction selector must not allocate retention vectors infallibly on release paths."
        );
    }
}
