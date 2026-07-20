//! CUDA megakernel barrier planning for dependency-typed dataflow waves.
//!
//! The planner is pure and deterministic: it converts a wave dependency DAG
//! into the minimum number of global-synchronization layers implied by those
//! dependencies. Waves inside one layer are independent and can be fused into
//! one cooperative megakernel phase without inserting a host-side barrier.

use crate::backend::accounting::{
    checked_add_u64_count as checked_add, checked_mul_u64_count as checked_mul,
    CudaArithmeticOverflow,
};
use crate::backend::staging_reserve::CudaStorageReserveFailure;
use crate::megakernel_plan_cache::{
    CudaMegakernelAnalysisKind, CudaMegakernelDeviceKey, CudaMegakernelPlanCache,
};
use crate::megakernel_scheduler::{
    CudaMegakernelExecutionPlan, CudaMegakernelGraphShape, CudaMegakernelMemoryError,
    CudaMegakernelScheduleSample,
};
use vyre_driver::megakernel_barrier::{
    plan_megakernel_barriers, plan_megakernel_barriers_with_scratch, MegakernelBarrierGroup,
    MegakernelBarrierPlan, MegakernelBarrierPlanError, MegakernelBarrierScratch,
    MegakernelWaveDependency,
};
use vyre_driver::megakernel_frontier::{
    plan_megakernel_frontier_memory_with_scratch, MegakernelFrontierMemoryPlanError,
    MegakernelFrontierWave,
};

/// Directed dependency between two CUDA megakernel dataflow waves.
pub type CudaMegakernelWaveDependency = MegakernelWaveDependency;

/// One barrier-free group of independent CUDA megakernel waves.
pub type CudaMegakernelBarrierGroup = MegakernelBarrierGroup;

/// Barrier plan for CUDA megakernel execution.
pub type CudaMegakernelBarrierPlan = MegakernelBarrierPlan;

/// Caller-owned scratch for repeated CUDA megakernel barrier planning.
pub type CudaMegakernelBarrierScratch = MegakernelBarrierScratch;

/// Barrier planning failure.
pub type CudaMegakernelBarrierPlanError = MegakernelBarrierPlanError;

/// Plan minimum global barriers for a CUDA megakernel wave dependency DAG.
pub fn plan_cuda_megakernel_barriers(
    wave_count: usize,
    dependencies: &[CudaMegakernelWaveDependency],
) -> Result<CudaMegakernelBarrierPlan, CudaMegakernelBarrierPlanError> {
    plan_megakernel_barriers(wave_count, dependencies)
}

/// Plan minimum global barriers using caller-owned temporary storage.
pub fn plan_cuda_megakernel_barriers_with_scratch(
    wave_count: usize,
    dependencies: &[CudaMegakernelWaveDependency],
    scratch: &mut CudaMegakernelBarrierScratch,
) -> Result<CudaMegakernelBarrierPlan, CudaMegakernelBarrierPlanError> {
    plan_megakernel_barriers_with_scratch(wave_count, dependencies, scratch)
}

/// Frontier-typed CUDA megakernel wave memory envelope.
pub type CudaMegakernelFrontierWave = MegakernelFrontierWave;

/// Dependency-aware CUDA megakernel execution plan for frontier waves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaMegakernelFrontierExecutionPlan {
    /// Cache-backed topology and memory-budget plan.
    pub execution: CudaMegakernelExecutionPlan,
    /// Minimum global-barrier grouping for the wave dependencies.
    pub barriers: CudaMegakernelBarrierPlan,
    /// Peak frontier bytes across any fused barrier-free group.
    pub peak_frontier_bytes: u64,
    /// Peak scratch bytes across any fused barrier-free group.
    pub peak_scratch_bytes: u64,
    /// Peak output bytes across any fused barrier-free group.
    pub peak_output_bytes: u64,
    /// Readback pressure fed into topology selection after combining runtime
    /// telemetry with static fused-wave output volume.
    pub amortized_readback_bytes: u64,
    /// Widest barrier-free group in wave count.
    pub max_group_width: usize,
}

