//! Cat-A GPU differential harness.
//!
//! Every vyre-libs Cat-A op is executed twice for the same input set:
//!
//!   1. Through `vyre_reference::reference_eval`  -  the pure-Rust reference interpreter
//!      that defines what the op *claims* to compute.
//!   2. Through `vyre_driver_wgpu::WgpuBackend::dispatch`  -  the real
//!      GPU path downstream consumers will actually hit.
//!
//! The test asserts byte-identity between the two unless the op's
//! `OpEntry` explicitly permits backend-defined transcendental drift
//! in ULPs. Any divergence beyond that contract is a P0 finding: a
//! Cat-A op that passes CPU conform but diverges on the 5090 would
//! silently corrupt every downstream consumer that dispatches the op
//! on GPU.
//!
//! This file is the single load-bearing gate between vyre-libs and
//! GPU-backed consumers. It ships with 0.6 so "release" is measurable.
//!
//! **Status 2026-04-20 on a live 5090:**
//!
//! Byte-identity CPU ↔ GPU differential sweep.
//!
//! V7-TEST-025 reset: the earlier per-op `diff_*` functions were
//! consolidated into a single `diff_universal_registry` test that
//! iterates every registered OpEntry. The consolidated test runs
//! every OpEntry whose build() + test_inputs() yields a Program and
//! fails loudly if fixtures or backend capabilities are missing.
//!
//! Per-op failure reproducers  -  when a specific GPU-side bug shows
//! up  -  live under findings.toml with a blocker + fix_plan. See
//! FINDING-GPU-5 / GPU-6 / GPU-7 / GPU-8 for the currently open
//! divergences (atomic lowering, matmul accumulator, wgpu validator
//! crash on substring, blake3 unsupported node).

#![allow(deprecated)]
use std::sync::OnceLock;

use vyre::ir::BufferAccess;
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::ir::Program;
use vyre_libs::harness::fp_contract::effective_tolerance;
use vyre_reference::value::Value;

fn backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        WgpuBackend::acquire()
            .expect("Fix: GPU adapter required for cat_a_gpu_differential. Run on a host with a working wgpu adapter.")
    })
}

fn run_cpu(program: &Program, inputs: Vec<Value>) -> Vec<Vec<u8>> {
    let outputs = vyre_reference::reference_eval(program, &inputs)
        .expect("reference backend must execute Cat-A op");
    outputs.into_iter().map(|v| v.to_bytes()).collect()
}

/// Run the standard optimizer pipeline (canonicalize + region_inline +
/// CSE + DCE) before handing off to the backend. Cat-A compositions
/// wrap their body in [`vyre::ir::Node::Region`] for debuggability;
/// the `region_inline` pass is what unrolls those wrappers into the
/// primitive nodes the wgpu backend actually knows how to lower.
fn lower_for_gpu(program: &Program) -> Program {
    vyre_foundation::optimizer::pre_lowering::optimize(program.clone())
}

fn run_gpu(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let lowered = lower_for_gpu(program);
    let config = dispatch_config_for(&lowered);
    backend()
        .dispatch(&lowered, &inputs, &config)
        .expect("5090 must execute Cat-A op")
}

fn dispatch_config_for(program: &Program) -> DispatchConfig {
    let mut config = DispatchConfig::default();
    let workgroup = program.workgroup_size();
    if workgroup[1] == 1 && workgroup[2] == 1 {
        return config;
    }

    let lanes = u64::from(workgroup[0])
        .saturating_mul(u64::from(workgroup[1]))
        .saturating_mul(u64::from(workgroup[2]));
    let max_writable_count = program
        .buffers()
        .iter()
        .filter(|decl| matches!(decl.access(), BufferAccess::ReadWrite) || decl.is_output())
        .map(|decl| u64::from(decl.count()))
        .max()
        .unwrap_or(1);
    assert!(
        max_writable_count <= lanes,
        "Fix: non-1D Cat-A program needs explicit multi-workgroup grid; workgroup={workgroup:?}, lanes={lanes}, writable={max_writable_count}"
    );
    config.grid_override = Some([1, 1, 1]);
    config
}

