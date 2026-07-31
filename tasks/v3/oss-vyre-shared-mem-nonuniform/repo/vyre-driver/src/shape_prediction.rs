//! N8 substrate: predicted-next-shape fingerprint API.
//!
//! Async dispatch path already exists (D3 / D7); the wait window
//! between submission and completion is dead CPU time. This module
//! owns the *prediction*: given recent dispatch fingerprints, what
//! is the most likely next dispatch? The runtime can then prefetch
//! the predicted pipeline cache key during the wait.
//!
//! Three prediction strategies, in order of preference:
//!
//! 1. **Repeat**  -  same fingerprint as the immediate predecessor
//!    (covers tight loops dispatching the same kernel).
//! 2. **Cycle of length N**  -  fingerprint = the one N steps ago, even when
//!    only a partial next cycle has been observed (covers attention's Q, K, V,
//!    scale, softmax, attend cycle before the second full cycle completes).
//! 3. **None**  -  history too sparse to predict; runtime skips the
//!    prefetch this iteration.
//!
//! Pure analysis; allocation-free after construction.

/// Fingerprint type the predictor operates over. Same shape as
/// [`crate::launch::program_vsa_fingerprint_words`] returns; the
/// callsite passes an opaque 8-word fingerprint.
pub type ShapeFingerprint = [u32; 8];

/// Bounded ring buffer of recent dispatch fingerprints. The
/// predictor looks back at most [`MAX_HISTORY`] entries.
#[derive(Debug, Clone)]
pub struct ShapeHistory {
    entries: [ShapeFingerprint; MAX_HISTORY],
    start: usize,
    len: usize,
}

/// Maximum number of historical fingerprints retained for prediction.
/// 16 is enough to catch attention-style 6-step cycles with one
/// repeat, and small enough to scan in O(N²) at predict time
/// without a measurable cost.
pub const MAX_HISTORY: usize = 16;

