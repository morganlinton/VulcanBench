//! Backend-neutral execution planning for persistent megakernel waves.
//!
//! Backends can feed telemetry and device budgets into this module to choose a
//! sparse, dense, hybrid, or fused execution topology before allocating device
//! scratch. The policy is deterministic, allocation-free, and validates byte
//! pressure before a backend reaches an API-specific allocation path.

const WARP_SPARSE_DENSITY: f64 = 0.03125;
const SPARSE_DENSITY: f64 = 0.125;
const DENSE_DENSITY: f64 = 0.70;
const BLOCK_DENSE_DENSITY: f64 = 0.85;
const FUSION_PRESSURE: f64 = 0.70;
const FUSION_PRESSURE_HYSTERESIS: f64 = 0.10;
const FRONTIER_HYSTERESIS: f64 = 0.025;
const MEMORY_RED_ZONE_BPS: u32 = 9_000;
const MEMORY_HYSTERESIS_BPS: u32 = 250;
const LAUNCH_PRESSURE_BPS: u32 = 1_500;
const LAUNCH_HYSTERESIS_BPS: u32 = 250;
const FUSION_READBACK_BYTES: u64 = 4_096;
const DENSE_AVERAGE_DEGREE_BPS: u64 = 20_000;
const WARP_SPARSE_AVERAGE_DEGREE_BPS: u64 = 80_000;

/// Per-candidate telemetry used to bias megakernel fusion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MegakernelExecutionSample {
    /// Observed candidate dispatch cost in nanoseconds.
    pub dispatch_cost_ns: f64,
    /// Observed active-frontier density in `[0, 1]`.
    pub frontier_density: f64,
    /// Observed final readback byte volume.
    pub readback_bytes: u64,
}

/// Device-side megakernel execution topology selected for a dataflow wave.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MegakernelExecutionTopology {
    /// Ultra-low-density frontier expansion where one warp owns sparse active
    /// nodes and avoids block-wide work distribution overhead.
    WarpSparseFrontier,
    /// Low-density frontier expansion with queue-like work distribution.
    SparseFrontier,
    /// Very high-density propagation where a block owns coalesced bitset lanes
    /// and amortizes shared-memory scans across many active facts.
    BlockDenseFrontier,
    /// Dense bitset-style propagation with coalesced scans.
    DenseFrontier,
    /// Mixed sparse/dense execution when density is in the transition band.
    HybridFrontier,
    /// Fused adjacent waves when launch/readback pressure dominates and memory
    /// budget leaves room for the fused plan.
    FusedWave,
}

/// Static graph shape used by topology selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MegakernelGraphShape {
    /// Logical graph node count.
    pub node_count: u64,
    /// Logical graph edge count.
    pub edge_count: u64,
}

/// Device memory envelope for a candidate megakernel plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MegakernelMemoryBudget {
    /// Estimated resident plus transient bytes required by the candidate plan.
    pub required_bytes: u64,
    /// Caller-approved device-memory budget for the plan.
    pub budget_bytes: u64,
}

/// Detailed megakernel memory plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MegakernelMemoryPlan {
    /// Graph-layout bytes retained on device.
    pub graph_bytes: u64,
    /// Frontier-state bytes retained on device.
    pub frontier_bytes: u64,
    /// Temporary scratch bytes required by the selected topology.
    pub scratch_bytes: u64,
    /// Final compact output/readback bytes.
    pub output_bytes: u64,
    /// Total peak bytes required by the plan.
    pub required_bytes: u64,
    /// Caller-approved byte budget.
    pub budget_bytes: u64,
    /// Required/budget pressure in basis points.
    pub memory_pressure_bps: u32,
}

/// Complete megakernel execution plan selected from runtime telemetry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MegakernelExecutionPlan {
    /// Final topology after memory-budget validation.
    pub topology: MegakernelExecutionTopology,
    /// Memory plan for the final topology.
    pub memory: MegakernelMemoryPlan,
    /// Whether the planner downgraded a denser/fused topology to sparse to fit
    /// the explicit memory budget.
    pub downgraded_to_sparse: bool,
}

