//! Parity tests for vyre-primitives graph::toposort + graph::reachable
//! + graph::level_wave.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::graph::level_wave::{
    cpu_ref as level_wave_cpu, level_wave_dispatch_grid, level_wave_program,
};
use vyre_primitives::graph::reachable::{reachable, reachable_program};
use vyre_primitives::graph::toposort::{toposort, toposort_program};

/// Build CSR for `toposort_program`: offsets indexed by `to`, targets
/// listing `from`-nodes. Mirrors the CPU `toposort` "outgoing"
/// adjacency (edges grouped by their `to`-endpoint).
fn build_toposort_csr(node_count: u32, edges: &[(u32, u32)]) -> (Vec<u32>, Vec<u32>) {
    let n = node_count as usize;
    let mut counts = vec![0u32; n];
    for &(_from, to) in edges {
        counts[to as usize] += 1;
    }
    let mut offsets = vec![0u32; n + 1];
    for i in 0..n {
        offsets[i + 1] = offsets[i] + counts[i];
    }
    let mut cursor = offsets.clone();
    let mut targets = vec![0u32; edges.len()];
    for &(from, to) in edges {
        let slot = cursor[to as usize] as usize;
        targets[slot] = from;
        cursor[to as usize] += 1;
    }
    (offsets, targets)
}

fn run_toposort(backend: &CudaBackend, node_count: u32, edges: &[(u32, u32)]) -> Vec<u32> {
    let (offsets, targets) = build_toposort_csr(node_count, edges);
    let program = toposort_program(node_count, "offsets", "targets", "indeg", "queue", "order");
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(&offsets),
        // targets buffer; the kernel reads up to edge_count.
        if targets.is_empty() {
            vec![0u8; 4]
        } else {
            u32_bytes(&targets)
        },
        // indeg scratch  -  zero-init.
        vec![0u8; node_count.max(1) as usize * 4],
        // queue scratch  -  zero-init.
        vec![0u8; node_count.max(1) as usize * 4],
        // order out  -  zero-init.
        vec![0u8; node_count.max(1) as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    // workgroup [1,1,1], serial lane-0 kernel.
    config.grid_override = Some([1, 1, 1]);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[2]);
    out.truncate(node_count as usize);
    out
}

#[test]
fn cuda_toposort_two_node_chain() {
    with_live_backend("cuda_toposort_two_node_chain", |backend| {
        let edges = vec![(0u32, 1u32)];
        let cpu = toposort(2, &edges).expect("acyclic");
        let gpu = run_toposort(backend, 2, &edges);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![1, 0]);
    });
}

#[test]
fn cuda_toposort_diamond() {
    with_live_backend("cuda_toposort_diamond", |backend| {
        // 0 depends on 1 and 2; both depend on 3.
        let edges = vec![(0u32, 1u32), (0, 2), (1, 3), (2, 3)];
        let cpu = toposort(4, &edges).expect("acyclic diamond");
        let gpu = run_toposort(backend, 4, &edges);
        assert_eq!(gpu, cpu);
        let pos = |v: u32| gpu.iter().position(|&x| x == v).unwrap();
        assert!(pos(3) < pos(1));
        assert!(pos(3) < pos(2));
        assert!(pos(1) < pos(0));
        assert!(pos(2) < pos(0));
    });
}

#[test]
fn cuda_toposort_no_edges_emits_all_nodes() {
    with_live_backend("cuda_toposort_no_edges_emits_all_nodes", |backend| {
        let edges: Vec<(u32, u32)> = vec![];
        let cpu = toposort(3, &edges).expect("no-edge toposort");
        let gpu = run_toposort(backend, 3, &edges);
        // Both should permute {0,1,2}; the kernel emits in iteration
        // order which equals CPU LIFO order.
        let mut sorted_gpu = gpu.clone();
        sorted_gpu.sort_unstable();
        assert_eq!(sorted_gpu, vec![0, 1, 2]);
        let mut sorted_cpu = cpu.clone();
        sorted_cpu.sort_unstable();
        assert_eq!(sorted_cpu, sorted_gpu);
    });
}

// ---------------------------------------------------------------------
// reachable_program
// ---------------------------------------------------------------------

