//! Live CUDA validation for generated compiler-grade release macro workloads.

use vyre::ir::BufferAccess;
use vyre::DispatchConfig;
use vyre_bench::cases::release_workloads::{
    build_release_macro_case_for_records, release_macro_program_specs_for_records,
    ReleaseMacroGeneratedCase,
};
use vyre_driver_cuda::CudaBackend;

const REDUCED_RECORDS: u32 = 512;
const RELEASE_MACRO_CASES: usize = 10;

fn dispatch_input_buffer_count(case: &ReleaseMacroGeneratedCase) -> usize {
    case.program
        .buffers()
        .iter()
        .filter(|buffer| {
            matches!(
                buffer.access(),
                BufferAccess::ReadOnly | BufferAccess::Uniform
            ) || matches!(buffer.access(), BufferAccess::ReadWrite) && !buffer.is_output
        })
        .count()
}

fn output_summary(bytes: &[u8]) -> String {
    let first_words = bytes
        .chunks_exact(4)
        .take(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();
    format!("{} bytes, first u32 words {first_words:?}", bytes.len())
}

#[test]
fn reduced_release_macro_workloads_match_cpu_oracles_on_live_cuda() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend must acquire on the GPU-required release validation host.");
    let specs = release_macro_program_specs_for_records(REDUCED_RECORDS);
    assert_eq!(
        specs.len(),
        RELEASE_MACRO_CASES,
        "Fix: live CUDA release macro validation must exercise every compiler-grade workload."
    );

    for spec in specs {
        let case = build_release_macro_case_for_records(spec.id, spec.records)
            .expect("Fix: every advertised release macro spec must build a generated case.");
        assert_eq!(
            case.inputs.len(),
            dispatch_input_buffer_count(&case),
            "Fix: generated release macro dispatch inputs must match non-write-only buffers for {}.",
            case.spec.id
        );

        let outputs = backend
            .dispatch(&case.program, &case.inputs, &DispatchConfig::default())
            .unwrap_or_else(|err| {
                panic!(
                    "Fix: live CUDA dispatch failed for release macro workload {}: {err}",
                    case.spec.id
                )
            });
        assert_eq!(
            outputs.len(),
            case.expected_outputs.len(),
            "Fix: CUDA output buffer count must match the CPU oracle for {}.",
            case.spec.id
        );
        assert_eq!(
            outputs, case.expected_outputs,
            "Fix: live CUDA release macro workload {} ({}) diverged from CPU oracle: expected {}, got {}.",
            case.spec.id,
            case.spec.name,
            output_summary(&case.expected_outputs[0]),
            output_summary(&outputs[0])
        );
    }
}
