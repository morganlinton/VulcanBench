//! Adversarial test for `and` operator using `max_value` archetype.

use vyre_ops::logical::and::op::cpu_and;

#[test]
fn and_survives_max_value_boundary_without_panic() {
    let mut output = Vec::new();
    let a: u32 = u32::MAX;
    let b: u32 = i32::MIN as u32;

    let mut input = Vec::new();
    input.extend_from_slice(&a.to_le_bytes());
    input.extend_from_slice(&b.to_le_bytes());

    cpu_and(&input, &mut output);

    let res = u32::from_le_bytes(
        output
            .try_into()
            .expect("Output should be exactly 4 bytes without panicking or returning short"),
    );

    assert_eq!(
        res,
        a & b,
        "Expected bitwise AND of u32::MAX and i32::MIN as u32 to succeed correctly"
    );

    // Also testing i32::MAX per requirements
    let mut output2 = Vec::new();
    let c: u32 = i32::MAX as u32;

    let mut input2 = Vec::new();
    input2.extend_from_slice(&a.to_le_bytes());
    input2.extend_from_slice(&c.to_le_bytes());

    cpu_and(&input2, &mut output2);

    let res2 = u32::from_le_bytes(
        output2
            .try_into()
            .expect("Output should be exactly 4 bytes without panicking or returning short"),
    );

    assert_eq!(
        res2,
        a & c,
        "Expected bitwise AND of u32::MAX and i32::MAX as u32 to succeed correctly"
    );
}
