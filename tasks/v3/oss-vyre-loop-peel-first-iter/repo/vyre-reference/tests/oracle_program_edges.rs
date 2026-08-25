//! Whole-program adversarial contracts for the CPU reference oracle.
//!
//! Expression-level tests are not enough: backend conformance compares final
//! output buffers. These tests pin edge-case values after they pass through
//! `Program::wrapped`, statement execution, typed stores, and output readback.

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_reference::reference_eval;
use vyre_reference::value::Value;

fn output_program(element: DataType, count: u32, body: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, element).with_count(count)],
        [1, 1, 1],
        body,
    )
}

fn run_output_bytes(program: &Program) -> Vec<u8> {
    run_output_bytes_with_inputs(program, &[])
}

fn run_output_bytes_with_inputs(program: &Program, inputs: &[Value]) -> Vec<u8> {
    let outputs = reference_eval(program, inputs).expect("Fix: oracle edge program must execute");
    assert_eq!(
        outputs.len(),
        1,
        "Fix: oracle edge fixtures must declare one output buffer"
    );
    outputs[0].to_bytes()
}

fn run_program_error(program: &Program) -> String {
    reference_eval(program, &[])
        .expect_err("Fix: oracle edge fixture must fail")
        .to_string()
}

fn u32_chunks(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(
        bytes.len() % 4,
        0,
        "Fix: u32/F32/Bool output bytes must be word-aligned"
    );
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("chunk length checked")))
        .collect()
}

fn store(index: u32, value: Expr) -> Node {
    Node::store("out", Expr::u32(index), value)
}

#[test]
fn f32_edge_values_are_canonical_after_program_store_and_readback() {
    let positive_subnormal = f32::from_bits(0x0000_0001);
    let negative_subnormal = f32::from_bits(0x8000_0001);
    let payload_nan = f32::from_bits(0x7FA1_2345);
    let negative_payload_nan = f32::from_bits(0xFFA1_2345);
    let cases = [
        Expr::f32(positive_subnormal),
        Expr::f32(negative_subnormal),
        Expr::f32(payload_nan),
        Expr::f32(negative_payload_nan),
        Expr::f32(f32::NEG_INFINITY),
        Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(Expr::f32(negative_subnormal)),
            right: Box::new(Expr::f32(negative_subnormal)),
        },
        Expr::UnOp {
            op: UnOp::Sqrt,
            operand: Box::new(Expr::f32(-1.0)),
        },
    ];
    let program = output_program(
        DataType::F32,
        cases.len() as u32,
        cases
            .into_iter()
            .enumerate()
            .map(|(index, expr)| store(index as u32, expr))
            .collect(),
    );

    assert_eq!(
        u32_chunks(&run_output_bytes(&program)),
        vec![
            0x0000_0000,
            0x8000_0000,
            0x7FC0_0000,
            0x7FC0_0000,
            f32::NEG_INFINITY.to_bits(),
            0x8000_0000,
            0x7FC0_0000,
        ],
        "reference_eval must expose canonical f32 buffer bits, not host f64 artifacts"
    );
}

#[test]
fn f32_classification_ops_survive_bool_output_readback() {
    let payload_nan = f32::from_bits(0x7FA1_2345);
    let negative_subnormal = f32::from_bits(0x8000_0001);
    let cases = [
        Expr::UnOp {
            op: UnOp::IsNan,
            operand: Box::new(Expr::f32(payload_nan)),
        },
        Expr::UnOp {
            op: UnOp::IsInf,
            operand: Box::new(Expr::f32(f32::NEG_INFINITY)),
        },
        Expr::UnOp {
            op: UnOp::IsFinite,
            operand: Box::new(Expr::f32(negative_subnormal)),
        },
        Expr::UnOp {
            op: UnOp::IsFinite,
            operand: Box::new(Expr::f32(f32::INFINITY)),
        },
        Expr::BinOp {
            op: BinOp::Eq,
            left: Box::new(Expr::f32(0.0)),
            right: Box::new(Expr::f32(negative_subnormal)),
        },
        Expr::BinOp {
            op: BinOp::Lt,
            left: Box::new(Expr::f32(payload_nan)),
            right: Box::new(Expr::f32(1.0)),
        },
    ];
    let program = output_program(
        DataType::Bool,
        cases.len() as u32,
        cases
            .into_iter()
            .enumerate()
            .map(|(index, expr)| store(index as u32, expr))
            .collect(),
    );

    assert_eq!(
        u32_chunks(&run_output_bytes(&program)),
        vec![1, 1, 1, 0, 1, 0],
        "bool output bytes must pin IEEE-754 classification and comparison semantics"
    );
}

