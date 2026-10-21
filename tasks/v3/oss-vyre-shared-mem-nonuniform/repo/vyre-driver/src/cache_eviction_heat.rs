//! N5 substrate: spec-cache eviction policy with frequency × recency
//! heat decay.
//!
//! F1/F3 cache compiled pipelines by `SpecCacheKey` but never evict.
//! Long-running daemons that scan many repositories in sequence
//! accumulate dead entries that pin VRAM-resident
//! pipelines. This module owns the *decision*: given a list of
//! cache entry stats and a capacity, return which entries to drop.
//!
//! The score is `hit_count / (1 + age_seconds / DECAY_HALF_LIFE_S)`  -
//! a hot, recent entry stays; a cold, old entry leaves. Pure
//! arithmetic; the actual cache surgery lives in the F1/F3 cache
//! modules and is the consumer's responsibility.

/// Half-life (seconds) for the heat decay term. Entries older than
/// this lose half their hit-count weight; doubled, lose three
/// quarters; etc. Tuned for scan workloads where a "warm" entry is
/// one used in the last few minutes of a long sweep.
pub const DECAY_HALF_LIFE_S: f64 = 300.0;

/// Per-entry stats the eviction policy needs. Caller (the F1/F3
/// cache layer) keeps these alongside each entry and passes a
/// snapshot when capacity pressure triggers.
#[derive(Debug, Clone, Copy)]
pub struct CacheEntryStats {
    /// Stable identifier for the entry (cache slot index, hash,
    /// SpecCacheKey index, etc). Pure pass-through  -  the policy
    /// only uses it to name which entries to evict.
    pub id: u64,
    /// Total hits since the entry was inserted.
    pub hit_count: u32,
    /// Wall-clock time (seconds since epoch or any monotonic clock)
    /// the entry was last hit. Same clock reference as
    /// `current_time_s`.
    pub last_hit_time_s: f64,
}

impl CacheEntryStats {
    /// Heat score: high = keep, low = evict. Combines frequency
    /// (hit_count) with recency via exponential half-life decay.
    #[must_use]
    pub fn heat(&self, current_time_s: f64) -> f64 {
        if !current_time_s.is_finite() || !self.last_hit_time_s.is_finite() {
            return 0.0;
        }
        let age = (current_time_s - self.last_hit_time_s).max(0.0);
        let decay_factor = 0.5_f64.powf(age / DECAY_HALF_LIFE_S);
        let heat = f64::from(self.hit_count) * decay_factor;
        if heat.is_finite() {
            heat
        } else {
            0.0
        }
    }
}

/// Decide which entry IDs to evict given a fixed capacity. Returns
/// the IDs in eviction order (lowest heat first); caller drops
/// until under capacity.
///
/// Entries with identical heat (e.g. two cold entries with the same
/// `hit_count` and `last_hit_time_s`) are evicted in input order
/// for determinism  -  bench reproducibility matters here.
#[must_use]
pub fn entries_to_evict(
    entries: &[CacheEntryStats],
    capacity: usize,
    current_time_s: f64,
) -> Vec<u64> {
    match try_entries_to_evict(entries, capacity, current_time_s) {
        Ok(evicted) => evicted,
        Err(_error) => Vec::new(),
    }
}

