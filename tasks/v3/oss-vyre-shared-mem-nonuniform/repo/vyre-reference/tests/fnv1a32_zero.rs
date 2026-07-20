//! Test: fnv1a32 zero.
#![allow(missing_docs)]

use vyre_primitives::HashFnv1a as Marker;
use vyre_reference::{dual_impls::ReferenceEvaluator, workgroup::Memory};

#[test]
fn fnv1a_handles_all_zero_buffer_without_panic() {
    let fnv = Marker;
    let input = Memory::from_bytes(vec![0; 4096]);
    let result = fnv
        .evaluate(&[input])
        .expect("evaluation should succeed without panic");

    assert_eq!(
        result.bytes().len(),
        4,
        "fnv1a32 output should be exactly 4 bytes"
    );
}
