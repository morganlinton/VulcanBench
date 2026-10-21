// Integration test module for the containing Vyre package.

use vyre_conform::specs::primitive::bitwise::shl::vyre_op;

#[test]
fn shl_survives_zero_buffer_without_panic() {
    let spec = vyre_op();
    let cpu = spec.cpu_fn;
    
    let input = b""; // zero length buffer
    let mut output = vec![];
    
    // Call the CPU function with an empty buffer
    // According to Santh rules, we assert it doesn't panic and either returns Err or handled safely.
    // The spec needs a buffer large enough for 2 operands. 
    // Since input is zero size, it should fail gracefully.
    
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (cpu)(input, &mut output);
    }));
    
    assert!(result.is_ok(), "Fix: shl on empty/zero input panicked instead of rejecting gracefully");
}
