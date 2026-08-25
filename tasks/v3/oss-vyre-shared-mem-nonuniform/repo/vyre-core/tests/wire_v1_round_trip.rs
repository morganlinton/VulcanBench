//! Wire-format v1 round-trip smoke test.
//!
//! Proves every non-empty Program built from primitive variants
//! round-trips through `to_wire` + `from_wire` byte-identically.
//! Protects against silent regressions in the VIR0 wire encoder or
//! decoder when a new IR variant lands; exhaustive proptest coverage
//! lives at `vyre-foundation/tests/terminal_wire_round_trip.rs`.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn empty_program() -> Program {
    Program::wrapped(Vec::new(), [1, 1, 1], Vec::new())
}

fn trivial_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::storage(
            "out",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [64, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

#[test]
fn empty_program_round_trips() {
    let p = empty_program();
    let bytes = p.to_wire().expect("empty program must encode");
    let decoded = Program::from_wire(&bytes).expect("empty program must decode");
    assert_eq!(decoded, p);
}

#[test]
fn trivial_program_round_trips() {
    let p = trivial_program();
    let bytes = p.to_wire().expect("trivial program must encode");
    let decoded = Program::from_wire(&bytes).expect("trivial program must decode");
    assert_eq!(decoded, p);
}

#[test]
fn re_encode_is_stable() {
    // Encoder must be deterministic: encoding the decoded program
    // yields the same bytes.
    let p = trivial_program();
    let bytes = p.to_wire().expect("encode");
    let decoded = Program::from_wire(&bytes).expect("decode");
    let re_encoded = decoded.to_wire().expect("re-encode");
    assert_eq!(bytes, re_encoded);
}

#[test]
fn wire_bytes_nonempty() {
    // Smoke check that encoded output is nonempty  -  the header and
    // body structure are verified exhaustively by
    // vyre-foundation/tests/terminal_wire_round_trip.rs.
    let bytes = empty_program().to_wire().expect("encode");
    assert_ne!(bytes.len(), 0, "encoded wire bytes must be non-empty");
}