/// Build CSR for `reachable_program` (forward adjacency): offsets
/// indexed by `from`, targets list `to`-nodes. The kind-mask bit is
/// always set since `reachable_program` calls
/// `csr_forward_traverse(.., u32::MAX)`.
fn build_forward_csr(node_count: u32, edges: &[(u32, u32)]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let n = node_count as usize;
    let mut counts = vec![0u32; n];
    for &(from, _to) in edges {
        counts[from as usize] += 1;
    }
    let mut offsets = vec![0u32; n + 1];
    for i in 0..n {
        offsets[i + 1] = offsets[i] + counts[i];
    }
    let mut cursor = offsets.clone();
    let mut targets = vec![0u32; edges.len()];
    for &(from, to) in edges {
        let slot = cursor[from as usize] as usize;
        targets[slot] = to;
        cursor[from as usize] += 1;
    }
    let kind_mask = vec![u32::MAX; edges.len()];
    (offsets, targets, kind_mask)
}

fn pack_bitset(node_count: u32, members: &[u32]) -> Vec<u32> {
    let words = node_count.div_ceil(32).max(1) as usize;
    let mut bits = vec![0u32; words];
    for &v in members {
        bits[(v / 32) as usize] |= 1u32 << (v & 31);
    }
    bits
}

fn unpack_bitset(packed: &[u32], node_count: u32) -> Vec<u32> {
    let mut out = Vec::new();
    for v in 0..node_count {
        if (packed[(v / 32) as usize] >> (v & 31)) & 1 == 1 {
            out.push(v);
        }
    }
    out
}

fn run_reachable(
    backend: &CudaBackend,
    node_count: u32,
    edges: &[(u32, u32)],
    sources: &[u32],
    max_iters: u32,
) -> Vec<u32> {
    let (offsets, targets, kind_mask) = build_forward_csr(node_count, edges);
    let words = node_count.div_ceil(32).max(1) as usize;
    let edge_count = edges.len() as u32;
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let sources_packed = pack_bitset(node_count, sources);
    let program = reachable_program(node_count, edge_count.max(1), "src", "reach", max_iters);

    // The fused program declares pg_nodes, pg_edge_offsets, pg_edge_targets,
    // pg_edge_kind_mask, pg_node_tags, src, reach, reach_frontier_a,
    // reach_frontier_b. Order them by binding.
    let buffer_names: Vec<&str> = program.buffers().iter().map(|b| b.name()).collect();
    let mut inputs: Vec<Vec<u8>> = Vec::with_capacity(buffer_names.len());
    for name in &buffer_names {
        let declared_words = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == *name)
            .map(|buffer| buffer.count().max(1) as usize)
            .expect("buffer name came from program declaration");
        let buf = match *name {
            "pg_nodes" => u32_bytes(&pg_nodes),
            "pg_edge_offsets" => u32_bytes(&offsets),
            "pg_edge_targets" => {
                if targets.is_empty() {
                    vec![0u8; 4]
                } else {
                    u32_bytes(&targets)
                }
            }
            "pg_edge_kind_mask" => {
                if kind_mask.is_empty() {
                    vec![0u8; 4]
                } else {
                    u32_bytes(&kind_mask)
                }
            }
            "pg_node_tags" => u32_bytes(&pg_node_tags),
            "src" => u32_bytes(&sources_packed),
            "reach" | "reach_frontier_a" | "reach_frontier_b" => vec![0u8; declared_words * 4],
            other => panic!("Unexpected buffer in reachable_program: {other}"),
        };
        inputs.push(buf);
    }
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((node_count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    // Locate "reach" in the output positions (RW buffers are returned
    // in declaration order).
    let reach_idx = buffer_names
        .iter()
        .filter(|n| {
            matches!(
                **n,
                "pg_nodes"
                    | "pg_edge_offsets"
                    | "pg_edge_targets"
                    | "pg_edge_kind_mask"
                    | "pg_node_tags"
                    | "src"
                    | "reach"
                    | "reach_frontier_a"
                    | "reach_frontier_b"
            )
        })
        .position(|n| *n == "reach")
        .expect("reach buffer present");
    // RW outputs only; pg_* and src are ReadOnly so they don't appear
    // in `outputs`. Count RW buffers up to "reach".
    let rw_index_of_reach = program
        .buffers()
        .iter()
        .filter(|b| b.access() == BufferAccess::ReadWrite)
        .position(|b| b.name() == "reach")
        .expect("reach is RW");
    let _ = reach_idx;
    let mut packed = bytes_u32(&outputs[rw_index_of_reach]);
    packed.truncate(words);
    unpack_bitset(&packed, node_count)
}

#[test]
fn cuda_reachable_two_step_chain() {
    with_live_backend("cuda_reachable_two_step_chain", |backend| {
        // 0 -> 1 -> 2; sources={0}; expect {0,1,2}.
        let edges = vec![(0u32, 1u32), (1, 2)];
        let cpu: Vec<u32> = {
            let mut v: Vec<u32> = reachable(3, &edges, &[0]).unwrap().into_iter().collect();
            v.sort_unstable();
            v
        };
        let gpu = run_reachable(backend, 3, &edges, &[0], 4);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0, 1, 2]);
    });
}