/// Assert CPU and GPU paths agree byte-for-byte on `program` given the
/// same input buffer set. `inputs_cpu` mirrors `inputs_gpu` (both are
/// the same bytes in the same declaration order)  -  they're separated
/// only because CPU wants `Value` and GPU wants `Vec<u8>`.
fn f32_to_ordered(bits: u32) -> u32 {
    if (bits & 0x8000_0000) != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

fn assert_buffer_within_tolerance(
    op: &str,
    buffer_index: usize,
    cpu: &[u8],
    gpu: &[u8],
    tolerance: u32,
) {
    assert_eq!(
        cpu.len(),
        gpu.len(),
        "{op}: CPU vs 5090 buffer #{buffer_index} length diverged under tolerance {tolerance}. CPU={} GPU={}",
        cpu.len(),
        gpu.len()
    );
    if tolerance == 0 {
        assert_eq!(
            cpu, gpu,
            "{op}: CPU vs 5090 diverged on RW buffer #{buffer_index}.\n  CPU: {:x?}\n  GPU: {:x?}",
            cpu, gpu
        );
        return;
    }
    assert_eq!(
        cpu.len() % 4,
        0,
        "{op}: tolerance-based compare requires f32-aligned output bytes. Fix: keep non-byte-identity ops on f32 outputs only."
    );
    for (lane, (cpu_word, gpu_word)) in cpu.chunks_exact(4).zip(gpu.chunks_exact(4)).enumerate() {
        let cpu_bits = u32::from_le_bytes(cpu_word.try_into().expect("4-byte chunk"));
        let gpu_bits = u32::from_le_bytes(gpu_word.try_into().expect("4-byte chunk"));
        let diff = f32_to_ordered(cpu_bits).abs_diff(f32_to_ordered(gpu_bits));
        assert!(
            diff <= tolerance,
            "{op}: CPU vs 5090 diverged above {} ULP on RW buffer #{} lane {}.\n  CPU bits: 0x{:08x}\n  GPU bits: 0x{:08x}\n  CPU: {:x?}\n  GPU: {:x?}",
            tolerance,
            buffer_index,
            lane,
            cpu_bits,
            gpu_bits,
            cpu,
            gpu
        );
    }
}

fn assert_diff(op: &'static str, tolerance: u32, program: &Program, inputs: Vec<Vec<u8>>) {
    let cpu_inputs: Vec<Value> = inputs.iter().cloned().map(Value::from).collect();
    let cpu = run_cpu(program, cpu_inputs);
    let gpu = run_gpu(program, inputs);
    assert_eq!(
        cpu.len(),
        gpu.len(),
        "{op}: CPU produced {} RW buffers, GPU produced {}",
        cpu.len(),
        gpu.len()
    );
    for (i, (c, g)) in cpu.iter().zip(gpu.iter()).enumerate() {
        assert_buffer_within_tolerance(op, i, c, g, tolerance);
    }
}

fn entry_by_id(op_id: &str) -> &'static vyre_libs::harness::OpEntry {
    vyre_libs::harness::all_entries()
        .find(|entry| entry.id == op_id)
        .expect("Fix: expected OpEntry to be registered")
}

fn run_entry_diff(entry: &'static vyre_libs::harness::OpEntry) {
    let program = (entry.build)();
    let input_cases = entry
        .test_inputs
        .expect("Fix: regression entry must provide test_inputs")();
    for inputs in input_cases {
        assert_diff(
            entry.id,
            effective_tolerance(entry.id, &program),
            &program,
            inputs,
        );
    }
}

