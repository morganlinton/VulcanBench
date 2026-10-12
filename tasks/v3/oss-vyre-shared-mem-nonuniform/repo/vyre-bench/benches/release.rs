#![allow(missing_docs)]
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn registry_inventory_collection(criterion: &mut Criterion) {
    criterion.bench_function("cold_setup_registry_inventory_collection", |bencher| {
        bencher.iter(|| {
            let registry = vyre_bench::registry::collect_all();
            assert!(
                registry.len() >= 10,
                "Fix: release bench inventory must expose the primitive corpus, not an empty harness."
            );
            registry.len()
        });
    });
}

fn bitset_and_cpu_ref_scale(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("primitive_cpu_ref/bitset_and");
    for &words in &[32usize, 256, 2_048, 16_384, 131_072] {
        let lhs: Vec<u32> = (0..words as u32).collect();
        let rhs: Vec<u32> = (0..words as u32).rev().collect();
        group.throughput(Throughput::Bytes((words * 4) as u64 * 2));
        group.bench_with_input(
            BenchmarkId::from_parameter(words),
            &(lhs.clone(), rhs.clone()),
            |bencher, (lhs, rhs)| {
                bencher.iter(|| {
                    let output =
                        vyre_primitives::bitset::and::cpu_ref(black_box(lhs), black_box(rhs));
                    black_box(output);
                });
            },
        );
    }
    group.finish();
}

fn bitset_and_cpu_ref_into_scale(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("primitive_cpu_ref/bitset_and_into");
    for &words in &[32usize, 256, 2_048, 16_384, 131_072] {
        let lhs: Vec<u32> = (0..words as u32).collect();
        let rhs: Vec<u32> = (0..words as u32).rev().collect();
        let mut output: Vec<u32> = Vec::with_capacity(words);
        group.throughput(Throughput::Bytes((words * 4) as u64 * 2));
        group.bench_with_input(
            BenchmarkId::from_parameter(words),
            &(lhs.clone(), rhs.clone()),
            |bencher, (lhs, rhs)| {
                bencher.iter(|| {
                    output.clear();
                    vyre_primitives::bitset::and::cpu_ref_into(
                        black_box(lhs),
                        black_box(rhs),
                        black_box(&mut output),
                    );
                    black_box(output.len());
                });
            },
        );
    }
    group.finish();
}

fn dominator_tree_vram_ceiling_bytes(
    node_count: u32,
    edge_count: u32,
    pred_edge_count: u32,
) -> u64 {
    let offset_bytes = (node_count as u64 + 1) * 4;
    let edge_target_bytes = edge_count.max(1) as u64 * 4;
    let pred_target_bytes = pred_edge_count.max(1) as u64 * 4;
    let idom_bytes = node_count.max(1) as u64 * 4;
    let depth_bytes = node_count.max(1) as u64 * 4;
    offset_bytes * 2 + edge_target_bytes + pred_target_bytes + idom_bytes + depth_bytes
}

fn dominator_tree_linear_chain_edges(nodes: u32) -> Vec<(u32, u32)> {
    (0..nodes.saturating_sub(1))
        .map(|node| (node, node + 1))
        .collect()
}

fn dominator_tree_fanout_tree_edges(nodes: u32) -> Vec<(u32, u32)> {
    let mut edges = Vec::new();
    for node in 0..nodes {
        let left = 2 * node + 1;
        let right = 2 * node + 2;
        if left < nodes {
            edges.push((node, left));
        }
        if right < nodes {
            edges.push((node, right));
        }
    }
    edges
}

fn dominator_tree_cpu_oracle_scale(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("primitive_cpu_ref/dominator_tree");
    for &(shape, build_edges) in &[
        (
            "linear_chain",
            dominator_tree_linear_chain_edges as fn(u32) -> Vec<(u32, u32)>,
        ),
        ("fanout_tree", dominator_tree_fanout_tree_edges),
    ] {
        for &nodes in &[1_000u32, 10_000, 100_000, 1_000_000] {
            let edges = build_edges(nodes);
            group.throughput(Throughput::Elements(nodes as u64));
            group.bench_with_input(
                BenchmarkId::new(shape, nodes),
                &(nodes, edges),
                |bencher, (nodes, edges)| {
                    bencher.iter(|| {
                        let output = vyre_primitives::graph::dominator_tree::cpu_ref(
                            black_box(*nodes),
                            0,
                            black_box(edges),
                        );
                        black_box(output);
                    });
                },
            );
        }
    }
    group.finish();
}

