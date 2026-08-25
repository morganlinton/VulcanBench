//! Universal Cat-A harness integration test.
//!
//! Rebuilds the harness checks used by the Cat-A operator registry and emits a
//! stable `blake3` fingerprint for each registered program.

#![allow(deprecated)]
use blake3::Hash;
use vyre::ir::Program;
use vyre::{
    backend::{backend_dispatches, registered_backends},
    BackendRegistration, DispatchConfig,
};
use vyre_foundation::optimizer::pre_lowering::optimize;
use vyre_foundation::validate::{BackendCapabilities, ValidationOptions};
use vyre_libs::harness::{all_entries, OpEntry};
use vyre_reference::value::Value;

#[test]
fn universal_cat_a_harness() {
    for entry in all_entries() {
        let program = build_program(entry);

        assert_valid(&program, entry.id);

        let wire = program
            .to_wire()
            .unwrap_or_else(|error| panic!("[harness] {} wire encode failed: {error}", entry.id));

        let round_trip = Program::from_wire(&wire)
            .unwrap_or_else(|error| panic!("[harness] {} wire decode failed: {error}", entry.id));

        assert_eq!(
            program, round_trip,
            "[harness] {}: wire round-trip must be stable",
            entry.id
        );

        let optimized_once = optimize(program.clone());
        let optimized_twice = optimize(optimized_once.clone());
        assert_eq!(
            optimized_once, optimized_twice,
            "[harness] {}: optimize(optimize(p)) must equal optimize(p)",
            entry.id
        );

        let fingerprint = blake3::hash(&wire);
        println!(
            "[harness] {} fingerprint={} output_buffers={}",
            entry.id,
            fingerprint,
            output_buffer_indices(&program).len()
        );

        check_oracle(entry, &program, fingerprint);
        check_registered_backends(entry, &program);
    }
}

fn assert_valid(program: &Program, id: &str) {
    let errors = vyre_foundation::validate::validate_with_options(
        program,
        ValidationOptions::universal().with_backend_capabilities(BackendCapabilities {
            supports_subgroup_ops: true,
            supports_indirect_dispatch: true,
            supports_specialization_constants: true,
            has_mul_high: true,
            has_dual_issue_fp32_int32: true,
            has_tensor_core_int: true,
            has_native_f16: true,
            has_warp_shuffle: true,
            has_shared_memory: true,
            has_transcendental_polynomial_emit: true,
            supports_distributed_collectives: true,
            max_native_int_width: 64,
        }),
    )
    .errors;
    assert!(
        errors.is_empty(),
        "[harness] {} validation failed: {:?}",
        id,
        errors
            .into_iter()
            .map(|error| error.message().to_string())
            .collect::<Vec<_>>()
    );
}

fn check_oracle(entry: &OpEntry, program: &Program, _fingerprint: Hash) {
    let cpu_ref = entry.expected_output;
    if cpu_ref.is_none() {
        panic!(
            "{} has no expected_output. Fix: every Cat-A entry must provide a reference oracle.",
            entry.id
        );
    }

    let Some(test_inputs) = entry.test_inputs else {
        panic!(
            "{} has no test_inputs. Fix: every Cat-A entry must provide reference input cases.",
            entry.id
        );
    };

    let input_cases = test_inputs();
    let expected_cases = cpu_ref.unwrap()();
    assert_eq!(
        input_cases.len(),
        expected_cases.len(),
        "[harness] {}: test_inputs and expected_output vector count mismatch",
        entry.id
    );

    for (case_idx, (input_bytes, cpu_output)) in input_cases
        .into_iter()
        .zip(expected_cases.into_iter())
        .enumerate()
    {
        let reference_inputs = input_bytes
            .iter()
            .map(|bytes| Value::Bytes(bytes.as_slice().into()))
            .collect::<Vec<_>>();

        let reference_output = vyre_reference::reference_eval(program, &reference_inputs)
            .unwrap_or_else(|error| {
                panic!(
                    "[harness] {} (case {}): reference interpreter failed: {error}",
                    entry.id, case_idx
                )
            })
            .into_iter()
            .map(|value| value.to_bytes())
            .collect::<Vec<_>>();

        let output_indices = output_buffer_indices(program);
        assert_eq!(
            output_indices.len(),
            reference_output.len(),
            "[harness] {} (case {}): expected {} output buffer(s) from reference run but got {}",
            entry.id,
            case_idx,
            output_indices.len(),
            reference_output.len()
        );
        assert_eq!(
            output_indices.len(),
            cpu_output.len(),
            "[harness] {} (case {}): expected {} output buffer(s) from expected_output but got {}",
            entry.id,
            case_idx,
            output_indices.len(),
            cpu_output.len()
        );

        for (decl_index, output_position) in output_indices.iter().copied().enumerate() {
            let expected = &reference_output[decl_index];
            let actual = &cpu_output[decl_index];
            let diff = first_byte_diff(expected, actual)
                .map(|index| {
                    format!(
                        "first differing byte {index}; reference_word={:?} oracle_word={:?}",
                        le_word_at(expected, index),
                        le_word_at(actual, index)
                    )
                })
                .unwrap_or_else(|| "byte contents match but lengths differ".to_string());
            assert_eq!(
                expected, actual,
                "[harness] {} (case {}): output mismatch for declaration index {output_position}; {diff}; reference_len={} oracle_len={}",
                entry.id,
                case_idx,
                expected.len(),
                actual.len()
            );
        }
    }
}

