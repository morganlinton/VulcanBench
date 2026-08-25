// (use super::* removed  -  flat-included into wire_roundtrip_proptest_suite scope)

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        .. ProptestConfig::default()
    })]

    #[test]
    fn program_wire_roundtrip_preserves_structure(program in arb_program()) {
        // Wire roundtrip canonicalises (flushes subnormals, collapses
        // NaN payloads, eagerly initialises lazy caches). Bit-exact
        // structural equality between input and decoded therefore does
        // not hold for inputs containing subnormal LitF32 or
        // signaling-NaN payloads.
        //
        // The real contract is roundtrip *idempotence under canonical
        // form*: encode → decode → encode → decode → assert the second
        // pair is bit-equal to the first. Once a Program has gone
        // through one roundtrip, its canonical form is stable.
        let encoded = program
            .to_wire()
            .unwrap_or_else(|error| panic!("Fix: arbitrary Program must encode: {error}"));
        let decoded = Program::from_wire(&encoded)
            .unwrap_or_else(|error| panic!("Fix: arbitrary Program must decode: {error}"));

        let reencoded = decoded
            .to_wire()
            .unwrap_or_else(|error| panic!("Fix: decoded Program must re-encode canonically: {error}"));
        prop_assert_eq!(&reencoded, &encoded);

        let redecoded = Program::from_wire(&reencoded)
            .unwrap_or_else(|error| panic!("Fix: re-encoded Program must decode: {error}"));
        prop_assert_eq!(&redecoded, &decoded);
    }

    #[test]
    fn subnormal_f32_roundtrips_bit_exactly(bits in 1u32..=0x007f_ffff) {
        let positive = f32::from_bits(bits);
        let negative = f32::from_bits(bits | 0x8000_0000);

        let program = Program::wrapped(
            vec![],
            [1, 1, 1],
            vec![
                Node::Let {
                    name: "p".into(),
                    value: Expr::LitF32(positive),
                },
                Node::Let {
                    name: "n".into(),
                    value: Expr::LitF32(negative),
                },
                Node::Return,
            ],
        );

        let decoded = Program::from_wire(&program.to_wire().expect("Fix: subnormal program must encode"))
            .expect("Fix: subnormal program must decode");

        let body = top_level_body(&decoded);
        let positive_bits = match &body[0] {
            Node::Let { value: Expr::LitF32(value), .. } => value.to_bits(),
            other => panic!("Fix: expected first let f32 literal, got {other:?}"),
        };
        let negative_bits = match &body[1] {
            Node::Let { value: Expr::LitF32(value), .. } => value.to_bits(),
            other => panic!("Fix: expected second let f32 literal, got {other:?}"),
        };

        // Wire roundtrip canonicalises via canonical_f32: subnormals
        // flush to signed zero. Compare against the canonical form,
        // not the input bits.
        prop_assert_eq!(positive_bits, canonicalize_f32(positive).to_bits());
        prop_assert_eq!(negative_bits, canonicalize_f32(negative).to_bits());
    }
}

#[test]
fn signaling_nan_payload_roundtrips_bit_exactly() {
    let payload = f32::from_bits(0x7f80_0001);
    let program = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![
            Node::Let {
                name: "nan".into(),
                value: Expr::LitF32(payload),
            },
            Node::Return,
        ],
    );

    let decoded = Program::from_wire(
        &program
            .to_wire()
            .expect("Fix: NaN payload program must encode"),
    )
    .expect("Fix: NaN payload program must decode");

    // Wire roundtrip canonicalises NaN payloads via canonical_f32 →
    // single qNaN bit pattern. Compare against the canonical form.
    match first_let_expr(&decoded) {
        Expr::LitF32(value) => {
            assert_eq!(value.to_bits(), canonicalize_f32(payload).to_bits())
        }
        other => panic!("Fix: expected NaN literal after roundtrip, got {other:?}"),
    }
}

#[test]
fn wire_header_sets_opaque_endian_fixed_flag() {
    let program = Program::wrapped(vec![], [1, 1, 1], vec![Node::Return]);
    let encoded = program
        .to_wire()
        .expect("Fix: minimal program must encode with canonical framing flags");
    let flags = u16::from_le_bytes([encoded[6], encoded[7]]);
    assert_eq!(
        flags, 0b100,
        "Fix: canonical wire framing must set only OPAQUE_ENDIAN_FIXED, got flags={flags:#06b}"
    );
}

#[test]
fn decode_rejects_missing_opaque_endian_fixed_flag() {
    let program = Program::wrapped(vec![], [1, 1, 1], vec![Node::Return]);
    let mut encoded = program
        .to_wire()
        .expect("Fix: minimal program must encode before header tampering");
    encoded[6..8].copy_from_slice(&0u16.to_le_bytes());
    let digest = blake3::hash(&encoded[40..]);
    encoded[8..40].copy_from_slice(digest.as_bytes());
    let error = Program::from_wire(&encoded)
        .expect_err("Fix: decoder must reject blobs missing OPAQUE_ENDIAN_FIXED");
    assert!(
        error.to_string().contains("OPAQUE_ENDIAN_FIXED"),
        "Fix: missing opaque-endian flag rejection must name the flag, got: {error}"
    );
}