impl Default for ShapeHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ShapeHistory {
    /// Empty history  -  no prediction is possible.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: [[0u32; 8]; MAX_HISTORY],
            start: 0,
            len: 0,
        }
    }

    /// Record a dispatch fingerprint. The predictor uses the most
    /// recent [`MAX_HISTORY`] entries to predict the next.
    pub fn record(&mut self, fingerprint: ShapeFingerprint) {
        if self.len < MAX_HISTORY {
            let idx = (self.start + self.len) % MAX_HISTORY;
            self.entries[idx] = fingerprint;
            self.len += 1;
        } else {
            self.entries[self.start] = fingerprint;
            self.start = (self.start + 1) % MAX_HISTORY;
        }
    }

    /// Most recent fingerprint, or `None` if history is empty.
    #[must_use]
    pub fn latest(&self) -> Option<&ShapeFingerprint> {
        if self.len == 0 {
            return None;
        }
        Some(&self.entries[(self.start + self.len - 1) % MAX_HISTORY])
    }

    /// Number of entries currently retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when no entries have been recorded yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True when the retained history window contains `fingerprint`.
    ///
    /// This lets backend-side prediction caches evict cloned predicted
    /// programs that can no longer be predicted by the bounded history.
    #[must_use]
    pub fn contains(&self, fingerprint: &ShapeFingerprint) -> bool {
        (0..self.len).any(|idx| self.get(idx) == *fingerprint)
    }

    fn get(&self, logical_idx: usize) -> ShapeFingerprint {
        debug_assert!(logical_idx < self.len);
        self.entries[(self.start + logical_idx) % MAX_HISTORY]
    }

    /// Predict the next dispatch fingerprint. Returns `None` when
    /// the history is too sparse or no pattern matches.
    ///
    /// Strategy:
    /// 1. If the last two entries are equal, predict another repeat.
    /// 2. Otherwise, look for the smallest cycle length `N` such that every
    ///    retained entry with an entry `N` positions earlier matches it.
    ///    This predicts partial cycles as soon as one lag agrees, e.g.
    ///    `A,B,C,A,B -> C`, instead of waiting for `A,B,C,A,B,C`.
    /// 3. No prediction.
    #[must_use]
    pub fn predict_next(&self) -> Option<ShapeFingerprint> {
        let n = self.len;
        if n == 0 {
            return None;
        }
        // Strategy 1: repeat.
        if n >= 2 && self.get(n - 1) == self.get(n - 2) {
            return Some(self.get(n - 1));
        }
        // Strategy 2: cycle of length 2..n. Partial-cycle detection matters
        // for prefetch: after A,B,C,A,B the next useful fingerprint is C, and
        // waiting for A,B,C,A,B,C loses one dispatch worth of overlap.
        for cycle in 2..n {
            let mut matches = true;
            for i in cycle..n {
                if self.get(i) != self.get(i - cycle) {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Some(self.get(n - cycle));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(seed: u32) -> ShapeFingerprint {
        let mut a = [0u32; 8];
        for (i, slot) in a.iter_mut().enumerate() {
            *slot = seed.wrapping_mul(31).wrapping_add(i as u32);
        }
        a
    }

    #[test]
    fn empty_history_predicts_nothing() {
        let h = ShapeHistory::new();
        assert!(h.predict_next().is_none());
    }

    #[test]
    fn single_entry_history_cannot_predict() {
        let mut h = ShapeHistory::new();
        h.record(fp(1));
        assert!(h.predict_next().is_none());
    }

    #[test]
    fn repeated_fingerprint_predicts_repeat() {
        let mut h = ShapeHistory::new();
        h.record(fp(1));
        h.record(fp(1));
        assert_eq!(h.predict_next(), Some(fp(1)));
    }

    #[test]
    fn two_step_cycle_is_predicted() {
        let mut h = ShapeHistory::new();
        h.record(fp(1));
        h.record(fp(2));
        h.record(fp(1));
        h.record(fp(2));
        assert_eq!(h.predict_next(), Some(fp(1)));
    }

    #[test]
    fn three_step_cycle_is_predicted() {
        let mut h = ShapeHistory::new();
        h.record(fp(1));
        h.record(fp(2));
        h.record(fp(3));
        h.record(fp(1));
        h.record(fp(2));
        h.record(fp(3));
        assert_eq!(h.predict_next(), Some(fp(1)));
    }

    #[test]
    fn partial_three_step_cycle_is_predicted_before_second_cycle_completes() {
        let mut h = ShapeHistory::new();
        h.record(fp(1));
        h.record(fp(2));
        h.record(fp(3));
        h.record(fp(1));
        h.record(fp(2));
        assert_eq!(h.predict_next(), Some(fp(3)));
    }

    #[test]
    fn partial_long_cycle_prefetches_next_phase() {
        let mut h = ShapeHistory::new();
        for seed in [10, 20, 30, 40, 10, 20, 30] {
            h.record(fp(seed));
        }
        assert_eq!(h.predict_next(), Some(fp(40)));
    }

    #[test]
    fn no_pattern_means_no_prediction() {
        let mut h = ShapeHistory::new();
        h.record(fp(1));
        h.record(fp(2));
        h.record(fp(3));
        h.record(fp(4));
        assert!(h.predict_next().is_none());
    }

    #[test]
    fn history_caps_at_max_entries() {
        let mut h = ShapeHistory::new();
        for i in 0..(MAX_HISTORY + 5) {
            h.record(fp(i as u32));
        }
        assert_eq!(h.len(), MAX_HISTORY);
        // Earliest entry is fp(5), latest is fp(MAX_HISTORY+4).
        assert_eq!(h.latest(), Some(&fp((MAX_HISTORY + 4) as u32)));
        assert!(!h.contains(&fp(0)));
        assert!(h.contains(&fp(5)));
        assert!(h.contains(&fp((MAX_HISTORY + 4) as u32)));
    }
}
