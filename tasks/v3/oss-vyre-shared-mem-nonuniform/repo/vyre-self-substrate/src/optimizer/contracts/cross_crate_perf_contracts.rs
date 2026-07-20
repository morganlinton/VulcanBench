//! Cross-crate performance contracts for release-path integrations.

use std::collections::BTreeSet;

use super::optimization_registry::OptimizationRegistry;

/// A required performance dependency between producer and consumer crates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrossCratePerfContract {
    /// Contract identifier used by release gates.
    pub id: &'static str,
    /// Crate or subsystem that produces the workload.
    pub producer: &'static str,
    /// Crate or subsystem that must preserve the optimization.
    pub consumer: &'static str,
    /// Required registered optimization pass.
    pub required_pass_id: &'static str,
    /// Observable condition that proves the contract still matters.
    pub trigger: &'static str,
    /// Test or benchmark that owns the contract.
    pub gate: &'static str,
}

/// Release contracts preventing upstream crate changes from disabling runtime wins.
pub const RELEASE_CROSS_CRATE_PERF_CONTRACTS: &[CrossCratePerfContract] = &[
    CrossCratePerfContract {
        id: "vyrec-token-stream-keeps-compact-readback",
        producer: "vyrec",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.compact-read-ranges",
        trigger: "diagnostics or frontend artifacts read partial GPU outputs",
        gate: "preprocessor gpu parity",
    },
    CrossCratePerfContract {
        id: "vyrec-small-results-keep-result-compaction",
        producer: "vyrec",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.result-compaction",
        trigger: "frontend diagnostics or summaries produce small sparse outputs",
        gate: "result_compaction",
    },
    CrossCratePerfContract {
        id: "vyrec-diagnostics-keep-device-aggregation",
        producer: "vyrec",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.device-diagnostic-aggregation",
        trigger: "frontend diagnostics originate from large token or fact streams",
        gate: "device_diagnostic_aggregation",
    },
    CrossCratePerfContract {
        id: "frontend-and-dataflow-keep-unified-token-fact-graph",
        producer: "vyrec/dataflow",
        consumer: "vyre-self",
        required_pass_id: "struct.device-resident-token-fact-graph",
        trigger:
            "parser tokens, semantic facts, diagnostics, and dataflow facts share CUDA residency",
        gate: "device_resident_token_fact_graph",
    },
    CrossCratePerfContract {
        id: "vyrec-token-fact-graph-keeps-cuda-layout-adapter",
        producer: "vyrec",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.token-fact-graph-layout-adapter",
        trigger: "unified token/fact graph reaches CUDA megakernel scheduling",
        gate: "token_fact_graph_cuda_adapter",
    },
    CrossCratePerfContract {
        id: "vyrec-frontier-graph-keeps-cuda-execution-planner",
        producer: "vyrec",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.token-fact-frontier-execution",
        trigger: "frontier-typed token/fact graph needs dependency-aware CUDA megakernel execution",
        gate: "token_fact_frontier_execution",
    },
    CrossCratePerfContract {
        id: "vyrec-predicate-frontiers-keep-branch-compaction",
        producer: "vyrec",
        consumer: "vyre-self",
        required_pass_id: "struct.branch-compaction",
        trigger: "parser or semantic predicate frontiers contain empty branch arms",
        gate: "branch_compaction",
    },
    CrossCratePerfContract {
        id: "vyrec-corpus-batch-keeps-plan-cache",
        producer: "vyrec",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.megakernel-plan-cache",
        trigger: "translation units reuse token and semantic graph shapes",
        gate: "linux corpus parity",
    },
    CrossCratePerfContract {
        id: "ifds-keeps-resident-csr-queue",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.resident-csr-queue-api",
        trigger: "IFDS solves reuse exploded-supergraph CSR topology",
        gate: "ifds_direct_resident_structure",
    },
    CrossCratePerfContract {
        id: "dataflow-batch-analysis-keeps-resident-csr-queue-batch",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.resident-csr-queue-batch-api",
        trigger: "many analyses or seeds traverse the same resident CSR topology",
        gate: "csr_frontier_queue_gpu_parity",
    },
    CrossCratePerfContract {
        id: "dataflow-multi-analysis-keeps-cuda-multi-query",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.multi-query-resident-execution",
        trigger: "many analyses execute over one resident graph",
        gate: "multi_query_execution",
    },
    CrossCratePerfContract {
        id: "dataflow-batch-analysis-keeps-csr-batch-memory-plan",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.resident-csr-batch-memory-plan",
        trigger:
            "large query batches need deterministic sharding before resident scratch allocation",
        gate: "csr_frontier_queue_batch_resident",
    },
    CrossCratePerfContract {
        id: "sparse-facts-keep-frontier-queue",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.sparse-frontier-queue",
        trigger: "low-density facts should not execute dense scans",
        gate: "csr_frontier_queue_gpu_parity",
    },
    CrossCratePerfContract {
        id: "dataflow-low-density-facts-keep-bitset-compression",
        producer: "dataflow",
        consumer: "vyre-self",
        required_pass_id: "struct.bitset-compression",
        trigger: "low-density dataflow facts should not move full dense bitsets",
        gate: "bitset_compression",
    },
    CrossCratePerfContract {
        id: "dataflow-ultra-sparse-facts-keep-warp-sparse-frontier",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.warp-sparse-frontier",
        trigger: "ultra-low-density facts should not burn block-wide sparse work",
        gate: "megakernel_scheduler",
    },
    CrossCratePerfContract {
        id: "dataflow-dense-facts-keep-block-dense-frontier",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.block-dense-frontier",
        trigger: "high-density facts should use block-coalesced propagation",
        gate: "megakernel_scheduler",
    },
    CrossCratePerfContract {
        id: "dataflow-shared-facts-keep-frontier-partitioning",
        producer: "dataflow",
        consumer: "vyre-self",
        required_pass_id: "struct.frontier-partitioning",
        trigger: "many facts update shared graph nodes before CUDA frontier execution",
        gate: "frontier_partitioning",
    },
    CrossCratePerfContract {
        id: "dataflow-repeated-analysis-keeps-fixed-graph-replay",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.resident-graph-session",
        trigger: "same resident graph is evaluated repeatedly",
        gate: "resident_graph_session",
    },
    CrossCratePerfContract {
        id: "dataflow-iterative-analysis-keeps-device-convergence",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.device-side-convergence",
        trigger: "iterative dataflow exposes a changed flag",
        gate: "megakernel_convergence",
    },
    CrossCratePerfContract {
        id: "dataflow-dependent-dataflow-keeps-device-work-queue",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.device-work-queue",
        trigger: "dependent dataflow schedules work discovered by previous device work",
        gate: "device_work_queue",
    },
    CrossCratePerfContract {
        id: "dataflow-compatible-stages-keep-launch-fusion",
        producer: "dataflow",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.adjacent-launch-fusion",
        trigger: "adjacent dataflow stages share a resident memory layout",
        gate: "launch_fusion",
    },
    CrossCratePerfContract {
        id: "vyre-pipeline-keeps-memory-budget",
        producer: "vyre-driver",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.megakernel-memory-budget",
        trigger: "pipeline schedules megakernel execution under memory pressure",
        gate: "megakernel_scheduler",
    },
    CrossCratePerfContract {
        id: "vyre-release-claims-keep-speedup-gate",
        producer: "release",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.megakernel-speedup-gate",
        trigger: "release claims resident megakernel 100x to 1000x speedup",
        gate: "megakernel_speedup_gate",
    },
    CrossCratePerfContract {
        id: "vyre-launch-errors-keep-kernel-diagnostics",
        producer: "vyre-driver",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.kernel-failure-diagnostics",
        trigger: "selected CUDA kernel requires features or limits that may be unavailable",
        gate: "kernel_failure_diagnostics",
    },
    CrossCratePerfContract {
        id: "vyre-pipeline-keeps-barrier-planner",
        producer: "vyre-driver",
        consumer: "vyre-cuda",
        required_pass_id: "cuda.megakernel-barrier-min",
        trigger: "frontier waves have independent dependency groups",
        gate: "megakernel_barrier_planner",
    },
    CrossCratePerfContract {
        id: "dataflow-layout-keeps-normalization",
        producer: "dataflow",
        consumer: "vyre-self",
        required_pass_id: "dataflow.graph-normalization",
        trigger: "equivalent graphs should hit stable layout caches",
        gate: "fixed_point_graph",
    },
];

