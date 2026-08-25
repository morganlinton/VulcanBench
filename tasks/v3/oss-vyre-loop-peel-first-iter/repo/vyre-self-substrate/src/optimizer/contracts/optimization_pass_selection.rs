//! Benchmark-driven optimization pass selection.

use super::optimization_registry::{OptimizationPassExplanation, OptimizationRegistry};

/// Runtime workload statistics used to decide whether expensive passes are justified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptimizationWorkloadStats {
    /// Number of graph nodes or IR work items.
    pub nodes: u64,
    /// Number of dependency edges or fact transitions.
    pub edges: u64,
    /// Active frontier density in the inclusive range 0.0..=1.0.
    pub frontier_density: f32,
    /// Number of times this graph shape is expected to run.
    pub repeated_runs: u32,
    /// Estimated peak bytes required by the candidate plan.
    pub estimated_peak_bytes: u64,
    /// Bytes expected to cross device-to-host readback.
    pub readback_bytes: u64,
}

/// Select expensive optimization passes only when workload statistics justify them.
pub fn select_benchmark_driven_passes<'a>(
    registry: &'a OptimizationRegistry,
    stats: OptimizationWorkloadStats,
) -> Result<Vec<OptimizationPassExplanation<'static>>, String> {
    validate_stats(stats)?;
    let mut selected = Vec::new();
    select_benchmark_driven_passes_into(registry, stats, &mut selected)?;
    Ok(selected)
}

/// Select expensive optimization passes into caller-owned storage.
pub fn select_benchmark_driven_passes_into(
    registry: &OptimizationRegistry,
    stats: OptimizationWorkloadStats,
    selected: &mut Vec<OptimizationPassExplanation<'static>>,
) -> Result<(), String> {
    validate_stats(stats)?;
    selected.clear();
    if stats.readback_bytes >= 4096 {
        selected.push(registry.explain_pass_fire(
            "cuda.compact-read-ranges",
            "predicted readback is large enough to justify compaction",
            "transfer only requested output ranges back to the host",
        )?);
    }

    if stats.repeated_runs >= 2 && stats.edges >= 1024 {
        selected.push(registry.explain_pass_fire(
            "cuda.megakernel-plan-cache",
            "graph shape repeats enough times to amortize cache lookup",
            "reuse executable megakernel plan across repeated dispatch",
        )?);
    }

    if stats.estimated_peak_bytes >= 1 << 20 {
        selected.push(registry.explain_pass_fire(
            "cuda.megakernel-memory-budget",
            "estimated peak allocation exceeds one mebibyte",
            "bound device memory before launching the selected plan",
        )?);
    }

    if stats.nodes >= 1024 && stats.edges >= 4096 {
        selected.push(registry.explain_pass_fire(
            "cuda.megakernel-topology-select",
            "graph is large enough for topology choice to dominate launch overhead",
            "choose sparse dense hybrid or fused topology from measured graph statistics",
        )?);
    }

    if stats.frontier_density <= 0.03125 && stats.edges >= 4096 {
        selected.push(registry.explain_pass_fire(
            "cuda.warp-sparse-frontier",
            "frontier density is low enough that block-wide sparse dispatch wastes lanes",
            "route ultra-sparse active nodes through warp-specialized frontier execution",
        )?);
    }

    if stats.frontier_density <= 0.08 && stats.edges >= 4096 {
        selected.push(registry.explain_pass_fire(
            "cuda.sparse-frontier-queue",
            "frontier density is low enough that dense scans waste lanes",
            "drive traversal from device-side sparse active queue",
        )?);
    }

    if stats.repeated_runs >= 2 && stats.frontier_density <= 0.20 && stats.edges >= 4096 {
        selected.push(registry.explain_pass_fire(
            "cuda.resident-csr-queue-batch-api",
            "multiple sparse queries share one resident CSR topology",
            "submit all queue traversals as one resident sequence with one host fence",
        )?);
        selected.push(registry.explain_pass_fire(
            "cuda.resident-csr-batch-memory-plan",
            "sparse query batch may exceed resident scratch if allocated monolithically",
            "shard the batch from byte budget before allocating resident scratch",
        )?);
    }

    if stats.frontier_density >= 0.55 && stats.edges >= 4096 {
        if stats.frontier_density >= 0.85 {
            selected.push(registry.explain_pass_fire(
                "cuda.block-dense-frontier",
                "frontier density is high enough to amortize block-wide shared-memory propagation",
                "route dense active facts through block-specialized frontier execution",
            )?);
        }
        selected.push(registry.explain_pass_fire(
            "cuda.megakernel-barrier-min",
            "dense frontier makes global synchronization cost visible",
            "group independent waves to minimize global barriers",
        )?);
    }

    if stats.repeated_runs >= 2 && stats.edges >= 4096 {
        selected.push(registry.explain_pass_fire(
            "cuda.device-side-convergence",
            "iterative dataflow repeats enough work that host polling would dominate",
            "keep convergence detection device-side and read only the final changed flag",
        )?);
    }

    registry.validate_pass_order(selected.iter().map(|entry| entry.pass.id))?;
    Ok(())
}

