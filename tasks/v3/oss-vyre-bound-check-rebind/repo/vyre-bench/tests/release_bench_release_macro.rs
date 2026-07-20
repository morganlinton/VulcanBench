//! Contracts for the Criterion entrypoint exercising compiler-grade release workloads.

use vyre::ir::BufferAccess;
use vyre_bench::cases::release_workloads::{
    build_release_count_macro_case_for_records, build_release_macro_case_for_records,
    build_release_macro_program, release_macro_program_specs,
    release_macro_program_specs_for_records,
};
use vyre_reference::{reference_eval, value::Value};

const GENERATED_RECORD_COUNTS: u32 = 128;
const RELEASE_MACRO_CASES: usize = 10;
const GENERATED_ORACLE_CASES: usize = GENERATED_RECORD_COUNTS as usize * RELEASE_MACRO_CASES;

fn dispatch_input_buffer_count(
    case: &vyre_bench::cases::release_workloads::ReleaseMacroGeneratedCase,
) -> usize {
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

fn reference_outputs(
    case: &vyre_bench::cases::release_workloads::ReleaseMacroGeneratedCase,
) -> Vec<Vec<u8>> {
    let values = case
        .inputs
        .iter()
        .cloned()
        .map(Value::from)
        .collect::<Vec<_>>();
    reference_eval(&case.program, &values)
        .unwrap_or_else(|error| {
            panic!(
                "Fix: generated release macro case {} must execute on the reference interpreter: {error}",
                case.spec.id
            )
        })
        .into_iter()
        .map(|value| value.to_bytes())
        .collect()
}

#[test]
fn release_macro_program_specs_build_real_programs_for_criterion() {
    let specs = release_macro_program_specs();
    assert!(
        specs.len() >= 10,
        "Fix: Criterion release macro program builder coverage must include every synthetic release family."
    );

    let mut total_records = 0_u64;
    for spec in specs {
        assert!(
            spec.records >= 1_048_576,
            "Fix: release macro spec `{}` must stay at release scale.",
            spec.id
        );
        assert!(
            spec.min_speedup_x >= 100,
            "Fix: release macro spec `{}` must carry the CUDA 100x release contract.",
            spec.id
        );
        let program = build_release_macro_program(spec.id)
            .unwrap_or_else(|| panic!("Fix: release macro spec `{}` must build.", spec.id));
        let input_buffers = program
            .buffers()
            .iter()
            .filter(|buffer| {
                matches!(
                    buffer.access(),
                    vyre::ir::BufferAccess::ReadOnly | vyre::ir::BufferAccess::Uniform
                )
            })
            .count();
        assert_eq!(
            input_buffers, spec.input_buffers,
            "Fix: release macro spec `{}` input count must match generated Program.",
            spec.id
        );
        assert!(
            program.buffers().iter().any(|buffer| {
                matches!(
                    buffer.access(),
                    vyre::ir::BufferAccess::WriteOnly | vyre::ir::BufferAccess::ReadWrite
                )
            }),
            "Fix: release macro spec `{}` must produce at least one output buffer.",
            spec.id
        );
        total_records += u64::from(spec.records);
    }

    assert!(
        total_records >= 10 * 1_048_576,
        "Fix: Criterion release macro coverage must span at least ten million logical records."
    );
}

#[test]
fn generated_release_macro_cases_reference_eval_match_cpu_oracles_across_many_record_counts() {
    let mut cases_checked = 0usize;

    for records in 1..=GENERATED_RECORD_COUNTS {
        let specs = release_macro_program_specs_for_records(records);
        assert_eq!(
            specs.len(),
            RELEASE_MACRO_CASES,
            "Fix: generated release macro coverage must keep every compiler-grade family at records={records}."
        );

        for spec in specs {
            let case =
                build_release_macro_case_for_records(spec.id, spec.records).unwrap_or_else(|| {
                    panic!(
                        "Fix: generated release macro spec `{}` must build.",
                        spec.id
                    )
                });
            assert_eq!(
                case.spec, spec,
                "Fix: generated case spec must round-trip for {} at records={records}.",
                spec.id
            );
            assert_eq!(
                case.inputs.len(),
                dispatch_input_buffer_count(&case),
                "Fix: generated dispatch inputs must match non-write-only program buffers for {} at records={records}.",
                spec.id
            );

            let actual_outputs = reference_outputs(&case);
            assert_eq!(
                actual_outputs, case.expected_outputs,
                "Fix: reference interpreter diverged from CPU oracle for {} at records={records}.",
                spec.id
            );
            cases_checked += 1;
        }
    }

    assert_eq!(
        cases_checked, GENERATED_ORACLE_CASES,
        "Fix: release macro generated oracle matrix must execute more than a thousand concrete cases."
    );
}

#[test]
fn release_count_case_builder_excludes_bitmap_scatter_but_all_case_builder_executes_it() {
    let scatter_id = "release.string_bitmap_scatter.1m";
    assert!(
        build_release_count_macro_case_for_records(scatter_id, 33).is_none(),
        "Fix: count-only release macro builder must not advertise bitmap output as a count word."
    );

    let case = build_release_macro_case_for_records(scatter_id, 33)
        .expect("Fix: all-pattern release macro builder must include bitmap scatter.");
    assert_eq!(
        case.inputs.len(),
        3,
        "Fix: bitmap scatter dispatch must initialize out_flags plus both bitmap inputs."
    );
    assert_eq!(
        case.expected_outputs[0].len(),
        8,
        "Fix: 33 bitmap scatter records must produce two u32 output words."
    );
    assert_eq!(
        reference_outputs(&case),
        case.expected_outputs,
        "Fix: bitmap scatter all-pattern builder must produce a reference-executable oracle."
    );
}

#[test]
fn release_criterion_entrypoint_includes_release_macro_build_group() {
    let source = include_str!("../benches/release.rs");
    assert!(
        source.contains("compiler_grade_release_program_build_scale"),
        "Fix: release Criterion entrypoint must include compiler-grade release program builders."
    );
    assert!(
        source.contains("compiler_grade_release/program_build"),
        "Fix: benchmark group must be named as release macro program-build coverage."
    );
    assert!(
        source.contains("build_release_macro_program"),
        "Fix: Criterion must build release macro Programs, not only scan registry metadata."
    );
}