/// Memory planning failure for megakernel execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MegakernelMemoryError {
    /// A byte-count multiplication or addition overflowed.
    ByteCountOverflow {
        /// Field being computed when overflow happened.
        field: &'static str,
    },
    /// The candidate plan exceeds the caller-approved device-memory budget.
    OverBudget {
        /// Selected topology.
        topology: MegakernelExecutionTopology,
        /// Required peak bytes.
        required_bytes: u64,
        /// Caller-approved budget bytes.
        budget_bytes: u64,
        /// Graph node count.
        node_count: u64,
        /// Graph edge count.
        edge_count: u64,
    },
}

impl std::fmt::Display for MegakernelMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ByteCountOverflow { field } => write!(
                f,
                "megakernel memory planner overflowed while computing {field}. Fix: shard the graph or lower the candidate topology before planning device residency."
            ),
            Self::OverBudget {
                topology,
                required_bytes,
                budget_bytes,
                node_count,
                edge_count,
            } => write!(
                f,
                "megakernel {topology:?} plan requires {required_bytes} bytes but budget allows {budget_bytes} bytes for graph nodes={node_count} edges={edge_count}. Fix: choose a sparse topology, reduce fusion pressure, shard the graph, or raise the explicit device-memory budget."
            ),
        }
    }
}

impl std::error::Error for MegakernelMemoryError {}

/// Topology decision with the pressure metrics that caused it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MegakernelTopologyDecision {
    /// Selected execution topology.
    pub topology: MegakernelExecutionTopology,
    /// Required/budget memory pressure in basis points.
    pub memory_pressure_bps: u32,
    /// Edge/node average degree proxy in basis points.
    pub average_degree_bps: u64,
    /// Launch overhead divided by observed dispatch cost in basis points.
    pub launch_pressure_bps: u32,
}

impl MegakernelTopologyDecision {
    /// Stable single-line explanation for release logs and scheduler debugging.
    #[must_use]
    pub fn stable_explanation(&self) -> String {
        format!(
            "megakernel-topology-v1|topology={:?}|memory_pressure_bps={}|average_degree_bps={}|launch_pressure_bps={}|reason={}",
            self.topology,
            self.memory_pressure_bps,
            self.average_degree_bps,
            self.launch_pressure_bps,
            self.reason_code()
        )
    }

    fn reason_code(&self) -> &'static str {
        match self.topology {
            MegakernelExecutionTopology::WarpSparseFrontier => "ultra_sparse_warp_specialized",
            MegakernelExecutionTopology::SparseFrontier if self.memory_pressure_bps >= 9_000 => {
                "memory_pressure_sparse_safety"
            }
            MegakernelExecutionTopology::SparseFrontier => "low_density_sparse_queue",
            MegakernelExecutionTopology::BlockDenseFrontier => "high_density_block_specialized",
            MegakernelExecutionTopology::DenseFrontier => "dense_coalesced_frontier",
            MegakernelExecutionTopology::HybridFrontier => "transition_band_hybrid",
            MegakernelExecutionTopology::FusedWave => "launch_and_readback_pressure_fused",
        }
    }
}

/// Select the megakernel execution topology for one candidate wave.
#[must_use]
pub fn select_megakernel_topology(
    sample: MegakernelExecutionSample,
    graph: MegakernelGraphShape,
    memory: MegakernelMemoryBudget,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
) -> MegakernelTopologyDecision {
    let memory_pressure_bps = pressure_bps(memory.required_bytes, memory.budget_bytes);
    let average_degree_bps = pressure_bps_u64(graph.edge_count, graph.node_count);
    let launch_pressure_bps =
        if sample.dispatch_cost_ns <= 0.0 || !sample.dispatch_cost_ns.is_finite() {
            0
        } else {
            finite_ratio_bps(
                launch_overhead_ns.max(0.0),
                sample.dispatch_cost_ns,
                "launch overhead pressure",
            )
        };
    let density = finite_unit(sample.frontier_density);
    let fusion = finite_unit(fusion_pressure);
    let topology = if memory_pressure_bps >= MEMORY_RED_ZONE_BPS {
        MegakernelExecutionTopology::SparseFrontier
    } else if fusion >= FUSION_PRESSURE
        && launch_pressure_bps >= LAUNCH_PRESSURE_BPS
        && sample.readback_bytes >= FUSION_READBACK_BYTES
        && memory_pressure_bps
            <= checked_bps_sub(MEMORY_RED_ZONE_BPS, 500, "fusion memory red-zone margin")
    {
        MegakernelExecutionTopology::FusedWave
    } else if density <= WARP_SPARSE_DENSITY && average_degree_bps <= WARP_SPARSE_AVERAGE_DEGREE_BPS
    {
        MegakernelExecutionTopology::WarpSparseFrontier
    } else if density <= SPARSE_DENSITY {
        MegakernelExecutionTopology::SparseFrontier
    } else if density >= BLOCK_DENSE_DENSITY && average_degree_bps >= DENSE_AVERAGE_DEGREE_BPS {
        MegakernelExecutionTopology::BlockDenseFrontier
    } else if density >= DENSE_DENSITY && average_degree_bps >= DENSE_AVERAGE_DEGREE_BPS {
        MegakernelExecutionTopology::DenseFrontier
    } else {
        MegakernelExecutionTopology::HybridFrontier
    };
    MegakernelTopologyDecision {
        topology,
        memory_pressure_bps,
        average_degree_bps,
        launch_pressure_bps,
    }
}

