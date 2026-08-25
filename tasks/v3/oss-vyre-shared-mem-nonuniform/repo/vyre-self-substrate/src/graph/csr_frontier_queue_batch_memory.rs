//! Memory planning for resident CSR frontier-queue batches.

use super::csr_frontier_queue_scratch::resident_csr_queue_scratch_bytes_per_query;

/// Memory plan for sharding resident CSR queue batches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentCsrQueueBatchMemoryPlan {
    /// Number of input queries.
    pub query_count: usize,
    /// Largest query count used by any resident dispatch chunk in the plan.
    pub max_queries_per_dispatch: usize,
    /// Number of dispatch batches required.
    pub dispatch_batches: usize,
    /// Peak resident scratch bytes required by one query in the plan.
    pub bytes_per_query: usize,
    /// Peak resident scratch bytes for any planned dispatch batch.
    pub peak_batch_scratch_bytes: usize,
}

/// Errors produced while planning resident CSR queue batch memory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResidentCsrQueueBatchMemoryPlanError {
    /// No queries were requested.
    EmptyBatch,
    /// Queue capacity was zero.
    EmptyQueueCapacity,
    /// Arithmetic overflow occurred while computing byte requirements.
    ScratchBytesOverflow,
    /// Memory budget cannot fit even one query.
    BudgetTooSmall {
        /// Resident scratch bytes required by one query.
        bytes_per_query: usize,
        /// Caller-provided maximum resident scratch bytes.
        max_scratch_bytes: usize,
    },
}

impl std::fmt::Display for ResidentCsrQueueBatchMemoryPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyBatch => f.write_str(
                "resident CSR queue batch has zero queries. Fix: skip dispatch or pass at least one frontier.",
            ),
            Self::EmptyQueueCapacity => f.write_str(
                "resident CSR queue batch has zero queue capacity. Fix: allocate at least one active node slot.",
            ),
            Self::ScratchBytesOverflow => f.write_str(
                "resident CSR queue batch scratch byte calculation overflowed. Fix: shard the query batch before planning.",
            ),
            Self::BudgetTooSmall {
                bytes_per_query,
                max_scratch_bytes,
            } => write!(
                f,
                "resident CSR queue batch needs {bytes_per_query} scratch bytes per query but budget allows {max_scratch_bytes}. Fix: increase max_scratch_bytes or use a smaller graph shard."
            ),
        }
    }
}

impl std::error::Error for ResidentCsrQueueBatchMemoryPlanError {}

