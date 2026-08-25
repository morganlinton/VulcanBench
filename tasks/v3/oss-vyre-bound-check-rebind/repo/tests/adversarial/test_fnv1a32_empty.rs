// Integration test module for the containing Vyre package.

#![allow(missing_docs)]
use vyre_foundation::transform::compiler::string_interner::fnv1a32;

#[test]
fn fnv1a32_empty_survives_empty_input_buffer_without_panic() {
    // Catches: panic or UB on empty input buffer. Proves: I11 (no panic), I12 (no UB).
    let input: &[u8] = &[];
    
    // The spec for fnv1a32 returns 0x811c9dc5u32 for empty input (the FNV offset basis).
    let hash = std::panic::catch_unwind(|| {
        fnv1a32(input)
    }).expect("fnv1a32 catastrophic failure: panicked on empty input buffer");
    
    assert_eq!(hash, 0x811c_9dc5u32, "fnv1a32 catastrophic failure: empty input did not return the offset basis");
}
