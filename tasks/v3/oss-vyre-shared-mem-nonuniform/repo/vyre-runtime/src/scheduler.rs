//! Multi-GPU work stealing scheduler (Innovation I.7).
//!
//! Partitions a large Program or batch of Programs across all
//! registered physical devices.

use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use vyre_driver::{BackendError, VyreBackend};

/// A unit of work assigned to one GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shard {
    /// Stable backend identifier for the GPU backend receiving this shard.
    pub backend_id: &'static str,
    /// Half-open byte/item range assigned to the backend.
    pub work_range: Range<usize>,
}

/// Dynamic work-stealing scheduler.
pub struct WorkStealingScheduler {
    backends: Vec<Arc<dyn VyreBackend>>,
    /// Atomic work index used by dispatch loops to let fast backends
    /// steal more fine-grained work units. Worker threads call
    /// [`Self::claim_next_unit`] which atomically increments the index;
    /// the returned value is the unit index they own. This is the
    /// work-stealing primitive  -  fast backends pull more units, slow
    /// backends pull fewer.
    work_index: AtomicUsize,
}

impl WorkStealingScheduler {
    /// Create a scheduler over the live runtime backends available to the process.
    pub fn new(backends: Vec<Arc<dyn VyreBackend>>) -> Self {
        Self {
            backends,
            work_index: AtomicUsize::new(0),
        }
    }

    /// Partition a large haystack across available GPUs.
    pub fn partition(&self, total_len: usize) -> Vec<Shard> {
        match self.try_partition(total_len) {
            Ok(shards) => shards,
            Err(_error) => Vec::new(),
        }
    }

    /// Partition a large haystack across available GPUs with explicit staging
    /// allocation failure reporting.
    pub fn try_partition(&self, total_len: usize) -> Result<Vec<Shard>, BackendError> {
        let mut shards = Vec::new();
        self.try_partition_into(total_len, &mut shards)?;
        Ok(shards)
    }

    /// Atomically claim the next fine-grained work unit. Worker threads
    /// call this in a loop; the returned value is the unit index they
    /// own. When the returned index is `>= num_units`, the worker is
    /// done. This is the work-stealing primitive: fast backends call
    /// `claim_next_unit` more times in the same wall-clock window.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_runtime::scheduler::WorkStealingScheduler;
    /// let scheduler = WorkStealingScheduler::new(Vec::new());
    /// assert_eq!(scheduler.claim_next_unit(), 0);
    /// assert_eq!(scheduler.claim_next_unit(), 1);
    /// scheduler.reset_unit_cursor();
    /// assert_eq!(scheduler.claim_next_unit(), 0);
    /// ```
    #[must_use]
    pub fn claim_next_unit(&self) -> usize {
        self.work_index.fetch_add(1, Ordering::AcqRel)
    }

    /// Reset the work-unit cursor to zero. Call between dispatches that
    /// reuse the same scheduler.
    pub fn reset_unit_cursor(&self) {
        self.work_index.store(0, Ordering::Release);
    }

    /// Partition a large haystack into many fine-grained work units
    /// assigned round-robin to backends. A caller-side dispatch loop
    /// uses [`Self::claim_next_unit`] to let worker threads atomically
    /// claim units so fast backends steal more work.
    pub fn partition_into(&self, total_len: usize, out: &mut Vec<Shard>) {
        // Clearing to empty on failure leaves NO work units — the dispatch loop
        // then claims nothing and the scan silently processes nothing (Law 10
        // recall loss). Fail loud; callers use try_partition_into.
        if let Err(error) = self.try_partition_into(total_len, out) {
            panic!("vyre-runtime work-unit partition failed: {error}");
        }
    }

