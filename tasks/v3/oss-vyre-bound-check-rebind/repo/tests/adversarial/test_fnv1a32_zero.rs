// Integration test module for the containing Vyre package.

#![allow(missing_docs)]
use vyre_primitives::HashFnv1a as Marker;
use vyre_reference::{dual_impls::ReferenceEvaluator, workgroup::Memory};

// FINDING-1000: The vyre spec returns a u32 (4 bytes), but the CPU reference currently
// returns a u64 (8 bytes). This adversarial test currently fails.
#[test]
fn fnv1a_handles_all_zero_buffer_without_panic() {
    let fnv = Marker;
    let input = Memory::from_bytes(vec![0; 4096]);
    let result = fnv
        .evaluate(&[input])
        .expect("evaluation should succeed without panic");

    // The spec requires no panic. The actual hash value is expected to be a valid fnv1a hash.
    assert_eq!(
        result.bytes().len(),
        4,
        "fnv1a32 output should be exactly 4 bytes"
    );
}