#[test]
fn f32_comparison_ops_preserve_unordered_nan_and_signed_zero_semantics() {
    fn canonical_compare_input(value: f32) -> f32 {
        if value.is_nan() {
            f32::from_bits(0x7fc0_0000)
        } else if value.is_subnormal() {
            f32::from_bits(value.to_bits() & 0x8000_0000)
        } else {
            value
        }
    }

    fn expected(op: BinOp, left: f32, right: f32) -> bool {
        let left = canonical_compare_input(left);
        let right = canonical_compare_input(right);
        match op {
            BinOp::Eq => left == right,
            BinOp::Ne => left != right,
            BinOp::Lt => left < right,
            BinOp::Le => left <= right,
            BinOp::Gt => left > right,
            BinOp::Ge => left >= right,
            _ => unreachable!("comparison matrix only contains comparison ops"),
        }
    }

    let payload_nan = f32::from_bits(0x7fa1_2345);
    let negative_payload_nan = f32::from_bits(0xffa1_2345);
    let positive_subnormal = f32::from_bits(0x0000_0001);
    let negative_subnormal = f32::from_bits(0x8000_0001);
    let pairs = [
        (payload_nan, 1.0),
        (1.0, payload_nan),
        (payload_nan, negative_payload_nan),
        (-0.0, 0.0),
        (positive_subnormal, 0.0),
        (negative_subnormal, -0.0),
        (-1.0, 1.0),
        (2.0, 2.0),
    ];
    let ops = [
        BinOp::Eq,
        BinOp::Ne,
        BinOp::Lt,
        BinOp::Le,
        BinOp::Gt,
        BinOp::Ge,
    ];
    let mut body = Vec::with_capacity(pairs.len() * ops.len());
    let mut expected_words = Vec::with_capacity(pairs.len() * ops.len());

    for (pair_index, &(left, right)) in pairs.iter().enumerate() {
        for (op_index, &op) in ops.iter().enumerate() {
            let output_index = pair_index * ops.len() + op_index;
            body.push(store(
                output_index as u32,
                Expr::BinOp {
                    op,
                    left: Box::new(Expr::f32(left)),
                    right: Box::new(Expr::f32(right)),
                },
            ));
            expected_words.push(u32::from(expected(op, left, right)));
        }
    }

    let program = output_program(DataType::Bool, expected_words.len() as u32, body);

    assert_eq!(
        u32_chunks(&run_output_bytes(&program)),
        expected_words,
        "reference_eval must pin full f32 comparison semantics: NaN is unordered, Ne is true for unordered pairs, and signed/subnormal zero canonicalization happens before comparison."
    );
}

#[test]
fn large_loop_accumulation_uses_wrapping_u32_oracle_semantics() {
    let iterations = 131_072u32;
    let expected = (u64::from(iterations) * u64::from(iterations - 1) / 2) as u32;
    let program = output_program(
        DataType::U32,
        1,
        vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(iterations),
                vec![Node::assign(
                    "acc",
                    Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(Expr::var("acc")),
                        right: Box::new(Expr::var("i")),
                    },
                )],
            ),
            store(0, Expr::var("acc")),
        ],
    );

    assert_eq!(
        u32_chunks(&run_output_bytes(&program)),
        vec![expected],
        "large statement loops must keep deterministic wrapping-u32 oracle behavior"
    );
}

