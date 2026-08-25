use super::*;

#[test]
fn lattice_join_specific_values() {
    let a = [0x0000_FFFFu32, 0xAAAA_AAAA, 0x0000_0000, 0xFFFF_FFFF];
    let b = [0xFFFF_0000u32, 0x5555_5555, 0x0000_0000, 0x0000_0000];
    let program = vyre_libs::math::lattice_join("a", "b", "out", 4);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 16]),
        ],
    )
    .expect("lattice_join must execute");

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![0xFFFF_FFFF, 0xFFFF_FFFF, 0x0000_0000, 0xFFFF_FFFF]
    );
}

#[test]
fn lattice_join_commutative() {
    let a = [0x1234_5678u32, 0x9ABC_DEF0];
    let b = [0x0F0F_0F0Fu32, 0xF0F0_F0F0];
    let p1 = vyre_libs::math::lattice_join("a", "b", "out", 2);
    let p2 = vyre_libs::math::lattice_join("b", "a", "out", 2);

    let o1 = vyre_reference::reference_eval(
        &p1,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 8]),
        ],
    )
    .unwrap();
    let o2 = vyre_reference::reference_eval(
        &p2,
        &[
            Value::from(u32_bytes(&b)),
            Value::from(u32_bytes(&a)),
            Value::from(vec![0u8; 8]),
        ],
    )
    .unwrap();

    assert_eq!(
        o1[0].to_bytes(),
        o2[0].to_bytes(),
        "join must be commutative"
    );
}

#[test]
fn lattice_join_associative() {
    let x = [0x00FF_00FFu32];
    let y = [0x0F0F_0F0Fu32];
    let z = [0x1111_1111u32];

    // (x | y) | z
    let p_xy = vyre_libs::math::lattice_join("x", "y", "xy", 1);
    let p_xyz = vyre_libs::math::lattice_join("xy", "z", "out", 1);

    let o_xy = vyre_reference::reference_eval(
        &p_xy,
        &[
            Value::from(u32_bytes(&x)),
            Value::from(u32_bytes(&y)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let o_xyz = vyre_reference::reference_eval(
        &p_xyz,
        &[
            Value::from(o_xy[0].to_bytes()),
            Value::from(u32_bytes(&z)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    // x | (y | z)
    let p_yz = vyre_libs::math::lattice_join("y", "z", "yz", 1);
    let o_yz = vyre_reference::reference_eval(
        &p_yz,
        &[
            Value::from(u32_bytes(&y)),
            Value::from(u32_bytes(&z)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let p_xyz2 = vyre_libs::math::lattice_join("x", "yz", "out", 1);
    let o_xyz2 = vyre_reference::reference_eval(
        &p_xyz2,
        &[
            Value::from(u32_bytes(&x)),
            Value::from(o_yz[0].to_bytes()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&o_xyz[0].to_bytes()),
        decode_u32_words(&o_xyz2[0].to_bytes()),
        "join must be associative"
    );
}

#[test]
fn lattice_join_identity_all_zeros() {
    let a = [0xDEAD_BEEFu32, 0xCAFE_BABE];
    let zero = [0u32, 0];
    let program = vyre_libs::math::lattice_join("a", "zero", "out", 2);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&zero)),
            Value::from(vec![0u8; 8]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![0xDEAD_BEEF, 0xCAFE_BABE]
    );
}

#[test]
fn lattice_join_size_one() {
    let a = [0b1010u32];
    let b = [0b0101u32];
    let program = vyre_libs::math::lattice_join("a", "b", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), vec![0b1111]);
}

#[test]
fn lattice_join_all_ones_absorbs() {
    let a = [0x1234_5678u32];
    let ones = [0xFFFF_FFFFu32];
    let program = vyre_libs::math::lattice_join("a", "ones", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&ones)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), vec![0xFFFF_FFFF]);
}

#[test]
fn lattice_join_large_size_power_of_two() {
    let n = 256u32;
    let a = vec![0x5555_5555u32; n as usize];
    let b = vec![0xAAAA_AAAAu32; n as usize];
    let program = vyre_libs::math::lattice_join("a", "b", "out", n);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; (n * 4) as usize]),
        ],
    )
    .unwrap();

    let expected = vec![0xFFFF_FFFFu32; n as usize];
    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), expected);
}

#[test]
fn try_lattice_join_rejects_aliased_names() {
    let err = vyre_libs::math::try_lattice_join("a", "a", "out", 4).unwrap_err();
    assert!(
        err.to_string().contains("alias") || err.to_string().contains("name"),
        "aliasing a and b must be rejected: {err}"
    );
}

