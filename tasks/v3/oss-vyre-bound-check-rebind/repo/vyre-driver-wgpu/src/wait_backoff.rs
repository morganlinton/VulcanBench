//! Shared adaptive wait backoff for short GPU callback polling loops.

use std::time::{Duration, Instant};

/// Bounded spin-then-park policy for GPU callback wait loops.
#[derive(Debug, Clone)]
pub(crate) struct AdaptiveWaitBackoff {
    idle_polls: u32,
    spin_polls: u32,
    min_park: Duration,
    max_park: Duration,
    max_shift: u32,
}

impl AdaptiveWaitBackoff {
    /// Build a wait backoff from microsecond park bounds.
    #[must_use]
    pub(crate) fn from_micros(
        spin_polls: u32,
        min_park_micros: u64,
        max_park_micros: u64,
        max_shift: u32,
    ) -> Self {
        Self {
            idle_polls: 0,
            spin_polls,
            min_park: Duration::from_micros(min_park_micros),
            max_park: Duration::from_micros(max_park_micros),
            max_shift,
        }
    }

    /// Pause once without yielding the OS thread, bounded by a deadline.
    pub(crate) fn idle_until(&mut self, deadline: Instant) {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            std::hint::spin_loop();
            return;
        };
        self.idle_for(remaining);
    }

    /// Pause once without yielding the OS thread, bounded by remaining time.
    pub(crate) fn idle_for(&mut self, remaining: Duration) {
        self.idle_polls = bounded_poll_increment(self.idle_polls);
        if self.idle_polls <= self.spin_polls {
            std::hint::spin_loop();
            return;
        }

        let shift = (self.idle_polls - self.spin_polls).min(self.max_shift.min(31));
        let multiplier = 1_u32 << shift;
        let park = checked_park_duration(self.min_park, multiplier, self.max_park)
            .min(self.max_park)
            .min(remaining);
        if park.is_zero() {
            std::hint::spin_loop();
        } else {
            std::thread::park_timeout(park);
        }
    }
}

fn bounded_poll_increment(value: u32) -> u32 {
    match value.checked_add(1) {
        Some(next) => next,
        None => u32::MAX,
    }
}

fn checked_park_duration(min_park: Duration, multiplier: u32, max_park: Duration) -> Duration {
    match min_park.checked_mul(multiplier) {
        Some(duration) => duration,
        None => max_park,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_wait_backoff_does_not_panic_on_counter_or_shift_extremes() {
        let mut backoff = AdaptiveWaitBackoff {
            idle_polls: u32::MAX,
            spin_polls: 0,
            min_park: Duration::from_micros(1),
            max_park: Duration::ZERO,
            max_shift: u32::MAX,
        };

        backoff.idle_for(Duration::ZERO);

        assert_eq!(backoff.idle_polls, u32::MAX);
    }

    #[test]
    fn adaptive_wait_backoff_source_has_no_release_path_panic_arithmetic() {
        let source = include_str!("wait_backoff.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: wait backoff production source must precede tests");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(".unwrap_or_else(")
                && !production.contains(".checked_shl("),
            "Fix: WGPU adaptive wait backoff must bound extreme polling state instead of aborting."
        );
        assert!(
            production.contains("bounded_poll_increment")
                && production.contains("checked_park_duration")
                && production.contains("self.max_shift.min(31)"),
            "Fix: WGPU adaptive wait backoff must explicitly bound counter, duration, and shift growth."
        );
    }
}
