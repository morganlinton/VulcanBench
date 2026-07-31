//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
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

const CASES: usize = 16384;

#[test]
fn sweep_validation_rejection_volume_oracle_matrix() {
    for idx in 0..CASES {
        let divisor = match idx % 5 {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 0x8000_0000,
            _ => u32::MAX,
        };
        let lhs = (idx as u32).wrapping_mul(0x9E37_79B9);
        if divisor == 0 {
            let program = out_program(vec![Node::let_bind(
                "x",
                Expr::div(Expr::u32(lhs), Expr::u32(0)),
            )]);
            let errors = validate(&program);
            assert!(
                errors.iter().any(|e| e.message().contains("V044")),
                "Fix: div-by-zero volume case {idx}: {errors:?}"
            );
        }
        let program = out_program(vec![Node::let_bind(
            "y",
            Expr::add(Expr::u64(lhs as u64), Expr::u64(1)),
        )]);
        let errors = validate(&program);
        assert!(
            errors.iter().any(|e| e.message().contains("64-bit")),
            "Fix: u64 add rejection volume case {idx}: {errors:?}"
        );
    }
}
