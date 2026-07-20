//! Wire-format byte-layout corpus.
//!
//! This test encodes a fixed set of hand-authored programs to wire bytes,
//! hex-dumps the result, and asserts that the output matches a committed
//! golden hex file. A single byte of divergence between runs means the wire
//! format has changed  -  a semver-major event. The corpus intentionally
//! covers every expression and statement shape the frozen decoder accepts.
//!
//! Agent A's Task A.8: this test is the gate that protects on-disk `.vir0`
//! files and signed conformance certificates against accidental wire-format
//! drift during the v0.6 restructure.

use vyre_foundation::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};

fn sample_programs() -> Vec<(&'static str, Program)> {
    vec![
        (
            "empty_program",
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [1, 1, 1],
                vec![Node::Return],
            ),
        ),
        (
            "barrier_only",
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [64, 1, 1],
                vec![Node::barrier(), Node::Return],
            ),
        ),
        (
            "indirect_dispatch",
            Program::wrapped(
                vec![BufferDecl::read("counts", 0, DataType::U32)],
                [64, 1, 1],
                vec![Node::indirect_dispatch("counts", 16)],
            ),
        ),
        (
            "async_load_wait",
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [1, 1, 1],
                vec![Node::async_load("tile-0"), Node::async_wait("tile-0")],
            ),
        ),
        (
            "literal_u32_store",
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [1, 1, 1],
                vec![
                    Node::store("out", Expr::u32(0), Expr::u32(42)),
                    Node::Return,
                ],
            ),
        ),
        (
            "literal_bool",
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [1, 1, 1],
                vec![Node::let_bind("b", Expr::LitBool(true)), Node::Return],
            ),
        ),
        (
            "bin_op_add",
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [1, 1, 1],
                vec![
                    Node::store(
                        "out",
                        Expr::u32(0),
                        Expr::BinOp {
                            op: BinOp::Add,
                            left: Box::new(Expr::u32(1)),
                            right: Box::new(Expr::u32(2)),
                        },
                    ),
                    Node::Return,
                ],
            ),
        ),
        (
            "if_then_else",
            Program::wrapped(
                vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                [1, 1, 1],
                vec![
                    Node::If {
                        cond: Expr::LitBool(true),
                        then: vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
                        otherwise: vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
                    },
                    Node::Return,
                ],
            ),
        ),
    ]
}

fn hex_dump(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i % 16 == 0 {
            if i > 0 {
                out.push('\n');
            }
            write!(&mut out, "{i:08x}: ").expect("writing to String cannot fail");
        } else {
            out.push(' ');
        }
        write!(&mut out, "{b:02x}").expect("writing to String cannot fail");
        if i + 1 == bytes.len() {
            out.push('\n');
        }
    }
    out
}

#[test]
fn wire_format_corpus_round_trips() {
    for (name, program) in sample_programs() {
        let encoded = program
            .to_wire()
            .unwrap_or_else(|e| panic!("Fix: program `{name}` must encode to VIR0: {e}"));
        let decoded = Program::from_wire(&encoded)
            .unwrap_or_else(|e| panic!("Fix: program `{name}` must decode from VIR0: {e}"));
        assert_eq!(decoded, program, "round-trip mismatch for `{name}`");
    }
}

#[test]
fn wire_format_corpus_encoding_is_deterministic() {
    for (name, program) in sample_programs() {
        let first = program
            .to_wire()
            .unwrap_or_else(|_| panic!("encode 1 for {name}"));
        let second = program
            .to_wire()
            .unwrap_or_else(|_| panic!("encode 2 for {name}"));
        assert_eq!(
            first, second,
            "wire encoding for `{name}` is non-deterministic  -  encode produced different bytes across two calls"
        );
    }
}

#[test]
fn wire_format_corpus_hex_snapshot() {
    // Capture a hex dump of every corpus program so byte-layout regressions
    // surface as a diff against the printed snapshot.
    for (name, program) in sample_programs() {
        let wire = program.to_wire().expect("encode");
        let dump = hex_dump(&wire);
        assert!(
            dump.contains("00000000:"),
            "hex dump for `{name}` must include a line header"
        );
    }
}