#[test]
fn cuda_reachable_disconnected() {
    with_live_backend("cuda_reachable_disconnected", |backend| {
        // 0 -> 1, 2 -> 3; sources={0}; expect {0,1}.
        let edges = vec![(0u32, 1u32), (2, 3)];
        let cpu: Vec<u32> = {
            let mut v: Vec<u32> = reachable(4, &edges, &[0]).unwrap().into_iter().collect();
            v.sort_unstable();
            v
        };
        let gpu = run_reachable(backend, 4, &edges, &[0], 4);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0, 1]);
    });
}

#[test]
fn cuda_reachable_diamond_converges() {
    with_live_backend("cuda_reachable_diamond_converges", |backend| {
        let edges = vec![(0u32, 1u32), (0, 2), (1, 3), (2, 3)];
        let cpu: Vec<u32> = {
            let mut v: Vec<u32> = reachable(4, &edges, &[0]).unwrap().into_iter().collect();
            v.sort_unstable();
            v
        };
        let gpu = run_reachable(backend, 4, &edges, &[0], 4);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![0, 1, 2, 3]);
    });
}

#[test]
fn cuda_reachable_depth_cap_returns_only_requested_waves() {
    with_live_backend(
        "cuda_reachable_depth_cap_returns_only_requested_waves",
        |backend| {
            let edges = vec![(0u32, 1u32), (1, 2), (2, 3), (3, 4)];
            let gpu = run_reachable(backend, 5, &edges, &[0], 2);

            assert_eq!(gpu, vec![0, 1, 2]);
        },
    );
}

#[test]
fn cuda_reachable_multi_block_wave_handoff() {
    with_live_backend("cuda_reachable_multi_block_wave_handoff", |backend| {
        let edges = vec![(255u32, 256u32), (256, 512)];
        let gpu = run_reachable(backend, 513, &edges, &[255], 2);

        assert_eq!(gpu, vec![255, 256, 512]);
    });
}

#[test]
fn cuda_reachable_cycle_feeds_only_new_bits() {
    with_live_backend("cuda_reachable_cycle_feeds_only_new_bits", |backend| {
        let edges = vec![(0u32, 1u32), (1, 0), (1, 2), (2, 2)];
        let gpu = run_reachable(backend, 3, &edges, &[0], 8);

        assert_eq!(gpu, vec![0, 1, 2]);
    });
}

// ---------------------------------------------------------------------
// level_wave_program
// ---------------------------------------------------------------------