fn dominator_tree_program_build_scale(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("primitive_program_build/dominator_tree");
    for &nodes in &[1_000u32, 10_000, 100_000, 1_000_000] {
        let edge_count = nodes.saturating_sub(1);
        let vram_bytes = dominator_tree_vram_ceiling_bytes(nodes, edge_count, edge_count);
        group.throughput(Throughput::Elements(nodes as u64));
        group.bench_with_input(
            BenchmarkId::new("program", nodes),
            &nodes,
            |bencher, nodes| {
                bencher.iter(|| {
                    let program =
                        vyre_primitives::graph::dominator_tree::try_dominator_tree_program(
                            black_box(*nodes),
                            black_box(edge_count),
                            black_box(edge_count),
                            "idom",
                        )
                        .expect("Fix: benchmark dominator-tree sizes must stay buildable.");
                    black_box(program);
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("vram_bytes", nodes),
            &vram_bytes,
            |bencher, bytes| {
                bencher.iter(|| black_box(*bytes));
            },
        );
    }
    group.finish();
}

fn compiler_grade_release_program_build_scale(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("compiler_grade_release/program_build");
    let specs = vyre_bench::cases::release_workloads::release_macro_program_specs();
    assert!(
        specs.len() >= 10,
        "Fix: release Criterion benchmark must exercise the release macro workload builders."
    );
    for spec in specs {
        group.throughput(Throughput::Elements(spec.records as u64));
        group.bench_with_input(
            BenchmarkId::new("macro", spec.id),
            &spec,
            |bencher, spec| {
                bencher.iter(|| {
                    let program =
                        vyre_bench::cases::release_workloads::build_release_macro_program(
                            black_box(spec.id),
                        )
                        .expect("Fix: every release macro benchmark spec must build a Program.");
                    assert_eq!(
                        program
                            .buffers()
                            .iter()
                            .filter(|buffer| {
                                matches!(
                                    buffer.access(),
                                    vyre::ir::BufferAccess::ReadOnly
                                        | vyre::ir::BufferAccess::Uniform
                                )
                            })
                            .count(),
                        spec.input_buffers,
                        "Fix: release macro workload spec input-buffer count must match the generated Program."
                    );
                    black_box(program.fingerprint());
                });
            },
        );
    }
    group.finish();
}

fn nvme_gpu_ingest_telemetry_projection_scale(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("runtime_io/nvme_gpu_ingest_telemetry");
    for spec in vyre_bench::cases::nvme_gpu_ingest::nvme_gpu_ingest_specs() {
        let total_bytes = spec
            .total_bytes()
            .expect("Fix: ingest benchmark byte count must not overflow.");
        group.throughput(Throughput::Bytes(total_bytes));
        group.bench_with_input(
            BenchmarkId::new(spec.path_label(), total_bytes),
            spec,
            |bencher, spec| {
                bencher.iter(|| {
                    let telemetry =
                        vyre_bench::cases::nvme_gpu_ingest::synthesize_completed_ingest_telemetry(
                            black_box(*spec),
                        )
                        .expect("Fix: release ingest telemetry shape must be representable.");
                    vyre_bench::cases::nvme_gpu_ingest::validate_zero_copy_ingest_telemetry(
                        black_box(*spec),
                        black_box(telemetry),
                    )
                    .expect("Fix: release ingest telemetry must preserve zero-copy accounting.");
                    let points = vyre_bench::cases::nvme_gpu_ingest::ingest_telemetry_metric_points(
                        black_box(*spec),
                        black_box(telemetry),
                    );
                    black_box(points);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    release,
    registry_inventory_collection,
    bitset_and_cpu_ref_scale,
    bitset_and_cpu_ref_into_scale,
    dominator_tree_cpu_oracle_scale,
    dominator_tree_program_build_scale,
    compiler_grade_release_program_build_scale,
    nvme_gpu_ingest_telemetry_projection_scale
);
criterion_main!(release);
