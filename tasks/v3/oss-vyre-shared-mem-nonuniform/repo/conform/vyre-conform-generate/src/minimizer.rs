//! Binary-search counterexample shrinking for u32 witnesses.
//!
//! Given a witness `failing` and a predicate that returns `true` when the
//! witness triggers the bug, shrinks toward the smallest magnitude
//! counterexample. Deterministic, halts in O(log value) steps.

/// Shrinking engine.
pub struct CounterexampleMinimizer;

impl CounterexampleMinimizer {
    /// Shrink a failing u32 witness toward the smallest still-failing value.
    ///
    /// The shrinker tries progressively smaller halves; if a smaller value
    /// still satisfies the predicate it becomes the new candidate. Returns
    /// the minimum failing witness seen.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_conform_generate::CounterexampleMinimizer;
    /// // Predicate: "value > 100 triggers the bug".
    /// let minimized = CounterexampleMinimizer::shrink_u32(1_000, |v| v > 100);
    /// assert_eq!(minimized, 101);
    /// ```
    pub fn shrink_u32<F>(failing: u32, predicate: F) -> u32
    where
        F: Fn(u32) -> bool,
    {
        debug_assert!(
            predicate(failing),
            "caller must pass a value the predicate rejects"
        );
        let mut best = failing;
        let mut lo = 0u32;
        let mut hi = failing;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if predicate(mid) {
                best = mid;
                hi = mid;
            } else {
                if mid == u32::MAX {
                    break;
                }
                lo = mid + 1;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinks_to_boundary() {
        let min = CounterexampleMinimizer::shrink_u32(1_000_000, |v| v >= 42);
        assert_eq!(min, 42);
    }

    #[test]
    fn shrinks_to_zero_when_all_fail() {
        let min = CounterexampleMinimizer::shrink_u32(500, |_| true);
        assert_eq!(min, 0);
    }

    #[test]
    fn shrinks_minimal_input_stays_same() {
        let min = CounterexampleMinimizer::shrink_u32(7, |v| v == 7);
        assert_eq!(min, 7);
    }
}