    /// Partition into caller-owned storage with explicit staging allocation
    /// failure reporting.
    pub fn try_partition_into(
        &self,
        total_len: usize,
        out: &mut Vec<Shard>,
    ) -> Result<(), BackendError> {
        let n = self.backends.len();
        out.clear();
        if n == 0 || total_len == 0 {
            return Ok(());
        }
        let work_unit_size = partition_work_unit_size(total_len, n);
        let num_units = total_len.div_ceil(work_unit_size);
        vyre_foundation::allocation::try_reserve_vec_to_capacity(out, num_units).map_err(
            |error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: scheduler could not reserve {num_units} GPU work shard(s): {error}. Shard the workload before work-stealing partitioning."
                ),
            },
        )?;
        let mut start = 0;
        for i in 0..num_units {
            let end = (start + work_unit_size).min(total_len);
            out.push(Shard {
                backend_id: self.backends[i % n].id(),
                work_range: start..end,
            });
            start = end;
        }
        Ok(())
    }
}

fn partition_work_unit_size(total_len: usize, backend_count: usize) -> usize {
    if total_len == 0 || backend_count == 0 {
        return 1;
    }
    let denominator = backend_count.checked_mul(4).unwrap_or(usize::MAX);
    (total_len / denominator.max(1)).max(1)
}

#[cfg(test)]
fn partition_ranges(total_len: usize, backend_count: usize) -> Vec<Range<usize>> {
    if backend_count == 0 || total_len == 0 {
        return Vec::new();
    }
    let work_unit_size = partition_work_unit_size(total_len, backend_count);
    let num_units = total_len.div_ceil(work_unit_size);
    let mut ranges = Vec::with_capacity(num_units);
    let mut start = 0;
    for _ in 0..num_units {
        let end = (start + work_unit_size).min(total_len);
        ranges.push(start..end);
        start = end;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::{partition_ranges, WorkStealingScheduler};
    use std::sync::Arc;
    use vyre_driver::backend::{DispatchConfig, VyreBackend};
    use vyre_foundation::ir::Program;

    struct TestBackend(&'static str);

    impl vyre_driver::backend::private::Sealed for TestBackend {}

    impl VyreBackend for TestBackend {
        fn id(&self) -> &'static str {
            self.0
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn partition_ranges_produces_fine_grained_units() {
        let ranges = partition_ranges(10, 3);
        assert_eq!(ranges.len(), 10);
        assert_eq!(
            ranges,
            vec![0..1, 1..2, 2..3, 3..4, 4..5, 5..6, 6..7, 7..8, 8..9, 9..10]
        );
    }

    #[test]
    fn partition_ranges_never_emits_empty_shards() {
        let ranges = partition_ranges(2, 8);
        assert_eq!(ranges, vec![0..1, 1..2]);
    }

    #[test]
    fn partition_ranges_uses_overflow_safe_work_unit_math() {
        let ranges = partition_ranges(2, usize::MAX);
        assert_eq!(ranges[0], 0..1);
        assert_eq!(ranges[1], 1..2);
        assert_eq!(
            super::partition_work_unit_size(2, usize::MAX),
            1,
            "backend_count * 4 overflow must not panic or enlarge the work unit"
        );
    }

    #[test]
    fn scheduler_partition_into_reuses_output_storage() {
        let scheduler = WorkStealingScheduler::new(vec![
            Arc::new(TestBackend("a")),
            Arc::new(TestBackend("b")),
            Arc::new(TestBackend("c")),
        ]);
        let mut shards = Vec::with_capacity(10);

        scheduler.partition_into(10, &mut shards);
        let ptr = shards.as_ptr();
        scheduler.partition_into(10, &mut shards);

        assert_eq!(shards.as_ptr(), ptr);
        assert_eq!(shards.len(), 10);
        assert_eq!(shards[0].backend_id, "a");
        assert_eq!(shards[0].work_range, 0..1);
        assert_eq!(shards[1].backend_id, "b");
        assert_eq!(shards[1].work_range, 1..2);
        assert_eq!(shards[9].backend_id, "a");
        assert_eq!(shards[9].work_range, 9..10);
        assert_eq!(
            scheduler
                .work_index
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }
}
