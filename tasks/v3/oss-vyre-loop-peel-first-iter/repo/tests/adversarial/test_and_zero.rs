//! Adversarial test for `primitive.logical.and` against the `zero` archetype.
//!
//! This test defends invariants I11 (no panic) and I12 (no UB). It feeds
//! the hostile `zero` archetype (an all-zero buffer) to the CPU reference
//! implementation and asserts that it handles it gracefully by producing
//! the correct boundary value (zeros) rather than panicking.

use vyre_ops::logical::and::op::cpu_and;

#[test]
fn and_survives_zero_buffer_without_panic() {
    // The `and` primitive operation consumes 8 bytes of input (two 32-bit integers, 4 bytes each).
    // An all-zero hostile payload of appropriate size ensures the execution path
    // doesn't falsely assume bounds handling protects against arithmetic panics.
    let input: &[u8] = &[0; 8];
    let mut output = Vec::new();

    // The conformance spec requires this function to evaluate correctly without panicking
    // regardless of what bytes are placed within bounds.
    cpu_and(input, &mut output);

    // The output for `0 & 0` must exactly equal `[0, 0, 0, 0]` in little-endian.
    assert_eq!(
        output,
        &[0, 0, 0, 0],
        "CATASTROPHIC FAILURE: Expected 'and' to gracefully evaluate an all-zero buffer by returning zero, but it produced unexpected output. This indicates an implementation failure in preserving boundary laws that could trigger device hang in dependent shaders."
    );
}
