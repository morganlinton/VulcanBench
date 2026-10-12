//! Quantized datatype contracts for the reference oracle.
//!
//! The spec exposes INT4/FP4/NF4/FP8 datatypes for GPU inference paths. The
//! CPU oracle must therefore preserve their fixed-width storage bytes exactly
//! and return typed zero payloads for out-of-bounds loads instead of degrading
//! through empty `Bytes`.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::{reference_eval, value::Value};

fn run_single_load_store(ty: DataType, input: Vec<u8>, index: u32) -> Vec<u8> {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, ty.clone()).with_count(1),
            BufferDecl::output("out", 1, ty).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(index)),
        )],
    );
    let outputs = reference_eval(&program, &[Value::Bytes(input.into())])
        .expect("quantized load/store oracle program must execute");
    outputs[0].to_bytes()
}

#[test]
fn quantized_scalar_load_store_preserves_raw_storage_bits() {
    for (ty, encoded) in [
        (DataType::I4, vec![0x0F]),
        (DataType::FP4, vec![0x06]),
        (DataType::NF4, vec![0x08]),
        (DataType::F8E4M3, vec![0x7F]),
        (DataType::F8E5M2, vec![0x7B]),
    ] {
        let out = run_single_load_store(ty.clone(), encoded.clone(), 0);
        assert_eq!(
            out.len(),
            encoded.len(),
            "{ty} output length must match input"
        );
        assert_eq!(out, encoded, "{ty} in-bounds load/store must be byte-exact");
    }
}

#[test]
fn quantized_scalar_oob_load_returns_typed_zero_byte() {
    for ty in [
        DataType::I4,
        DataType::FP4,
        DataType::NF4,
        DataType::F8E4M3,
        DataType::F8E5M2,
    ] {
        assert_eq!(
            run_single_load_store(ty.clone(), vec![0xFF], 99),
            vec![0],
            "{ty} OOB load must return a one-byte typed zero, not empty Bytes"
        );
    }
}

#[test]
fn half_and_bfloat_oob_loads_return_two_byte_typed_zero() {
    for ty in [DataType::F16, DataType::BF16, DataType::I16, DataType::U16] {
        assert_eq!(
            run_single_load_store(ty.clone(), vec![0xFF, 0xFF], 99),
            vec![0, 0],
            "{ty} OOB load must preserve its two-byte storage shape"
        );
    }
}

#[test]
fn packed_i4_reference_buffer_len_reports_logical_elements() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::I4).with_count(8),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::buf_len("input"))],
    );

    let outputs = reference_eval(&program, &[Value::Bytes(vec![0u8; 4].into())])
        .expect("Fix: packed I4 buffer length oracle must execute.");

    assert_eq!(
        outputs[0].to_bytes(),
        8u32.to_le_bytes(),
        "Fix: four bytes of I4 storage must report eight logical elements to Expr::buf_len."
    );
}
