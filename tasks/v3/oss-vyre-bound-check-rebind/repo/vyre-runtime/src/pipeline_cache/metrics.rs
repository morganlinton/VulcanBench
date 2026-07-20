//! Pipeline-cache instrumentation: the public snapshot type and the
//! internal atomic counter struct shared by every concrete backend.

use std::sync::atomic::{AtomicU64, Ordering};

/// Pipeline-cache instrumentation counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipelineCacheMetrics {
    /// Lookup attempts.
    pub lookups: u64,
    /// Successful lookups.
    pub hits: u64,
    /// Failed lookups.
    pub misses: u64,
    /// Accepted put attempts.
    pub puts: u64,
    /// Rejected put attempts, usually because a blob exceeds the byte budget.
    pub rejected_puts: u64,
    /// Entries evicted by capacity or byte-budget pressure.
    pub evictions: u64,
    /// Bytes removed by eviction.
    pub evicted_bytes: u64,
    /// Explicit flush attempts.
    pub flushes: u64,
    /// Explicit flush failures.
    pub flush_errors: u64,
    /// Current retained bytes when the backend can report them cheaply.
    pub cached_bytes: u64,
    /// Current retained entries when the backend can report them cheaply.
    pub entries: u64,
}

/// Pipeline-cache metric arithmetic failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineCacheMetricError {
    field: &'static str,
    message: String,
}

impl PipelineCacheMetricError {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    /// Metric field that failed arithmetic.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// Actionable failure text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for PipelineCacheMetricError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for PipelineCacheMetricError {}

impl PipelineCacheMetrics {
    /// Cache-hit rate in parts per million.
    #[must_use]
    pub fn hit_rate_ppm(&self) -> u32 {
        self.try_hit_rate_ppm().unwrap_or(u32::MAX)
    }

    /// Fallibly compute cache-hit rate in parts per million.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineCacheMetricError`] when the numerator or final value
    /// cannot fit the public metric ABI.
    pub fn try_hit_rate_ppm(&self) -> Result<u32, PipelineCacheMetricError> {
        if self.lookups == 0 {
            return Ok(0);
        }
        let numerator =
            self.hits
                .checked_mul(1_000_000)
                .ok_or_else(|| PipelineCacheMetricError::new(
                    "hit_rate_ppm",
                    "pipeline cache hit-rate numerator overflowed u64. Fix: snapshot and reset cache metrics before counters saturate.",
                ))?;
        let value = numerator / self.lookups;
        u32::try_from(value).map_err(|source| {
            PipelineCacheMetricError::new(
                "hit_rate_ppm",
                format!(
                    "pipeline cache hit-rate ppm cannot fit u32: {source}. Fix: snapshot and reset cache metrics before counters saturate."
                ),
            )
        })
    }

    pub(super) fn checked_add(self, rhs: Self) -> Self {
        self.saturating_add(rhs)
    }

    pub(super) fn try_checked_add(self, rhs: Self) -> Result<Self, PipelineCacheMetricError> {
        Ok(Self {
            lookups: try_metric_add(self.lookups, rhs.lookups, "lookups")?,
            hits: try_metric_add(self.hits, rhs.hits, "hits")?,
            misses: try_metric_add(self.misses, rhs.misses, "misses")?,
            puts: try_metric_add(self.puts, rhs.puts, "puts")?,
            rejected_puts: try_metric_add(self.rejected_puts, rhs.rejected_puts, "rejected puts")?,
            evictions: try_metric_add(self.evictions, rhs.evictions, "evictions")?,
            evicted_bytes: try_metric_add(self.evicted_bytes, rhs.evicted_bytes, "evicted bytes")?,
            flushes: try_metric_add(self.flushes, rhs.flushes, "flushes")?,
            flush_errors: try_metric_add(self.flush_errors, rhs.flush_errors, "flush errors")?,
            cached_bytes: try_metric_add(self.cached_bytes, rhs.cached_bytes, "cached bytes")?,
            entries: try_metric_add(self.entries, rhs.entries, "entries")?,
        })
    }

    fn saturating_add(self, rhs: Self) -> Self {
        Self {
            lookups: self.lookups.saturating_add(rhs.lookups),
            hits: self.hits.saturating_add(rhs.hits),
            misses: self.misses.saturating_add(rhs.misses),
            puts: self.puts.saturating_add(rhs.puts),
            rejected_puts: self.rejected_puts.saturating_add(rhs.rejected_puts),
            evictions: self.evictions.saturating_add(rhs.evictions),
            evicted_bytes: self.evicted_bytes.saturating_add(rhs.evicted_bytes),
            flushes: self.flushes.saturating_add(rhs.flushes),
            flush_errors: self.flush_errors.saturating_add(rhs.flush_errors),
            cached_bytes: self.cached_bytes.saturating_add(rhs.cached_bytes),
            entries: self.entries.saturating_add(rhs.entries),
        }
    }
}

fn try_metric_add(
    lhs: u64,
    rhs: u64,
    label: &'static str,
) -> Result<u64, PipelineCacheMetricError> {
    lhs.checked_add(rhs).ok_or_else(|| {
        PipelineCacheMetricError::new(
            label,
            format!(
                "pipeline cache metric {label} overflowed u64. Fix: reset or shard pipeline cache metrics before aggregation."
            ),
        )
    })
}

#[derive(Debug, Default)]
pub(super) struct PipelineCacheCounters {
    pub(super) lookups: AtomicU64,
    pub(super) hits: AtomicU64,
    pub(super) misses: AtomicU64,
    pub(super) puts: AtomicU64,
    pub(super) rejected_puts: AtomicU64,
    pub(super) evictions: AtomicU64,
    pub(super) evicted_bytes: AtomicU64,
    pub(super) flushes: AtomicU64,
    pub(super) flush_errors: AtomicU64,
}

impl PipelineCacheCounters {
    pub(super) fn increment(counter: &AtomicU64, label: &'static str) {
        Self::add(counter, 1, label);
    }