/// Validate release contracts against the registered optimization universe.
pub fn validate_release_cross_crate_perf_contracts(
    registry: &OptimizationRegistry,
) -> Result<(), String> {
    let mut ids = BTreeSet::new();

    for contract in RELEASE_CROSS_CRATE_PERF_CONTRACTS {
        validate_contract(*contract)?;
        if !ids.insert(contract.id) {
            return Err(format!(
                "duplicate cross-crate perf contract `{}`. Fix: keep one owner for each contract.",
                contract.id
            ));
        }
        registry.get(contract.required_pass_id).ok_or_else(|| {
            format!(
                "cross-crate perf contract `{}` references unknown optimization pass `{}`. Fix: register the required pass before gating it.",
                contract.id, contract.required_pass_id
            )
        })?;
    }

    Ok(())
}

/// Validate that selected passes include every required pass for the triggered contracts.
pub fn validate_triggered_contract_selection<'a, I>(
    triggered_contract_ids: I,
    selected_pass_ids: &[&str],
) -> Result<(), String>
where
    I: IntoIterator<Item = &'a str>,
{
    let selected: BTreeSet<&str> = selected_pass_ids.iter().copied().collect();

    for contract_id in triggered_contract_ids {
        let contract = RELEASE_CROSS_CRATE_PERF_CONTRACTS
            .iter()
            .find(|contract| contract.id == contract_id)
            .ok_or_else(|| {
                format!(
                    "unknown cross-crate perf contract `{contract_id}`. Fix: register the contract before gating selection."
                )
            })?;

        if !selected.contains(contract.required_pass_id) {
            return Err(format!(
                "cross-crate perf contract `{}` requires pass `{}` for {} -> {}. Fix: keep the optimization selected when trigger `{}` is present.",
                contract.id,
                contract.required_pass_id,
                contract.producer,
                contract.consumer,
                contract.trigger
            ));
        }
    }

    Ok(())
}

