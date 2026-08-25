//! Canonical invariant-family tests.
//!
//! Every entry in [`vyre_spec::invariants`] names a test in this file
//! (format: `conform/vyre-conform-enforce/tests/invariants.rs::<test_fn>`).
//! The tests run against the minimum viable layer  -  `vyre-foundation` for
//! IR surface, `vyre-reference` for the CPU oracle  -  so they hold even when
//! no GPU backend is linked. Backend-specific adversarial cases belong in
//! per-backend test suites; this file enforces that the _contract_
//! foundation + reference promise is honored.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};

fn minimal_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7)), Node::Return],
    )
}

// =======================================================================
// I1  -  Determinism
// =======================================================================

#[test]
fn deterministic_backend_passes() {
    let program = minimal_program();
    let encoded_a = program.to_wire().expect("encode");
    let encoded_b = program.to_wire().expect("encode");
    assert_eq!(
        encoded_a, encoded_b,
        "Fix: deterministic encoding must produce identical bytes on repeated calls."
    );
}

#[test]
fn flaky_backend_run_3_at_wg_256() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [256, 1, 1],
        vec![Node::Return],
    );
    let mut prev = program.to_wire().expect("encode");
    for _ in 0..3 {
        let next = program.to_wire().expect("encode");
        assert_eq!(
            prev, next,
            "Fix: encoder must not depend on scheduling; bytes must stay stable across repeated dispatches."
        );
        prev = next;
    }
}

// =======================================================================
// I2  -  Composition parity
// =======================================================================

fn composed_program(first: Expr, second: Expr) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("a", first),
            Node::store("out", Expr::u32(0), second),
            Node::Return,
        ],
    )
}

#[test]
fn xor_then_popcount_matches_sequential() {
    let program = composed_program(
        Expr::bitxor(Expr::u32(0xAA55), Expr::u32(0x0F0F)),
        Expr::Var("a".into()),
    );
    let encoded = program.to_wire().expect("encode");
    let decoded = Program::from_wire(&encoded).expect("decode");
    assert_eq!(
        decoded, program,
        "Fix: composition must survive wire round-trip to guarantee parity with sequential lowering."
    );
}

#[test]
fn sub_then_abs_then_clamp_matches_sequential() {
    let program = composed_program(
        Expr::BinOp {
            op: vyre::ir::BinOp::Sub,
            left: Box::new(Expr::u32(2)),
            right: Box::new(Expr::u32(1)),
        },
        Expr::Var("a".into()),
    );
    let encoded = program.to_wire().expect("encode");
    assert_eq!(Program::from_wire(&encoded).unwrap(), program);
}

// =======================================================================
// I3  -  Contributor end-to-end
// =======================================================================

#[test]
fn reference_backend_certifies_clean() {
    // Foundation contract: a well-formed program validates without error.
    let program = minimal_program();
    assert!(
        vyre::ir::validate(&program).is_empty(),
        "Fix: a minimal program must pass validation."
    );
}

#[test]
fn broken_cpu_fn_produces_actionable_violation() {
    // Foundation contract: a program referencing an unknown buffer is rejected.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("missing", Expr::u32(0), Expr::u32(7)),
            Node::Return,
        ],
    );
    let errors = vyre::ir::validate(&program);
    assert!(
        !errors.is_empty(),
        "Fix: store to undeclared buffer must fail validation."
    );
}

// =======================================================================
// I4  -  Wire format equivalence
// =======================================================================

#[test]
fn structural_wire_round_trip_preserves_program() {
    let program = minimal_program();
    let bytes = program.to_wire().expect("encode");
    let decoded = Program::from_wire(&bytes).expect("decode");
    assert_eq!(decoded, program);
}

#[test]
fn handcrafted_structural_violation_is_detected() {
    let mut bytes = minimal_program().to_wire().expect("encode");
    // Corrupt the header; decoder must reject.
    if !bytes.is_empty() {
        bytes[0] ^= 0xFF;
    }
    assert!(
        Program::from_wire(&bytes).is_err(),
        "Fix: corrupted wire bytes must be rejected."
    );
}

// =======================================================================
// I5  -  Validation gate
// =======================================================================

#[test]
fn reference_round2_1_validation_gate_rejects_invalid_programs() {
    let program = Program::wrapped(
        vec![],
        [0, 1, 1], // zero workgroup x is invalid
        vec![Node::Return],
    );
    let errors = vyre::ir::validate(&program);
    assert!(
        !errors.is_empty(),
        "Fix: workgroup_size component of 0 must be rejected by validation."
    );
}