    pub(super) fn add(counter: &AtomicU64, value: u64, label: &'static str) {
        if let Err(error) = Self::try_add(counter, value, label) {
            tracing::warn!(error = %error, label, "pipeline cache counter saturated");
            counter.store(u64::MAX, Ordering::Relaxed);
        }
    }

    pub(super) fn try_add(
        counter: &AtomicU64,
        value: u64,
        label: &'static str,
    ) -> Result<(), PipelineCacheMetricError> {
        let mut current = counter.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_add(value) else {
                return Err(PipelineCacheMetricError::new(
                    label,
                    format!(
                        "pipeline cache counter {label} overflowed u64. Fix: snapshot and reset cache metrics before counters saturate."
                    ),
                ));
            };
            match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Ok(()),
                Err(observed) => current = observed,
            }
        }
    }

    pub(super) fn snapshot(&self, cached_bytes: u64, entries: u64) -> PipelineCacheMetrics {
        PipelineCacheMetrics {
            lookups: self.lookups.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            puts: self.puts.load(Ordering::Relaxed),
            rejected_puts: self.rejected_puts.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            evicted_bytes: self.evicted_bytes.load(Ordering::Relaxed),
            flushes: self.flushes.load(Ordering::Relaxed),
            flush_errors: self.flush_errors.load(Ordering::Relaxed),
            cached_bytes,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU64;

    use super::{PipelineCacheCounters, PipelineCacheMetrics};

    #[test]
    fn pipeline_cache_metrics_generated_hit_rates_are_exact_ppm() {
        for hits in 0..=1024_u64 {
            let metrics = PipelineCacheMetrics {
                lookups: 2048,
                hits,
                ..PipelineCacheMetrics::default()
            };
            assert_eq!(metrics.hit_rate_ppm(), ((hits * 1_000_000) / 2048) as u32);
        }
    }

    #[test]
    fn pipeline_cache_metric_try_aggregation_rejects_overflow_without_panic() {
        let lhs = PipelineCacheMetrics {
            cached_bytes: u64::MAX,
            ..PipelineCacheMetrics::default()
        };
        let rhs = PipelineCacheMetrics {
            cached_bytes: 1,
            ..PipelineCacheMetrics::default()
        };

        let error = lhs
            .try_checked_add(rhs)
            .expect_err("Fix: fallible pipeline cache metric aggregation must reject overflow");
        assert_eq!(error.field(), "cached bytes");
        assert!(error.message().contains("Fix:"));
    }

    #[test]
    fn pipeline_cache_metric_compat_aggregation_saturates_on_overflow() {
        let lhs = PipelineCacheMetrics {
            cached_bytes: u64::MAX,
            hits: 41,
            ..PipelineCacheMetrics::default()
        };
        let rhs = PipelineCacheMetrics {
            cached_bytes: 1,
            hits: 1,
            ..PipelineCacheMetrics::default()
        };

        let metrics = lhs.checked_add(rhs);

        assert_eq!(metrics.cached_bytes, u64::MAX);
        assert_eq!(metrics.hits, 42);
    }

    #[test]
    fn pipeline_cache_counter_add_uses_checked_shared_arithmetic() {
        let counter = AtomicU64::new(41);

        PipelineCacheCounters::add(&counter, 1, "generated counter");

        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 42);
    }

    #[test]
    fn pipeline_cache_counter_try_add_rejects_overflow_without_panic() {
        let counter = AtomicU64::new(u64::MAX);

        let error = PipelineCacheCounters::try_add(&counter, 1, "generated counter")
            .expect_err("Fix: fallible counter add must reject overflow");

        assert_eq!(error.field(), "generated counter");
        assert!(error.message().contains("Fix:"));
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn pipeline_cache_counter_compat_add_saturates_on_overflow() {
        let counter = AtomicU64::new(u64::MAX);

        PipelineCacheCounters::add(&counter, 1, "generated counter");

        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn pipeline_cache_hit_rate_try_path_rejects_overflow_without_panic() {
        let metrics = PipelineCacheMetrics {
            lookups: 1,
            hits: u64::MAX,
            ..PipelineCacheMetrics::default()
        };

        let error = metrics
            .try_hit_rate_ppm()
            .expect_err("Fix: fallible hit-rate path must reject numerator overflow");

        assert_eq!(error.field(), "hit_rate_ppm");
        assert!(error.message().contains("Fix:"));
        assert_eq!(metrics.hit_rate_ppm(), u32::MAX);
    }
}