/// Dependency-aware frontier execution planning failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaMegakernelFrontierExecutionPlanError {
    /// Dependency graph cannot be barrier-planned.
    Barrier(CudaMegakernelBarrierPlanError),
    /// Peak wave bytes overflowed while grouping a barrier-free phase.
    ByteCountOverflow {
        /// Field being accumulated.
        field: &'static str,
    },
    /// Static graph or fused frontier bytes exceed the caller-approved budget.
    GroupOverBudget {
        /// Required bytes before topology selection.
        required_bytes: u64,
        /// Caller-provided budget.
        budget_bytes: u64,
        /// Budget region being checked.
        field: &'static str,
    },
    /// Cache-backed execution memory planning failed.
    Memory(CudaMegakernelMemoryError),
    /// Frontier planning result storage could not be reserved.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Number of elements requested.
        requested: usize,
        /// Allocator error text.
        message: String,
    },
}

impl CudaArithmeticOverflow for CudaMegakernelFrontierExecutionPlanError {
    fn arithmetic_overflow(field: &'static str) -> Self {
        Self::ByteCountOverflow { field }
    }
}

impl CudaStorageReserveFailure for CudaMegakernelFrontierExecutionPlanError {
    fn storage_reserve_failed(field: &'static str, requested: usize, message: String) -> Self {
        Self::StorageReserveFailed {
            field,
            requested,
            message,
        }
    }
}

impl std::fmt::Display for CudaMegakernelFrontierExecutionPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Barrier(error) => error.fmt(f),
            Self::ByteCountOverflow { field } => write!(
                f,
                "CUDA megakernel frontier execution planner overflowed while accumulating {field}. Fix: shard the frontier wave group or split the fused phase."
            ),
            Self::GroupOverBudget {
                required_bytes,
                budget_bytes,
                field,
            } => write!(
                f,
                "CUDA megakernel frontier execution planner requires {required_bytes} bytes for {field} but budget allows {budget_bytes}. Fix: shard the graph/frontier waves or raise the explicit CUDA megakernel budget."
            ),
            Self::Memory(error) => error.fmt(f),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "CUDA megakernel frontier execution planner could not reserve {requested} {field} entries: {message}. Fix: shard the frontier waves before planning."
            ),
        }
    }
}

impl std::error::Error for CudaMegakernelFrontierExecutionPlanError {}

impl From<CudaMegakernelBarrierPlanError> for CudaMegakernelFrontierExecutionPlanError {
    fn from(error: CudaMegakernelBarrierPlanError) -> Self {
        Self::Barrier(error)
    }
}

impl From<CudaMegakernelMemoryError> for CudaMegakernelFrontierExecutionPlanError {
    fn from(error: CudaMegakernelMemoryError) -> Self {
        Self::Memory(error)
    }
}

impl From<MegakernelFrontierMemoryPlanError> for CudaMegakernelFrontierExecutionPlanError {
    fn from(error: MegakernelFrontierMemoryPlanError) -> Self {
        match error {
            MegakernelFrontierMemoryPlanError::Barrier(error) => Self::Barrier(error),
            MegakernelFrontierMemoryPlanError::ByteCountOverflow { field } => {
                Self::ByteCountOverflow { field }
            }
            MegakernelFrontierMemoryPlanError::GroupOverBudget {
                required_bytes,
                budget_bytes,
                field,
            } => Self::GroupOverBudget {
                required_bytes,
                budget_bytes,
                field,
            },
            MegakernelFrontierMemoryPlanError::StorageReserveFailed {
                field,
                requested,
                message,
            } => Self::StorageReserveFailed {
                field,
                requested,
                message,
            },
        }
    }
}