fn validate_contract(contract: CrossCratePerfContract) -> Result<(), String> {
    for (field, value) in [
        ("id", contract.id),
        ("producer", contract.producer),
        ("consumer", contract.consumer),
        ("required_pass_id", contract.required_pass_id),
        ("trigger", contract.trigger),
        ("gate", contract.gate),
    ] {
        if value.trim().is_empty() {
            return Err(format!(
                "cross-crate perf contract `{}` has empty {field}. Fix: every contract needs producer, consumer, required pass, trigger, and gate.",
                contract.id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_cross_crate_contracts_reference_registered_passes() {
        let registry = OptimizationRegistry::with_release_builtins();

        validate_release_cross_crate_perf_contracts(&registry)
            .expect("Fix: release contracts must reference registered optimization passes");
    }

    #[test]
    fn triggered_contract_selection_accepts_required_passes() {
        validate_triggered_contract_selection(
            [
                "vyrec-token-stream-keeps-compact-readback",
                "sparse-facts-keep-frontier-queue",
            ],
            &["cuda.compact-read-ranges", "cuda.sparse-frontier-queue"],
        )
        .expect("Fix: selected passes satisfy triggered contracts");
    }

    #[test]
    fn triggered_contract_selection_rejects_missing_runtime_pass() {
        let err = validate_triggered_contract_selection(
            ["ifds-keeps-resident-csr-queue"],
            &["cuda.sparse-frontier-queue"],
        )
        .expect_err("missing resident CSR queue pass should fail the contract");

        assert!(err.contains("cuda.resident-csr-queue-api"), "{err}");
        assert!(err.contains("dataflow -> vyre-cuda"), "{err}");
    }

    #[test]
    fn triggered_contract_selection_rejects_unknown_contract() {
        let err = validate_triggered_contract_selection(["not-registered"], &[])
            .expect_err("unknown contract should fail loudly");

        assert!(err.contains("unknown cross-crate perf contract"), "{err}");
    }
}
