//! RELEASE TEST LANE 15  -  every-op random-input stress test.
//!
//! For every OpEntry that ships `test_inputs` + `expected_output`, generate
//! bounded random inputs (10_000 when `CI_STRESS=1`) via a manual
//! `proptest::test_runner::TestRunner`, run each through the CPU reference
//! and the wgpu backend, and assert byte-identity (int) or within-ULP
//! (float) equivalence.

#![allow(clippy::filter_map_bool_then, clippy::unnecessary_map_or)]
#![allow(deprecated)]
use std::sync::OnceLock;

use proptest::test_runner::{Config, TestRunner};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_libs::harness::{all_entries, fp_contract};
use vyre_reference::value::Value;

mod common;

use common::every_op_random_inputs::{
    compare_outputs, gpu_dispatch_inputs, is_program_graph_frontier, missing_capability_reason,
    op_seed, random_amg_v_cycle_inputs, random_buffer_for, random_program_graph_frontier,
    randomize_buffer,
};

fn require_backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        WgpuBackend::acquire().expect(
            "every_op_random_input_stress: GPU adapter probe failed. Fix: verify nvidia-smi, WGPU_BACKEND, Vulkan drivers, and wgpu adapter selection.",
        )
    })
}

#[test]
fn every_op_random_input_stress() {
    let backend = require_backend();

    let count = if std::env::var("CI_STRESS").map_or(false, |v| v == "1") {
        10_000
    } else {
        std::env::var("VYRE_RANDOM_CASES")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|&value| value > 0)
            .unwrap_or(8)
    };

    let mut total_cases = 0u64;
    let mut failures = Vec::new();
    let op_filter = std::env::var("VYRE_RANDOM_OP_FILTER").ok();
    let mut matched_ops = 0usize;

    for entry in all_entries() {
        if let Some(filter) = op_filter.as_deref() {
            if !entry.id.contains(filter) {
                continue;
            }
        }
        matched_ops += 1;

        if entry.test_inputs.is_none() || entry.expected_output.is_none() {
            panic!(
                "{} is missing test_inputs or expected_output. Fix: every op in the random-input stress sweep must provide both.",
                entry.id
            );
        }

        let program = (entry.build)();

        if let Some(reason) = missing_capability_reason(backend, &program) {
            panic!("{} missing backend capability: {reason}. Fix: wire the op or capability before stress testing.", entry.id);
        }

        let fixture_inputs = entry.test_inputs.unwrap()();
        if fixture_inputs.is_empty() {
            panic!(
                "{} has no fixture inputs. Fix: provide at least one stress seed input.",
                entry.id
            );
        }
        let fixture_case = &fixture_inputs[0];
        let buffer_lens: Vec<usize> = fixture_case.iter().map(|b| b.len()).collect();

        let seed = op_seed(entry.id);
        println!(
            "stress: {}  -  evaluating {} deterministic random cases",
            entry.id, count
        );
        let config = Config {
            rng_seed: proptest::test_runner::RngSeed::Fixed(seed),
            ..Config::default()
        };
        let mut runner = TestRunner::new(config);

        let lowered = optimize(program.clone());
        let mut op_cases = 0u64;
        let mut op_failures = 0usize;

        for case_idx in 0..count {
            let random_inputs = if entry.id.contains("amg_v_cycle") {
                random_amg_v_cycle_inputs(fixture_case, &mut runner)
            } else {
                let mut random_inputs = Vec::with_capacity(buffer_lens.len());
                for (buffer_idx, &len) in buffer_lens.iter().enumerate() {
                    if randomize_buffer(entry.id, &program, buffer_idx) {
                        let buffer = program
                            .buffers()
                            .get(buffer_idx)
                            .expect("fixture input index must match program buffer index");
                        let random = if is_program_graph_frontier(&program, buffer_idx) {
                            random_program_graph_frontier(&program, len, &mut runner)
                        } else {
                            random_buffer_for(entry.id, buffer, len, &mut runner)
                        };
                        random_inputs.push(random);
                    } else {
                        random_inputs.push(fixture_case[buffer_idx].clone());
                    }
                }
                random_inputs
            };

            let cpu_values: Vec<Value> = random_inputs.iter().cloned().map(Value::from).collect();
            let cpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                vyre_reference::reference_eval(&program, &cpu_values)
            }));

            let cpu_outputs = match cpu_result {
                Ok(Ok(outputs)) => outputs
                    .into_iter()
                    .map(|v| v.to_bytes())
                    .collect::<Vec<_>>(),
                Ok(Err(_)) | Err(_) => {
                    // Reference rejected or panicked  -  no oracle for this input.
                    continue;
                }
            };

            let gpu_inputs = gpu_dispatch_inputs(&program, &random_inputs);
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                backend.dispatch(&lowered, &gpu_inputs, &DispatchConfig::default())
            }));
            let gpu_outputs = match gpu_result {
                Ok(Ok(o)) => o,
                Ok(Err(e)) => {
                    op_failures += 1;
                    if op_failures == 1 {
                        failures.push(format!(
                            "{} seed={} case={}: GPU dispatch error: {}",
                            entry.id, seed, case_idx, e
                        ));
                    }
                    continue;
                }
                Err(_) => {
                    op_failures += 1;
                    if op_failures == 1 {
                        failures.push(format!(
                            "{} seed={} case={}: GPU dispatch panicked",
                            entry.id, seed, case_idx
                        ));
                    }
                    continue;
                }
            };

            let tolerance = fp_contract::effective_tolerance(entry.id, &program);
            if let Err(msg) = compare_outputs(
                entry.id,
                &program,
                &cpu_outputs,
                &gpu_outputs,
                tolerance,
                seed,
            ) {
                op_failures += 1;
                if op_failures == 1 {
                    failures.push(format!("{} case={}: {msg}", entry.id, case_idx));
                }
            }

            op_cases += 1;
        }

        total_cases += op_cases;
        println!(
            "stress: {}  -  {} random cases evaluated, {} failures",
            entry.id, op_cases, op_failures
        );
    }

    if let Some(filter) = op_filter {
        assert!(
            matched_ops > 0,
            "VYRE_RANDOM_OP_FILTER={filter:?} matched no OpEntry ids. Fix: pass a substring of the target op id."
        );
    }

    if !failures.is_empty() {
        panic!(
            "every_op_random_input_stress failed for {} op(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    println!(
        "every_op_random_input_stress: {} total random cases passed",
        total_cases
    );
}
