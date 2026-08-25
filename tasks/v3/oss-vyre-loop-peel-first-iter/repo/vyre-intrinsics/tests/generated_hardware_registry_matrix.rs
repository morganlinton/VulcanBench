//! Generated registry matrix for Cat-C hardware intrinsic descriptors.
//!
//! The registry is the handoff point used by conformance, lowering, and
//! backend release gates. These tests pin the declarative shape metadata so
//! future CUDA/Vulkan/WGPU gates can reason about arity and semantics without
//! reverse-engineering fixture buffers.

use std::collections::BTreeMap;

use vyre_intrinsics::harness::{all_entries, HardwareSemantic, OpEntry, OpShape};
use vyre_reference::value::Value;

#[derive(Clone, Copy)]
struct ExpectedHardwareEntry {
    id: &'static str,
    shape: OpShape,
}

const EXPECTED: &[ExpectedHardwareEntry] = &[
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::bit_reverse_u32",
        shape: OpShape::new(1, 1, 4, HardwareSemantic::UnaryU32Map),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::fma_f32",
        shape: OpShape::new(3, 1, 4, HardwareSemantic::FmaF32),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::inverse_sqrt_f32",
        shape: OpShape::new(1, 1, 4, HardwareSemantic::InverseSqrtF32),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::popcount_u32",
        shape: OpShape::new(1, 1, 4, HardwareSemantic::UnaryU32Map),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::storage_barrier",
        shape: OpShape::new(1, 1, 4, HardwareSemantic::BarrierIdentityU32),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::subgroup_add",
        shape: OpShape::new(1, 1, 4, HardwareSemantic::SubgroupAddU32),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::subgroup_ballot",
        shape: OpShape::new(1, 1, 4, HardwareSemantic::SubgroupBallotU32),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::subgroup_shuffle",
        shape: OpShape::new(2, 1, 4, HardwareSemantic::SubgroupShuffleU32),
    },
    ExpectedHardwareEntry {
        id: "vyre-intrinsics::hardware::workgroup_barrier",
        shape: OpShape::new(1, 1, 4, HardwareSemantic::BarrierIdentityU32),
    },
];

fn hardware_entries() -> BTreeMap<&'static str, &'static OpEntry> {
    all_entries()
        .filter(|entry| entry.id.starts_with("vyre-intrinsics::hardware::"))
        .map(|entry| (entry.id, entry))
        .collect()
}

fn run_cpu(entry: &OpEntry, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let program = (entry.build)();
    let values = inputs
        .iter()
        .map(|bytes| Value::Bytes(bytes.clone().into()))
        .collect::<Vec<_>>();
    vyre_reference::reference_eval(&program, &values)
        .expect("Fix: registered hardware intrinsic must execute on the CPU oracle.")
        .into_iter()
        .map(|value| value.to_bytes())
        .collect()
}

#[test]
fn generated_hardware_registry_shapes_match_declared_surface() {
    let entries = hardware_entries();
    assert_eq!(
        entries.len(),
        EXPECTED.len(),
        "hardware registry must contain exactly the declared Cat-C surface"
    );

    for expected in EXPECTED {
        let entry = entries
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing hardware registry entry {}", expected.id));
        let shape = entry
            .shape()
            .unwrap_or_else(|| panic!("missing OpShape for {}", expected.id));
        assert_eq!(entry.category(), Some("hardware"), "{}", expected.id);
        assert_eq!(shape, expected.shape, "{}", expected.id);
        assert_eq!(shape.lane_bytes, 4, "{}", expected.id);
        assert_eq!(shape.output_buffers, 1, "{}", expected.id);

        let fixture_inputs = (entry
            .test_inputs
            .expect("Fix: hardware entry must expose generated test inputs"))(
        );
        let fixture_expected = (entry
            .expected_output
            .expect("Fix: hardware entry must expose generated expected outputs"))(
        );
        assert_eq!(
            fixture_inputs.len(),
            fixture_expected.len(),
            "{}",
            expected.id
        );
        for (case_inputs, case_expected) in fixture_inputs.iter().zip(fixture_expected.iter()) {
            assert_eq!(
                case_inputs.len(),
                shape.total_buffers() as usize,
                "{} fixture arity must match OpShape",
                expected.id
            );
            assert_eq!(
                case_expected.len(),
                shape.output_buffers as usize,
                "{} expected arity must match OpShape",
                expected.id
            );
            for output in case_expected {
                assert!(
                    !output.is_empty() && output.len() % shape.lane_bytes as usize == 0,
                    "{} expected output must be non-empty lane-aligned bytes",
                    expected.id
                );
            }
            assert_eq!(
                run_cpu(entry, case_inputs),
                *case_expected,
                "{}",
                expected.id
            );
        }
    }
}

#[test]
fn generated_hardware_registry_is_stable_across_thousands_of_lookup_paths() {
    let entries = hardware_entries();
    let mut assertions = 0usize;

    for seed in 0usize..4096 {
        let expected = EXPECTED[seed % EXPECTED.len()];
        let entry = entries
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing hardware registry entry {}", expected.id));
        let shape = entry.shape().expect("hardware entry must expose OpShape");
        let fixture_inputs = (entry.test_inputs.expect("hardware test inputs required"))();
        let fixture_expected = (entry
            .expected_output
            .expect("hardware expected output required"))();
        let case = seed % fixture_inputs.len();

        assert_eq!(entry.id, expected.id);
        assert_eq!(entry.category(), Some("hardware"));
        assert_eq!(shape, expected.shape);
        assert_eq!(shape.total_buffers() as usize, fixture_inputs[case].len());
        assert_eq!(shape.output_buffers as usize, fixture_expected[case].len());
        assert_eq!(
            run_cpu(entry, &fixture_inputs[case]),
            fixture_expected[case]
        );
        assertions += 6;
    }

    assert_eq!(assertions, 4096 * 6);
}
