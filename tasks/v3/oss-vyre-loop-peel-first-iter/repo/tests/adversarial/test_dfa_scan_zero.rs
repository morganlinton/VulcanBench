//! Adversarial test for match.dfa_scan against zeroed buffer

use vyre_reference::dual_impls::ReferenceEvaluator;
use vyre_reference::workgroup::Memory;
use vyre_primitives::PatternMatchDfa;

/// Catches: I11/I12 violations on zeroed DFA buffer. Proves: Graceful rejection on zero inputs instead of panic or UB.
#[test]
fn match_dfa_scan_zero_survives_all_zero_buffer_without_panic() {
    let primitive = PatternMatchDfa {
        dfa: vec![0u8; 1024],
    };
    
    // Evaluate the primitive with a zeroed buffer
    let inputs = vec![Memory::from_bytes(vec![0u8; 1024])];
    let result = primitive.evaluate(&inputs);
    
    result.expect_err("Catches: Hostile zeroed DFA buffer should not crash. Proves: Graceful rejection on zero inputs");
}