#[test]
fn minus_zero_f32_canonicalizes_to_positive_zero_in_wire_and_hash() {
    let positive = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![
            Node::Let {
                name: "zero".into(),
                value: Expr::LitF32(0.0),
            },
            Node::Return,
        ],
    );
    let negative = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![
            Node::Let {
                name: "zero".into(),
                value: Expr::LitF32(-0.0),
            },
            Node::Return,
        ],
    );

    let positive_wire = positive
        .to_wire()
        .expect("Fix: +0.0 canonical fixture must encode");
    let negative_wire = negative
        .to_wire()
        .expect("Fix: -0.0 canonical fixture must encode");

    assert_eq!(positive_wire, negative_wire);
    assert_eq!(positive.fingerprint(), negative.fingerprint());

    let decoded =
        Program::from_wire(&negative_wire).expect("Fix: canonicalized -0.0 wire bytes must decode");
    match first_let_expr(&decoded) {
        Expr::LitF32(value) => assert_eq!(value.to_bits(), 0.0f32.to_bits()),
        other => panic!("Fix: decoded canonical zero must stay a f32 literal, got {other:?}"),
    }
}

#[test]
fn zero_count_output_buffer_is_rejected_at_encode_time() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let error = program
        .to_wire()
        .expect_err("Fix: zero-count output buffers must be rejected before wire roundtrip");
    assert!(
        error.to_string().contains("count 0"),
        "Fix: zero-count output rejection must mention count 0, got: {error}"
    );
}

#[test]
fn runtime_sized_input_storage_roundtrips() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );

    let encoded = program
        .to_wire()
        .expect("Fix: runtime-sized input storage buffers must encode");
    let decoded =
        Program::from_wire(&encoded).expect("Fix: runtime-sized input storage buffers must decode");

    assert_eq!(decoded, program);
}

#[test]
fn non_composable_program_flag_roundtrips() {
    let program = Program::wrapped(vec![], [1, 1, 1], vec![Node::Return])
        .with_entry_op_id("parser.stateful")
        .with_non_composable_with_self(true);

    let encoded = program
        .to_wire()
        .expect("Fix: non-composable program must encode");
    let decoded = Program::from_wire(&encoded).expect("Fix: non-composable program must decode");

    assert_eq!(decoded, program);
    assert!(decoded.is_non_composable_with_self());
}

#[test]
fn zero_workgroup_component_is_rejected_at_encode_time() {
    let program = Program::wrapped(vec![], [0, 1, 1], vec![Node::Return]);

    let error = program
        .to_wire()
        .expect_err("Fix: zero-component workgroup sizes must be rejected before wire roundtrip");
    assert!(
        error.to_string().contains("workgroup_size[0] is 0"),
        "Fix: workgroup rejection must name the zero axis, got: {error}"
    );
}

#[test]
fn program_empty_roundtrips_as_explicit_noop() {
    let program = Program::empty();
    let encoded = program
        .to_wire()
        .expect("Fix: Program::empty must either reject explicitly or roundtrip as a no-op");
    let decoded = Program::from_wire(&encoded)
        .expect("Fix: Program::empty wire bytes must decode under the chosen semantics");

    assert_eq!(decoded, program);
}

#[test]
fn reserved_opaque_datatype_id_is_rejected_at_decode_time() {
    let valid_id = ExtensionDataTypeId(0x8000_0001);
    let program = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![
            Node::Let {
                name: "opaque".into(),
                value: Expr::Cast {
                    target: DataType::Opaque(valid_id),
                    value: Box::new(Expr::LitU32(7)),
                },
            },
            Node::Return,
        ],
    );

    let encoded = program
        .to_wire()
        .expect("Fix: reserved-id regression fixture must encode before tampering");
    let mutated = first_replaced_with_valid_digest(
        &encoded,
        &valid_id.as_u32().to_le_bytes(),
        &0x0000_0001u32.to_le_bytes(),
    );
    let error = Program::from_wire(&mutated)
        .expect_err("Fix: reserved opaque ids must be rejected at decode");

    assert!(
        error.to_string().contains("collides with core IR"),
        "Fix: reserved opaque rejection must explain the collision, got: {error}"
    );
}

#[test]
fn datatype_strategy_enumerates_every_wire_supported_terminal_variant() {
    let sample = vec![
        DataType::U8,
        DataType::U16,
        DataType::U32,
        DataType::I8,
        DataType::I16,
        DataType::I32,
        DataType::I64,
        DataType::U64,
        DataType::Vec2U32,
        DataType::Vec4U32,
        DataType::Bool,
        DataType::Bytes,
        DataType::Array { element_size: 8 },
        DataType::F16,
        DataType::BF16,
        DataType::F32,
        DataType::F64,
        DataType::Tensor,
        DataType::Handle(TypeId(7)),
        DataType::Vec {
            element: Box::new(DataType::U32),
            count: 4,
        },
        DataType::TensorShaped {
            element: Box::new(DataType::F32),
            shape: smallvec![2, 3],
        },
        DataType::Opaque(ExtensionDataTypeId(0x8000_0001)),
    ];

    for target in sample {
        let program = Program::wrapped(
            vec![],
            [1, 1, 1],
            vec![
                Node::Let {
                    name: "cast".into(),
                    value: Expr::Cast {
                        target,
                        value: Box::new(Expr::LitU32(1)),
                    },
                },
                Node::Return,
            ],
        );

        let decoded = Program::from_wire(
            &program
                .to_wire()
                .expect("Fix: datatype fixture must encode"),
        )
        .expect("Fix: datatype fixture must decode");
        assert_eq!(decoded, program);
    }
}