// ---------------------------------------------------------------------------
// Lattice Meet (bitwise AND)
// ---------------------------------------------------------------------------

#[test]
fn lattice_meet_specific_values() {
    let a = [0x0000_FFFFu32, 0xAAAA_AAAA, 0x0000_0000, 0xFFFF_FFFF];
    let b = [0xFFFF_0000u32, 0x5555_5555, 0x0000_0000, 0x0000_0000];
    let program = vyre_libs::math::lattice_meet("a", "b", "out", 4);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 16]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![0x0000_0000, 0x0000_0000, 0x0000_0000, 0x0000_0000]
    );
}

#[test]
fn lattice_meet_commutative() {
    let a = [0x1234_5678u32];
    let b = [0x0F0F_0F0Fu32];
    let p1 = vyre_libs::math::lattice_meet("a", "b", "out", 1);
    let p2 = vyre_libs::math::lattice_meet("b", "a", "out", 1);

    let o1 = vyre_reference::reference_eval(
        &p1,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let o2 = vyre_reference::reference_eval(
        &p2,
        &[
            Value::from(u32_bytes(&b)),
            Value::from(u32_bytes(&a)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(o1[0].to_bytes(), o2[0].to_bytes());
}

#[test]
fn lattice_meet_associative() {
    let x = [0x00FF_00FFu32];
    let y = [0x0F0F_0F0Fu32];
    let z = [0x1111_1111u32];

    let p_xy = vyre_libs::math::lattice_meet("x", "y", "xy", 1);
    let o_xy = vyre_reference::reference_eval(
        &p_xy,
        &[
            Value::from(u32_bytes(&x)),
            Value::from(u32_bytes(&y)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let p_xyz = vyre_libs::math::lattice_meet("xy", "z", "out", 1);
    let o_xyz = vyre_reference::reference_eval(
        &p_xyz,
        &[
            Value::from(o_xy[0].to_bytes()),
            Value::from(u32_bytes(&z)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    let p_yz = vyre_libs::math::lattice_meet("y", "z", "yz", 1);
    let o_yz = vyre_reference::reference_eval(
        &p_yz,
        &[
            Value::from(u32_bytes(&y)),
            Value::from(u32_bytes(&z)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let p_xyz2 = vyre_libs::math::lattice_meet("x", "yz", "out", 1);
    let o_xyz2 = vyre_reference::reference_eval(
        &p_xyz2,
        &[
            Value::from(u32_bytes(&x)),
            Value::from(o_yz[0].to_bytes()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&o_xyz[0].to_bytes()),
        decode_u32_words(&o_xyz2[0].to_bytes()),
        "meet must be associative"
    );
}

#[test]
fn lattice_meet_identity_all_ones() {
    let a = [0xDEAD_BEEFu32];
    let ones = [0xFFFF_FFFFu32];
    let program = vyre_libs::math::lattice_meet("a", "ones", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&ones)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), vec![0xDEAD_BEEF]);
}

#[test]
fn lattice_meet_all_zeros_annihilates() {
    let a = [0xFFFF_FFFFu32];
    let zero = [0u32];
    let program = vyre_libs::math::lattice_meet("a", "zero", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&zero)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), vec![0]);
}

#[test]
fn lattice_meet_idempotent() {
    let a = [0xA5A5_A5A5u32];
    let program = vyre_libs::math::lattice_meet("a", "b", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&a)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), vec![0xA5A5_A5A5]);
}

#[test]
fn lattice_join_meet_absorption() {
    let a = [0xA5A5_A5A5u32];
    let b = [0x5A5A_5A5Au32];

    // a | (a & b) == a
    let p_meet = vyre_libs::math::lattice_meet("a", "b", "m", 1);
    let o_meet = vyre_reference::reference_eval(
        &p_meet,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let p_join = vyre_libs::math::lattice_join("a", "m", "out", 1);
    let o_out = vyre_reference::reference_eval(
        &p_join,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(o_meet[0].to_bytes()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&o_out[0].to_bytes()), vec![0xA5A5_A5A5]);
}

// ---------------------------------------------------------------------------
// Min-Plus Semiring Multiplication (saturating addition)
// ---------------------------------------------------------------------------

#[test]
fn semiring_min_plus_mul_basic() {
    let a = [10u32, 20, u32::MAX, u32::MAX - 1];
    let b = [1u32, 2, 3, 4];
    let program = vyre_libs::math::semiring_min_plus_mul("a", "b", "out", 4);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 16]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![11, 22, u32::MAX, u32::MAX]
    );
}