fn first_byte_diff(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .or_else(|| (left.len() != right.len()).then_some(left.len().min(right.len())))
}

fn le_word_at(bytes: &[u8], byte_index: usize) -> Option<u32> {
    let word_index = byte_index / 4;
    let start = word_index.checked_mul(4)?;
    let chunk = bytes.get(start..start + 4)?;
    Some(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

fn check_registered_backends(entry: &OpEntry, program: &Program) {
    let Some(test_inputs) = entry.test_inputs else {
        panic!(
            "{} has no test_inputs. Fix: every Cat-A entry must provide backend input cases.",
            entry.id
        );
    };
    let Some(expected_output) = entry.expected_output else {
        panic!("{} has no expected_output. Fix: every Cat-A entry must provide backend expected output.", entry.id);
    };

    let input_cases = test_inputs();
    let expected_cases = expected_output();

    for backend in registered_backends() {
        run_backend_contract(entry, program, backend, &input_cases, &expected_cases);
    }
}

fn run_backend_contract(
    entry: &OpEntry,
    program: &Program,
    backend: &BackendRegistration,
    input_cases: &[Vec<Vec<u8>>],
    expected_cases: &[Vec<Vec<u8>>],
) {
    let probe = (backend.factory)();
    let instance = match probe {
        Ok(instance) => instance,
        Err(error) => {
            panic!(
                "backend {} probe failed for {}: {}. Fix: repair backend registration or environment instead of skipping.",
                backend.id, entry.id, error
            );
        }
    };

    if backend_dispatches(backend.id) {
        let lowered = optimize(program.clone());
        for (case_idx, (inputs, expected)) in input_cases.iter().zip(expected_cases).enumerate() {
            let actual = instance
                .dispatch(&lowered, inputs, &DispatchConfig::default())
                .unwrap_or_else(|error| {
                    panic!(
                        "[harness] backend {} {} (case {}): dispatch failed: {}",
                        backend.id, entry.id, case_idx, error
                    )
                });
            assert_eq!(
                actual, *expected,
                "[harness] backend {} {} (case {}): byte mismatch",
                backend.id, entry.id, case_idx
            );
        }
    } else {
        for (case_idx, inputs) in input_cases.iter().enumerate() {
            let error = instance
                .dispatch(program, inputs, &DispatchConfig::default())
                .expect_err(&format!(
                    "[harness] backend {} {} (case {}): non-dispatch backend unexpectedly dispatched",
                    backend.id, entry.id, case_idx
                ));
            let message = error.to_string();
            assert!(
                message.contains("Fix:"),
                "[harness] backend {} {} (case {}): actionable error required, got `{message}`",
                backend.id,
                entry.id,
                case_idx
            );
        }
    }
}

fn output_buffer_indices(program: &Program) -> Vec<usize> {
    program
        .output_buffer_indices()
        .iter()
        .map(|&index| index as usize)
        .collect()
}

fn build_program(entry: &OpEntry) -> Program {
    (entry.build)()
}