/// Select megakernel topology with previous-topology hysteresis.
#[must_use]
pub fn select_megakernel_topology_stable(
    sample: MegakernelExecutionSample,
    graph: MegakernelGraphShape,
    memory: MegakernelMemoryBudget,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
    previous_topology: MegakernelExecutionTopology,
) -> MegakernelTopologyDecision {
    let mut decision =
        select_megakernel_topology(sample, graph, memory, launch_overhead_ns, fusion_pressure);
    decision.topology = stabilize_topology(decision, sample, fusion_pressure, previous_topology);
    decision
}

fn stabilize_topology(
    decision: MegakernelTopologyDecision,
    sample: MegakernelExecutionSample,
    fusion_pressure: f64,
    previous_topology: MegakernelExecutionTopology,
) -> MegakernelExecutionTopology {
    if decision.memory_pressure_bps >= MEMORY_RED_ZONE_BPS {
        return decision.topology;
    }
    let density = finite_unit(sample.frontier_density);
    let fusion = finite_unit(fusion_pressure);
    if matches!(
        previous_topology,
        MegakernelExecutionTopology::SparseFrontier
            | MegakernelExecutionTopology::WarpSparseFrontier
    ) && decision.memory_pressure_bps
        >= checked_bps_sub(
            MEMORY_RED_ZONE_BPS,
            MEMORY_HYSTERESIS_BPS,
            "memory hysteresis floor",
        )
    {
        return MegakernelExecutionTopology::SparseFrontier;
    }

    match previous_topology {
        MegakernelExecutionTopology::WarpSparseFrontier
            if density <= WARP_SPARSE_DENSITY + FRONTIER_HYSTERESIS
                && decision.average_degree_bps <= WARP_SPARSE_AVERAGE_DEGREE_BPS =>
        {
            MegakernelExecutionTopology::WarpSparseFrontier
        }
        MegakernelExecutionTopology::SparseFrontier
            if density <= SPARSE_DENSITY + FRONTIER_HYSTERESIS =>
        {
            MegakernelExecutionTopology::SparseFrontier
        }
        MegakernelExecutionTopology::HybridFrontier
            if decision.topology == MegakernelExecutionTopology::SparseFrontier
                && density >= SPARSE_DENSITY - FRONTIER_HYSTERESIS =>
        {
            MegakernelExecutionTopology::HybridFrontier
        }
        MegakernelExecutionTopology::HybridFrontier
            if matches!(
                decision.topology,
                MegakernelExecutionTopology::DenseFrontier
                    | MegakernelExecutionTopology::BlockDenseFrontier
            ) && density <= DENSE_DENSITY + FRONTIER_HYSTERESIS =>
        {
            MegakernelExecutionTopology::HybridFrontier
        }
        MegakernelExecutionTopology::DenseFrontier
            if density >= DENSE_DENSITY - FRONTIER_HYSTERESIS
                && decision.average_degree_bps >= DENSE_AVERAGE_DEGREE_BPS =>
        {
            MegakernelExecutionTopology::DenseFrontier
        }
        MegakernelExecutionTopology::BlockDenseFrontier
            if density >= BLOCK_DENSE_DENSITY - FRONTIER_HYSTERESIS
                && decision.average_degree_bps >= DENSE_AVERAGE_DEGREE_BPS =>
        {
            MegakernelExecutionTopology::BlockDenseFrontier
        }
        MegakernelExecutionTopology::FusedWave
            if fusion >= FUSION_PRESSURE - FUSION_PRESSURE_HYSTERESIS
                && decision.launch_pressure_bps
                    >= checked_bps_sub(
                        LAUNCH_PRESSURE_BPS,
                        LAUNCH_HYSTERESIS_BPS,
                        "launch hysteresis floor",
                    )
                && sample.readback_bytes >= FUSION_READBACK_BYTES
                && decision.memory_pressure_bps
                    <= checked_bps_sub(
                        MEMORY_RED_ZONE_BPS,
                        MEMORY_HYSTERESIS_BPS,
                        "memory hysteresis floor",
                    ) =>
        {
            MegakernelExecutionTopology::FusedWave
        }
        _ => decision.topology,
    }
}