/// Fallible variant of [`entries_to_evict`] for daemon/cache paths that must
/// report allocator pressure instead of panicking.
///
/// # Errors
///
/// Returns an actionable error when ranking/result staging cannot reserve.
pub fn try_entries_to_evict(
    entries: &[CacheEntryStats],
    capacity: usize,
    current_time_s: f64,
) -> Result<Vec<u64>, String> {
    if entries.len() <= capacity {
        return Ok(Vec::new());
    }
    let mut ranked: Vec<(usize, &CacheEntryStats, f64)> = Vec::new();
    crate::allocation::try_reserve_vec_to_capacity(&mut ranked, entries.len()).map_err(|error| {
        format!(
            "cache eviction heat ranking could not reserve {} entry slot(s): {error}. Fix: shard the pipeline cache eviction batch.",
            entries.len()
        )
    })?;
    ranked.extend(
        entries
            .iter()
            .enumerate()
            .map(|(idx, e)| (idx, e, e.heat(current_time_s))),
    );
    let compare = |a: &(usize, &CacheEntryStats, f64), b: &(usize, &CacheEntryStats, f64)| {
        a.2.total_cmp(&b.2).then_with(|| a.0.cmp(&b.0))
    };
    let evict_count = entries.len() - capacity;
    if evict_count < ranked.len() {
        ranked.select_nth_unstable_by(evict_count, compare);
    }
    ranked[..evict_count].sort_by(compare);
    let mut evicted = Vec::new();
    crate::allocation::try_reserve_vec_to_capacity(&mut evicted, evict_count).map_err(|error| {
        format!(
            "cache eviction heat result could not reserve {evict_count} entry id slot(s): {error}. Fix: shard the pipeline cache eviction batch."
        )
    })?;
    evicted.extend(ranked.into_iter().take(evict_count).map(|(_, e, _)| e.id));
    Ok(evicted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, hits: u32, last_hit: f64) -> CacheEntryStats {
        CacheEntryStats {
            id,
            hit_count: hits,
            last_hit_time_s: last_hit,
        }
    }

    #[test]
    fn under_capacity_evicts_nothing() {
        let entries = vec![entry(1, 10, 100.0), entry(2, 5, 200.0)];
        assert!(entries_to_evict(&entries, 10, 300.0).is_empty());
    }

    #[test]
    fn cold_entry_evicted_before_hot_one() {
        let entries = vec![
            entry(1, 100, 290.0), // very recent, very hot
            entry(2, 1, 0.0),     // ancient, cold
        ];
        let evict = entries_to_evict(&entries, 1, 300.0);
        assert_eq!(evict, vec![2], "ancient cold entry evicted first");
    }

    #[test]
    fn equal_heat_evicts_in_input_order() {
        let entries = vec![
            entry(1, 10, 100.0),
            entry(2, 10, 100.0),
            entry(3, 10, 100.0),
        ];
        let evict = entries_to_evict(&entries, 1, 200.0);
        assert_eq!(evict, vec![1, 2], "tied heat → first two by input order");
    }

    #[test]
    fn frequency_dominates_recency_at_equal_age() {
        let entries = vec![
            entry(1, 1000, 100.0), // ancient but very hit
            entry(2, 1, 100.0),    // ancient and rarely hit
        ];
        let evict = entries_to_evict(&entries, 1, 1000.0);
        assert_eq!(evict, vec![2]);
    }

    #[test]
    fn recency_dominates_frequency_at_equal_hits() {
        // Both have 10 hits; one was 5 minutes ago, one was 1 hour ago.
        let entries = vec![
            entry(1, 10, 0.0),    // 1 hour ago
            entry(2, 10, 3300.0), // 5 minutes ago
        ];
        let evict = entries_to_evict(&entries, 1, 3600.0);
        assert_eq!(evict, vec![1], "older entry of same hit-count evicts first");
    }

    #[test]
    fn heat_decays_with_age() {
        let e = entry(0, 100, 0.0);
        let fresh = e.heat(0.0);
        let half_life = e.heat(DECAY_HALF_LIFE_S);
        let two_half_lives = e.heat(2.0 * DECAY_HALF_LIFE_S);
        assert!((fresh - 100.0).abs() < 1e-9);
        assert!((half_life - 50.0).abs() < 1e-9);
        assert!((two_half_lives - 25.0).abs() < 1e-9);
    }

    #[test]
    fn non_finite_timestamps_never_become_sticky() {
        let entries = vec![
            entry(1, u32::MAX, f64::NAN),
            entry(2, 1, 300.0),
            entry(3, u32::MAX, f64::INFINITY),
        ];
        let evict = entries_to_evict(&entries, 1, 300.0);
        assert_eq!(
            evict,
            vec![1, 3],
            "malformed cache metadata must lose to a finite live entry"
        );
    }

    #[test]
    fn non_finite_current_time_is_total_and_deterministic() {
        let entries = vec![
            entry(1, 10, 100.0),
            entry(2, 10, 100.0),
            entry(3, 10, 100.0),
        ];
        let evict = entries_to_evict(&entries, 1, f64::NAN);
        assert_eq!(
            evict,
            vec![1, 2],
            "invalid clock samples must preserve deterministic eviction order"
        );
    }

    #[test]
    fn try_entries_to_evict_matches_legacy_order() {
        let entries = vec![entry(1, 1, 0.0), entry(2, 10, 10.0), entry(3, 0, 20.0)];

        assert_eq!(
            try_entries_to_evict(&entries, 1, 20.0).unwrap(),
            entries_to_evict(&entries, 1, 20.0)
        );
    }
}
