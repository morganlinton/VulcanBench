use super::*;

#[test]
fn semiring_min_plus_mul_zero_is_identity() {
    let a = [42u32, 100, 0, u32::MAX];
    let zero = [0u32; 4];
    let program = vyre_libs::math::semiring_min_plus_mul("a", "zero", "out", 4);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&zero)),
            Value::from(vec![0u8; 16]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![42, 100, 0, u32::MAX]
    );
}

#[test]
fn semiring_min_plus_mul_saturates_at_max() {
    let a = [u32::MAX - 5u32];
    let b = [10u32];
    let program = vyre_libs::math::semiring_min_plus_mul("a", "b", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), vec![u32::MAX]);
}

#[test]
fn semiring_min_plus_mul_commutative() {
    let a = [17u32, 99];
    let b = [3u32, 50];
    let p1 = vyre_libs::math::semiring_min_plus_mul("a", "b", "out", 2);
    let p2 = vyre_libs::math::semiring_min_plus_mul("b", "a", "out", 2);

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
        "min-plus addition is commutative"
    );
}

#[test]
fn semiring_min_plus_mul_size_one() {
    let a = [5u32];
    let b = [7u32];
    let program = vyre_libs::math::semiring_min_plus_mul("a", "b", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), vec![12]);
}

#[test]
fn semiring_min_plus_mul_large_size() {
    let n = 128u32;
    let a = vec![1u32; n as usize];
    let b = vec![2u32; n as usize];
    let program = vyre_libs::math::semiring_min_plus_mul("a", "b", "out", n);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; (n * 4) as usize]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![3u32; n as usize]
    );
}

#[test]
fn try_semiring_min_plus_mul_rejects_aliased_out() {
    let err = vyre_libs::math::try_semiring_min_plus_mul("a", "b", "a", 4).unwrap_err();
    assert!(
        err.to_string().contains("alias") || err.to_string().contains("name"),
        "aliasing out with a must be rejected: {err}"
    );
}

// ---------------------------------------------------------------------------
// Boolean Semiring Matrix Multiplication (OR-AND)
// ---------------------------------------------------------------------------

#[test]
fn bool_semiring_matmul_specific_values() {
    let a = [
        1u32, 0, 1, //
        0, 1, 0,
    ];
    let b = [
        0u32, 1, //
        1, 0, //
        0, 0,
    ];
    let program = vyre_libs::math::bool_semiring_matmul("a", "b", "out", 2, 3, 2);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 16]),
        ],
    )
    .expect("bool_semiring_matmul must execute");

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![0, 1, 1, 0],
        "OR-AND matrix multiply must match GraphBLAS boolean semiring semantics"
    );
}

#[test]
fn bool_semiring_matmul_composes_two_hop_reachability() {
    let adjacency = [
        0u32, 1, 0, 0, //
        0, 0, 1, 0, //
        0, 0, 0, 1, //
        0, 0, 0, 0,
    ];
    let program = vyre_libs::math::bool_semiring_matmul("frontier", "adjacency", "out", 4, 4, 4);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&adjacency)),
            Value::from(u32_bytes(&adjacency)),
            Value::from(vec![0u8; 16 * 4]),
        ],
    )
    .expect("reachability boolean semiring multiply must execute");

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![
            0, 0, 1, 0, //
            0, 0, 0, 1, //
            0, 0, 0, 0, //
            0, 0, 0, 0,
        ],
        "A boolean-semiring square must produce exactly two-hop reachability"
    );
}

#[test]
fn try_bool_semiring_matmul_rejects_aliased_names() {
    let err = vyre_libs::math::try_bool_semiring_matmul("a", "b", "a", 2, 2, 2).unwrap_err();
    assert!(
        err.to_string().contains("alias") || err.to_string().contains("name"),
        "aliasing output with input must be rejected: {err}"
    );
}

#[test]
fn try_bool_semiring_matmul_rejects_output_shape_overflow() {
    let err = vyre_libs::math::try_bool_semiring_matmul("a", "b", "out", 1 << 20, 1, 1 << 20)
        .unwrap_err();
    assert!(
        err.to_string().contains("overflows"),
        "overflowing output matrix shape must be rejected: {err}"
    );
}

// ---------------------------------------------------------------------------
// Sketch Mix (Thomas Wang hash-and-mix)
// ---------------------------------------------------------------------------

#[test]
fn sketch_mix_specific_values() {
    let mix = |mut h: u32| {
        h = h.wrapping_add(!(h << 15));
        h ^= h >> 12;
        h = h.wrapping_add(h << 2);
        h ^= h >> 4;
        h = h.wrapping_mul(2057);
        h ^= h >> 16;
        h
    };

    let input = [1u32, 2, 3, 4];
    let program = vyre_libs::math::sketch_mix("input", "out", 4);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(u32_bytes(&input)), Value::from(vec![0u8; 16])],
    )
    .unwrap();

    let expected = [mix(1), mix(2), mix(3), mix(4)];
    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), expected.to_vec());
}

