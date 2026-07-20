//! Helpers shared across the const_fold test split (A13).

use crate::ir::{BufferDecl, DataType, Expr, Node};

pub(super) fn program_contains_literal(program: &crate::ir::Program, expected: u32) -> bool {
    fn expr_contains(e: &Expr, expected: u32) -> bool {
        match e {
            Expr::LitU32(v) => *v == expected,
            Expr::UnOp { operand, .. } => expr_contains(operand.as_ref(), expected),
            Expr::BinOp { left, right, .. } => {
                expr_contains(left.as_ref(), expected) || expr_contains(right.as_ref(), expected)
            }
            _ => false,
        }
    }
    fn node_contains(n: &Node, expected: u32) -> bool {
        match n {
            Node::Store { index, value, .. } => {
                expr_contains(index, expected) || expr_contains(value, expected)
            }
            Node::Let { value, .. } | Node::Assign { value, .. } => expr_contains(value, expected),
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                expr_contains(cond, expected)
                    || then.iter().any(|n| node_contains(n, expected))
                    || otherwise.iter().any(|n| node_contains(n, expected))
            }
            Node::Block(body) => body.iter().any(|n| node_contains(n, expected)),
            Node::Region { body, .. } => body.iter().any(|n| node_contains(n, expected)),
            _ => false,
        }
    }
    program.entry().iter().any(|n| node_contains(n, expected))
}

pub(super) fn simple_program(value_expr: Expr) -> crate::ir::Program {
    crate::ir::Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, crate::ir::BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), value_expr)],
    )
}