fn missing_capability_reason(program: &vyre::ir::Program) -> Option<String> {
    let required = vyre_foundation::program_caps::scan(program);
    let backend = backend();
    // Use the boolean capability queries from the frozen VyreBackend trait.
    let check = vyre_foundation::program_caps::check_backend_capabilities(
        backend.id(),
        backend.supports_subgroup_ops(),
        backend.supports_f16(),
        backend.supports_bf16(),
        backend.supports_indirect_dispatch(),
        true,
        backend.supports_distributed_collectives(),
        backend.max_workgroup_size(),
        &required,
    );
    check.err().map(|missing| missing.to_string())
}

#[test]
fn diff_softmax_regression() {
    run_entry_diff(entry_by_id("vyre-libs::nn::softmax"));
}

#[test]
fn diff_attention_regression() {
    run_entry_diff(entry_by_id("vyre-libs::nn::attention"));
}

#[test]
fn diff_flash_attention_regression() {
    run_entry_diff(entry_by_id("vyre-libs::nn::flash_attention"));
}

#[test]
fn diff_catalog_ast_cse_structural_hash_regression() {
    run_entry_diff(entry_by_id(
        "vyre-libs::catalog::parsing::ast_cse_structural_hash::consumer_a",
    ));
    run_entry_diff(entry_by_id(
        "vyre-libs::catalog::parsing::ast_cse_structural_hash::consumer_b",
    ));
}

#[test]
fn diff_catalog_fnv1a64_regression() {
    run_entry_diff(entry_by_id("vyre-libs::catalog::hash::fnv1a64::consumer_a"));
}

#[test]
fn diff_fnv1a64_then_catalog_fnv1a64_regression() {
    run_entry_diff(entry_by_id("vyre-libs::hash::fnv1a64"));
    run_entry_diff(entry_by_id("vyre-libs::catalog::hash::fnv1a64::consumer_a"));
}

/// FINDING-GPU-7 regression. `substring_search` (binding 0 haystack, 1 needle,
/// 2 matches; body = nested `If`(overflow-safe guard) -> `Loop`(per-needle-byte
/// equality fold) -> `Store`) once tripped the wgpu validator at
/// `wgpu_core.rs:1187` on that nested control-flow emission. The naga-emit
/// control-flow lowering work since 0.6.3 resolved it; this pins the
/// GPU-vs-CPU byte-identity of the matches buffer so the validator crash
/// cannot silently return. Asserts real bytes (matches buffer equality) via
/// `run_entry_diff` -> `assert_diff`, not a shape check.
#[cfg(feature = "matching-substring")]
#[test]
fn diff_substring_search_gpu_regression() {
    run_entry_diff(entry_by_id("vyre-libs::scan::substring_search"));
}

#[test]
fn diff_universal_registry() {
    let mut failures = Vec::new();
    for entry in vyre_libs::harness::all_entries() {
        let Some(inputs_fn) = entry.test_inputs else {
            panic!(
                "{} has no test_inputs. Fix: every registry entry must provide GPU differential inputs.",
                entry.id
            );
        };
        assert!(
            entry.expected_output.is_some(),
            "{} has no expected_output. Fix: every registry entry must provide an oracle fixture before GPU differential coverage can pass.",
            entry.id
        );
        let program = (entry.build)();
        if let Some(reason) = missing_capability_reason(&program) {
            panic!(
                "{} missing backend capability: {reason}. Fix: wire the op or capability before running the differential sweep.",
                entry.id
            );
        }
        let input_cases = inputs_fn();
        assert!(
            !input_cases.is_empty(),
            "{} has empty test_inputs. Fix: empty GPU differential fixtures are zero coverage.",
            entry.id
        );

        println!("Checking universal registry entry: {}", entry.id);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for inputs in input_cases {
                assert_diff(
                    entry.id,
                    effective_tolerance(entry.id, &program),
                    &program,
                    inputs,
                );
            }
        }));
        if result.is_err() {
            failures.push(entry.id);
        }
    }
    if !failures.is_empty() {
        panic!(
            "The following GPU-CPU differential checks failed: {:?}",
            failures
        );
    }
}