fn validate_stats(stats: OptimizationWorkloadStats) -> Result<(), String> {
    if !(0.0..=1.0).contains(&stats.frontier_density) {
        return Err(format!(
            "frontier density {} is outside 0.0..=1.0. Fix: clamp or recompute telemetry before selecting passes.",
            stats.frontier_density
        ));
    }
    if stats.nodes == 0 && stats.edges != 0 {
        return Err(
            "optimization workload has edges but zero nodes. Fix: validate graph statistics before selection."
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_keeps_tiny_one_shot_workloads_empty() {
        let registry = OptimizationRegistry::with_release_builtins();
        let selected = select_benchmark_driven_passes(
            &registry,
            OptimizationWorkloadStats {
                nodes: 12,
                edges: 24,
                frontier_density: 0.25,
                repeated_runs: 1,
                estimated_peak_bytes: 2048,
                readback_bytes: 128,
            },
        )
        .expect("Fix: valid tiny workload should select");

        assert!(selected.is_empty());
    }

    #[test]
    fn selector_enables_cuda_sparse_repeated_hot_path() {
        let registry = OptimizationRegistry::with_release_builtins();
        let selected = select_benchmark_driven_passes(
            &registry,
            OptimizationWorkloadStats {
                nodes: 4096,
                edges: 32768,
                frontier_density: 0.02,
                repeated_runs: 64,
                estimated_peak_bytes: 8 << 20,
                readback_bytes: 65536,
            },
        )
        .expect("Fix: valid sparse workload should select");
        let ids: Vec<&str> = selected.iter().map(|entry| entry.pass.id).collect();

        assert!(ids.contains(&"cuda.megakernel-plan-cache"));
        assert!(ids.contains(&"cuda.megakernel-topology-select"));
        assert!(ids.contains(&"cuda.warp-sparse-frontier"));
        assert!(ids.contains(&"cuda.sparse-frontier-queue"));
        assert!(ids.contains(&"cuda.resident-csr-queue-batch-api"));
        assert!(ids.contains(&"cuda.resident-csr-batch-memory-plan"));
        assert!(ids.contains(&"cuda.device-side-convergence"));
        assert!(ids.contains(&"cuda.megakernel-memory-budget"));
        assert!(ids.contains(&"cuda.compact-read-ranges"));
        assert!(!ids.contains(&"cuda.megakernel-barrier-min"));
        registry
            .validate_pass_order(ids)
            .expect("Fix: selected sparse hot-path passes must satisfy registry order");
    }

    #[test]
    fn selector_reuses_caller_owned_pass_vector_and_validates_order() {
        let registry = OptimizationRegistry::with_release_builtins();
        let mut selected = Vec::with_capacity(16);
        let ptr = selected.as_ptr();

        select_benchmark_driven_passes_into(
            &registry,
            OptimizationWorkloadStats {
                nodes: 4096,
                edges: 32768,
                frontier_density: 0.02,
                repeated_runs: 64,
                estimated_peak_bytes: 8 << 20,
                readback_bytes: 65536,
            },
            &mut selected,
        )
        .expect("Fix: valid sparse workload should select into caller storage");

        assert_eq!(selected.as_ptr(), ptr);
        assert!(selected
            .iter()
            .any(|entry| entry.pass.id == "cuda.megakernel-plan-cache"));
        registry
            .validate_pass_order(selected.iter().map(|entry| entry.pass.id))
            .expect("Fix: selected pass order must already satisfy registry order");
    }

    #[test]
    fn selector_enables_dense_barrier_planning_without_sparse_queue() {
        let registry = OptimizationRegistry::with_release_builtins();
        let selected = select_benchmark_driven_passes(
            &registry,
            OptimizationWorkloadStats {
                nodes: 2048,
                edges: 8192,
                frontier_density: 0.90,
                repeated_runs: 1,
                estimated_peak_bytes: 2 << 20,
                readback_bytes: 2048,
            },
        )
        .expect("Fix: valid dense workload should select");
        let ids: Vec<&str> = selected.iter().map(|entry| entry.pass.id).collect();

        assert!(ids.contains(&"cuda.megakernel-topology-select"));
        assert!(ids.contains(&"cuda.megakernel-barrier-min"));
        assert!(ids.contains(&"cuda.block-dense-frontier"));
        assert!(ids.contains(&"cuda.megakernel-memory-budget"));
        assert!(!ids.contains(&"cuda.sparse-frontier-queue"));
    }

    #[test]
    fn selector_rejects_invalid_telemetry() {
        let registry = OptimizationRegistry::with_release_builtins();
        let err = select_benchmark_driven_passes(
            &registry,
            OptimizationWorkloadStats {
                nodes: 0,
                edges: 1,
                frontier_density: 1.2,
                repeated_runs: 1,
                estimated_peak_bytes: 0,
                readback_bytes: 0,
            },
        )
        .expect_err("invalid telemetry should be rejected");

        assert!(err.contains("frontier density"), "{err}");
    }
}