/// Compute and validate a megakernel device-memory plan.
pub fn plan_megakernel_memory_budget(
    topology: MegakernelExecutionTopology,
    graph: MegakernelGraphShape,
    bytes_per_node: u64,
    bytes_per_edge: u64,
    frontier_bytes: u64,
    scratch_bytes: u64,
    output_bytes: u64,
    budget_bytes: u64,
) -> Result<MegakernelMemoryPlan, MegakernelMemoryError> {
    let node_bytes = checked_mul(graph.node_count, bytes_per_node, "node layout bytes")?;
    let edge_bytes = checked_mul(graph.edge_count, bytes_per_edge, "edge layout bytes")?;
    let graph_bytes = checked_add(node_bytes, edge_bytes, "graph layout bytes")?;
    let topology_scratch_bytes = topology_scratch_bytes(topology, scratch_bytes)?;
    let required_without_output =
        checked_add(graph_bytes, frontier_bytes, "graph plus frontier bytes")?;
    let required_without_output = checked_add(
        required_without_output,
        topology_scratch_bytes,
        "scratch bytes",
    )?;
    let required_bytes = checked_add(required_without_output, output_bytes, "output bytes")?;
    if required_bytes > budget_bytes {
        return Err(MegakernelMemoryError::OverBudget {
            topology,
            required_bytes,
            budget_bytes,
            node_count: graph.node_count,
            edge_count: graph.edge_count,
        });
    }
    Ok(MegakernelMemoryPlan {
        graph_bytes,
        frontier_bytes,
        scratch_bytes: topology_scratch_bytes,
        output_bytes,
        required_bytes,
        budget_bytes,
        memory_pressure_bps: pressure_bps(required_bytes, budget_bytes),
    })
}

/// Select a megakernel topology and validate its device-memory plan.
pub fn plan_megakernel_execution(
    sample: MegakernelExecutionSample,
    graph: MegakernelGraphShape,
    bytes_per_node: u64,
    bytes_per_edge: u64,
    frontier_bytes: u64,
    scratch_bytes: u64,
    output_bytes: u64,
    budget_bytes: u64,
    launch_overhead_ns: f64,
    fusion_pressure: f64,
) -> Result<MegakernelExecutionPlan, MegakernelMemoryError> {
    let sparse_memory = plan_megakernel_memory_budget(
        MegakernelExecutionTopology::SparseFrontier,
        graph,
        bytes_per_node,
        bytes_per_edge,
        frontier_bytes,
        scratch_bytes,
        output_bytes,
        budget_bytes,
    )?;
    let decision = select_megakernel_topology(
        sample,
        graph,
        MegakernelMemoryBudget {
            required_bytes: sparse_memory.required_bytes,
            budget_bytes,
        },
        launch_overhead_ns,
        fusion_pressure,
    );
    match plan_megakernel_memory_budget(
        decision.topology,
        graph,
        bytes_per_node,
        bytes_per_edge,
        frontier_bytes,
        scratch_bytes,
        output_bytes,
        budget_bytes,
    ) {
        Ok(memory) => Ok(MegakernelExecutionPlan {
            topology: decision.topology,
            memory,
            downgraded_to_sparse: false,
        }),
        Err(MegakernelMemoryError::OverBudget { .. })
            if decision.topology != MegakernelExecutionTopology::SparseFrontier =>
        {
            Ok(MegakernelExecutionPlan {
                topology: MegakernelExecutionTopology::SparseFrontier,
                memory: sparse_memory,
                downgraded_to_sparse: true,
            })
        }
        Err(error) => Err(error),
    }
}

