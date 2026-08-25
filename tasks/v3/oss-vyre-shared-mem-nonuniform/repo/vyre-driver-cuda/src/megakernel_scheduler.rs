//! CUDA telemetry adapter for the scale-aware megakernel scheduler.

use vyre_driver::megakernel_execution::{
    plan_megakernel_execution, plan_megakernel_memory_budget, select_megakernel_topology,
    select_megakernel_topology_stable, MegakernelExecutionPlan, MegakernelExecutionSample,
    MegakernelExecutionTopology, MegakernelGraphShape, MegakernelMemoryBudget,
    MegakernelMemoryError, MegakernelMemoryPlan, MegakernelTopologyDecision,
};
use vyre_self_substrate::megakernel_schedule::{
    try_schedule_via_scale_aware_samples_into, MegakernelScaleSample, MegakernelScheduleError,
};

use crate::backend::CudaTelemetrySnapshot;

/// Per-candidate CUDA telemetry used to bias megakernel fusion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CudaMegakernelScheduleSample {
    /// Observed candidate dispatch cost in nanoseconds.
    pub dispatch_cost_ns: f64,
    /// Observed active-frontier density in `[0, 1]`.
    pub frontier_density: f64,
    /// Observed final readback byte volume.
    pub readback_bytes: u64,
}

/// Device-side megakernel execution topology selected for a dataflow wave.
pub type CudaMegakernelTopology = MegakernelExecutionTopology;

/// Static graph shape used by CUDA topology selection.
pub type CudaMegakernelGraphShape = MegakernelGraphShape;

/// Device memory envelope for a candidate CUDA megakernel plan.
pub type CudaMegakernelMemoryBudget = MegakernelMemoryBudget;

/// Detailed CUDA megakernel memory plan.
pub type CudaMegakernelMemoryPlan = MegakernelMemoryPlan;

/// Complete CUDA megakernel execution plan selected from runtime telemetry.
pub type CudaMegakernelExecutionPlan = MegakernelExecutionPlan;

/// Memory planning failure for CUDA megakernel execution.
pub type CudaMegakernelMemoryError = MegakernelMemoryError;

/// Topology decision with the pressure metrics that caused it.
pub type CudaMegakernelTopologyDecision = MegakernelTopologyDecision;

impl CudaMegakernelScheduleSample {
    /// Build one scheduler sample from an observed CUDA telemetry interval.
    ///
    /// `dispatch_cost_ns` is supplied by the caller because wall/device timing
    /// belongs to the benchmark or timed-dispatch boundary. Frontier density is
    /// derived from launched logical elements over scheduled CUDA thread slots,
    /// which is the runtime proxy available for arbitrary resident kernels.
    #[must_use]
    pub fn from_telemetry_snapshot(snapshot: CudaTelemetrySnapshot, dispatch_cost_ns: f64) -> Self {
        let frontier_density = f64::from(snapshot.logical_thread_utilization_bps) / 10_000.0;
        Self {
            dispatch_cost_ns,
            frontier_density,
            readback_bytes: snapshot.readback_bytes,
        }
    }

    fn execution_sample(self) -> MegakernelExecutionSample {
        MegakernelExecutionSample {
            dispatch_cost_ns: self.dispatch_cost_ns,
            frontier_density: self.frontier_density,
            readback_bytes: self.readback_bytes,
        }
    }
}

/// Select the CUDA megakernel execution topology for one candidate wave.
#[must_use]
pub fn select_cuda_megakernel_topology(
    sample: CudaMegakernelScheduleSample,
    graph: CudaMegakernelGraphShape,
    memory: CudaMegakernelMemoryBudget,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
) -> CudaMegakernelTopologyDecision {
    select_megakernel_topology(
        sample.execution_sample(),
        graph,
        memory,
        launch_overhead_ns,
        fusion_pressure,
    )
}

