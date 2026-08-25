//! CUDA telemetry to scale-aware megakernel scheduler contracts.

use vyre_driver::DispatchConfig;
use vyre_driver_cuda::{
    plan_cuda_frontier_megakernel_execution, schedule_megakernel_from_cuda_samples,
    select_cuda_megakernel_topology, CudaBackend, CudaMegakernelAnalysisKind,
    CudaMegakernelDeviceKey, CudaMegakernelFrontierWave, CudaMegakernelGraphShape,
    CudaMegakernelMemoryBudget, CudaMegakernelPlanCache, CudaMegakernelScheduleSample,
    CudaMegakernelTopology, CudaMegakernelWaveDependency,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

fn ramp_u32_bytes(count: usize) -> Vec<u8> {
    (0..count as u32).flat_map(u32::to_le_bytes).collect()
}

fn add_one_program(count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(count),
            BufferDecl::output("out", 1, DataType::U32).with_count(count),
        ],
        [256, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(1)),
        )],
    )
}

fn sample_dispatch(
    backend: &CudaBackend,
    count: usize,
    frontier_density: f64,
) -> (CudaMegakernelScheduleSample, f64) {
    let input = ramp_u32_bytes(count);
    let input_ref = input.as_slice();
    let config = DispatchConfig::default();
    backend.reset_telemetry();
    let timed = backend
        .dispatch_borrowed_timed(&add_one_program(count as u32), &[input_ref], &config)
        .expect("Fix: CUDA timed dispatch must succeed before scheduler telemetry can be trusted.");
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.kernel_launches, 1,
        "Fix: candidate sampling must observe exactly one CUDA kernel launch."
    );
    assert!(
        telemetry.readback_bytes >= input.len() as u64,
        "Fix: candidate sampling must observe output readback bytes."
    );
    (
        CudaMegakernelScheduleSample {
            dispatch_cost_ns: timed.wall_ns as f64,
            frontier_density,
            readback_bytes: telemetry.readback_bytes,
        },
        timed.enqueue_ns.unwrap_or(timed.wall_ns) as f64,
    )
}