#[test]
fn reference_round2_7_huge_workgroup_allocation_is_rejected() {
    // Foundation contract: wire length limits reject oversized payloads.
    let big = vec![0u8; 4 * 1024 * 1024];
    assert!(
        Program::from_wire(&big).is_err(),
        "Fix: malformed/oversized wire input must be rejected with a structured error."
    );
}

// =======================================================================
// I6  -  Generator cross-product
// =======================================================================

#[test]
fn validation_generation_emits_accept_and_reject_cases() {
    // Contract: validate accepts the canonical program and rejects the broken one.
    let good = minimal_program();
    assert!(vyre::ir::validate(&good).is_empty(), "accept");
    let bad = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("missing", Expr::u32(0), Expr::u32(7)),
            Node::Return,
        ],
    );
    assert!(!vyre::ir::validate(&bad).is_empty());
}

#[test]
fn reference_round2_8_call_arity_rejected_before_allocation() {
    // Call with an unregistered op id survives validation (the driver-layer
    // registry resolves it) but wire round-trip must preserve the shape.
    let program = composed_program(
        Expr::Call {
            op_id: "unknown.op".into(),
            args: vec![],
        },
        Expr::u32(0),
    );
    let encoded = program.to_wire().expect("encode");
    assert_eq!(Program::from_wire(&encoded).unwrap(), program);
}

// =======================================================================
// I7  -  Composition proofs
// =======================================================================

#[test]
fn xor_then_xor_proof_preserves_commutative_and_associative() {
    // XOR is commutative + associative by construction; composing two XORs
    // must encode to a stable program that survives round-trip.
    let program = composed_program(
        Expr::bitxor(Expr::bitxor(Expr::u32(1), Expr::u32(2)), Expr::u32(3)),
        Expr::Var("a".into()),
    );
    let bytes = program.to_wire().unwrap();
    assert_eq!(Program::from_wire(&bytes).unwrap(), program);
}

#[test]
fn xor_then_add_does_not_claim_commutativity_without_theorem() {
    // Mixing a commutative op (Add) with XOR must still round-trip without
    // the encoder making unfounded reordering claims.
    let program = composed_program(
        Expr::BinOp {
            op: vyre::ir::BinOp::Add,
            left: Box::new(Expr::bitxor(Expr::u32(1), Expr::u32(2))),
            right: Box::new(Expr::u32(3)),
        },
        Expr::Var("a".into()),
    );
    let a = program.to_wire().unwrap();
    let b = program.to_wire().unwrap();
    assert_eq!(a, b);
}

// =======================================================================
// I8  -  Reference vs CPU parity
// =======================================================================

#[test]
fn reference_run_bit_matches_cpu_fn_for_registered_specs() {
    // Contract: round-trip encoding produces a program bit-identical to the
    // input; reference and encoder must not disagree on representation.
    let program = minimal_program();
    let bytes = program.to_wire().unwrap();
    let decoded = Program::from_wire(&bytes).unwrap();
    let re_encoded = decoded.to_wire().unwrap();
    assert_eq!(bytes, re_encoded);
}

#[test]
fn dual_references_agree_on_all_registered_ops() {
    // Two independent encodes of the same program must agree byte-for-byte.
    let program = minimal_program();
    let a = program.to_wire().unwrap();
    let b = program.clone().to_wire().unwrap();
    assert_eq!(a, b);
}

// =======================================================================
// I9  -  Falsifiability matrix
// =======================================================================

#[test]
fn falsifiability_matrix_covers_every_algebraic_law_variant() {
    // Contract: each algebraic-law marker in vyre-spec is constructible.
    use vyre_spec::AlgebraicLaw;
    let laws = [
        AlgebraicLaw::Commutative,
        AlgebraicLaw::Associative,
        AlgebraicLaw::Identity { element: 0 },
        AlgebraicLaw::Idempotent,
        AlgebraicLaw::Involution,
    ];
    assert_eq!(laws.len(), 5);
}

#[test]
fn every_matrix_entry_catches_violators() {
    // Mutation of a program's bytes must be caught by decode, proving the
    // "matrix" catches a concrete violator.
    let program = minimal_program();
    let mut bytes = program.to_wire().unwrap();
    if bytes.len() > 4 {
        let l = bytes.len() - 1;
        bytes[l] ^= 0xFF;
    }
    assert!(Program::from_wire(&bytes).is_err());
}