/// Select CUDA megakernel topology with previous-topology hysteresis.
#[must_use]
pub fn select_cuda_megakernel_topology_stable(
    sample: CudaMegakernelScheduleSample,
    graph: CudaMegakernelGraphShape,
    memory: CudaMegakernelMemoryBudget,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
    previous_topology: CudaMegakernelTopology,
) -> CudaMegakernelTopologyDecision {
    select_megakernel_topology_stable(
        sample.execution_sample(),
        graph,
        memory,
        launch_overhead_ns,
        fusion_pressure,
        previous_topology,
    )
}

/// Compute and validate a CUDA megakernel device-memory plan.
pub fn plan_cuda_megakernel_memory_budget(
    topology: CudaMegakernelTopology,
    graph: CudaMegakernelGraphShape,
    bytes_per_node: u64,
    bytes_per_edge: u64,
    frontier_bytes: u64,
    scratch_bytes: u64,
    output_bytes: u64,
    budget_bytes: u64,
) -> Result<CudaMegakernelMemoryPlan, CudaMegakernelMemoryError> {
    plan_megakernel_memory_budget(
        topology,
        graph,
        bytes_per_node,
        bytes_per_edge,
        frontier_bytes,
        scratch_bytes,
        output_bytes,
        budget_bytes,
    )
}

/// Select a CUDA megakernel topology and validate its device-memory plan.
pub fn plan_cuda_megakernel_execution(
    sample: CudaMegakernelScheduleSample,
    graph: CudaMegakernelGraphShape,
    bytes_per_node: u64,
    bytes_per_edge: u64,
    frontier_bytes: u64,
    scratch_bytes: u64,
    output_bytes: u64,
    budget_bytes: u64,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
) -> Result<CudaMegakernelExecutionPlan, CudaMegakernelMemoryError> {
    plan_megakernel_execution(
        sample.execution_sample(),
        graph,
        bytes_per_node,
        bytes_per_edge,
        frontier_bytes,
        scratch_bytes,
        output_bytes,
        budget_bytes,
        launch_overhead_ns,
        fusion_pressure,
    )
}

impl MegakernelScaleSample for CudaMegakernelScheduleSample {
    fn dispatch_cost_ns(&self) -> f64 {
        self.dispatch_cost_ns
    }

    fn frontier_density(&self) -> f64 {
        self.frontier_density
    }

    fn readback_bytes(&self) -> u64 {
        self.readback_bytes
    }
}

/// Schedule megakernel fusion pressure from CUDA telemetry samples.
pub fn schedule_megakernel_from_cuda_samples(
    samples: &[CudaMegakernelScheduleSample],
    launch_overhead_ns: f64,
    n_steps: u32,
    dt: f64,
) -> Result<Vec<f64>, MegakernelScheduleError> {
    let mut out = Vec::new();
    schedule_megakernel_from_cuda_samples_into(samples, launch_overhead_ns, n_steps, dt, &mut out)?;
    Ok(out)
}

/// Schedule megakernel fusion pressure into caller-owned output storage.
pub fn schedule_megakernel_from_cuda_samples_into(
    samples: &[CudaMegakernelScheduleSample],
    launch_overhead_ns: f64,
    n_steps: u32,
    dt: f64,
    out: &mut Vec<f64>,
) -> Result<(), MegakernelScheduleError> {
    try_schedule_via_scale_aware_samples_into(samples, launch_overhead_ns, n_steps, dt, out)
}

#[cfg(test)]
mod tests {
    use super::{
        plan_cuda_megakernel_execution, plan_cuda_megakernel_memory_budget,
        schedule_megakernel_from_cuda_samples_into, select_cuda_megakernel_topology,
        select_cuda_megakernel_topology_stable, CudaMegakernelGraphShape,
        CudaMegakernelMemoryBudget, CudaMegakernelMemoryError, CudaMegakernelScheduleSample,
        CudaMegakernelTopology,
    };
    use crate::backend::CudaTelemetrySnapshot;
    use vyre_self_substrate::megakernel_schedule::MegakernelScheduleError;

