//! CUDA adapter for backend-neutral multi-query execution planning.

use vyre_driver::multi_query_execution::{
    plan_multi_query_execution, plan_multi_query_execution_with_scratch, MultiQuery,
    MultiQueryExecutionError, MultiQueryExecutionPlan, MultiQueryExecutionScratch, MultiQueryGroup,
};

/// One CUDA analysis/query planned against a resident graph.
pub type CudaMultiQuery = MultiQuery;
/// One grouped CUDA multi-query launch envelope.
pub type CudaMultiQueryGroup = MultiQueryGroup;
/// Complete CUDA multi-query execution plan.
pub type CudaMultiQueryExecutionPlan = MultiQueryExecutionPlan;
/// Caller-owned scratch for repeated CUDA multi-query planning.
pub type CudaMultiQueryExecutionScratch = MultiQueryExecutionScratch;
/// CUDA multi-query planning errors.
pub type CudaMultiQueryExecutionError = MultiQueryExecutionError;

/// Plan CUDA multi-query execution over shared resident graphs.
pub fn plan_cuda_multi_query_execution(
    queries: &[CudaMultiQuery],
    budget_bytes: u64,
) -> Result<CudaMultiQueryExecutionPlan, CudaMultiQueryExecutionError> {
    plan_multi_query_execution(queries, budget_bytes)
}

/// Plan CUDA multi-query execution using caller-owned planning scratch.
pub fn plan_cuda_multi_query_execution_with_scratch(
    queries: &[CudaMultiQuery],
    budget_bytes: u64,
    scratch: &mut CudaMultiQueryExecutionScratch,
) -> Result<CudaMultiQueryExecutionPlan, CudaMultiQueryExecutionError> {
    plan_multi_query_execution_with_scratch(queries, budget_bytes, scratch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_driver::ResidentGraphReuseTelemetry;

    #[test]
    fn cuda_multi_query_execution_is_adapter_not_algorithm_fork() {
        let source = include_str!("multi_query_execution.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: CUDA multi-query adapter source must contain production section");

        assert!(source.contains("use vyre_driver::multi_query_execution::{"));
        assert!(source.contains("pub type CudaMultiQuery = MultiQuery;"));
        assert!(source.contains("plan_multi_query_execution(queries, budget_bytes)"));
        assert!(source
            .contains("plan_multi_query_execution_with_scratch(queries, budget_bytes, scratch)"));
        assert!(!production.contains("FxHashMap"));
        assert!(!production.contains("FxHashSet"));
        assert!(!production.contains("sort_unstable_by_key_if_needed"));
        assert!(!production.contains("fn append_memory_fit_groups"));
        assert!(!production.contains("fn group_resident_bytes"));
    }

    #[test]
    fn cuda_multi_query_adapter_preserves_shared_batching_contract() {
        let plan = plan_cuda_multi_query_execution(
            &[
                query(3, 0xabc, 0x10, 4_096, 64, 128, 32),
                query(1, 0xabc, 0x10, 4_096, 32, 64, 16),
                query(2, 0xabc, 0x10, 4_096, 48, 96, 24),
            ],
            8_192,
        )
        .expect("Fix: compatible CUDA queries should batch through the shared planner");

        assert_eq!(plan.launch_count, 1);
        assert_eq!(plan.avoided_launches, 2);
        assert_eq!(plan.avoided_host_fences, 2);
        assert_eq!(plan.avoided_graph_upload_bytes, 8_192);
        assert_eq!(
            plan.graph_reuse,
            ResidentGraphReuseTelemetry::from_counters(1, 2, 4_096, 8_192)
        );
        assert_eq!(plan.groups[0].queries, vec![1, 2, 3]);
    }

    #[test]
    fn cuda_multi_query_adapter_reuses_shared_scratch() {
        let mut scratch = CudaMultiQueryExecutionScratch::try_with_capacity(64)
            .expect("Fix: CUDA multi-query scratch should reserve through shared planner");
        let queries = (0..64)
            .map(|index| query(index, 0xabc, 0x10, 4_096, 4, 8, 4))
            .collect::<Vec<_>>();

        let plan = plan_cuda_multi_query_execution_with_scratch(&queries, 16_384, &mut scratch)
            .expect("Fix: CUDA multi-query scratch adapter should route to shared planner");

        assert_eq!(plan.launch_count, 1);
        assert_eq!(plan.groups[0].queries.len(), 64);
        assert!(scratch.group_index_capacity() >= 64);
        assert!(scratch.retained_query_bucket_capacity() >= 64);
    }

    fn query(
        query: u32,
        graph_layout_hash: u64,
        traversal_key: u64,
        graph_upload_bytes: u64,
        frontier_bytes: u64,
        scratch_bytes: u64,
        output_bytes: u64,
    ) -> CudaMultiQuery {
        CudaMultiQuery {
            query,
            graph_layout_hash,
            traversal_key,
            graph_upload_bytes,
            frontier_bytes,
            scratch_bytes,
            output_bytes,
        }
    }
}