fn finite_unit(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn pressure_bps(numerator: u64, denominator: u64) -> u32 {
    let clamped = pressure_bps_u64(numerator, denominator).min(10_000);
    match u32::try_from(clamped) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(
                "megakernel pressure conversion failed after clamping value {clamped}: {error}. Fix: inspect ratio/clamp invariants before topology selection."
            );
            10_000
        }
    }
}

fn pressure_bps_u64(numerator: u64, denominator: u64) -> u64 {
    crate::numeric::ratio_basis_points_u64_wide(
        numerator,
        denominator,
        if numerator == 0 { 0 } else { u64::MAX },
        "megakernel scheduler pressure",
        "megakernel execution",
    )
}

fn finite_ratio_bps(numerator: f64, denominator: f64, label: &'static str) -> u32 {
    crate::numeric::finite_f64_ratio_basis_points_round(
        numerator,
        denominator,
        u32::MAX,
        u32::MAX,
        label,
        "megakernel execution",
    )
}

fn checked_bps_sub(value: u32, margin: u32, label: &'static str) -> u32 {
    if let Some(result) = value.checked_sub(margin) {
        return result;
    }
    tracing::error!(
        "megakernel {label} underflowed basis-point threshold. Fix: configure hysteresis below the threshold."
    );
    0
}

fn topology_scratch_bytes(
    topology: MegakernelExecutionTopology,
    base_scratch_bytes: u64,
) -> Result<u64, MegakernelMemoryError> {
    match topology {
        MegakernelExecutionTopology::WarpSparseFrontier => Ok(base_scratch_bytes.max(32)),
        MegakernelExecutionTopology::SparseFrontier => Ok(base_scratch_bytes),
        MegakernelExecutionTopology::BlockDenseFrontier => checked_mul(
            base_scratch_bytes.max(1024),
            2,
            "block dense topology scratch bytes",
        ),
        MegakernelExecutionTopology::DenseFrontier => {
            checked_mul(base_scratch_bytes, 2, "dense topology scratch bytes")
        }
        MegakernelExecutionTopology::HybridFrontier => {
            checked_mul(base_scratch_bytes, 3, "hybrid topology scratch bytes")
        }
        MegakernelExecutionTopology::FusedWave => {
            checked_mul(base_scratch_bytes, 4, "fused topology scratch bytes")
        }
    }
}

