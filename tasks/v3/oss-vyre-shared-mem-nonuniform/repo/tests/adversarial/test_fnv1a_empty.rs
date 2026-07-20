// Integration test module for the containing Vyre package.

use vyre_primitives::HashFnv1a;
use vyre_reference::dual_impls::ReferenceEvaluator;
use vyre_reference::workgroup::Memory;

#[test]
fn fnv1a_rejects_empty_buffer_gracefully() {
    let fnv1a = HashFnv1a;
    let inputs = vec![Memory::from_bytes(vec![])];

    let result = fnv1a.evaluate(&inputs);
    
    // We expect the result to be FNV_OFFSET (0xcbf29ce484222325), as per fnv1a spec 
    // for an empty buffer, or a spec-defined error. We assert that it does not panic.
    let memory = result.expect("HashFnv1a should survive an empty input buffer without panicking and return the initial offset.");
    
    let expected_hash: u64 = 0xcbf29ce484222325;
    let actual_hash = u64::from_le_bytes(memory.bytes().try_into().expect("Output memory should be 8 bytes long (u64)."));
    
    assert_eq!(actual_hash, expected_hash, "HashFnv1a of empty buffer should be FNV_OFFSET");
}