/// Plan dependency-aware CUDA megakernel execution for frontier-typed waves.
///
/// The planner first minimizes global barriers from wave dependencies, then
/// computes the peak memory envelope of any barrier-free fused group, and
/// finally asks the CUDA plan cache for a memory-validated execution topology.
pub fn plan_cuda_frontier_megakernel_execution(
    cache: &mut CudaMegakernelPlanCache,
    graph_layout_hash: u64,
    analysis_kind: CudaMegakernelAnalysisKind,
    device: CudaMegakernelDeviceKey,
    sample: CudaMegakernelScheduleSample,
    graph: CudaMegakernelGraphShape,
    bytes_per_node: u64,
    bytes_per_edge: u64,
    waves: &[CudaMegakernelFrontierWave],
    dependencies: &[CudaMegakernelWaveDependency],
    budget_bytes: u64,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
) -> Result<CudaMegakernelFrontierExecutionPlan, CudaMegakernelFrontierExecutionPlanError> {
    let mut scratch =
        CudaMegakernelBarrierScratch::try_with_capacity(waves.len(), dependencies.len())?;
    plan_cuda_frontier_megakernel_execution_with_scratch(
        cache,
        graph_layout_hash,
        analysis_kind,
        device,
        sample,
        graph,
        bytes_per_node,
        bytes_per_edge,
        waves,
        dependencies,
        budget_bytes,
        launch_overhead_ns,
        fusion_pressure,
        &mut scratch,
    )
}

/// Plan dependency-aware CUDA megakernel execution using caller-owned barrier scratch.
pub fn plan_cuda_frontier_megakernel_execution_with_scratch(
    cache: &mut CudaMegakernelPlanCache,
    graph_layout_hash: u64,
    analysis_kind: CudaMegakernelAnalysisKind,
    device: CudaMegakernelDeviceKey,
    sample: CudaMegakernelScheduleSample,
    graph: CudaMegakernelGraphShape,
    bytes_per_node: u64,
    bytes_per_edge: u64,
    waves: &[CudaMegakernelFrontierWave],
    dependencies: &[CudaMegakernelWaveDependency],
    budget_bytes: u64,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
    scratch: &mut CudaMegakernelBarrierScratch,
) -> Result<CudaMegakernelFrontierExecutionPlan, CudaMegakernelFrontierExecutionPlanError> {
    let graph_bytes = graph_resident_bytes(graph, bytes_per_node, bytes_per_edge)?;
    let memory_plan = plan_megakernel_frontier_memory_with_scratch(
        waves,
        dependencies,
        graph_bytes,
        budget_bytes,
        sample.readback_bytes,
        scratch,
    )?;

    let topology_sample = CudaMegakernelScheduleSample {
        readback_bytes: memory_plan.amortized_readback_bytes,
        ..sample
    };
    let execution = cache.get_or_plan_execution(
        graph_layout_hash,
        analysis_kind,
        device,
        topology_sample,
        graph,
        bytes_per_node,
        bytes_per_edge,
        memory_plan.peak_frontier_bytes,
        memory_plan.peak_scratch_bytes,
        memory_plan.peak_output_bytes,
        budget_bytes,
        launch_overhead_ns,
        fusion_pressure,
    )?;

    Ok(CudaMegakernelFrontierExecutionPlan {
        execution,
        barriers: memory_plan.barriers,
        peak_frontier_bytes: memory_plan.peak_frontier_bytes,
        peak_scratch_bytes: memory_plan.peak_scratch_bytes,
        peak_output_bytes: memory_plan.peak_output_bytes,
        amortized_readback_bytes: topology_sample.readback_bytes,
        max_group_width: memory_plan.max_group_width,
    })
}