fn run_level_wave(backend: &CudaBackend, depths: &[u32], max_depth: u32) -> Vec<u32> {
    // step_body: counter[lane] += 1. Buffer must be declared by the
    // wrapper; we pass it explicitly by hand-writing a Program that
    // composes a counter-buffer onto level_wave_program.
    let lane = Expr::InvocationId { axis: 0 };
    let lane_count = depths.len() as u32;
    let step = vec![Node::store(
        "counter",
        lane.clone(),
        Expr::add(Expr::load("counter", lane.clone()), Expr::u32(1)),
    )];
    let inner = level_wave_program(step, "depths", max_depth, lane_count);

    // Re-wrap to add the counter buffer (the inner program declared
    // only `depths`).
    let mut buffers: Vec<BufferDecl> = inner.buffers().to_vec();
    buffers.push(
        BufferDecl::storage(
            "counter",
            buffers.len() as u32,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(lane_count),
    );
    let program = Program::wrapped(buffers, inner.workgroup_size, inner.entry().to_vec());

    let inputs: Vec<Vec<u8>> = vec![u32_bytes(depths), vec![0u8; lane_count as usize * 4]];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(level_wave_dispatch_grid(lane_count));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(lane_count as usize);
    out
}

fn run_level_wave_cross_block_dependency(
    backend: &CudaBackend,
    depths: &[u32],
    max_depth: u32,
) -> Vec<u32> {
    let lane = Expr::InvocationId { axis: 0 };
    let lane_count = depths.len() as u32;
    let depth = Expr::load("depths", lane.clone());
    let step = vec![
        Node::if_then(
            Expr::eq(depth.clone(), Expr::u32(0)),
            vec![Node::store("counter", lane.clone(), Expr::u32(1))],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(depth, Expr::u32(1)),
                Expr::ge(lane.clone(), Expr::u32(256)),
            ),
            vec![Node::store(
                "counter",
                lane.clone(),
                Expr::add(
                    Expr::load("counter", Expr::sub(lane.clone(), Expr::u32(256))),
                    Expr::u32(1),
                ),
            )],
        ),
    ];
    let inner = level_wave_program(step, "depths", max_depth, lane_count);
    let mut buffers: Vec<BufferDecl> = inner.buffers().to_vec();
    buffers.push(
        BufferDecl::storage(
            "counter",
            buffers.len() as u32,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(lane_count),
    );
    let program = Program::wrapped(buffers, inner.workgroup_size, inner.entry().to_vec());
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(depths), vec![0u8; lane_count as usize * 4]];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(level_wave_dispatch_grid(lane_count));
    let outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("dispatch");
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(lane_count as usize);
    out
}

#[test]
fn cuda_level_wave_visits_each_lane_exactly_once() {
    with_live_backend("cuda_level_wave_visits_each_lane_exactly_once", |backend| {
        let depths = vec![0u32, 1, 2, 1, 0];
        let max_depth = 3;
        // CPU oracle: each lane's counter must end at exactly 1.
        let mut cpu = vec![0u32; depths.len()];
        level_wave_cpu(&depths, max_depth, |lane, _depth| {
            cpu[lane as usize] += 1;
        });
        let gpu = run_level_wave(backend, &depths, max_depth);
        assert_eq!(gpu, cpu);
        assert!(
            gpu.iter().all(|&c| c == 1),
            "every lane visited once: {:?}",
            gpu
        );
    });
}

#[test]
fn cuda_level_wave_skips_lanes_outside_max_depth() {
    with_live_backend("cuda_level_wave_skips_lanes_outside_max_depth", |backend| {
        // max_depth=2 → lanes with depth>=2 never fire.
        let depths = vec![0u32, 1, 2, 3, 0];
        let max_depth = 2;
        let mut cpu = vec![0u32; depths.len()];
        level_wave_cpu(&depths, max_depth, |lane, _depth| {
            cpu[lane as usize] += 1;
        });
        let gpu = run_level_wave(backend, &depths, max_depth);
        assert_eq!(gpu, cpu);
        assert_eq!(gpu, vec![1, 1, 0, 0, 1]);
    });
}

#[test]
fn cuda_level_wave_multi_block_orders_cross_block_dependencies() {
    with_live_backend(
        "cuda_level_wave_multi_block_orders_cross_block_dependencies",
        |backend| {
            let mut depths = vec![2u32; 513];
            for lane in 0..256 {
                depths[lane] = 0;
                depths[lane + 256] = 1;
            }
            let gpu = run_level_wave_cross_block_dependency(backend, &depths, 2);
            assert!(
                gpu[..256].iter().all(|&value| value == 1),
                "depth-0 producer lanes must fire before dependent wave: {:?}",
                &gpu[..256]
            );
            assert!(
                gpu[256..512].iter().all(|&value| value == 2),
                "depth-1 lanes in block 1 must see block-0 depth-0 writes"
            );
            assert_eq!(gpu[512], 0);
        },
    );
}
