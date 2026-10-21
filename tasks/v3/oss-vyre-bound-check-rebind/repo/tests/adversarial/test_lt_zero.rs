// Integration test module for the containing Vyre package.

use vyre_primitives::CompareLt;
use vyre_reference::dual_impls::ReferenceEvaluator;
use vyre_reference::workgroup::Memory;

// FINDING-ADV-LT-ZERO - hostile zero-buffer compare must not panic (registry adversarial corpus).
// Expected catastrophic failure: `lt` on all-zero memory buffers should gracefully return a boundary value
// or an `EvalError` without panicking, crashing, or invoking UB.

#[test]
fn lt_survives_zero_memory_buffer_gracefully() {
    let eval = CompareLt;
    // Construct zero inputs simulating the "zero" archetype
    let mem1 = Memory::from_bytes(vec![0; 4]);
    let mem2 = Memory::from_bytes(vec![0; 4]);
    let result = eval.evaluate(&[mem1, mem2]);
    
    // As per spec it should return Ok(common::scalar(0)) since 0 < 0 is false
    // we assert that it survived and correctly returns Ok(mem).
    let mem = result.expect("lt on zero memory must yield Ok and not panic");
    assert_eq!(mem.bytes(), &[0, 0, 0, 0], "lt evaluated incorrectly on zero buffer: expected 0 as boundary scalar but got different output");
}
