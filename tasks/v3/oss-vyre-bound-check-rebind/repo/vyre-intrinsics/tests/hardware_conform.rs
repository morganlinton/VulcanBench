//! Cat-C hardware intrinsic differential harness.
//!
//! Iterates every registered `OpEntry` whose id begins with
//! `vyre-intrinsics::hardware::` and asserts the CPU reference matches
//! the declared `expected_output` bit-for-bit. This is the lightweight
//! gate; GPU conform tests run separately through the backend lowering
//! and dispatch suites.

use vyre_intrinsics::harness::{all_entries, OpEntry};
use vyre_reference::value::Value;

fn run_cpu(entry: &OpEntry, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let program = (entry.build)();
    let values: Vec<Value> = inputs
        .iter()
        .map(|b| Value::Bytes(b.clone().into()))
        .collect();
    vyre_reference::reference_eval(&program, &values)
        .expect("intrinsic must execute on CPU reference")
        .into_iter()
        .map(|v| v.to_bytes())
        .collect()
}

#[test]
fn hardware_intrinsics_match_expected_output() {
    let entries: Vec<_> = all_entries()
        .filter(|e| e.id.starts_with("vyre-intrinsics::hardware::"))
        .collect();
    assert!(
        !entries.is_empty(),
        "no intrinsic entries registered  -  feature gates or registration broken"
    );
    for entry in entries {
        let inputs = (entry.test_inputs.expect("test_inputs required"))();
        let expected = (entry.expected_output.expect("expected_output required"))();
        assert_eq!(
            inputs.len(),
            expected.len(),
            "{}: fixture count mismatch",
            entry.id
        );
        for (case, (case_inputs, case_expected)) in inputs.iter().zip(expected.iter()).enumerate() {
            let got = run_cpu(entry, case_inputs);
            assert_eq!(
                &got, case_expected,
                "{} case {}: CPU ref drifted from expected_output",
                entry.id, case
            );
        }
    }
}