#[test]
fn u32_arithmetic_edge_cases_survive_program_store_and_readback() {
    let cases = [
        Expr::BinOp {
            op: BinOp::Div,
            left: Box::new(Expr::u32(42)),
            right: Box::new(Expr::load("in", Expr::u32(0))),
        },
        Expr::BinOp {
            op: BinOp::Mod,
            left: Box::new(Expr::u32(42)),
            right: Box::new(Expr::load("in", Expr::u32(0))),
        },
        Expr::BinOp {
            op: BinOp::Shl,
            left: Box::new(Expr::u32(1)),
            right: Box::new(Expr::u32(32)),
        },
        Expr::BinOp {
            op: BinOp::Shl,
            left: Box::new(Expr::u32(1)),
            right: Box::new(Expr::u32(33)),
        },
        Expr::BinOp {
            op: BinOp::Shr,
            left: Box::new(Expr::u32(0x8000_0000)),
            right: Box::new(Expr::u32(32)),
        },
        Expr::BinOp {
            op: BinOp::AbsDiff,
            left: Box::new(Expr::u32(0)),
            right: Box::new(Expr::u32(u32::MAX)),
        },
        Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(Expr::u32(u32::MAX)),
            right: Box::new(Expr::u32(1)),
        },
        Expr::BinOp {
            op: BinOp::Mul,
            left: Box::new(Expr::u32(u32::MAX)),
            right: Box::new(Expr::u32(2)),
        },
    ];
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(cases.len() as u32),
        ],
        [1, 1, 1],
        cases
            .into_iter()
            .enumerate()
            .map(|(index, expr)| store(index as u32, expr))
            .collect(),
    );

    assert_eq!(
        u32_chunks(&run_output_bytes_with_inputs(
            &program,
            &[Value::Bytes(0u32.to_le_bytes().to_vec().into())]
        )),
        vec![
            u32::MAX,
            0,
            1,
            2,
            0x8000_0000,
            u32::MAX,
            0,
            u32::MAX.wrapping_mul(2),
        ],
        "u32 whole-program oracle semantics must pin div/mod-by-zero, shift masking, absdiff, and wrapping arithmetic"
    );
}

#[test]
fn signed_division_errors_are_structured_at_program_boundary() {
    let program = output_program(
        DataType::I32,
        1,
        vec![store(
            0,
            Expr::BinOp {
                op: BinOp::Div,
                left: Box::new(Expr::i32(i32::MIN)),
                right: Box::new(Expr::i32(-1)),
            },
        )],
    );
    let error = run_program_error(&program);

    assert!(
        error.contains("Fix:") && error.contains("i32") && error.contains("division"),
        "signed division overflow must surface an actionable program-boundary error, got `{error}`"
    );
}

/// A signed remainder (`BinOp::Mod` on i32 operands) is typed U32 by the
/// foundation typechecker but evaluates to a SIGNED `Value::I32`; storing it into
/// an I32 output buffer and reading back must yield the signed remainder bits.
/// This is the end-to-end semantics the same-width store coercion now permits
/// (`store(i32_buffer, rem(i32, i32))` — previously rejected V045). The oracle
/// stores the value's raw little-endian bytes, so the bit pattern round-trips
/// regardless of the U32-vs-I32 type label.
#[test]
fn signed_remainder_stored_into_i32_buffer_reads_back_signed() {
    let program = output_program(
        DataType::I32,
        1,
        vec![store(
            0,
            Expr::BinOp {
                op: BinOp::Mod,
                left: Box::new(Expr::i32(-7)),
                right: Box::new(Expr::i32(3)),
            },
        )],
    );
    let bytes = run_output_bytes(&program);
    let value = i32::from_le_bytes(bytes[..4].try_into().expect("4 output bytes"));
    assert_eq!(
        value, -1,
        "-7 % 3 must read back as the signed remainder -1 from the i32 buffer, \
         got {value} (bytes {bytes:?})"
    );
}

/// The reverse direction: a signed quotient (`BinOp::Div` on i32 operands, typed
/// I32 via Frame::Bin) stored into a U32 buffer must preserve the signed bit
/// pattern (two's-complement), proving the same-width store coercion is a
/// bit-exact reinterpret in both directions.
#[test]
fn signed_quotient_stored_into_u32_buffer_preserves_bits() {
    let program = output_program(
        DataType::U32,
        1,
        vec![store(
            0,
            Expr::BinOp {
                op: BinOp::Div,
                left: Box::new(Expr::i32(-7)),
                right: Box::new(Expr::i32(3)),
            },
        )],
    );
    let bytes = run_output_bytes(&program);
    // -7 / 3 truncates toward zero = -2; its two's-complement bits = 0xFFFF_FFFE.
    let as_u32 = u32::from_le_bytes(bytes[..4].try_into().expect("4 output bytes"));
    assert_eq!(
        as_u32, 0xFFFF_FFFE,
        "-7 / 3 = -2 stored into a u32 buffer must keep the two's-complement bits, \
         got {as_u32:#010x}"
    );
    assert_eq!(as_u32 as i32, -2, "the bits reinterpret to -2 as i32");
}