fn checked_add(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, MegakernelMemoryError> {
    lhs.checked_add(rhs)
        .ok_or(MegakernelMemoryError::ByteCountOverflow { field })
}

fn checked_mul(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, MegakernelMemoryError> {
    lhs.checked_mul(rhs)
        .ok_or(MegakernelMemoryError::ByteCountOverflow { field })
}

#[cfg(test)]
mod tests {
    use super::{
        plan_megakernel_execution, plan_megakernel_memory_budget, select_megakernel_topology,
        select_megakernel_topology_stable, MegakernelExecutionSample, MegakernelExecutionTopology,
        MegakernelGraphShape, MegakernelMemoryBudget, MegakernelMemoryError,
    };

    #[test]
    fn topology_selector_uses_sparse_dense_hybrid_and_fused_bands() {
        let graph = MegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let memory = MegakernelMemoryBudget {
            required_bytes: 1_000,
            budget_bytes: 10_000,
        };
        let warp_sparse = select_megakernel_topology(
            MegakernelExecutionSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.01,
                readback_bytes: 256,
            },
            graph,
            memory,
            100.0,
            0.0,
        );
        assert_eq!(
            warp_sparse.topology,
            MegakernelExecutionTopology::WarpSparseFrontier
        );
        assert_eq!(
            warp_sparse.stable_explanation(),
            "megakernel-topology-v1|topology=WarpSparseFrontier|memory_pressure_bps=1000|average_degree_bps=40000|launch_pressure_bps=1000|reason=ultra_sparse_warp_specialized"
        );

        let block_dense = select_megakernel_topology(
            MegakernelExecutionSample {
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
            MegakernelExecutionTopology::BlockDenseFrontier
        );

        let hybrid = select_megakernel_topology(
            MegakernelExecutionSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.35,
                readback_bytes: 512,
            },
            graph,
            memory,
            100.0,
            0.0,
        );
        assert_eq!(hybrid.topology, MegakernelExecutionTopology::HybridFrontier);

        let fused = select_megakernel_topology(
            MegakernelExecutionSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.50,
                readback_bytes: 1 << 20,
            },
            graph,
            memory,
            250.0,
            0.90,
        );
        assert_eq!(fused.topology, MegakernelExecutionTopology::FusedWave);
        assert_eq!(fused.launch_pressure_bps, 2_500);
    }

    #[test]
    fn stable_topology_selector_prevents_variant_flapping_near_thresholds() {
        let graph = MegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let memory = MegakernelMemoryBudget {
            required_bytes: 1_000,
            budget_bytes: 10_000,
        };
        let sparse_to_hybrid = select_megakernel_topology_stable(
            MegakernelExecutionSample {
                dispatch_cost_ns: 1_000.0,
                frontier_density: 0.14,
                readback_bytes: 512,
            },
            graph,
            memory,
            100.0,
            0.0,
            MegakernelExecutionTopology::SparseFrontier,
        );
        assert_eq!(
            sparse_to_hybrid.topology,
            MegakernelExecutionTopology::SparseFrontier
        );
    }

    #[test]
    fn memory_planner_bounds_peak_bytes_by_topology() {
        let graph = MegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let plan = plan_megakernel_memory_budget(
            MegakernelExecutionTopology::FusedWave,
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
    fn memory_planner_rejects_budget_and_overflow_failures() {
        let graph = MegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let err = plan_megakernel_memory_budget(
            MegakernelExecutionTopology::DenseFrontier,
            graph,
            16,
            8,
            4_096,
            2_048,
            512,
            32 * 1024,
        )
        .expect_err("over-budget dense plan must fail before allocation");
        assert!(matches!(
            err,
            MegakernelMemoryError::OverBudget {
                topology: MegakernelExecutionTopology::DenseFrontier,
                ..
            }
        ));
        assert!(err.to_string().contains("Fix: choose a sparse topology"));

        let overflow = plan_megakernel_memory_budget(
            MegakernelExecutionTopology::SparseFrontier,
            MegakernelGraphShape {
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
            overflow,
            MegakernelMemoryError::ByteCountOverflow {
                field: "node layout bytes"
            }
        ));
    }

    #[test]
    fn generated_execution_plans_never_exceed_budget_or_hide_overflow() {
        let mut state = 0x4d59_5df4_d0f3_3173_u64;
        for case_index in 0..1024usize {
            let node_count = 1 + next_u64(&mut state) % 8_192;
            let edge_count = node_count + next_u64(&mut state) % 65_536;
            let bytes_per_node = 1 + next_u64(&mut state) % 64;
            let bytes_per_edge = 1 + next_u64(&mut state) % 32;
            let frontier_bytes = next_u64(&mut state) % 65_536;
            let scratch_bytes = next_u64(&mut state) % 16_384;
            let output_bytes = next_u64(&mut state) % 8_192;
            let budget_bytes = 64 * 1024 + next_u64(&mut state) % (4 * 1024 * 1024);
            let sample = MegakernelExecutionSample {
                dispatch_cost_ns: 100.0 + (next_u64(&mut state) % 10_000) as f64,
                frontier_density: (next_u64(&mut state) % 10_001) as f64 / 10_000.0,
                readback_bytes: next_u64(&mut state) % (1 << 20),
            };

            let result = plan_megakernel_execution(
                sample,
                MegakernelGraphShape {
                    node_count,
                    edge_count,
                },
                bytes_per_node,
                bytes_per_edge,
                frontier_bytes,
                scratch_bytes,
                output_bytes,
                budget_bytes,
                250.0,
                0.85,
            );
            match result {
                Ok(plan) => {
                    assert!(
                        plan.memory.required_bytes <= plan.memory.budget_bytes,
                        "case {case_index}"
                    );
                    assert!(plan.memory.memory_pressure_bps <= 10_000);
                }
                Err(MegakernelMemoryError::OverBudget {
                    required_bytes,
                    budget_bytes,
                    ..
                }) => assert!(required_bytes > budget_bytes, "case {case_index}"),
                Err(MegakernelMemoryError::ByteCountOverflow { .. }) => {}
            }
        }
    }

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }
}