    #[test]
    fn megakernel_scheduler_is_cuda_adapter_not_planner_fork() {
        let source = include_str!("megakernel_scheduler.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: CUDA megakernel scheduler production source must be discoverable.");

        assert!(source.contains("use vyre_driver::megakernel_execution::{"));
        assert!(source.contains("plan_megakernel_execution("));
        assert!(source.contains("plan_megakernel_memory_budget("));
        assert!(source.contains("select_megakernel_topology("));
        assert!(source.contains("select_megakernel_topology_stable("));
        assert!(!production.contains("CudaArithmeticOverflow"));
        assert!(!production.contains("checked_add_u64_count"));
        assert!(!production.contains("checked_mul_u64_count"));
        assert!(!production.contains("fn pressure_bps("));
        assert!(!production.contains("fn topology_scratch_bytes("));
        assert!(!production.contains("enum CudaMegakernelTopology"));
        assert!(
            !source.contains(concat!("min(10_000)", " as u32")),
            "Fix: CUDA scheduler pressure export must use a checked conversion even after clamping."
        );
        assert!(
            !production.contains(".expect("),
            "Fix: CUDA megakernel scheduler production code must not panic after checked arithmetic."
        );
    }

    #[test]
    fn cuda_sample_adapter_reuses_output_capacity() {
        let samples = [
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 10.0,
                frontier_density: 0.0,
                readback_bytes: 0,
            },
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 20.0,
                frontier_density: 1.0,
                readback_bytes: 4096,
            },
        ];
        let mut out = Vec::with_capacity(4);
        let ptr = out.as_ptr();
        schedule_megakernel_from_cuda_samples_into(&samples, 5.0, 8, 0.25, &mut out)
            .expect("Fix: valid CUDA scheduler samples must schedule");
        assert_eq!(out.len(), 2);
        assert_eq!(out.as_ptr(), ptr);
        assert!(out[1] > out[0]);
    }

    #[test]
    fn cuda_sample_adapter_uses_runtime_telemetry_without_parallel_staging() {
        let sample = CudaMegakernelScheduleSample::from_telemetry_snapshot(
            CudaTelemetrySnapshot {
                readback_bytes: 4096,
                logical_thread_utilization_bps: 3750,
                ..CudaTelemetrySnapshot::default()
            },
            123.0,
        );

        assert_eq!(
            sample,
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 123.0,
                frontier_density: 0.375,
                readback_bytes: 4096,
            }
        );
    }

    #[test]
    fn topology_selector_prefers_sparse_for_low_density_or_memory_pressure() {
        let sample = CudaMegakernelScheduleSample {
            dispatch_cost_ns: 1_000.0,
            frontier_density: 0.01,
            readback_bytes: 1024,
        };
        let graph = CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 10_000,
        };
        let low_density = select_cuda_megakernel_topology(
            sample,
            graph,
            CudaMegakernelMemoryBudget {
                required_bytes: 1_000,
                budget_bytes: 10_000,
            },
            100.0,
            0.0,
        );
        assert_eq!(low_density.topology, CudaMegakernelTopology::SparseFrontier);

        let memory_red_zone = select_cuda_megakernel_topology(
            CudaMegakernelScheduleSample {
                frontier_density: 0.95,
                readback_bytes: 1 << 20,
                ..sample
            },
            graph,
            CudaMegakernelMemoryBudget {
                required_bytes: 95,
                budget_bytes: 100,
            },
            500.0,
            1.0,
        );
        assert_eq!(
            memory_red_zone.topology,
            CudaMegakernelTopology::SparseFrontier
        );
        assert_eq!(memory_red_zone.memory_pressure_bps, 9_500);
    }

    #[test]
    fn topology_selector_uses_warp_sparse_for_ultra_low_density() {
        let decision = select_cuda_megakernel_topology(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.01,
                readback_bytes: 256,
            },
            CudaMegakernelGraphShape {
                node_count: 1_000,
                edge_count: 4_000,
            },
            CudaMegakernelMemoryBudget {
                required_bytes: 1_000,
                budget_bytes: 10_000,
            },
            100.0,
            0.0,
        );

        assert_eq!(
            decision.topology,
            CudaMegakernelTopology::WarpSparseFrontier
        );
        assert_eq!(decision.average_degree_bps, 40_000);
        assert_eq!(
            decision.stable_explanation(),
            "megakernel-topology-v1|topology=WarpSparseFrontier|memory_pressure_bps=1000|average_degree_bps=40000|launch_pressure_bps=1000|reason=ultra_sparse_warp_specialized"
        );
    }

    #[test]
    fn topology_selector_uses_dense_hybrid_and_fused_bands() {
        let graph = CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let memory = CudaMegakernelMemoryBudget {
            required_bytes: 1_000,
            budget_bytes: 10_000,
        };
        let block_dense = select_cuda_megakernel_topology(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.90,
                readback_bytes: 512,
            },
            graph,
            memory,
            100.0,
            0.0,
        );
        assert_eq!(
            block_dense.topology,
            CudaMegakernelTopology::BlockDenseFrontier
        );

        let dense = select_cuda_megakernel_topology(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.75,
                readback_bytes: 512,
            },
            graph,
            memory,
            100.0,
            0.0,
        );
        assert_eq!(dense.topology, CudaMegakernelTopology::DenseFrontier);

        let hybrid = select_cuda_megakernel_topology(
            CudaMegakernelScheduleSample {
                frontier_density: 0.35,
                ..CudaMegakernelScheduleSample {
                    dispatch_cost_ns: 1_000.0,
                    frontier_density: 0.0,
                    readback_bytes: 512,
                }
            },
            graph,
            memory,
            100.0,
            0.0,
        );
        assert_eq!(hybrid.topology, CudaMegakernelTopology::HybridFrontier);

        let fused = select_cuda_megakernel_topology(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 1 << 20,
            },
            graph,
            memory,
            250.0,
            0.90,
        );
        assert_eq!(fused.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(fused.launch_pressure_bps, 2_500);
    }

    #[test]
    fn stable_topology_selector_prevents_cuda_variant_flapping_near_thresholds() {
        let graph = CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let memory = CudaMegakernelMemoryBudget {
            required_bytes: 1_000,
            budget_bytes: 10_000,
        };
        let sparse_to_hybrid = select_cuda_megakernel_topology_stable(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.14,
                readback_bytes: 512,
            },
            graph,
            memory,
            100.0,
            0.0,
            CudaMegakernelTopology::SparseFrontier,
        );
        assert_eq!(
            sparse_to_hybrid.topology,
            CudaMegakernelTopology::SparseFrontier
        );

        let dense_to_hybrid = select_cuda_megakernel_topology_stable(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.68,
                readback_bytes: 512,
            },
            graph,
            memory,
            100.0,
            0.0,
            CudaMegakernelTopology::DenseFrontier,
        );
        assert_eq!(
            dense_to_hybrid.topology,
            CudaMegakernelTopology::DenseFrontier
        );

        let fused_to_hybrid = select_cuda_megakernel_topology_stable(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 1 << 20,
            },
            graph,
            memory,
            130.0,
            0.65,
            CudaMegakernelTopology::FusedWave,
        );
        assert_eq!(fused_to_hybrid.topology, CudaMegakernelTopology::FusedWave);
    }

    #[test]
    fn memory_planner_bounds_peak_bytes_by_topology() {
        let graph = CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let plan = plan_cuda_megakernel_memory_budget(
            CudaMegakernelTopology::FusedWave,
            graph,
            16,
            8,
            4_096,
            2_048,
            512,
            128 * 1024,
        )
        .expect("Fix: valid fused plan should fit the explicit device-memory budget");

        assert_eq!(plan.graph_bytes, 48_000);
        assert_eq!(plan.scratch_bytes, 8_192);
        assert_eq!(plan.required_bytes, 60_800);
        assert!(plan.memory_pressure_bps > 0);
    }

    #[test]
    fn memory_planner_fails_loudly_when_budget_is_exceeded() {
        let graph = CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let err = plan_cuda_megakernel_memory_budget(
            CudaMegakernelTopology::DenseFrontier,
            graph,
            16,
            8,
            4_096,
            2_048,
            512,
            32 * 1024,
        )
        .expect_err("over-budget dense plan must fail before CUDA allocation");

        assert!(matches!(
            err,
            CudaMegakernelMemoryError::OverBudget {
                topology: CudaMegakernelTopology::DenseFrontier,
                ..
            }
        ));
        assert!(
            err.to_string().contains("Fix: choose a sparse topology"),
            "memory planner errors must be actionable: {err}"
        );
    }

    #[test]
    fn memory_planner_rejects_overflowing_graph_shapes() {
        let err = plan_cuda_megakernel_memory_budget(
            CudaMegakernelTopology::SparseFrontier,
            CudaMegakernelGraphShape {
                node_count: u64::MAX,
                edge_count: 0,
            },
            2,
            0,
            0,
            0,
            0,
            u64::MAX,
        )
        .expect_err("overflowing graph byte count must be rejected");
        assert!(matches!(
            err,
            CudaMegakernelMemoryError::ByteCountOverflow {
                field: "node layout bytes"
            }
        ));
    }

    #[test]
    fn topology_pressure_math_is_exact_for_u64_scale_inputs() {
        let decision = select_cuda_megakernel_topology(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.95,
                readback_bytes: u64::MAX,
            },
            CudaMegakernelGraphShape {
                node_count: 1_u64 << 60,
                edge_count: 1_u64 << 62,
            },
            CudaMegakernelMemoryBudget {
                required_bytes: 1_u64 << 62,
                budget_bytes: 1_u64 << 63,
            },
            250.0,
            0.0,
        );

        assert_eq!(decision.memory_pressure_bps, 5_000);
        assert_eq!(
            decision.average_degree_bps,
            (((u128::from(1_u64 << 62)) * 10_000) / u128::from(1_u64 << 60)) as u64
        );
    }

    #[test]
    fn execution_planner_selects_fused_when_budget_allows() {
        let plan = plan_cuda_megakernel_execution(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 1 << 20,
            },
            CudaMegakernelGraphShape {
                node_count: 1_000,
                edge_count: 4_000,
            },
            16,
            8,
            4_096,
            2_048,
            512,
            128 * 1024,
            250.0,
            0.90,
        )
        .expect("Fix: fused execution should fit this explicit device-memory budget");

        assert_eq!(plan.topology, CudaMegakernelTopology::FusedWave);
        assert!(!plan.downgraded_to_sparse);
        assert_eq!(plan.memory.scratch_bytes, 8_192);
    }

    #[test]
    fn execution_planner_downgrades_to_sparse_before_over_budget_failure() {
        let plan = plan_cuda_megakernel_execution(
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 1 << 20,
            },
            CudaMegakernelGraphShape {
                node_count: 1_000,
                edge_count: 4_000,
            },
            16,
            8,
            4_096,
            10_000,
            512,
            80_000,
            250.0,
            0.90,
        )
        .expect("Fix: sparse downgrade should fit even when fused topology exceeds the budget");

        assert_eq!(plan.topology, CudaMegakernelTopology::SparseFrontier);
        assert!(plan.downgraded_to_sparse);
        assert_eq!(plan.memory.scratch_bytes, 10_000);
    }

    #[test]
    fn cuda_sample_adapter_does_not_stage_parallel_vectors() {
        let src = include_str!("megakernel_scheduler.rs");
        assert!(
            !src.contains(concat!("let mut costs", " = Vec"))
                && !src.contains(concat!("let mut frontier_density", " = Vec"))
                && !src.contains(concat!("let mut readback_bytes", " = Vec")),
            "CUDA megakernel scheduler must consume native samples directly instead of allocating parallel staging vectors"
        );
    }

    #[test]
    fn cuda_sample_adapter_preserves_scheduler_validation_errors() {
        let samples = [CudaMegakernelScheduleSample {
            dispatch_cost_ns: 10.0,
            frontier_density: 1.5,
            readback_bytes: 0,
        }];
        let err = super::schedule_megakernel_from_cuda_samples(&samples, 0.0, 8, 0.25)
            .expect_err("invalid frontier density must be rejected");
        assert!(matches!(
            err,
            MegakernelScheduleError::InvalidFrontierDensity { index: 0, .. }
        ));
    }
}