#[test]
fn sketch_mix_zero_input() {
    let input = [0u32];
    let program = vyre_libs::math::sketch_mix("input", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(u32_bytes(&input)), Value::from(vec![0u8; 4])],
    )
    .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes())[0];
    // Zero is a fixed point of the mix? Let's compute:
    // h = 0; h += !(0<<15) = !0 = 0xFFFFFFFF; h ^= h>>12 = 0xFFFFFFFF ^ 0x000FFFFF = 0xFFF00000
    // ... not a fixed point. Just assert it's deterministic.
    let mix = |mut h: u32| {
        h = h.wrapping_add(!(h << 15));
        h ^= h >> 12;
        h = h.wrapping_add(h << 2);
        h ^= h >> 4;
        h = h.wrapping_mul(2057);
        h ^= h >> 16;
        h
    };
    assert_eq!(got, mix(0), "sketch_mix(0) must match CPU oracle");
}

#[test]
fn sketch_mix_large_size() {
    let n = 64u32;
    let input: Vec<u32> = (0..n).collect();
    let program = vyre_libs::math::sketch_mix("input", "out", n);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&input)),
            Value::from(vec![0u8; (n * 4) as usize]),
        ],
    )
    .unwrap();

    let mix = |mut h: u32| {
        h = h.wrapping_add(!(h << 15));
        h ^= h >> 12;
        h = h.wrapping_add(h << 2);
        h ^= h >> 4;
        h = h.wrapping_mul(2057);
        h ^= h >> 16;
        h
    };
    let expected: Vec<u32> = (0..n).map(mix).collect();
    assert_eq!(decode_u32_words(&outputs[0].to_bytes()), expected);
}

#[test]
fn sketch_mix_max_u32() {
    let input = [u32::MAX];
    let program = vyre_libs::math::sketch_mix("input", "out", 1);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(u32_bytes(&input)), Value::from(vec![0u8; 4])],
    )
    .unwrap();

    let mix = |mut h: u32| {
        h = h.wrapping_add(!(h << 15));
        h ^= h >> 12;
        h = h.wrapping_add(h << 2);
        h ^= h >> 4;
        h = h.wrapping_mul(2057);
        h ^= h >> 16;
        h
    };
    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![mix(u32::MAX)]
    );
}

#[test]
fn sketch_mix_diffusion_neighbours_differ() {
    let n = 16u32;
    let input: Vec<u32> = (0..n).collect();
    let program = vyre_libs::math::sketch_mix("input", "out", n);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&input)),
            Value::from(vec![0u8; (n * 4) as usize]),
        ],
    )
    .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    // Every adjacent output should differ (diffusion property  -  not a
    // cryptographic guarantee, but a canary for copy-paste bugs).
    for window in got.windows(2) {
        assert_ne!(
            window[0], window[1],
            "adjacent sketch outputs must differ for distinct inputs"
        );
    }
}

#[test]
fn try_sketch_mix_rejects_aliased_in_out() {
    let err = vyre_libs::math::try_sketch_mix("x", "x", 4).unwrap_err();
    assert!(
        err.to_string().contains("alias") || err.to_string().contains("name"),
        "aliasing input and output must be rejected: {err}"
    );
}

// ---------------------------------------------------------------------------
// Cross-cutting algebraic laws
// ---------------------------------------------------------------------------

#[test]
fn lattice_join_meet_distributivity_holds_for_bitsets() {
    // For the bitset lattice (OR/AND), distributivity holds:
    // a | (b & c) == (a | b) & (a | c)
    let a = [0xF0F0_F0F0u32];
    let b = [0x0F0F_0F0Fu32];
    let c = [0xAAAA_5555u32];

    // LHS: a | (b & c)
    let p_bc = vyre_libs::math::lattice_meet("b", "c", "bc", 1);
    let o_bc = vyre_reference::reference_eval(
        &p_bc,
        &[
            Value::from(u32_bytes(&b)),
            Value::from(u32_bytes(&c)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let p_lhs = vyre_libs::math::lattice_join("a", "bc", "out", 1);
    let o_lhs = vyre_reference::reference_eval(
        &p_lhs,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(o_bc[0].to_bytes()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    // RHS: (a | b) & (a | c)
    let p_ab = vyre_libs::math::lattice_join("a", "b", "ab", 1);
    let o_ab = vyre_reference::reference_eval(
        &p_ab,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&b)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let p_ac = vyre_libs::math::lattice_join("a", "c", "ac", 1);
    let o_ac = vyre_reference::reference_eval(
        &p_ac,
        &[
            Value::from(u32_bytes(&a)),
            Value::from(u32_bytes(&c)),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();
    let p_rhs = vyre_libs::math::lattice_meet("ab", "ac", "out", 1);
    let o_rhs = vyre_reference::reference_eval(
        &p_rhs,
        &[
            Value::from(o_ab[0].to_bytes()),
            Value::from(o_ac[0].to_bytes()),
            Value::from(vec![0u8; 4]),
        ],
    )
    .unwrap();

    assert_eq!(
        decode_u32_words(&o_lhs[0].to_bytes()),
        decode_u32_words(&o_rhs[0].to_bytes()),
        "bitset lattice must satisfy distributivity"
    );
}
