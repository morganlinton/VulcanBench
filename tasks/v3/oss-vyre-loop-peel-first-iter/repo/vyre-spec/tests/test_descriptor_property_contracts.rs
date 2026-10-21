//! Generated property coverage for invariant test descriptors.

use proptest::prelude::*;
use vyre_spec::{EngineInvariant, TestDescriptor};

const NAMES: &[&str] = &[
    "generated_happy_path",
    "generated_adversarial_path",
    "generated_wire_roundtrip",
    "generated_cuda_parity",
];

const PURPOSES: &[&str] = &[
    "Happy path: generated descriptor",
    "Adversarial path: generated descriptor",
    "Generated descriptor for certificate coverage",
    "",
];

const INVARIANTS: &[EngineInvariant] = &[
    EngineInvariant::I1,
    EngineInvariant::I2,
    EngineInvariant::I3,
    EngineInvariant::I4,
    EngineInvariant::I5,
    EngineInvariant::I6,
    EngineInvariant::I7,
    EngineInvariant::I8,
    EngineInvariant::I9,
    EngineInvariant::I10,
    EngineInvariant::I11,
    EngineInvariant::I12,
    EngineInvariant::I13,
    EngineInvariant::I14,
    EngineInvariant::I15,
];

proptest! {
    #[test]
    fn generated_test_descriptors_preserve_static_fields(
        name_index in 0usize..NAMES.len(),
        purpose_index in 0usize..PURPOSES.len(),
        invariant_index in 0usize..INVARIANTS.len(),
    ) {
        let descriptor = TestDescriptor {
            name: NAMES[name_index],
            purpose: PURPOSES[purpose_index],
            invariant: INVARIANTS[invariant_index],
        };

        prop_assert_eq!(descriptor.name, NAMES[name_index]);
        prop_assert_eq!(descriptor.purpose, PURPOSES[purpose_index]);
        prop_assert_eq!(descriptor.invariant, INVARIANTS[invariant_index]);
        prop_assert_eq!(
            descriptor.invariant.to_string(),
            format!("I{}", descriptor.invariant.ordinal())
        );
    }
}