fn graph_resident_bytes(
    graph: CudaMegakernelGraphShape,
    bytes_per_node: u64,
    bytes_per_edge: u64,
) -> Result<u64, CudaMegakernelFrontierExecutionPlanError> {
    let node_bytes = checked_mul::<CudaMegakernelFrontierExecutionPlanError>(
        graph.node_count,
        bytes_per_node,
        "node layout bytes",
    )?;
    let edge_bytes = checked_mul::<CudaMegakernelFrontierExecutionPlanError>(
        graph.edge_count,
        bytes_per_edge,
        "edge layout bytes",
    )?;
    checked_add::<CudaMegakernelFrontierExecutionPlanError>(
        node_bytes,
        edge_bytes,
        "graph layout bytes",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        plan_cuda_frontier_megakernel_execution,
        plan_cuda_frontier_megakernel_execution_with_scratch, plan_cuda_megakernel_barriers,
        plan_cuda_megakernel_barriers_with_scratch, CudaMegakernelBarrierPlanError,
        CudaMegakernelBarrierScratch, CudaMegakernelFrontierExecutionPlanError,
        CudaMegakernelFrontierWave, CudaMegakernelWaveDependency,
    };
    use crate::megakernel_plan_cache::{
        CudaMegakernelAnalysisKind, CudaMegakernelDeviceKey, CudaMegakernelPlanCache,
    };
    use crate::megakernel_scheduler::{
        CudaMegakernelGraphShape, CudaMegakernelScheduleSample, CudaMegakernelTopology,
    };

    #[test]
    fn independent_waves_share_one_barrier_free_group() {
        let plan = plan_cuda_megakernel_barriers(4, &[])
            .expect("Fix: independent CUDA megakernel waves should not need barriers.");

        assert_eq!(plan.global_barriers, 0);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].waves, vec![0, 1, 2, 3]);
    }

    #[test]
    fn dependency_chain_requires_one_barrier_between_each_wave() {
        let plan = plan_cuda_megakernel_barriers(
            4,
            &[
                CudaMegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                CudaMegakernelWaveDependency {
                    before: 1,
                    after: 2,
                },
                CudaMegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
        )
        .expect("Fix: acyclic CUDA megakernel wave chain should be schedulable.");

        assert_eq!(plan.global_barriers, 3);
        assert_eq!(plan.groups[0].waves, vec![0]);
        assert_eq!(plan.groups[1].waves, vec![1]);
        assert_eq!(plan.groups[2].waves, vec![2]);
        assert_eq!(plan.groups[3].waves, vec![3]);
    }

    #[test]
    fn diamond_dependencies_fuse_middle_waves() {
        let plan = plan_cuda_megakernel_barriers(
            4,
            &[
                CudaMegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                CudaMegakernelWaveDependency {
                    before: 0,
                    after: 2,
                },
                CudaMegakernelWaveDependency {
                    before: 1,
                    after: 3,
                },
                CudaMegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
        )
        .expect("Fix: diamond CUDA megakernel dependencies should preserve middle-wave fusion.");

        assert_eq!(plan.global_barriers, 2);
        assert_eq!(plan.groups[0].waves, vec![0]);
        assert_eq!(plan.groups[1].waves, vec![1, 2]);
        assert_eq!(plan.groups[2].waves, vec![3]);
    }

    #[test]
    fn barrier_planner_uses_csr_adjacency_for_wide_wave_graphs() {
        let dependencies = (1..1_025)
            .map(|after| CudaMegakernelWaveDependency { before: 0, after })
            .collect::<Vec<_>>();
        let plan = plan_cuda_megakernel_barriers(1_025, &dependencies)
            .expect("Fix: wide CUDA megakernel dependency fanout must schedule without per-wave adjacency allocation.");

        assert_eq!(plan.global_barriers, 1);
        assert_eq!(plan.groups[0].waves, vec![0]);
        assert_eq!(plan.groups[1].waves.len(), 1_024);
        let src = include_str!("../../vyre-driver/src/megakernel_barrier.rs");
        assert!(
            !src.contains(concat!("vec![", "Vec::new(); wave_count]")),
            "Fix: CUDA megakernel barrier planner must use contiguous CSR adjacency instead of allocating one Vec per wave."
        );
        assert!(
            !src.contains(concat!("outgoing_offsets[..wave_count]", ".to_vec()")),
            "Fix: CUDA megakernel barrier planner must reuse the counts buffer as the CSR write cursor instead of allocating an O(wave_count) cursor Vec."
        );
        assert!(
            !src.contains(concat!("Vec", "Deque")),
            "Fix: CUDA megakernel barrier planner should use contiguous current/next ready vectors, not deque queue mechanics, for wide wave layers."
        );
        assert!(
            !src.contains(concat!("saturating", "_add")),
            "Fix: CUDA megakernel barrier dependency accounting is bounded by the validated graph shape and must not hide invariant violations with saturating arithmetic."
        );
        assert!(
            src.contains("field: \"outgoing dependency count\"")
                && src.contains("field: \"incoming dependency count\"")
                && src.contains("field: \"outgoing dependency offsets\"")
                && src.contains("field: \"outgoing target cursor\""),
            "Fix: CUDA megakernel barrier CSR construction must use checked arithmetic for dependency counters, offsets, and cursors."
        );
        assert!(
            src.contains("reserve_typed_vec_to_capacity as reserve_vec_to_capacity")
                && src.contains("fn fill_barrier_vec_zeroed(")
                && src.contains("StorageReserveFailed"),
            "Fix: CUDA megakernel barrier and frontier group staging must reserve through shared fallible CUDA staging instead of panicking under scale pressure."
        );

        assert!(
            !src.contains(concat!("Vec::with_capacity", "(wave_count)"))
                && !src.contains(concat!("Vec::with_capacity", "(barriers.groups.len())"))
                && !src.contains(concat!("Vec::with_capacity", "(group.waves.len().min(8))"))
                && !src.contains(concat!(".reserve", "(wave_count)"))
                && !src.contains(concat!("scratch.outgoing_counts", ".resize"))
                && !src.contains(concat!("scratch.indegree", ".resize"))
                && !src.contains(concat!("scratch.outgoing_targets", ".resize")),
            "Fix: CUDA megakernel barrier planner must not use infallible capacity growth in release topology planning."
        );
        assert!(
            !src.contains(concat!(
                "scratch.outgoing_counts[dependency.before]",
                " += 1"
            ))
                && !src.contains(concat!("scratch.indegree[dependency.after]", " += 1"))
                && !src.contains(concat!(
                    "let next = scratch.outgoing_offsets.last().copied().unwrap_or(0)",
                    " + *count"
                )),
            "Fix: CUDA megakernel barrier planning must not use unchecked usize arithmetic for CSR construction."
        );
    }

    #[test]
    fn barrier_planner_reuses_caller_owned_csr_scratch_across_shapes() {
        let mut scratch = CudaMegakernelBarrierScratch::try_with_capacity(1_025, 1_024)
            .expect("Fix: wide reusable CUDA megakernel barrier scratch should fit");
        let wide_dependencies = (1..1_025)
            .map(|after| CudaMegakernelWaveDependency { before: 0, after })
            .collect::<Vec<_>>();
        let wide =
            plan_cuda_megakernel_barriers_with_scratch(1_025, &wide_dependencies, &mut scratch)
                .expect(
                    "Fix: wide CUDA megakernel dependency fanout should plan with reusable scratch",
                );
        let wave_capacity = scratch.wave_capacity();
        let dependency_capacity = scratch.dependency_capacity();

        assert_eq!(wide.groups[1].waves.len(), 1_024);

        let narrow = plan_cuda_megakernel_barriers_with_scratch(
            4,
            &[
                CudaMegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                CudaMegakernelWaveDependency {
                    before: 1,
                    after: 2,
                },
                CudaMegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
            &mut scratch,
        )
        .expect("Fix: narrow CUDA megakernel dependency chain should reuse larger scratch");

        assert_eq!(narrow.global_barriers, 3);
        assert!(scratch.wave_capacity() >= wave_capacity);
        assert!(scratch.dependency_capacity() >= dependency_capacity);
    }

    #[test]
    fn frontier_execution_planner_accepts_reusable_barrier_scratch() {
        let mut cache = CudaMegakernelPlanCache::new();
        let mut scratch = CudaMegakernelBarrierScratch::try_with_capacity(3, 2)
            .expect("Fix: frontier reusable CUDA megakernel barrier scratch should fit");
        let waves = [
            CudaMegakernelFrontierWave {
                frontier_bytes: 128,
                scratch_bytes: 64,
                output_bytes: 32,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: 256,
                scratch_bytes: 128,
                output_bytes: 64,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: 512,
                scratch_bytes: 256,
                output_bytes: 128,
            },
        ];
        let dependencies = [
            CudaMegakernelWaveDependency {
                before: 0,
                after: 1,
            },
            CudaMegakernelWaveDependency {
                before: 1,
                after: 2,
            },
        ];

        let plan = plan_cuda_frontier_megakernel_execution_with_scratch(
            &mut cache,
            77,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.4,
                readback_bytes: 16,
            },
            CudaMegakernelGraphShape {
                node_count: 256,
                edge_count: 512,
            },
            16,
            8,
            &waves,
            &dependencies,
            1 << 20,
            400.0,
            1.0,
            &mut scratch,
        )
        .expect("Fix: frontier megakernel planner should accept caller-owned barrier scratch");

        assert_eq!(plan.barriers.global_barriers, 2);
        assert!(scratch.wave_capacity() >= 3);
        assert!(scratch.dependency_capacity() >= 2);
    }

    #[test]
    fn invalid_or_cyclic_dependencies_fail_loudly() {
        let invalid = plan_cuda_megakernel_barriers(
            2,
            &[CudaMegakernelWaveDependency {
                before: 0,
                after: 2,
            }],
        )
        .expect_err("Fix: invalid CUDA megakernel wave index must fail before planning.");
        assert!(matches!(
            invalid,
            CudaMegakernelBarrierPlanError::InvalidWave { .. }
        ));

        let cycle = plan_cuda_megakernel_barriers(
            2,
            &[
                CudaMegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                CudaMegakernelWaveDependency {
                    before: 1,
                    after: 0,
                },
            ],
        )
        .expect_err(
            "Fix: cyclic CUDA megakernel dependencies require explicit fixed-point kernels.",
        );
        assert_eq!(
            cycle,
            CudaMegakernelBarrierPlanError::Cycle {
                unscheduled_waves: 2
            }
        );
    }

    #[test]
    fn frontier_execution_plan_uses_peak_barrier_group_memory() {
        let mut cache = CudaMegakernelPlanCache::new();
        let plan = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            42,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.90,
                readback_bytes: 1 << 20,
            },
            CudaMegakernelGraphShape {
                node_count: 1_000,
                edge_count: 4_000,
            },
            16,
            8,
            &[
                CudaMegakernelFrontierWave {
                    frontier_bytes: 1_024,
                    scratch_bytes: 512,
                    output_bytes: 256,
                },
                CudaMegakernelFrontierWave {
                    frontier_bytes: 2_048,
                    scratch_bytes: 1_024,
                    output_bytes: 512,
                },
                CudaMegakernelFrontierWave {
                    frontier_bytes: 4_096,
                    scratch_bytes: 2_048,
                    output_bytes: 1_024,
                },
                CudaMegakernelFrontierWave {
                    frontier_bytes: 8_192,
                    scratch_bytes: 4_096,
                    output_bytes: 2_048,
                },
            ],
            &[
                CudaMegakernelWaveDependency {
                    before: 0,
                    after: 1,
                },
                CudaMegakernelWaveDependency {
                    before: 0,
                    after: 2,
                },
                CudaMegakernelWaveDependency {
                    before: 1,
                    after: 3,
                },
                CudaMegakernelWaveDependency {
                    before: 2,
                    after: 3,
                },
            ],
            128 * 1024,
            250.0,
            0.95,
        )
        .expect("Fix: frontier-typed CUDA megakernel execution plan should fit the budget.");

        assert_eq!(plan.barriers.global_barriers, 2);
        assert_eq!(plan.barriers.groups[1].waves, vec![1, 2]);
        assert_eq!(plan.peak_frontier_bytes, 8_192);
        assert_eq!(plan.peak_scratch_bytes, 4_096);
        assert_eq!(plan.peak_output_bytes, 2_048);
        assert_eq!(plan.amortized_readback_bytes, 1 << 20);
        assert_eq!(plan.max_group_width, 2);
        assert_eq!(plan.execution.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(plan.execution.memory.frontier_bytes, 8_192);
    }

    #[test]
    fn frontier_execution_uses_static_group_output_to_trigger_fusion() {
        let mut cache = CudaMegakernelPlanCache::new();
        let plan = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            77,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 0,
            },
            CudaMegakernelGraphShape {
                node_count: 1_000,
                edge_count: 4_000,
            },
            16,
            8,
            &[
                CudaMegakernelFrontierWave {
                    frontier_bytes: 1_024,
                    scratch_bytes: 512,
                    output_bytes: 3_072,
                },
                CudaMegakernelFrontierWave {
                    frontier_bytes: 1_024,
                    scratch_bytes: 512,
                    output_bytes: 3_072,
                },
            ],
            &[],
            128 * 1024,
            250.0,
            0.95,
        )
        .expect("Fix: static output-amortized CUDA frontier plan should fit the budget.");

        assert_eq!(plan.peak_output_bytes, 6_144);
        assert_eq!(plan.amortized_readback_bytes, 6_144);
        assert_eq!(
            plan.execution.topology,
            CudaMegakernelTopology::FusedWave,
            "Fix: high static fused-group output pressure must trigger megakernel fusion even when the previous telemetry interval had no final readback."
        );
    }

    #[test]
    fn frontier_execution_splits_independent_layers_to_fit_fused_memory_budget() {
        let mut cache = CudaMegakernelPlanCache::new();
        let waves = [
            CudaMegakernelFrontierWave {
                frontier_bytes: 10,
                scratch_bytes: 10,
                output_bytes: 10,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: 10,
                scratch_bytes: 10,
                output_bytes: 10,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: 10,
                scratch_bytes: 10,
                output_bytes: 10,
            },
        ];
        let plan = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            909,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 4_096,
            },
            CudaMegakernelGraphShape {
                node_count: 1,
                edge_count: 0,
            },
            0,
            0,
            &waves,
            &[],
            100,
            250.0,
            0.95,
        )
        .expect("Fix: independent CUDA frontier waves should split into memory-fit fused chunks instead of failing the release path.");

        assert_eq!(plan.barriers.groups.len(), 3);
        assert_eq!(plan.barriers.global_barriers, 2);
        assert_eq!(plan.max_group_width, 1);
        assert_eq!(plan.peak_frontier_bytes, 10);
        assert_eq!(plan.peak_scratch_bytes, 10);
        assert_eq!(plan.peak_output_bytes, 10);
        assert_eq!(plan.execution.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(plan.execution.memory.required_bytes, 60);
    }

    #[test]
    fn frontier_execution_rejects_graph_bytes_over_budget_without_zero_budget_default() {
        let mut cache = CudaMegakernelPlanCache::new();
        let error = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            910,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 4_096,
            },
            CudaMegakernelGraphShape {
                node_count: 100,
                edge_count: 100,
            },
            8,
            8,
            &[CudaMegakernelFrontierWave {
                frontier_bytes: 1,
                scratch_bytes: 1,
                output_bytes: 1,
            }],
            &[],
            1_000,
            250.0,
            0.95,
        )
        .expect_err("resident graph bytes above budget must fail before split planning");

        assert_eq!(
            error,
            CudaMegakernelFrontierExecutionPlanError::GroupOverBudget {
                required_bytes: 1_600,
                budget_bytes: 1_000,
                field: "resident graph bytes",
            }
        );
    }

    #[test]
    fn frontier_execution_rejects_single_wave_that_cannot_fit_group_budget() {
        let mut cache = CudaMegakernelPlanCache::new();
        let error = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            911,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 4_096,
            },
            CudaMegakernelGraphShape {
                node_count: 1,
                edge_count: 0,
            },
            0,
            0,
            &[CudaMegakernelFrontierWave {
                frontier_bytes: 100,
                scratch_bytes: 100,
                output_bytes: 100,
            }],
            &[],
            500,
            250.0,
            0.95,
        )
        .expect_err("single fused wave above group budget must fail before topology planning");

        assert_eq!(
            error,
            CudaMegakernelFrontierExecutionPlanError::GroupOverBudget {
                required_bytes: 600,
                budget_bytes: 500,
                field: "single fused frontier wave bytes",
            }
        );
    }

    #[test]
    fn frontier_execution_plan_reuses_cached_topology_for_equivalent_pressure() {
        let mut cache = CudaMegakernelPlanCache::new();
        let waves = [
            CudaMegakernelFrontierWave {
                frontier_bytes: 1_024,
                scratch_bytes: 512,
                output_bytes: 256,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: 2_048,
                scratch_bytes: 1_024,
                output_bytes: 512,
            },
        ];
        let first = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            42,
            CudaMegakernelAnalysisKind::ParserFrontend,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.90,

                readback_bytes: 1 << 20,
            },
            CudaMegakernelGraphShape {
                node_count: 1_000,
                edge_count: 4_000,
            },
            16,
            8,
            &waves,
            &[],
            128 * 1024,
            250.0,
            0.95,
        )
        .expect("Fix: first frontier execution plan should fit.");
        let second = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            42,
            CudaMegakernelAnalysisKind::ParserFrontend,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.91,
                readback_bytes: 1 << 20,
            },
            CudaMegakernelGraphShape {
                node_count: 1_000,
                edge_count: 4_000,
            },
            16,
            8,
            &waves,
            &[],
            128 * 1024,
            250.0,
            0.95,
        )
        .expect("Fix: equivalent frontier execution pressure should reuse cached topology.");

        assert_eq!(first.execution.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(second.execution.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(cache.stats().hits, 1);
    }

    #[test]
    fn frontier_execution_plan_fails_loudly_on_wave_byte_overflow() {
        let mut cache = CudaMegakernelPlanCache::new();
        let error = plan_cuda_frontier_megakernel_execution(
            &mut cache,
            42,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            CudaMegakernelScheduleSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.90,
                readback_bytes: 1 << 20,
            },
            CudaMegakernelGraphShape {
                node_count: 1,
                edge_count: 1,
            },
            1,
            1,
            &[
                CudaMegakernelFrontierWave {
                    frontier_bytes: u64::MAX,
                    scratch_bytes: 1,
                    output_bytes: 1,
                },
                CudaMegakernelFrontierWave {
                    frontier_bytes: 1,
                    scratch_bytes: 1,
                    output_bytes: 1,
                },
            ],
            &[],
            u64::MAX,
            250.0,
            0.95,
        )
        .expect_err("Fix: overflowed frontier wave bytes must fail before CUDA launch planning.");

        assert_eq!(
            error,
            CudaMegakernelFrontierExecutionPlanError::ByteCountOverflow {
                field: "fused wave bytes"
            }
        );
    }

    fn device() -> CudaMegakernelDeviceKey {
        CudaMegakernelDeviceKey {
            sm_major: 12,
            sm_minor: 0,
            warp_size: 32,
            supports_grid_sync: true,
            supports_tensor_cores: true,
            max_workgroup_size: 1024,
        }
    }
}
