// Integration test module for the containing Vyre package.

use vyre_ops::logical::or::op::cpu_or;

#[test]
fn or_survives_max_value_boundary_without_panic() {
    let a = u32::MAX;
    let b = u32::from_le_bytes(i32::MIN.to_le_bytes());
    
    let mut input = Vec::new();
    input.extend_from_slice(&a.to_le_bytes());
    input.extend_from_slice(&b.to_le_bytes());
    let mut output = Vec::new();
    
    // Test that feeding max boundary values (u32::MAX, i32::MIN edge) 
    // to the or CPU reference function does not panic and gives expected result.
    cpu_or(&input, &mut output);
    
    let expected = a | b;
    let actual = u32::from_le_bytes(output.try_into().expect("Output should be exactly 4 bytes long"));
    
    assert_eq!(actual, expected, "Expected bitwise OR output on max_value archetype inputs to be exactly correct");
}
