//! Generated boundary matrix for the cpu-ref backend adapter.
//!
//! The reference backend is the byte oracle used by conformance and parity
//! harnesses, so this test drives the backend trait surface with thousands of
//! generated edge-heavy inputs instead of only hand-picked examples.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_reference::CpuRefBackend;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

mod support;
use support::{dispatch_with_inputs, u32_out_buffer};

#[derive(Clone, Copy)]
struct BinaryCase {
    name: &'static str,
    expr: fn(Expr, Expr) -> Expr,
    expected: fn(u32, u32) -> u32,
}

const BINARY_CASES: &[BinaryCase] = &[
    BinaryCase {
        name: "add",
        expr: Expr::add,
        expected: u32::wrapping_add,
    },
    BinaryCase {
        name: "sub",
        expr: Expr::sub,
        expected: u32::wrapping_sub,
    },
    BinaryCase {
        name: "mul",
        expr: Expr::mul,
        expected: u32::wrapping_mul,
    },
    BinaryCase {
        name: "xor",
        expr: Expr::bitxor,
        expected: |a, b| a ^ b,
    },
    BinaryCase {
        name: "and",
        expr: Expr::bitand,
        expected: |a, b| a & b,
    },
    BinaryCase {
        name: "or",
        expr: Expr::bitor,
        expected: |a, b| a | b,
    },
];

fn binary_program(expr: fn(Expr, Expr) -> Expr) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32),
            BufferDecl::read("b", 1, DataType::U32),
            u32_out_buffer("out", 2),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("idx", Expr::u32(0)),
            Node::store(
                "out",
                Expr::var("idx"),
                expr(
                    Expr::load("a", Expr::var("idx")),
                    Expr::load("b", Expr::var("idx")),
                ),
            ),
        ],
    )
}

fn generated_pair(seed: u32) -> (u32, u32) {
    let a = seed.wrapping_mul(0x9e37_79b9).rotate_left(seed & 31) ^ 0xa5a5_5a5a;
    let b = seed
        .wrapping_add(0x7f4a_7c15)
        .rotate_right((seed >> 5) & 31)
        ^ 0x5a5a_a5a5;
    (a, b)
}

#[test]
fn generated_binary_operation_matrix_matches_host_wrapping_semantics() {
    let edge_pairs = [
        (0, 0),
        (0, 1),
        (1, 0),
        (u32::MAX, 0),
        (0, u32::MAX),
        (u32::MAX, 1),
        (1, u32::MAX),
        (u32::MAX, u32::MAX),
        (u32::MIN, u32::MAX),
        (u32::MAX, u32::MIN),
        (0x8000_0000, 2),
        (2, 0x8000_0000),
        (0x7fff_ffff, 2),
        (2, 0x7fff_ffff),
    ];

    let mut assertions = 0usize;
    for case in BINARY_CASES {
        let program = binary_program(case.expr);
        for &(a, b) in &edge_pairs {
            let outputs = dispatch_with_inputs(
                &program,
                &[a.to_le_bytes().to_vec(), b.to_le_bytes().to_vec()],
            );
            assert_eq!(
                outputs,
                vec![(case.expected)(a, b).to_le_bytes().to_vec()],
                "{} failed for edge pair ({a:#010x}, {b:#010x})",
                case.name
            );
            assertions += 1;
        }

        for seed in 0..4096u32 {
            let (a, b) = generated_pair(seed);
            let outputs = dispatch_with_inputs(
                &program,
                &[a.to_le_bytes().to_vec(), b.to_le_bytes().to_vec()],
            );
            assert_eq!(
                outputs,
                vec![(case.expected)(a, b).to_le_bytes().to_vec()],
                "{} failed for generated seed {seed}",
                case.name
            );
            assertions += 1;
        }
    }

    assert_eq!(assertions, BINARY_CASES.len() * (edge_pairs.len() + 4096));
}

#[test]
fn generated_borrowed_dispatch_matrix_matches_owned_dispatch() {
    let backend = CpuRefBackend;
    let config = DispatchConfig::default();
    let mut assertions = 0usize;

    for case in BINARY_CASES {
        let program = binary_program(case.expr);
        for seed in 0..2048u32 {
            let (a, b) = generated_pair(seed ^ 0x55aa_33cc);
            let a_bytes = a.to_le_bytes();
            let b_bytes = b.to_le_bytes();
            let owned = backend
                .dispatch(&program, &[a_bytes.to_vec(), b_bytes.to_vec()], &config)
                .expect("Fix: owned cpu-ref dispatch must accept generated inputs.");
            let borrowed = backend
                .dispatch_borrowed(&program, &[&a_bytes[..], &b_bytes[..]], &config)
                .expect("Fix: borrowed cpu-ref dispatch must accept generated inputs.");

            assert_eq!(
                borrowed, owned,
                "{} borrowed dispatch drifted from owned dispatch for seed {seed}",
                case.name
            );
            assertions += 1;
        }
    }

    assert_eq!(assertions, BINARY_CASES.len() * 2048);
}
