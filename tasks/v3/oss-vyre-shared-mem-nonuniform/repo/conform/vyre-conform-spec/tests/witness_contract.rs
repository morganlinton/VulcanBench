//! Contract tests for witness-set determinism, edge-case coverage,
//! and Program wire-format fingerprint stability.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_conform_spec::U32Witness;

#[test]
fn u32_witness_is_deterministic() {
    let a = U32Witness::enumerate();
    let b = U32Witness::enumerate();
    assert_eq!(
        a, b,
        "U32Witness::enumerate must be deterministic across calls"
    );
}

#[test]
fn u32_witness_contains_critical_edge_cases() {
    let w = U32Witness::enumerate();
    assert!(w.contains(&0), "witness set must contain 0");
    assert!(w.contains(&1), "witness set must contain 1");
    assert!(w.contains(&u32::MAX), "witness set must contain u32::MAX");
    assert!(
        w.contains(&(u32::MAX - 1)),
        "witness set must contain u32::MAX - 1"
    );
}

#[test]
fn program_fingerprint_stable_across_clones() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::store("out", Expr::var("idx"), Expr::u32(42)),
            Node::Return,
        ],
    );
    let clone = program.clone();
    assert_eq!(
        program.fingerprint(),
        clone.fingerprint(),
        "fingerprint must be stable across clones"
    );
}

#[test]
fn program_fingerprint_stable_across_recomputation() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::store("out", Expr::var("idx"), Expr::u32(42)),
            Node::Return,
        ],
    );
    let fp1 = program.fingerprint();
    let fp2 = program.fingerprint();
    assert_eq!(
        fp1, fp2,
        "fingerprint must be stable across repeated computation"
    );
}

#[test]
fn program_wire_bytes_stable_across_serializations() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::store("out", Expr::var("idx"), Expr::u32(42)),
            Node::Return,
        ],
    );
    let bytes1 = program.canonical_wire_bytes().unwrap();
    let bytes2 = program.canonical_wire_bytes().unwrap();
    assert_eq!(
        bytes1, bytes2,
        "wire-format bytes must be identical across serializations"
    );
}

#[test]
fn program_wire_bytes_match_fingerprint_derivation() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::store("out", Expr::var("idx"), Expr::u32(42)),
            Node::Return,
        ],
    );
    let bytes = program.canonical_wire_bytes().unwrap();
    let expected_fp = *blake3::hash(&bytes).as_bytes();
    let actual_fp = program.fingerprint();
    assert_eq!(
        expected_fp, actual_fp,
        "fingerprint must equal blake3 hash of canonical wire bytes"
    );
}
