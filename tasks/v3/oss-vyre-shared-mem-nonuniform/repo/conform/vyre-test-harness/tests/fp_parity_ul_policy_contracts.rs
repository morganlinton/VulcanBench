//! Contracts for [`vyre_test_harness::fp_parity`]  -  ULP budgets were spelled out in
//! module docs (`REFERENCE_TRANSCENDENTAL_ULP_BUDGET`, elementary vs transcendental
//! backend envelopes) but had no direct regression tests in this crate.

#![forbid(unsafe_code)]

use std::sync::Arc;

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program, UnOp};
use vyre_test_harness::fp_parity::{
    f32_ulp_tolerance, BACKEND_ELEMENTARY_F32_ULP_BUDGET, BACKEND_TRANSCENDENTAL_ULP_BUDGET,
    REFERENCE_TRANSCENDENTAL_ULP_BUDGET,
};

fn minimal_elementary_f32_copy_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::F32),
            BufferDecl::output("out", 1, DataType::F32),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len("out")),
                vec![Node::store(
                    "out",
                    Expr::var("idx"),
                    Expr::load("in", Expr::var("idx")),
                )],
            ),
        ],
    )
}

fn minimal_tanh_f32_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::F32),
            BufferDecl::output("out", 1, DataType::F32),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len("out")),
                vec![Node::store(
                    "out",
                    Expr::var("idx"),
                    Expr::UnOp {
                        op: UnOp::Tanh,
                        operand: Box::new(Expr::load("in", Expr::var("idx"))),
                    },
                )],
            ),
        ],
    )
}

#[test]
fn reference_transcendental_budget_matches_documented_audit_anchor() {
    assert_eq!(
        REFERENCE_TRANSCENDENTAL_ULP_BUDGET, 4,
        "documented ceiling for the deterministic reference vs correctly-rounded transcendentals"
    );
}

#[test]
fn elementary_f32_program_uses_elementary_backend_budget() {
    let program = minimal_elementary_f32_copy_program();
    let expected = if cfg!(feature = "strict-fp") {
        0
    } else {
        BACKEND_ELEMENTARY_F32_ULP_BUDGET
    };
    assert_eq!(
        f32_ulp_tolerance(&program),
        expected,
        "non-transcendental F32 programs follow elementary budget unless strict-fp forces byte identity"
    );
}

#[test]
fn transcendental_f32_program_uses_transcendental_backend_budget() {
    let program = minimal_tanh_f32_program();
    assert_eq!(
        f32_ulp_tolerance(&program),
        BACKEND_TRANSCENDENTAL_ULP_BUDGET,
        "documented native-transcendental envelope must apply whenever IR contains UnOp::Tanh"
    );
}

#[test]
fn transcendental_inside_nested_region_is_detected() {
    let inner = vec![
        Node::let_bind("idx", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("idx"), Expr::buf_len("out")),
            vec![Node::store(
                "out",
                Expr::var("idx"),
                Expr::UnOp {
                    op: UnOp::Sqrt,
                    operand: Box::new(Expr::load("in", Expr::var("idx"))),
                },
            )],
        ),
    ];
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::F32),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::F32),
        ],
        [64, 1, 1],
        vec![Node::Region {
            generator: Ident::from("vyre-test-harness::fixture.transcendental_region"),
            source_region: None,
            body: Arc::new(inner),
        }],
    );
    assert_eq!(
        f32_ulp_tolerance(&program),
        BACKEND_TRANSCENDENTAL_ULP_BUDGET,
        "policy scan must recurse through Region bodies so nested sqrt cannot hide behind a wrapper"
    );
}