#[test]
fn cuda_runtime_telemetry_drives_scale_aware_megakernel_schedule() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let (small, launch_ns) = sample_dispatch(&backend, 8, 0.25);
    let (large, _) = sample_dispatch(&backend, 65_536, 0.95);
    assert!(
        large.readback_bytes > small.readback_bytes,
        "Fix: large telemetry sample must expose larger readback pressure."
    );

    let scheduler_small = CudaMegakernelScheduleSample {
        dispatch_cost_ns: 100.0,
        ..small
    };
    let scheduler_large = CudaMegakernelScheduleSample {
        dispatch_cost_ns: 10_000.0,
        ..large
    };
    let schedule = schedule_megakernel_from_cuda_samples(
        &[scheduler_small, scheduler_large],
        launch_ns,
        64,
        0.25,
    )
    .expect("Fix: CUDA telemetry samples must feed the scale-aware megakernel scheduler.");
    assert_eq!(schedule.len(), 2);
    assert!(
        schedule[1] > schedule[0],
        "Fix: CUDA telemetry-backed scheduling must prefer the dense, readback-heavy fusion candidate."
    );
    let dense_decision = select_cuda_megakernel_topology(
        scheduler_large,
        CudaMegakernelGraphShape {
            node_count: 65_536,
            edge_count: 262_144,
        },
        CudaMegakernelMemoryBudget {
            required_bytes: large.readback_bytes.saturating_mul(2),
            budget_bytes: large.readback_bytes.saturating_mul(16),
        },
        launch_ns,
        schedule[1],
    );
    assert!(
        matches!(
            dense_decision.topology,
            CudaMegakernelTopology::DenseFrontier | CudaMegakernelTopology::FusedWave
        ),
        "Fix: real dense CUDA telemetry must select a dense or fused megakernel topology, got {:?}.",
        dense_decision
    );
    let mut execution_cache = CudaMegakernelPlanCache::new();
    let execution_plan = execution_cache
        .get_or_plan_execution(
            0xC0DA_5090,
            CudaMegakernelAnalysisKind::Dataflow,
            CudaMegakernelDeviceKey::from(&backend.caps),
            scheduler_large,
            CudaMegakernelGraphShape {
                node_count: 65_536,
                edge_count: 262_144,
            },
            16,
            8,
            large.readback_bytes,
            large.readback_bytes / 4,
            large.readback_bytes,
            large.readback_bytes.saturating_mul(64),
            launch_ns,
            schedule[1],
        )
        .expect("Fix: live CUDA telemetry-selected megakernel plan must fit the explicit device-memory budget.");
    let cached_execution_plan = execution_cache
        .get_or_plan_execution(
            0xC0DA_5090,
            CudaMegakernelAnalysisKind::Dataflow,
            CudaMegakernelDeviceKey::from(&backend.caps),
            CudaMegakernelScheduleSample {
                frontier_density: (large.frontier_density + 0.01).min(1.0),
                ..scheduler_large
            },
            CudaMegakernelGraphShape {
                node_count: 65_536,
                edge_count: 262_144,
            },
            16,
            8,
            large.readback_bytes,
            large.readback_bytes / 4,
            large.readback_bytes,
            large.readback_bytes.saturating_mul(64),
            launch_ns,
            schedule[1],
        )
        .expect(
            "Fix: cache-equivalent live CUDA megakernel execution plan must still validate memory.",
        );
    assert_eq!(
        execution_plan.topology, dense_decision.topology,
        "Fix: CUDA executable megakernel plan must use the same topology selected from live telemetry."
    );
    assert_eq!(
        cached_execution_plan.topology, execution_plan.topology,
        "Fix: CUDA execution plan cache must reuse topology for equivalent live pressure buckets."
    );
    assert!(
        execution_plan.memory.required_bytes <= execution_plan.memory.budget_bytes,
        "Fix: CUDA execution planner must prove the selected topology fits before launch."
    );
    assert!(
        !execution_plan.downgraded_to_sparse,
        "Fix: high-budget dense CUDA telemetry should not be downgraded to sparse execution."
    );
    assert_eq!(
        execution_cache.stats().hits,
        1,
        "Fix: live cache-backed CUDA execution planning must record topology reuse."
    );
    let frontier_plan = plan_cuda_frontier_megakernel_execution(
        &mut execution_cache,
        0xC0DA_5091,
        CudaMegakernelAnalysisKind::ParserFrontend,
        CudaMegakernelDeviceKey::from(&backend.caps),
        scheduler_large,
        CudaMegakernelGraphShape {
            node_count: 65_536,
            edge_count: 262_144,
        },
        16,
        8,
        &[
            CudaMegakernelFrontierWave {
                frontier_bytes: large.readback_bytes / 8,
                scratch_bytes: large.readback_bytes / 16,
                output_bytes: large.readback_bytes / 16,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: large.readback_bytes / 4,
                scratch_bytes: large.readback_bytes / 16,
                output_bytes: large.readback_bytes / 8,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: large.readback_bytes / 4,
                scratch_bytes: large.readback_bytes / 16,
                output_bytes: large.readback_bytes / 8,
            },
            CudaMegakernelFrontierWave {
                frontier_bytes: large.readback_bytes / 2,
                scratch_bytes: large.readback_bytes / 8,
                output_bytes: large.readback_bytes / 4,
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
        large.readback_bytes.saturating_mul(64),
        launch_ns,
        schedule[1],
    )
    .expect("Fix: live CUDA dependency-aware frontier megakernel plan must fit the explicit memory budget.");
    assert_eq!(
        frontier_plan.barriers.global_barriers, 2,
        "Fix: live CUDA frontier waves must be grouped into dependency-minimal barrier phases."
    );
    assert_eq!(
        frontier_plan.barriers.groups[1].waves,
        vec![1, 2],
        "Fix: independent live CUDA frontier waves must fuse into one barrier-free phase."
    );
    assert!(
        matches!(
            frontier_plan.execution.topology,
            CudaMegakernelTopology::DenseFrontier | CudaMegakernelTopology::FusedWave
        ),
        "Fix: dense live CUDA frontier planning must keep a dense/fused execution topology."
    );
    assert!(
        frontier_plan.execution.memory.required_bytes
            <= frontier_plan.execution.memory.budget_bytes,
        "Fix: dependency-aware live CUDA frontier plan must prove memory fit before launch."
    );
    let sparse_decision = select_cuda_megakernel_topology(
        CudaMegakernelScheduleSample {
            frontier_density: 0.01,
            ..scheduler_small
        },
        CudaMegakernelGraphShape {
            node_count: 65_536,
            edge_count: 262_144,
        },
        CudaMegakernelMemoryBudget {
            required_bytes: small.readback_bytes,
            budget_bytes: large.readback_bytes.saturating_mul(16),
        },
        launch_ns,
        schedule[0],
    );
    assert!(
        matches!(
            sparse_decision.topology,
            CudaMegakernelTopology::WarpSparseFrontier | CudaMegakernelTopology::SparseFrontier
        ),
        "Fix: sparse CUDA telemetry must not be routed through dense megakernel topology; got {:?}.",
        sparse_decision.topology
    );

    let mut plan_cache = CudaMegakernelPlanCache::new();
    let device_key = CudaMegakernelDeviceKey::from(&backend.caps);
    let graph = CudaMegakernelGraphShape {
        node_count: 65_536,
        edge_count: 262_144,
    };
    let memory = CudaMegakernelMemoryBudget {
        required_bytes: large.readback_bytes.saturating_mul(2),
        budget_bytes: large.readback_bytes.saturating_mul(16),
    };
    let cached_first = plan_cache.get_or_select_topology(
        0xC0DA_5090,
        CudaMegakernelAnalysisKind::Dataflow,
        device_key,
        large,
        graph,
        memory,
        launch_ns,
        schedule[1],
    );
    let cached_second = plan_cache.get_or_select_topology(
        0xC0DA_5090,
        CudaMegakernelAnalysisKind::Dataflow,
        device_key,
        CudaMegakernelScheduleSample {
            frontier_density: (large.frontier_density + 0.01).min(1.0),
            ..large
        },
        graph,
        CudaMegakernelMemoryBudget {
            required_bytes: memory.required_bytes.saturating_add(256),
            budget_bytes: memory.budget_bytes,
        },
        launch_ns,
        schedule[1],
    );
    assert_eq!(
        cached_first, cached_second,
        "Fix: CUDA megakernel plan cache must reuse same graph/analysis/device pressure bucket instead of reselecting every query."
    );
    assert_eq!(
        plan_cache.stats().hits,
        1,
        "Fix: live CUDA megakernel plan cache must record a hit for repeated equivalent pressure."
    );

    let red_zone = plan_cache
        .get_or_select_topology(
            0xC0DA_5090,
            CudaMegakernelAnalysisKind::Dataflow,
            device_key,
            large,
            graph,
            CudaMegakernelMemoryBudget {
                required_bytes: memory.budget_bytes.saturating_mul(95) / 100,
                budget_bytes: memory.budget_bytes,
            },
            launch_ns,
            schedule[1],
        )
        .expect(
            "Fix: CUDA megakernel red-zone topology selection should fit cache telemetry counters.",
        );
    assert_eq!(
        red_zone.topology,
        CudaMegakernelTopology::SparseFrontier,
        "Fix: CUDA megakernel plan cache must not reuse a fused/dense plan when memory pressure moves into the red zone."
    );
}