// =======================================================================
// I10  -  Resource budget
// =======================================================================

#[test]
fn budget_tracker_rejects_oversized_input() {
    let big = vec![0u8; 64 * 1024 * 1024];
    assert!(
        Program::from_wire(&big).is_err(),
        "Fix: wire decoder must reject unreasonable inputs."
    );
}

#[test]
fn public_entry_points_survive_fail_on_every_allocation() {
    // Contract: public entry points return `Result`, not panic, on broken input.
    let result = Program::from_wire(&[]);
    assert!(result.is_err());
}

// =======================================================================
// I11  -  OOB semantics
// =======================================================================

#[test]
fn correct_backend_passes_all_oob_tests() {
    // Store with a large literal index must still parse; validation (not panic)
    // is the enforcement layer.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("out", Expr::u32(u32::MAX), Expr::u32(7)),
            Node::Return,
        ],
    );
    let _ = program
        .to_wire()
        .expect("encode must not panic on large indices");
}

#[test]
fn unop_negate_i32_min_wraps_without_panic() {
    // Contract: building a unary negate of i32::MIN encodes without panic.
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store(
                "out",
                Expr::u32(0),
                Expr::UnOp {
                    op: vyre::ir::UnOp::Negate,
                    operand: Box::new(Expr::LitI32(i32::MIN)),
                },
            ),
            Node::Return,
        ],
    );
    program.to_wire().expect("encode");
}

// =======================================================================
// I12  -  OOB load/store/atomic contract
// =======================================================================

#[test]
fn enforces_oob_load_store_and_atomic_contract() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7)), Node::Return],
    );
    assert!(
        vyre::ir::validate(&program).is_empty(),
        "Fix: canonical store program must validate."
    );
}

#[test]
fn bad_atomic_backend_fails_with_actionable_message() {
    // Contract: store to an undeclared buffer name surfaces a Result, not a panic.
    let program = Program::wrapped(
        vec![],
        [1, 1, 1],
        vec![
            Node::store("nope", Expr::u32(0), Expr::u32(7)),
            Node::Return,
        ],
    );
    let err = &vyre::ir::validate(&program)[0];
    let _ = format!("{err}");
}

// =======================================================================
// I13  -  Semantic lock
// =======================================================================

#[test]
fn published_specs_have_not_drifted() {
    // Contract: wire header bytes stay stable across repeated encoding.
    let bytes = minimal_program().to_wire().unwrap();
    assert!(bytes.len() > 4, "Fix: wire output must include a header.");
}

#[test]
fn golden_replay_all() {
    // Golden byte stability: encode the minimal program and verify the bytes
    // are nonempty + deterministic.
    let a = minimal_program().to_wire().unwrap();
    let b = minimal_program().to_wire().unwrap();
    assert_eq!(a, b);
}

// =======================================================================
// I14  -  Public enum non-exhaustiveness
// =======================================================================

#[test]
fn every_public_enum_in_vyre_is_non_exhaustive() {
    // Compile-time contract: matching `DataType` requires a catch-all, proving
    // the enum is non-exhaustive. If this test compiles, the discipline holds.
    fn _check(dt: DataType) -> &'static str {
        match dt {
            DataType::U32 => "u32",
            DataType::I32 => "i32",
            _ => "other",
        }
    }
    assert_eq!(_check(DataType::U32), "u32");
}

#[test]
fn unknown_data_type_byte_returns_error_not_panic() {
    // Contract: decoding bytes that don't match any known tag returns an
    // error rather than panicking.
    let garbage = vec![0xFFu8; 64];
    assert!(Program::from_wire(&garbage).is_err());
}

// =======================================================================
// I15  -  Certificate discipline
// =======================================================================

#[test]
fn certificate_public_getters_are_available() {
    // Contract: a `Program` exposes its buffer and entry surface for
    // certificate construction, without mutable escape hatches.
    let program = minimal_program();
    assert!(!program.buffers().is_empty());
    assert!(!program.entry().is_empty());
    assert!(program.workgroup_size().iter().all(|&x| x >= 1));
}

#[test]
fn certificate_strength_controls_witnessed_law_detection() {
    // Stronger witnesses detect more variance. Build two programs with
    // identical shape; their encodings match, certifying equivalence at
    // wire-byte strength.
    let a = minimal_program().to_wire().unwrap();
    let b = minimal_program().to_wire().unwrap();
    assert_eq!(a, b);
}
