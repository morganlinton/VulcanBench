//! Release sweep R2 - oracle matrix (handwritten reference, hostile corpus).
//! Generated scaffold - oracle logic is explicit; do not reduce to `assert!(is_ok)`.
#![forbid(unsafe_code)]

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

fn out_program(nodes: Vec<Node>) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [1, 1, 1],
        nodes,
    )
}

struct RejectCase {
    name: &'static str,
    build: fn() -> Program,
    needle: &'static str,
}

fn cases() -> Vec<RejectCase> {
    vec![
        RejectCase {
            name: "div_zero",
            build: || {
                out_program(vec![Node::let_bind(
                    "x",
                    Expr::div(Expr::u32(1), Expr::u32(0)),
                )])
            },
            needle: "V044",
        },
        RejectCase {
            name: "mod_zero",
            build: || {
                out_program(vec![Node::let_bind(
                    "x",
                    Expr::rem(Expr::u32(1), Expr::i32(0)),
                )])
            },
            needle: "V044",
        },
        RejectCase {
            name: "add_u64",
            build: || {
                out_program(vec![Node::let_bind(
                    "x",
                    Expr::add(Expr::u64(1), Expr::u64(2)),
                )])
            },
            needle: "64-bit",
        },
        RejectCase {
            name: "mul_i64",
            build: || {
                out_program(vec![Node::let_bind(
                    "x",
                    Expr::mul(Expr::i64(1), Expr::i64(2)),
                )])
            },
            needle: "64-bit",
        },
        RejectCase {
            name: "workgroup_zero",
            build: || {
                Program::wrapped(
                    vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
                    [0, 1, 1],
                    vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
                )
            },
            needle: "workgroup_size[0] is 0",
        },
        RejectCase {
            name: "duplicate_buffer",
            build: || {
                Program::wrapped(
                    vec![
                        BufferDecl::read("a", 0, DataType::U32).with_count(1),
                        BufferDecl::read("a", 1, DataType::U32).with_count(1),
                    ],
                    [1, 1, 1],
                    vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
                )
            },
            needle: "duplicate buffer name",
        },
        RejectCase {
            name: "duplicate_binding",
            build: || {
                Program::wrapped(
                    vec![
                        BufferDecl::read("a", 0, DataType::U32).with_count(1),
                        BufferDecl::read("b", 0, DataType::U32).with_count(1),
                    ],
                    [1, 1, 1],
                    vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
                )
            },
            needle: "duplicate binding slot",
        },
    ]
}

#[test]
fn sweep_validation_rejection_matrix_covers_contract_cases() {
    for case in cases() {
        let program = (case.build)();
        let errors = validate(&program);
        assert!(
            !errors.is_empty(),
            "case {} must be rejected, got no errors",
            case.name
        );
        assert!(
            errors.iter().any(|e| e.message().contains(case.needle)),
            "case {} must mention {:?}, got {:?}",
            case.name,
            case.needle,
            errors
        );
    }
}

#[test]
fn sweep_validation_rejection_matrix_parametric_divisors() {
    for divisor in [0u32, 1, 2, 0x8000_0000, u32::MAX] {
        for lhs in [0u32, 1, 42, u32::MAX] {
            if divisor == 0 {
                let program = out_program(vec![Node::let_bind(
                    "x",
                    Expr::div(Expr::u32(lhs), Expr::u32(divisor)),
                )]);
                let errors = validate(&program);
                assert!(
                    errors.iter().any(|e| e.message().contains("V044")),
                    "div by {divisor} must fail for lhs={lhs}: {:?}",
                    errors
                );
            }
        }
    }
}

#[test]
fn sweep_validation_rejection_matrix_workgroup_axes() {
    for wg in [[0u32, 1, 1], [1, 0, 1], [1, 1, 0], [0, 0, 1], [0, 0, 0]] {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            wg,
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        );
        let errors = validate(&program);
        for (axis, &size) in wg.iter().enumerate() {
            if size == 0 {
                let expected = format!(
                    "workgroup_size[{axis}] is 0. Fix: all workgroup dimensions must be >= 1."
                );
                assert_eq!(
                    errors
                        .iter()
                        .find(|e| e
                            .message()
                            .contains(&format!("workgroup_size[{axis}] is 0")))
                        .map(|e| e.message()),
                    Some(expected.as_str()),
                    "workgroup {wg:?} axis {axis}: {:?}",
                    errors
                );
            }
        }
    }
}