/// Plan query sharding for resident CSR queue batch execution.
pub fn plan_resident_csr_queue_batch_memory(
    query_count: usize,
    frontier_words: usize,
    queue_capacity: u32,
    max_scratch_bytes: usize,
) -> Result<ResidentCsrQueueBatchMemoryPlan, ResidentCsrQueueBatchMemoryPlanError> {
    if query_count == 0 {
        return Err(ResidentCsrQueueBatchMemoryPlanError::EmptyBatch);
    }
    if queue_capacity == 0 {
        return Err(ResidentCsrQueueBatchMemoryPlanError::EmptyQueueCapacity);
    }
    let bytes_per_query =
        resident_csr_queue_scratch_bytes_per_query(frontier_words, queue_capacity)
            .map_err(|_| ResidentCsrQueueBatchMemoryPlanError::ScratchBytesOverflow)?;
    if bytes_per_query > max_scratch_bytes {
        return Err(ResidentCsrQueueBatchMemoryPlanError::BudgetTooSmall {
            bytes_per_query,
            max_scratch_bytes,
        });
    }
    let max_queries_per_dispatch = query_count.min((max_scratch_bytes / bytes_per_query).max(1));
    let dispatch_batches = query_count.div_ceil(max_queries_per_dispatch);
    let peak_queries = query_count.min(max_queries_per_dispatch);
    let peak_batch_scratch_bytes = bytes_per_query
        .checked_mul(peak_queries)
        .ok_or(ResidentCsrQueueBatchMemoryPlanError::ScratchBytesOverflow)?;

    Ok(ResidentCsrQueueBatchMemoryPlan {
        query_count,
        max_queries_per_dispatch,
        dispatch_batches,
        bytes_per_query,
        peak_batch_scratch_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_bytes_per_query(frontier_words: usize, queue_capacity: u32) -> Option<usize> {
        resident_csr_queue_scratch_bytes_per_query(frontier_words, queue_capacity).ok()
    }

    fn next_lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed
    }

    #[test]
    fn memory_plan_keeps_batch_under_budget() {
        let plan = plan_resident_csr_queue_batch_memory(10, 2, 8, 104)
            .expect("Fix: two-query budget should plan five dispatch batches");

        assert_eq!(plan.query_count, 10);
        assert_eq!(plan.bytes_per_query, 52);
        assert_eq!(plan.max_queries_per_dispatch, 2);
        assert_eq!(plan.dispatch_batches, 5);
        assert_eq!(plan.peak_batch_scratch_bytes, 104);
    }

    #[test]
    fn memory_plan_uses_one_dispatch_when_budget_allows() {
        let plan = plan_resident_csr_queue_batch_memory(3, 1, 8, 1024)
            .expect("Fix: full batch should fit");

        assert_eq!(plan.bytes_per_query, 44);
        assert_eq!(plan.max_queries_per_dispatch, 3);
        assert_eq!(plan.dispatch_batches, 1);
        assert_eq!(plan.peak_batch_scratch_bytes, 132);
    }

    #[test]
    fn memory_plan_accounts_for_word_prefix_scratch_on_large_frontiers() {
        let plan = plan_resident_csr_queue_batch_memory(2, 256, 8, 12_368)
            .expect("Fix: two large-frontier queries should fit exact word-prefix budget");

        assert_eq!(plan.bytes_per_query, 6_184);
        assert_eq!(plan.max_queries_per_dispatch, 2);
        assert_eq!(plan.dispatch_batches, 1);
        assert_eq!(plan.peak_batch_scratch_bytes, 12_368);
    }

    #[test]
    fn memory_plan_rejects_unfit_or_empty_batches() {
        assert_eq!(
            plan_resident_csr_queue_batch_memory(0, 1, 8, 128)
                .expect_err("empty query batch should fail"),
            ResidentCsrQueueBatchMemoryPlanError::EmptyBatch
        );
        assert_eq!(
            plan_resident_csr_queue_batch_memory(1, 1, 0, 128)
                .expect_err("empty queue capacity should fail"),
            ResidentCsrQueueBatchMemoryPlanError::EmptyQueueCapacity
        );
        assert_eq!(
            plan_resident_csr_queue_batch_memory(1, 2, 8, 51)
                .expect_err("budget smaller than one query should fail"),
            ResidentCsrQueueBatchMemoryPlanError::BudgetTooSmall {
                bytes_per_query: 52,
                max_scratch_bytes: 51,
            }
        );
    }

    #[test]
    fn memory_plan_holds_adversarial_invariants_across_generated_inputs() {
        let mut seed = 0x51_53_45_e2_b4_9f_c3_aa_u64;
        let mut any_ok = 0usize;
        let mut any_err_budget = 0usize;
        let mut any_overflow = 0usize;

        for _ in 0..4096usize {
            seed = next_lcg(&mut seed);
            let q = 1usize + (seed as usize % 10_000);
            let frontier_words = ((seed >> 24) as usize) % 2_048;
            let queue_capacity = (((seed >> 12) as u32) % 4_096) + 1;
            seed = next_lcg(&mut seed);
            let raw_scratch = ((seed % 0x2000_0000) as usize) + 1;

            let expected = expected_bytes_per_query(frontier_words, queue_capacity);
            let (computed, expected) = match expected {
                Some(expected) => (
                    plan_resident_csr_queue_batch_memory(
                        q,
                        frontier_words,
                        queue_capacity,
                        raw_scratch,
                    ),
                    Some(expected),
                ),
                None => (
                    Err(ResidentCsrQueueBatchMemoryPlanError::ScratchBytesOverflow),
                    None,
                ),
            };

            if let (Ok(plan), Some(bytes_per_query)) = (computed.as_ref(), expected) {
                any_ok += 1;
                assert_eq!(plan.query_count, q, "query_count preserved across plans");
                assert!(plan.max_queries_per_dispatch >= 1);
                assert!(plan.max_queries_per_dispatch <= q);
                assert_eq!(
                    plan.dispatch_batches,
                    q.div_ceil(plan.max_queries_per_dispatch)
                );
                assert_eq!(plan.bytes_per_query, bytes_per_query);
                assert_eq!(
                    plan.peak_batch_scratch_bytes,
                    bytes_per_query
                        .checked_mul(plan.max_queries_per_dispatch.min(q))
                        .expect("Fix: successful planning should not overflow peak scratch"),
                );
                assert!(
                    plan.peak_batch_scratch_bytes <= raw_scratch,
                    "successful plan must fit scratch budget"
                );
            } else if matches!(
                computed,
                Err(ResidentCsrQueueBatchMemoryPlanError::BudgetTooSmall { .. })
            ) {
                any_err_budget += 1;
            } else if matches!(
                computed,
                Err(ResidentCsrQueueBatchMemoryPlanError::ScratchBytesOverflow)
            ) {
                any_overflow += 1;
            }
        }

        assert!(any_ok + any_err_budget + any_overflow >= 1_024);
    }
}
